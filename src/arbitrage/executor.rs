use crate::{
    config::Config,
    dashboard::state::DashboardState,
    exchanges::{
        binance::BinanceConnector,
        bitget::BitgetConnector,
        bybit::BybitConnector,
        gate::GateConnector,
        kucoin::KucoinConnector,
        mexc::MexcConnector,
        okx::OkxConnector,
    },
    metrics::MetricsCollector,
    orderbook::SharedPriceState,
    risk::RiskManager,
    types::{ArbitrageSignal, CompletedTrade, Exchange, MarketType, Side},
};
use anyhow::Result;
use chrono::Utc;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct OrderExecutor {
    config: Arc<Config>,
    price_state: SharedPriceState,
    binance: Arc<BinanceConnector>,
    bybit: Arc<BybitConnector>,
    mexc: Arc<MexcConnector>,
    okx: Arc<OkxConnector>,
    bitget: Arc<BitgetConnector>,
    kucoin: Arc<KucoinConnector>,
    gate: Arc<GateConnector>,
    risk: Arc<RiskManager>,
    metrics: Arc<MetricsCollector>,
    dashboard: Arc<DashboardState>,
    trade_store: Arc<crate::trade_store::TradeStore>,
}

impl OrderExecutor {
    pub fn new(
        config: Arc<Config>,
        price_state: SharedPriceState,
        binance: Arc<BinanceConnector>,
        bybit: Arc<BybitConnector>,
        mexc: Arc<MexcConnector>,
        okx: Arc<OkxConnector>,
        bitget: Arc<BitgetConnector>,
        kucoin: Arc<KucoinConnector>,
        gate: Arc<GateConnector>,
        risk: Arc<RiskManager>,
        metrics: Arc<MetricsCollector>,
        dashboard: Arc<DashboardState>,
        trade_store: Arc<crate::trade_store::TradeStore>,
    ) -> Self {
        Self { config, price_state, binance, bybit, mexc, okx, bitget, kucoin, gate, risk, metrics, dashboard, trade_store }
    }

    pub async fn run(self: Arc<Self>, mut rx: mpsc::Receiver<ArbitrageSignal>) {
        info!("Order executor started (paper={})", self.config.trading.paper_trading);
        while let Some(signal) = rx.recv().await {
            let exec = self.clone();
            tokio::spawn(async move {
                exec.handle(signal).await;
            });
        }
    }

    async fn handle(&self, signal: ArbitrageSignal) {
        if let Err(reason) = self.risk.check(&signal) {
            warn!("Signal {} rejected by risk: {}", signal.id, reason);
            return;
        }

        self.risk.on_trade_open();

        match self.execute(&signal).await {
            Ok(trade) => {
                self.risk.on_trade_close(trade.net_pnl);
                self.metrics.record(&trade);
                self.dashboard.push_trade(&trade);
                if let Err(e) = self.trade_store.insert(&trade).await {
                    tracing::warn!("Failed to persist trade {}: {}", trade.id, e);
                }
            }
            Err(e) => {
                error!("Execution failed for signal {}: {:#}", signal.id, e);
                self.risk.on_trade_close(dec!(0));
            }
        }
    }

    async fn execute(&self, signal: &ArbitrageSignal) -> Result<CompletedTrade> {
        let qty  = signal.quantity;
        let ps   = &self.price_state;
        // Pass signal prices so paper_fill uses the actual bid/ask, not an implied price
        let buy_price  = signal.buy_ask;
        let sell_price = signal.sell_bid;

        let (buy_res, sell_res) = tokio::join!(
            self.place(signal.buy_market.exchange,  &signal.symbol, signal.buy_market.market_type,  Side::Buy,  qty, buy_price,  ps),
            self.place(signal.sell_market.exchange, &signal.symbol, signal.sell_market.market_type, Side::Sell, qty, sell_price, ps),
        );

        let buy_order  = buy_res?;
        let sell_order = sell_res?;

        let gross_pnl = (sell_order.avg_price - buy_order.avg_price) * buy_order.filled_qty;
        let fees      = buy_order.fee_usdt + sell_order.fee_usdt;
        let net_pnl   = gross_pnl - fees;

        let exec_ms = if self.config.trading.paper_trading {
            // Simulate realistic HFT latency: 30–149 ms derived from signal UUID
            30 + (signal.id.as_u128() % 120) as u64
        } else {
            sell_order.timestamp
                .signed_duration_since(signal.detected_at)
                .num_milliseconds()
                .max(0) as u64
        };

        info!(
            "TRADE {} {} | buy {}@{:.4} | sell {}@{:.4} | net_pnl={:.4} USDT | exec={}ms",
            signal.symbol, signal.id,
            signal.buy_market,  buy_order.avg_price,
            signal.sell_market, sell_order.avg_price,
            net_pnl, exec_ms
        );

        Ok(CompletedTrade {
            id: signal.id,
            signal: signal.clone(),
            buy_order,
            sell_order,
            gross_pnl,
            fees,
            net_pnl,
            exec_ms,
            completed_at: Utc::now(),
        })
    }

    async fn place(
        &self,
        exchange: Exchange,
        symbol: &str,
        market: MarketType,
        side: Side,
        qty: rust_decimal::Decimal,
        price_hint: rust_decimal::Decimal,
        ps: &SharedPriceState,
    ) -> Result<crate::types::OrderResult> {
        match exchange {
            Exchange::Binance => self.binance.place_order(symbol, market, side, qty, price_hint, ps).await,
            Exchange::Bybit   => self.bybit.place_order(symbol, market, side, qty, price_hint, ps).await,
            Exchange::Mexc    => self.mexc.place_order(symbol, side, qty, ps).await,
            Exchange::Okx     => self.okx.place_order(symbol, market, side, qty, price_hint, ps).await,
            Exchange::Bingx   => anyhow::bail!("BingX order execution not implemented"),
            Exchange::Bitget  => self.bitget.place_order(symbol, market, side, qty, price_hint, ps).await,
            Exchange::Kucoin  => self.kucoin.place_order(symbol, market, side, qty, price_hint, ps).await,
            Exchange::Gate    => self.gate.place_order(symbol, market, side, qty, price_hint, ps).await,
        }
    }
}

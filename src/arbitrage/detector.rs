use crate::{
    config::Config,
    orderbook::SharedPriceState,
    types::{ArbitrageSignal, BookTicker, MarketId, MarketType, Exchange},
};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, info};
use uuid::Uuid;

// Fee rates per market (maker rates for futures, taker for spot market orders)
fn fee_rate(market: &MarketId) -> Decimal {
    match (market.exchange, market.market_type) {
        (Exchange::Binance, MarketType::Spot)    => dec!(0.001),
        (Exchange::Binance, MarketType::Futures) => dec!(0.0002),
        (Exchange::Bybit,   MarketType::Spot)    => dec!(0.001),
        (Exchange::Bybit,   MarketType::Futures) => dec!(0.00055),
        (Exchange::Mexc,    _)                   => dec!(0.002),
    }
}

pub struct ArbitrageDetector {
    config: Arc<Config>,
    price_state: SharedPriceState,
    signal_tx: mpsc::Sender<ArbitrageSignal>,
}

impl ArbitrageDetector {
    pub fn new(
        config: Arc<Config>,
        price_state: SharedPriceState,
        signal_tx: mpsc::Sender<ArbitrageSignal>,
    ) -> Self {
        Self { config, price_state, signal_tx }
    }

    /// Runs the detection loop — wakes every millisecond to scan all market pairs.
    pub async fn run(self: Arc<Self>) {
        info!("Arbitrage detector started (min_spread={:.3}%)",
              self.config.trading.min_spread_pct * dec!(100));
        loop {
            self.scan_once();
            sleep(Duration::from_millis(1)).await;
        }
    }

    fn scan_once(&self) {
        let tickers = self.price_state.all();
        if tickers.len() < 2 { return; }

        // Consider every ordered pair (i, j) where i ≠ j
        for i in 0..tickers.len() {
            for j in 0..tickers.len() {
                if i == j { continue; }
                let buy_market  = &tickers[i]; // we will BUY here
                let sell_market = &tickers[j]; // we will SELL here

                // Skip stale tickers (older than 500 ms)
                let now = Utc::now();
                if (now - buy_market.updated_at).num_milliseconds() > 500 { continue; }
                if (now - sell_market.updated_at).num_milliseconds() > 500 { continue; }

                if let Some(signal) = self.evaluate(buy_market, sell_market) {
                    if self.signal_tx.try_send(signal).is_err() {
                        debug!("Signal channel full — dropping");
                    }
                }
            }
        }
    }

    fn evaluate(&self, buy: &BookTicker, sell: &BookTicker) -> Option<ArbitrageSignal> {
        // We buy at ask on `buy` side and sell at bid on `sell` side
        let buy_ask  = buy.ask_price;
        let sell_bid = sell.bid_price;
        if buy_ask <= dec!(0) || sell_bid <= dec!(0) { return None; }

        let slippage  = self.config.trading.max_slippage_pct;
        let eff_buy   = buy_ask  * (dec!(1) + slippage);
        let eff_sell  = sell_bid * (dec!(1) - slippage);

        let buy_fee   = eff_buy  * fee_rate(&buy.market);
        let sell_fee  = eff_sell * fee_rate(&sell.market);

        let net_cost   = eff_buy + buy_fee;
        let net_recv   = eff_sell - sell_fee;
        let spread_pct = (net_recv - net_cost) / net_cost;

        if spread_pct < self.config.trading.min_spread_pct { return None; }

        let quantity = self.config.trading.trade_size_usdt / eff_buy;
        let expected_pnl = (net_recv - net_cost) * quantity;

        info!(
            "SIGNAL buy={} ask={:.4} | sell={} bid={:.4} | spread={:.4}% | pnl≈{:.4}$",
            buy.market, buy_ask, sell.market, sell_bid,
            spread_pct * dec!(100), expected_pnl
        );

        Some(ArbitrageSignal {
            id:               Uuid::new_v4(),
            buy_market:       buy.market,
            sell_market:      sell.market,
            buy_ask:          eff_buy,
            sell_bid:         eff_sell,
            spread_pct,
            quantity,
            expected_pnl_usdt: expected_pnl,
            detected_at:      Utc::now(),
        })
    }
}

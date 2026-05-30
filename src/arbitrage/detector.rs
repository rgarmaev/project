use crate::{
    config::Config,
    orderbook::SharedPriceState,
    pricing::{as_vol_penalty, imbalance, microprice, EwmaVolatility},
    types::{ArbitrageSignal, BookTicker, Exchange, MarketId, MarketType},
};
use chrono::Utc;
use parking_lot::Mutex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
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
    // Per-market EWMA volatility (Avellaneda-Stoikov 2008 / RiskMetrics 1996)
    vol_map: Mutex<HashMap<MarketId, EwmaVolatility>>,
}

impl ArbitrageDetector {
    pub fn new(
        config: Arc<Config>,
        price_state: SharedPriceState,
        signal_tx: mpsc::Sender<ArbitrageSignal>,
    ) -> Self {
        Self {
            config,
            price_state,
            signal_tx,
            vol_map: Mutex::new(HashMap::new()),
        }
    }

    /// Runs the detection loop — wakes every millisecond to scan all market pairs.
    pub async fn run(self: Arc<Self>) {
        info!(
            "Arbitrage detector started (min_spread={:.3}%  γ={}  k={}  τ={})",
            self.config.trading.min_spread_pct * dec!(100),
            self.config.trading.gamma,
            self.config.trading.k,
            self.config.trading.tau,
        );
        loop {
            self.scan_once();
            sleep(Duration::from_millis(1)).await;
        }
    }

    fn scan_once(&self) {
        let tickers = self.price_state.all();
        if tickers.len() < 2 {
            return;
        }

        // ── Update EWMA volatilities and snapshot current variances ──────────
        let variances: HashMap<MarketId, f64> = {
            let mut vol = self.vol_map.lock();
            for t in &tickers {
                let mid = (t.bid_price + t.ask_price) / dec!(2);
                vol.entry(t.market).or_insert_with(EwmaVolatility::new).update(mid);
            }
            // Expose sigma to PriceState so dashboard can read it
            for (market, ewma) in vol.iter() {
                if let Some(sigma) = ewma.sigma() {
                    self.price_state.set_sigma(*market, sigma);
                }
            }
            vol.iter()
                .map(|(k, v)| (*k, v.variance().unwrap_or(0.0)))
                .collect()
        };

        let now = Utc::now();
        for i in 0..tickers.len() {
            for j in 0..tickers.len() {
                if i == j {
                    continue;
                }
                let buy = &tickers[i];
                let sell = &tickers[j];

                if (now - buy.updated_at).num_milliseconds() > 500 { continue; }
                if (now - sell.updated_at).num_milliseconds() > 500 { continue; }

                if let Some(signal) = self.evaluate(buy, sell, &variances) {
                    if self.signal_tx.try_send(signal).is_err() {
                        debug!("Signal channel full — dropping");
                    }
                }
            }
        }
    }

    fn evaluate(
        &self,
        buy: &BookTicker,
        sell: &BookTicker,
        variances: &HashMap<MarketId, f64>,
    ) -> Option<ArbitrageSignal> {
        let buy_ask  = buy.ask_price;
        let sell_bid = sell.bid_price;
        if buy_ask <= dec!(0) || sell_bid <= dec!(0) { return None; }

        // ── 1. Microprice (Stoikov 2018): volume-weighted fair price ──────────
        // Replaces simple mid for fair-value comparison.
        let buy_mp = microprice(buy.bid_price, buy.bid_qty, buy.ask_price, buy.ask_qty)
            .unwrap_or_else(|| (buy.bid_price + buy.ask_price) / dec!(2));
        let sell_mp = microprice(sell.bid_price, sell.bid_qty, sell.ask_price, sell.ask_qty)
            .unwrap_or_else(|| (sell.bid_price + sell.ask_price) / dec!(2));

        // Microprice sanity check: if fair prices don't confirm the direction, skip.
        // Prevents chasing noise where actual bid/ask looks profitable but fair value disagrees.
        if sell_mp <= buy_mp { return None; }

        // ── 2. Imbalance filter ───────────────────────────────────────────────
        // High positive imbalance on buy side → strong buying pressure →
        //   we compete with aggressive buyers, execution price worse.
        // High negative imbalance on sell side → strong selling pressure →
        //   we compete with aggressive sellers, fill price worse.
        let buy_imb  = imbalance(buy.bid_qty, buy.ask_qty);
        let sell_imb = imbalance(sell.bid_qty, sell.ask_qty);
        let thresh = self.config.trading.imbalance_threshold;
        if buy_imb > thresh || sell_imb < -thresh {
            debug!(
                "Signal skipped: adverse imbalance buy_imb={:.2} sell_imb={:.2}",
                buy_imb, sell_imb
            );
            return None;
        }

        // ── 3. Standard spread calculation ───────────────────────────────────
        let slippage = self.config.trading.max_slippage_pct;
        let eff_buy  = buy_ask  * (dec!(1) + slippage);
        let eff_sell = sell_bid * (dec!(1) - slippage);

        let buy_fee  = eff_buy  * fee_rate(&buy.market);
        let sell_fee = eff_sell * fee_rate(&sell.market);

        let net_cost = eff_buy + buy_fee;
        let net_recv = eff_sell - sell_fee;
        let spread_pct = (net_recv - net_cost) / net_cost;

        // ── 4. AS-2008 volatility-adjusted minimum spread ─────────────────────
        // effective_min = base_min + γ·σ²·τ
        //
        // When volatility rises, we require a larger spread to compensate for
        // the increased adverse-selection risk during execution latency.
        let buy_var  = variances.get(&buy.market).copied().unwrap_or(0.0);
        let sell_var = variances.get(&sell.market).copied().unwrap_or(0.0);
        let avg_var  = (buy_var + sell_var) / 2.0;
        let vol_penalty = as_vol_penalty(avg_var, self.config.trading.gamma, self.config.trading.tau);
        let vol_penalty_dec = Decimal::try_from(vol_penalty).unwrap_or(dec!(0)).max(dec!(0));
        let effective_min = self.config.trading.min_spread_pct + vol_penalty_dec;

        if spread_pct < effective_min { return None; }

        let quantity = self.config.trading.trade_size_usdt / eff_buy;
        let expected_pnl = (net_recv - net_cost) * quantity;

        info!(
            "SIGNAL buy={} ask={:.4} mp={:.4} imb={:.2} | sell={} bid={:.4} mp={:.4} imb={:.2} | spread={:.4}% ≥ min={:.4}%(+σ={:.4}%) | pnl≈{:.4}$",
            buy.market,  buy_ask,  buy_mp,  buy_imb,
            sell.market, sell_bid, sell_mp, sell_imb,
            spread_pct * dec!(100),
            self.config.trading.min_spread_pct * dec!(100),
            vol_penalty_dec * dec!(100),
            expected_pnl
        );

        Some(ArbitrageSignal {
            id: Uuid::new_v4(),
            buy_market: buy.market,
            sell_market: sell.market,
            buy_ask: eff_buy,
            sell_bid: eff_sell,
            spread_pct,
            quantity,
            expected_pnl_usdt: expected_pnl,
            detected_at: Utc::now(),
        })
    }
}

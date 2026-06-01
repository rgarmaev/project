use crate::{
    config::Config,
    multi_feed::{MarketQuote, MultiPairState, MultiPairTick},
    types::{ArbitrageSignal, Exchange, MarketId, MarketType},
};
use chrono::Utc;
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ── Fee rates (taker, no VIP) ─────────────────────────────────────────────────

fn fee_rate(market: &MarketId) -> f64 {
    match (market.exchange, market.market_type) {
        (Exchange::Binance, MarketType::Spot)    => 0.00100,
        (Exchange::Binance, MarketType::Futures) => 0.00050,
        (Exchange::Bybit,   MarketType::Spot)    => 0.00100,
        (Exchange::Bybit,   MarketType::Futures) => 0.00055,
        (Exchange::Okx,     MarketType::Spot)    => 0.00080,
        (Exchange::Okx,     MarketType::Futures) => 0.00050,
        (Exchange::Bingx,   MarketType::Spot)    => 0.00100,
        (Exchange::Bingx,   MarketType::Futures) => 0.00050,
        _                                         => 0.00100,
    }
}

// ── f64 pricing helpers ───────────────────────────────────────────────────────

fn microprice(bid: f64, bid_qty: f64, ask: f64, ask_qty: f64) -> Option<f64> {
    let total = bid_qty + ask_qty;
    if total == 0.0 { return None; }
    Some((bid * ask_qty + ask * bid_qty) / total)
}

fn imbalance(bid_qty: f64, ask_qty: f64) -> f64 {
    let total = bid_qty + ask_qty;
    if total == 0.0 { return 0.0; }
    (bid_qty - ask_qty) / total
}

fn to_dec(v: f64) -> Decimal {
    v.to_string().parse().unwrap_or(Decimal::ZERO)
}

fn to_f64(d: Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

// ── EWMA volatility (λ=0.94, f64 internals) ──────────────────────────────────

struct F64Ewma {
    lambda:   f64,
    variance: f64,
    last_mid: Option<f64>,
    n:        u64,
}

impl F64Ewma {
    fn new() -> Self { Self { lambda: 0.94, variance: 0.0, last_mid: None, n: 0 } }

    fn update(&mut self, mid: f64) {
        if let Some(prev) = self.last_mid {
            if prev > 0.0 {
                let r = (mid - prev) / prev;
                self.variance = self.lambda * self.variance + (1.0 - self.lambda) * r * r;
                self.n += 1;
            }
        }
        self.last_mid = Some(mid);
    }

    fn variance(&self) -> Option<f64> {
        if self.n >= 2 { Some(self.variance) } else { None }
    }
}

// ── Market field enum ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Field {
    SpotBinance, PerpBinance,
    SpotBybit,   PerpBybit,
    SpotOkx,     PerpOkx,
    SpotBingx,   PerpBingx,
}

impl Field {
    fn get<'a>(&self, t: &'a MultiPairTick) -> Option<&'a MarketQuote> {
        match self {
            Self::SpotBinance => t.spot_binance.as_ref(),
            Self::PerpBinance => t.perp_binance.as_ref(),
            Self::SpotBybit   => t.spot_bybit.as_ref(),
            Self::PerpBybit   => t.perp_bybit.as_ref(),
            Self::SpotOkx     => t.spot_okx.as_ref(),
            Self::PerpOkx     => t.perp_okx.as_ref(),
            Self::SpotBingx   => t.spot_bingx.as_ref(),
            Self::PerpBingx   => t.perp_bingx.as_ref(),
        }
    }

    fn market_id(self) -> MarketId {
        match self {
            Self::SpotBinance => MarketId::new(Exchange::Binance, MarketType::Spot),
            Self::PerpBinance => MarketId::new(Exchange::Binance, MarketType::Futures),
            Self::SpotBybit   => MarketId::new(Exchange::Bybit,   MarketType::Spot),
            Self::PerpBybit   => MarketId::new(Exchange::Bybit,   MarketType::Futures),
            Self::SpotOkx     => MarketId::new(Exchange::Okx,     MarketType::Spot),
            Self::PerpOkx     => MarketId::new(Exchange::Okx,     MarketType::Futures),
            Self::SpotBingx   => MarketId::new(Exchange::Bingx,   MarketType::Spot),
            Self::PerpBingx   => MarketId::new(Exchange::Bingx,   MarketType::Futures),
        }
    }

    fn idx(self) -> u8 {
        match self {
            Self::SpotBinance => 0,
            Self::PerpBinance => 1,
            Self::SpotBybit   => 2,
            Self::PerpBybit   => 3,
            Self::SpotOkx     => 4,
            Self::PerpOkx     => 5,
            Self::SpotBingx   => 6,
            Self::PerpBingx   => 7,
        }
    }
}

const FIELDS: [Field; 8] = [
    Field::SpotBinance, Field::PerpBinance,
    Field::SpotBybit,   Field::PerpBybit,
    Field::SpotOkx,     Field::PerpOkx,
    Field::SpotBingx,   Field::PerpBingx,
];

// ── Detector ──────────────────────────────────────────────────────────────────

pub struct MultiPairDetector {
    config:    Arc<Config>,
    state:     MultiPairState,
    signal_tx: mpsc::Sender<ArbitrageSignal>,
    vol_map:   DashMap<(String, u8), F64Ewma>,
    last_mids: DashMap<(String, u8), f64>,
}

impl MultiPairDetector {
    pub fn new(
        config: Arc<Config>,
        state: MultiPairState,
        signal_tx: mpsc::Sender<ArbitrageSignal>,
    ) -> Self {
        Self {
            config,
            state,
            signal_tx,
            vol_map:   DashMap::new(),
            last_mids: DashMap::new(),
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!(
            "MultiPairDetector started — 8 markets × 56 combos per pair (min_spread={:.3}%  γ={}  τ={})",
            to_f64(self.config.trading.min_spread_pct) * 100.0,
            self.config.trading.gamma,
            self.config.trading.tau,
        );
        loop {
            self.scan_once();
            sleep(Duration::from_millis(1)).await;
        }
    }

    fn scan_once(&self) {
        let stale = Duration::from_millis(500);

        // ── Pass 1: update EWMA volatilities, snapshot variances ─────────────
        let mut variances: HashMap<(String, u8), f64> = HashMap::new();
        for entry in self.state.iter() {
            let sym  = entry.key();
            let tick = entry.value();
            for &field in &FIELDS {
                if let Some(q) = field.get(tick) {
                    let mid = (q.bid + q.ask) / 2.0;
                    let key = (sym.clone(), field.idx());
                    let changed = self.last_mids.get(&key)
                        .map(|prev| (*prev - mid).abs() > 1e-9)
                        .unwrap_or(true);
                    if changed {
                        self.last_mids.insert(key.clone(), mid);
                        self.vol_map
                            .entry(key.clone())
                            .and_modify(|e| e.update(mid))
                            .or_insert_with(|| { let mut e = F64Ewma::new(); e.update(mid); e });
                    }
                    if let Some(var) = self.vol_map.get(&key).and_then(|e| e.variance()) {
                        variances.insert(key, var);
                    }
                }
            }
        }

        // ── Pass 2: detect opportunities ─────────────────────────────────────
        for entry in self.state.iter() {
            let sym  = entry.key();
            let tick = entry.value();
            if tick.updated_at.elapsed() > stale { continue; }

            for &buy_field in &FIELDS {
                for &sell_field in &FIELDS {
                    if buy_field == sell_field { continue; }
                    if let (Some(buy_q), Some(sell_q)) = (buy_field.get(tick), sell_field.get(tick)) {
                        if buy_q.updated_at.elapsed() > stale || sell_q.updated_at.elapsed() > stale { continue; }
                        let buy_var  = variances.get(&(sym.clone(), buy_field.idx())).copied().unwrap_or(0.0);
                        let sell_var = variances.get(&(sym.clone(), sell_field.idx())).copied().unwrap_or(0.0);
                        let avg_var  = (buy_var + sell_var) / 2.0;
                        if let Some(signal) = self.evaluate(
                            sym,
                            buy_field.market_id(), buy_q,
                            sell_field.market_id(), sell_q,
                            avg_var,
                        ) {
                            if self.signal_tx.try_send(signal).is_err() {
                                warn!("Signal channel full — dropping");
                            }
                        }
                    }
                }
            }
        }
    }

    fn evaluate(
        &self,
        symbol:      &str,
        buy_market:  MarketId,
        buy_q:       &MarketQuote,
        sell_market: MarketId,
        sell_q:      &MarketQuote,
        avg_var:     f64,
    ) -> Option<ArbitrageSignal> {
        let buy_ask  = buy_q.ask;
        let sell_bid = sell_q.bid;
        if buy_ask <= 0.0 || sell_bid <= 0.0 { return None; }

        // ── 1. Microprice ─────────────────────────────────────────────────────
        let buy_mp  = microprice(buy_q.bid,  buy_q.bid_qty,  buy_q.ask,  buy_q.ask_qty)
            .unwrap_or((buy_q.bid  + buy_q.ask)  / 2.0);
        let sell_mp = microprice(sell_q.bid, sell_q.bid_qty, sell_q.ask, sell_q.ask_qty)
            .unwrap_or((sell_q.bid + sell_q.ask) / 2.0);

        let mp_spread = if buy_mp > 0.0 { (sell_mp - buy_mp) / buy_mp } else { 0.0 };
        if mp_spread < -0.005 {
            debug!(
                "{} skipped: microprice disagrees ({:.3}%)",
                symbol, mp_spread * 100.0
            );
            return None;
        }

        // ── 2. Imbalance ──────────────────────────────────────────────────────
        let buy_imb  = imbalance(buy_q.bid_qty,  buy_q.ask_qty);
        let sell_imb = imbalance(sell_q.bid_qty, sell_q.ask_qty);
        let thresh   = self.config.trading.imbalance_threshold;
        if buy_imb > thresh || sell_imb < -thresh {
            debug!(
                "{} skipped: adverse imbalance buy={:.2} sell={:.2}",
                symbol, buy_imb, sell_imb
            );
            return None;
        }

        // ── 3. Fee-adjusted spread ────────────────────────────────────────────
        let slippage = to_f64(self.config.trading.max_slippage_pct);
        let eff_buy  = buy_ask  * (1.0 + slippage);
        let eff_sell = sell_bid * (1.0 - slippage);
        let net_cost = eff_buy  * (1.0 + fee_rate(&buy_market));
        let net_recv = eff_sell * (1.0 - fee_rate(&sell_market));
        let spread   = (net_recv - net_cost) / net_cost;

        // ── 4. Vol-adjusted minimum spread ────────────────────────────────────
        let min_spread  = to_f64(self.config.trading.min_spread_pct);
        let vol_penalty = self.config.trading.gamma * avg_var * self.config.trading.tau;
        let eff_min     = min_spread + vol_penalty;

        if spread < eff_min { return None; }

        let trade_usdt = to_f64(self.config.trading.trade_size_usdt);
        let quantity   = trade_usdt / eff_buy;

        info!(
            "SIGNAL {} buy={} ask={:.4} mp={:.4} imb={:.2} | sell={} bid={:.4} mp={:.4} imb={:.2} | spread={:.4}% ≥ min={:.4}%",
            symbol,
            buy_market,  buy_ask,  buy_mp,  buy_imb,
            sell_market, sell_bid, sell_mp, sell_imb,
            spread * 100.0, eff_min * 100.0,
        );

        Some(ArbitrageSignal {
            id:                Uuid::new_v4(),
            symbol:            symbol.to_string(),
            buy_market,
            sell_market,
            buy_ask:           to_dec(eff_buy),
            sell_bid:          to_dec(eff_sell),
            spread_pct:        to_dec(spread),
            quantity:          to_dec(quantity),
            expected_pnl_usdt: to_dec((net_recv - net_cost) * quantity),
            detected_at:       Utc::now(),
        })
    }
}

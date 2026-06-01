use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

pub mod binance;
pub mod bybit;
pub mod okx;
pub mod bingx;

pub use binance::{run_binance_spot, run_binance_perp};
pub use bybit::{run_bybit_spot, run_bybit_linear};
pub use okx::{run_okx_spot, run_okx_swap};
pub use bingx::{run_bingx_spot, run_bingx_swap};

#[derive(Debug, Clone)]
pub struct MarketQuote {
    pub bid:        f64,
    pub ask:        f64,
    pub bid_qty:    f64,
    pub ask_qty:    f64,
    pub updated_at: Instant,
}

#[derive(Debug, Clone)]
pub struct MultiPairTick {
    pub spot_binance: Option<MarketQuote>,
    pub perp_binance: Option<MarketQuote>,
    pub spot_bybit:   Option<MarketQuote>,
    pub perp_bybit:   Option<MarketQuote>,
    pub spot_okx:     Option<MarketQuote>,
    pub perp_okx:     Option<MarketQuote>,
    pub spot_bingx:   Option<MarketQuote>,
    pub perp_bingx:   Option<MarketQuote>,
    pub updated_at:   Instant,
}

pub type MultiPairState = Arc<DashMap<String, MultiPairTick>>;

pub fn new_state() -> MultiPairState {
    Arc::new(DashMap::new())
}

/// "BTCUSDT" → "BTC-USDT"
pub fn to_dashed(ticker: &str) -> String {
    format!("{}-{}", &ticker[..ticker.len()-4], &ticker[ticker.len()-4..])
}

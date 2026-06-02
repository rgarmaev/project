use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

pub mod binance;
pub mod bybit;
pub mod okx;
pub mod bingx;
pub mod bitget;
pub mod kucoin;
pub mod gate;

pub use binance::{run_binance_spot, run_binance_perp};
pub use bybit::{run_bybit_spot, run_bybit_linear};
pub use okx::{run_okx_spot, run_okx_swap};
pub use bingx::{run_bingx_spot, run_bingx_swap};
pub use bitget::{run_bitget_spot, run_bitget_futures};
pub use kucoin::{run_kucoin_spot, run_kucoin_futures};
pub use gate::{run_gate_spot, run_gate_futures};

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
    pub spot_bitget:  Option<MarketQuote>,
    pub perp_bitget:  Option<MarketQuote>,
    pub spot_kucoin:  Option<MarketQuote>,
    pub perp_kucoin:  Option<MarketQuote>,
    pub spot_gate:    Option<MarketQuote>,
    pub perp_gate:    Option<MarketQuote>,
    pub updated_at:   Instant,
}

pub type MultiPairState = Arc<DashMap<String, MultiPairTick>>;

pub fn new_state() -> MultiPairState {
    Arc::new(DashMap::new())
}

/// "BTCUSDT" → "BTC-USDT"  (OKX, KuCoin spot, BingX)
pub fn to_dashed(ticker: &str) -> String {
    format!("{}-{}", &ticker[..ticker.len()-4], &ticker[ticker.len()-4..])
}

/// "BTCUSDT" → "BTC_USDT"  (Gate.io)
pub fn to_underscored(ticker: &str) -> String {
    format!("{}_{}", &ticker[..ticker.len()-4], &ticker[ticker.len()-4..])
}

/// "BTCUSDT" → "XBTUSDTM" for BTC, "ETHUSDTM" for others  (KuCoin futures)
pub fn to_kucoin_futures(ticker: &str) -> String {
    let base = &ticker[..ticker.len()-4];
    let kucoin_base = if base == "BTC" { "XBT" } else { base };
    format!("{}USDTM", kucoin_base)
}

/// Blank MultiPairTick — used in or_insert_with
pub fn blank_tick() -> MultiPairTick {
    MultiPairTick {
        spot_binance: None,
        perp_binance: None,
        spot_bybit:   None,
        perp_bybit:   None,
        spot_okx:     None,
        perp_okx:     None,
        spot_bingx:   None,
        perp_bingx:   None,
        spot_bitget:  None,
        perp_bitget:  None,
        spot_kucoin:  None,
        perp_kucoin:  None,
        spot_gate:    None,
        perp_gate:    None,
        updated_at:   Instant::now(),
    }
}

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;

pub struct MultiPairTick {
    pub spot_binance: Option<(f64, f64)>,  // (bid, ask)
    pub perp_binance: Option<(f64, f64)>,
    pub spot_bybit:   Option<(f64, f64)>,
    pub perp_bybit:   Option<(f64, f64)>,
    pub updated_at:   Instant,
}

pub type MultiPairState = Arc<DashMap<String, MultiPairTick>>;

pub fn new_state() -> MultiPairState {
    Arc::new(DashMap::new())
}

pub async fn run_binance_spot(_state: MultiPairState) { todo!() }
pub async fn run_binance_perp(_state: MultiPairState) { todo!() }
pub async fn run_bybit_spot(_state: MultiPairState)   { todo!() }
pub async fn run_bybit_linear(_state: MultiPairState) { todo!() }

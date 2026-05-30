use crate::types::{BookTicker, MarketId};
use dashmap::DashMap;
use std::sync::Arc;

/// Thread-safe snapshot of best bid/ask + per-market EWMA sigma.
#[derive(Debug, Default)]
pub struct PriceState {
    tickers: DashMap<MarketId, BookTicker>,
    sigmas: DashMap<MarketId, f64>,
}

impl PriceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&self, ticker: BookTicker) {
        self.tickers.insert(ticker.market, ticker);
    }

    pub fn get(&self, market: &MarketId) -> Option<BookTicker> {
        self.tickers.get(market).map(|e| e.value().clone())
    }

    pub fn all(&self) -> Vec<BookTicker> {
        self.tickers.iter().map(|e| e.value().clone()).collect()
    }

    /// Called by ArbitrageDetector after each EWMA update cycle.
    pub fn set_sigma(&self, market: MarketId, sigma: f64) {
        self.sigmas.insert(market, sigma);
    }

    pub fn get_sigma(&self, market: &MarketId) -> Option<f64> {
        self.sigmas.get(market).map(|v| *v)
    }

    pub fn all_sigmas(&self) -> Vec<(MarketId, f64)> {
        self.sigmas.iter().map(|e| (*e.key(), *e.value())).collect()
    }
}

pub type SharedPriceState = Arc<PriceState>;

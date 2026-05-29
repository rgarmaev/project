use crate::types::{BookTicker, MarketId};
use dashmap::DashMap;
use std::sync::Arc;

/// Thread-safe snapshot of best bid/ask across all connected markets.
#[derive(Debug, Default)]
pub struct PriceState {
    tickers: DashMap<MarketId, BookTicker>,
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
}

pub type SharedPriceState = Arc<PriceState>;

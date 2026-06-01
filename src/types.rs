use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Binance,
    Bybit,
    Mexc,
    Okx,
    Bingx,
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exchange::Binance => write!(f, "Binance"),
            Exchange::Bybit   => write!(f, "Bybit"),
            Exchange::Mexc    => write!(f, "MEXC"),
            Exchange::Okx     => write!(f, "OKX"),
            Exchange::Bingx   => write!(f, "BingX"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketType {
    Spot,
    Futures,
}

impl fmt::Display for MarketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarketType::Spot => write!(f, "Spot"),
            MarketType::Futures => write!(f, "Perp"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarketId {
    pub exchange: Exchange,
    pub market_type: MarketType,
}

impl MarketId {
    pub fn new(exchange: Exchange, market_type: MarketType) -> Self {
        Self { exchange, market_type }
    }
}

impl fmt::Display for MarketId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.exchange, self.market_type)
    }
}

#[derive(Debug, Clone)]
pub struct BookTicker {
    pub market: MarketId,
    pub bid_price: Decimal,
    pub bid_qty: Decimal,
    pub ask_price: Decimal,
    pub ask_qty: Decimal,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArbitrageSignal {
    pub id: Uuid,
    pub symbol: String,
    pub buy_market: MarketId,
    pub sell_market: MarketId,
    /// Price we'll pay on the buy leg (ask + slippage)
    pub buy_ask: Decimal,
    /// Price we'll receive on the sell leg (bid - slippage)
    pub sell_bid: Decimal,
    pub spread_pct: Decimal,
    pub quantity: Decimal,
    pub expected_pnl_usdt: Decimal,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResult {
    pub exchange: Exchange,
    pub market_type: MarketType,
    pub order_id: String,
    pub side: Side,
    pub filled_qty: Decimal,
    pub avg_price: Decimal,
    pub fee_usdt: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CompletedTrade {
    pub id: Uuid,
    pub signal: ArbitrageSignal,
    pub buy_order: OrderResult,
    pub sell_order: OrderResult,
    pub gross_pnl: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub exec_ms: u64,
    pub completed_at: DateTime<Utc>,
}

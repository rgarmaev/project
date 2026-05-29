use crate::{metrics::MetricsCollector, orderbook::PriceState, types::CompletedTrade};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{collections::VecDeque, sync::Arc};
use parking_lot::Mutex;
use tokio::sync::broadcast;

const MAX_TRADES: usize = 500;

#[derive(Serialize, Clone)]
pub struct TradeRecord {
    pub id: String,
    pub buy_market: String,
    pub sell_market: String,
    pub spread_pct: f64,
    pub gross_pnl: f64,
    pub fees: f64,
    pub net_pnl: f64,
    pub exec_ms: u64,
    pub time: DateTime<Utc>,
}

impl From<&CompletedTrade> for TradeRecord {
    fn from(t: &CompletedTrade) -> Self {
        fn d(v: rust_decimal::Decimal) -> f64 {
            v.to_string().parse().unwrap_or(0.0)
        }
        Self {
            id: t.id.to_string(),
            buy_market: t.signal.buy_market.to_string(),
            sell_market: t.signal.sell_market.to_string(),
            spread_pct: d(t.signal.spread_pct),
            gross_pnl: d(t.gross_pnl),
            fees: d(t.fees),
            net_pnl: d(t.net_pnl),
            exec_ms: t.exec_ms,
            time: t.completed_at,
        }
    }
}

#[derive(Serialize, Clone)]
pub struct PriceEntry {
    pub exchange: String,
    pub market: String,
    pub bid: f64,
    pub ask: f64,
    pub spread_pct: f64,
    pub stale: bool,
}

#[derive(Serialize, Clone)]
pub struct WsSnapshot {
    pub metrics: crate::metrics::MetricsSnapshot,
    pub prices: Vec<PriceEntry>,
    pub recent_trades: Vec<TradeRecord>,
}

pub struct DashboardState {
    trades: Mutex<VecDeque<TradeRecord>>,
    pub price_state: Arc<PriceState>,
    pub metrics: Arc<MetricsCollector>,
    pub broadcast_tx: broadcast::Sender<String>,
}

impl DashboardState {
    pub fn new(price_state: Arc<PriceState>, metrics: Arc<MetricsCollector>) -> Arc<Self> {
        let (broadcast_tx, _) = broadcast::channel(64);
        Arc::new(Self {
            trades: Mutex::new(VecDeque::with_capacity(MAX_TRADES)),
            price_state,
            metrics,
            broadcast_tx,
        })
    }

    pub fn push_trade(&self, trade: &CompletedTrade) {
        let mut buf = self.trades.lock();
        if buf.len() == MAX_TRADES {
            buf.pop_front();
        }
        buf.push_back(TradeRecord::from(trade));
    }

    pub fn recent_trades(&self, limit: usize) -> Vec<TradeRecord> {
        let buf = self.trades.lock();
        buf.iter().rev().take(limit).cloned().collect()
    }

    pub fn build_snapshot(&self) -> WsSnapshot {
        fn d(v: rust_decimal::Decimal) -> f64 {
            v.to_string().parse().unwrap_or(0.0)
        }
        let prices = self.price_state.all().into_iter().map(|t| {
            let stale = (Utc::now() - t.updated_at).num_seconds() > 5;
            let bid = d(t.bid_price);
            let ask = d(t.ask_price);
            let spread_pct = if bid > 0.0 { (ask - bid) / bid * 100.0 } else { 0.0 };
            PriceEntry {
                exchange: t.market.exchange.to_string(),
                market: t.market.market_type.to_string(),
                bid,
                ask,
                spread_pct,
                stale,
            }
        }).collect();

        WsSnapshot {
            metrics: self.metrics.snapshot(),
            prices,
            recent_trades: self.recent_trades(50),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{metrics::MetricsCollector, orderbook::PriceState};

    fn make_state() -> Arc<DashboardState> {
        DashboardState::new(Arc::new(PriceState::new()), Arc::new(MetricsCollector::new()))
    }

    #[test]
    fn ring_buffer_caps_at_500() {
        let state = make_state();
        {
            let mut buf = state.trades.lock();
            for i in 0u64..501 {
                if buf.len() == MAX_TRADES {
                    buf.pop_front();
                }
                buf.push_back(TradeRecord {
                    id: i.to_string(),
                    buy_market: "A".into(),
                    sell_market: "B".into(),
                    spread_pct: 0.1,
                    gross_pnl: 1.0,
                    fees: 0.1,
                    net_pnl: 0.9,
                    exec_ms: i,
                    time: Utc::now(),
                });
            }
        }
        assert_eq!(state.recent_trades(1000).len(), MAX_TRADES);
    }

    #[test]
    fn recent_trades_returns_most_recent_first() {
        let state = make_state();
        {
            let mut buf = state.trades.lock();
            for i in 0u64..5 {
                buf.push_back(TradeRecord {
                    id: i.to_string(),
                    buy_market: "A".into(),
                    sell_market: "B".into(),
                    spread_pct: 0.0,
                    gross_pnl: 0.0,
                    fees: 0.0,
                    net_pnl: 0.0,
                    exec_ms: i,
                    time: Utc::now(),
                });
            }
        }
        let trades = state.recent_trades(3);
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].exec_ms, 4);
        assert_eq!(trades[1].exec_ms, 3);
    }
}

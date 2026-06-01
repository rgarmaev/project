use crate::{
    config::Config,
    market_scanner::{MarketScanner, MarketSnapshot},
    metrics::MetricsCollector,
    multi_feed::MultiPairState,
    orderbook::PriceState,
    pricing::imbalance,
    pricing::microprice,
    types::CompletedTrade,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{collections::VecDeque, sync::Arc, time::Duration};
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
    pub buy_ask: f64,
    pub sell_bid: f64,
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
            buy_ask: d(t.signal.buy_ask),
            sell_bid: d(t.signal.sell_bid),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct PriceEntry {
    pub exchange: String,
    pub market: String,
    pub bid: f64,
    pub ask: f64,
    pub microprice: f64,     // Stoikov 2018: volume-weighted fair price
    pub spread_pct: f64,
    pub imbalance: f64,      // (bid_qty - ask_qty) / (bid_qty + ask_qty) ∈ [-1, 1]
    pub sigma_pct: f64,      // EWMA σ expressed as %, e.g. 0.05 = 0.05%
    pub stale: bool,
}

#[derive(Serialize, Clone)]
pub struct OpportunityRow {
    pub symbol:      String,
    pub buy_market:  String,
    pub sell_market: String,
    pub spread_pct:  f64,
    pub ask:         f64,
    pub bid:         f64,
}

#[derive(Serialize, Clone)]
pub struct WsSnapshot {
    pub metrics: crate::metrics::MetricsSnapshot,
    pub prices: Vec<PriceEntry>,
    pub recent_trades: Vec<TradeRecord>,
    pub effective_min_spread_pct: f64,  // AS-2008 vol-adjusted threshold (in %)
    pub symbol: String,
    pub top_opportunities: Vec<OpportunityRow>,
}

pub struct DashboardState {
    trades: Mutex<VecDeque<TradeRecord>>,
    pub price_state: Arc<PriceState>,
    pub metrics: Arc<MetricsCollector>,
    pub broadcast_tx: broadcast::Sender<String>,
    pub config: Arc<Config>,
    pub scanner: Arc<MarketScanner>,
    pub multi_feed: MultiPairState,
}

impl DashboardState {
    pub fn new(
        price_state: Arc<PriceState>,
        metrics: Arc<MetricsCollector>,
        config: Arc<Config>,
        scanner: Arc<MarketScanner>,
        multi_feed: MultiPairState,
    ) -> Arc<Self> {
        let (broadcast_tx, _) = broadcast::channel(64);
        Arc::new(Self {
            trades: Mutex::new(VecDeque::with_capacity(MAX_TRADES)),
            price_state,
            metrics,
            broadcast_tx,
            config,
            scanner,
            multi_feed,
        })
    }

    pub fn market_snapshot(&self) -> MarketSnapshot {
        self.scanner.full_snapshot()
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

        let tickers = self.price_state.all();
        let mut total_sigma: f64 = 0.0;
        let mut sigma_count: usize = 0;

        let prices: Vec<PriceEntry> = tickers.into_iter().map(|t| {
            let stale = (Utc::now() - t.updated_at).num_seconds() > 5;
            let bid = d(t.bid_price);
            let ask = d(t.ask_price);
            let spread_pct = if bid > 0.0 { (ask - bid) / bid * 100.0 } else { 0.0 };

            let mp = microprice(t.bid_price, t.bid_qty, t.ask_price, t.ask_qty)
                .map(|v| d(v))
                .unwrap_or((bid + ask) / 2.0);

            let imb = imbalance(t.bid_qty, t.ask_qty);

            let sigma = self.price_state.get_sigma(&t.market).unwrap_or(0.0);
            let sigma_pct = sigma * 100.0;
            if sigma > 0.0 {
                total_sigma += sigma;
                sigma_count += 1;
            }

            PriceEntry {
                exchange: t.market.exchange.to_string(),
                market: t.market.market_type.to_string(),
                bid,
                ask,
                microprice: mp,
                spread_pct,
                imbalance: imb,
                sigma_pct,
                stale,
            }
        }).collect();

        // AS-2008 effective minimum spread (in %) = base_min + γ·σ²_avg·τ
        // Read all parameters from live config — updates immediately when settings change
        let avg_sigma = if sigma_count > 0 { total_sigma / sigma_count as f64 } else { 0.0 };
        let avg_var = avg_sigma * avg_sigma;
        let gamma = self.config.trading.gamma;
        let tau   = self.config.trading.tau;
        let base_min_pct = d(self.config.trading.min_spread_pct) * 100.0;
        let effective_min_spread_pct = base_min_pct + gamma * avg_var * tau * 100.0;

        let stale = Duration::from_millis(500);
        let mut opps: Vec<OpportunityRow> = Vec::new();

        for entry in self.multi_feed.iter() {
            let sym  = entry.key();
            let tick = entry.value();
            if tick.updated_at.elapsed() > stale { continue; }

            let mut quotes: Vec<(&str, f64, f64)> = Vec::new();
            if let Some(q) = &tick.spot_binance {
                if q.updated_at.elapsed() <= stale { quotes.push(("Binance:Spot", q.bid, q.ask)); }
            }
            if let Some(q) = &tick.perp_binance {
                if q.updated_at.elapsed() <= stale { quotes.push(("Binance:Perp", q.bid, q.ask)); }
            }
            if let Some(q) = &tick.spot_bybit {
                if q.updated_at.elapsed() <= stale { quotes.push(("Bybit:Spot", q.bid, q.ask)); }
            }
            if let Some(q) = &tick.perp_bybit {
                if q.updated_at.elapsed() <= stale { quotes.push(("Bybit:Perp", q.bid, q.ask)); }
            }

            let mut best: Option<OpportunityRow> = None;
            for i in 0..quotes.len() {
                for j in 0..quotes.len() {
                    if i == j { continue; }
                    let (buy_name, _, buy_ask) = quotes[i];
                    let (sell_name, sell_bid, _) = quotes[j];
                    if buy_ask <= 0.0 || sell_bid <= 0.0 { continue; }
                    let spread_pct = (sell_bid - buy_ask) / buy_ask * 100.0;
                    let replace = best.as_ref().map(|b| spread_pct > b.spread_pct).unwrap_or(true);
                    if replace {
                        best = Some(OpportunityRow {
                            symbol:      sym.clone(),
                            buy_market:  buy_name.to_string(),
                            sell_market: sell_name.to_string(),
                            spread_pct,
                            ask:         buy_ask,
                            bid:         sell_bid,
                        });
                    }
                }
            }
            if let Some(row) = best { opps.push(row); }
        }

        opps.sort_by(|a, b| b.spread_pct.partial_cmp(&a.spread_pct).unwrap_or(std::cmp::Ordering::Equal));
        opps.truncate(250);

        WsSnapshot {
            metrics: self.metrics.snapshot(),
            prices,
            recent_trades: self.recent_trades(50),
            effective_min_spread_pct,
            symbol: self.config.pair(),
            top_opportunities: opps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, metrics::MetricsCollector, orderbook::PriceState};

    fn make_state() -> Arc<DashboardState> {
        let config = Arc::new(Config::load().unwrap());
        let scanner = crate::market_scanner::MarketScanner::new();
        let multi_feed = crate::multi_feed::new_state();
        DashboardState::new(
            Arc::new(PriceState::new()),
            Arc::new(MetricsCollector::new()),
            config,
            scanner,
            multi_feed,
        )
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
                    buy_ask: 100.0,
                    sell_bid: 101.0,
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
                    buy_ask: 0.0,
                    sell_bid: 0.0,
                });
            }
        }
        let trades = state.recent_trades(3);
        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].exec_ms, 4);
        assert_eq!(trades[1].exec_ms, 3);
    }
}

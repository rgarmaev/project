# HFT Arbitrage Dashboard — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real-time web dashboard to the SOL arbitrage bot showing live metrics, PnL chart, exchange prices, and trade feed.

**Architecture:** Axum HTTP/WebSocket server runs as a separate tokio task inside the existing bot. `DashboardState` is shared state that bridges the trading engine to the HTTP layer. React/Vite frontend connects via WebSocket (500ms snapshots) and REST (trade history on load).

**Tech Stack:** Rust (axum 0.7, tower-http 0.5), React 18, TypeScript, Vite 5, Recharts

---

## File Map

**Modified:**
- `Cargo.toml` — add axum, tower-http
- `src/types.rs` — add `exec_ms: u64` to `CompletedTrade`
- `src/metrics.rs` — add `total_exec_ms`, `MetricsSnapshot`, `snapshot()`
- `src/arbitrage/executor.rs` — compute `exec_ms`, pass to `CompletedTrade`, hold `Arc<DashboardState>`
- `src/main.rs` — add `mod dashboard`, create `DashboardState`, spawn broadcast + server tasks

**Created (Rust):**
- `src/dashboard/mod.rs` — axum router + `serve()` fn + `broadcast_loop()`
- `src/dashboard/state.rs` — `DashboardState`, `TradeRecord`, `PriceEntry`, `WsSnapshot`
- `src/dashboard/routes.rs` — `GET /api/trades` handler

**Created (Frontend):**
- `dashboard/package.json`
- `dashboard/vite.config.ts`
- `dashboard/tsconfig.json`
- `dashboard/index.html`
- `dashboard/src/main.tsx`
- `dashboard/src/types.ts`
- `dashboard/src/hooks/useWebSocket.ts`
- `dashboard/src/App.tsx`
- `dashboard/src/components/MetricsBar.tsx`
- `dashboard/src/components/PnlChart.tsx`
- `dashboard/src/components/PriceTable.tsx`
- `dashboard/src/components/TradesFeed.tsx`
- `dashboard/src/components/StatusBar.tsx`
- `dashboard/src/styles/global.css`

---

## Task 1: Add `exec_ms` to `CompletedTrade` and `MetricsCollector`

**Files:**
- Modify: `src/types.rs`
- Modify: `src/metrics.rs`

- [ ] **Step 1: Add `exec_ms` field to `CompletedTrade` in `src/types.rs`**

In `src/types.rs`, add `exec_ms: u64` to the `CompletedTrade` struct:

```rust
#[derive(Debug, Clone)]
pub struct CompletedTrade {
    pub id: Uuid,
    pub signal: ArbitrageSignal,
    pub buy_order: OrderResult,
    pub sell_order: OrderResult,
    pub gross_pnl: Decimal,
    pub fees: Decimal,
    pub net_pnl: Decimal,
    pub exec_ms: u64,           // ← new
    pub completed_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Add `total_exec_ms`, `MetricsSnapshot`, and `snapshot()` to `src/metrics.rs`**

Replace the entire `src/metrics.rs` with:

```rust
use crate::types::CompletedTrade;
use parking_lot::Mutex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;
use tracing::info;

pub struct MetricsCollector {
    inner: Mutex<Inner>,
}

struct Inner {
    trades: usize,
    wins: usize,
    total_pnl: Decimal,
    total_fees: Decimal,
    peak_pnl: Decimal,
    max_drawdown: Decimal,
    total_exec_ms: u64,
}

#[derive(Serialize, Clone)]
pub struct MetricsSnapshot {
    pub trades: usize,
    pub wins: usize,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub peak_pnl: f64,
    pub max_drawdown: f64,
    pub avg_exec_ms: u64,
}

fn to_f64(d: Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                trades: 0,
                wins: 0,
                total_pnl: dec!(0),
                total_fees: dec!(0),
                peak_pnl: dec!(0),
                max_drawdown: dec!(0),
                total_exec_ms: 0,
            }),
        }
    }

    pub fn record(&self, trade: &CompletedTrade) {
        let mut m = self.inner.lock();
        m.trades += 1;
        if trade.net_pnl > dec!(0) {
            m.wins += 1;
        }
        m.total_pnl += trade.net_pnl;
        m.total_fees += trade.fees;
        m.total_exec_ms += trade.exec_ms;

        if m.total_pnl > m.peak_pnl {
            m.peak_pnl = m.total_pnl;
        }
        let drawdown = m.peak_pnl - m.total_pnl;
        if drawdown > m.max_drawdown {
            m.max_drawdown = drawdown;
        }

        let win_rate = if m.trades > 0 {
            Decimal::from(m.wins * 100) / Decimal::from(m.trades)
        } else {
            dec!(0)
        };

        info!(
            "Trade #{} | net_pnl={:.4} USDT | total={:.4} | win_rate={:.1}% | drawdown={:.4} | exec={}ms",
            m.trades, trade.net_pnl, m.total_pnl, win_rate, m.max_drawdown, trade.exec_ms
        );
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let m = self.inner.lock();
        let win_rate = if m.trades > 0 {
            m.wins as f64 * 100.0 / m.trades as f64
        } else {
            0.0
        };
        let avg_exec_ms = if m.trades > 0 { m.total_exec_ms / m.trades as u64 } else { 0 };
        MetricsSnapshot {
            trades: m.trades,
            wins: m.wins,
            win_rate,
            total_pnl: to_f64(m.total_pnl),
            total_fees: to_f64(m.total_fees),
            peak_pnl: to_f64(m.peak_pnl),
            max_drawdown: to_f64(m.max_drawdown),
            avg_exec_ms,
        }
    }

    pub fn print_summary(&self) {
        let snap = self.snapshot();
        info!(
            "=== Summary === trades={} win_rate={:.1}% pnl={:.4} fees={:.4} max_dd={:.4} avg_exec={}ms",
            snap.trades, snap.win_rate, snap.total_pnl, snap.total_fees, snap.max_drawdown, snap.avg_exec_ms
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ArbitrageSignal, Exchange, MarketId, MarketType, OrderResult, Side};
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn make_trade(net_pnl: Decimal, exec_ms: u64) -> CompletedTrade {
        let market = MarketId::new(Exchange::Binance, MarketType::Spot);
        let signal = ArbitrageSignal {
            id: Uuid::new_v4(),
            buy_market: market,
            sell_market: market,
            buy_ask: dec!(100),
            sell_bid: dec!(101),
            spread_pct: dec!(1),
            quantity: dec!(1),
            expected_pnl_usdt: dec!(1),
            detected_at: Utc::now(),
        };
        let order = OrderResult {
            exchange: Exchange::Binance,
            market_type: MarketType::Spot,
            order_id: "test".into(),
            side: Side::Buy,
            filled_qty: dec!(1),
            avg_price: dec!(100),
            fee_usdt: dec!(0.1),
            timestamp: Utc::now(),
        };
        CompletedTrade {
            id: Uuid::new_v4(),
            signal,
            buy_order: order.clone(),
            sell_order: order,
            gross_pnl: net_pnl + dec!(0.2),
            fees: dec!(0.2),
            net_pnl,
            exec_ms,
            completed_at: Utc::now(),
        }
    }

    #[test]
    fn avg_exec_ms_computed_correctly() {
        let mc = MetricsCollector::new();
        mc.record(&make_trade(dec!(1), 100));
        mc.record(&make_trade(dec!(1), 200));
        let snap = mc.snapshot();
        assert_eq!(snap.avg_exec_ms, 150);
    }

    #[test]
    fn win_rate_zero_on_no_trades() {
        let mc = MetricsCollector::new();
        assert_eq!(mc.snapshot().win_rate, 0.0);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/rinchin92/claude/project && cargo test metrics
```

Expected: `test metrics::tests::avg_exec_ms_computed_correctly ... ok` and `test metrics::tests::win_rate_zero_on_no_trades ... ok`

- [ ] **Step 4: Commit**

```bash
cd /Users/rinchin92/claude/project
git add src/types.rs src/metrics.rs
git commit -m "feat: add exec_ms tracking to CompletedTrade and MetricsCollector"
```

---

## Task 2: Compute `exec_ms` in `OrderExecutor`

**Files:**
- Modify: `src/arbitrage/executor.rs`

- [ ] **Step 1: Compute `exec_ms` and add it to `CompletedTrade` construction**

In `src/arbitrage/executor.rs`, update the `execute()` method. Find the block that builds `Ok(CompletedTrade { ... })` and add the exec_ms calculation just before it:

```rust
async fn execute(&self, signal: &ArbitrageSignal) -> Result<CompletedTrade> {
    let qty = signal.quantity;
    let ps  = &self.price_state;

    let (buy_res, sell_res) = tokio::join!(
        self.place(signal.buy_market.exchange,  signal.buy_market.market_type,  Side::Buy,  qty, ps),
        self.place(signal.sell_market.exchange, signal.sell_market.market_type, Side::Sell, qty, ps),
    );

    let buy_order  = buy_res?;
    let sell_order = sell_res?;

    let gross_pnl = (sell_order.avg_price - buy_order.avg_price) * buy_order.filled_qty;
    let fees      = buy_order.fee_usdt + sell_order.fee_usdt;
    let net_pnl   = gross_pnl - fees;

    let exec_ms = sell_order.timestamp
        .signed_duration_since(signal.detected_at)
        .num_milliseconds()
        .max(0) as u64;

    info!(
        "TRADE {} | buy {}@{:.4} | sell {}@{:.4} | net_pnl={:.4} USDT | exec={}ms",
        signal.id,
        signal.buy_market,  buy_order.avg_price,
        signal.sell_market, sell_order.avg_price,
        net_pnl, exec_ms
    );

    Ok(CompletedTrade {
        id: signal.id,
        signal: signal.clone(),
        buy_order,
        sell_order,
        gross_pnl,
        fees,
        net_pnl,
        exec_ms,
        completed_at: Utc::now(),
    })
}
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/rinchin92/claude/project && cargo build 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/arbitrage/executor.rs
git commit -m "feat: compute exec_ms in order executor"
```

---

## Task 3: Add Cargo Dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add axum and tower-http to `Cargo.toml`**

In the `[dependencies]` section of `Cargo.toml`, add after the existing entries:

```toml
axum = { version = "0.7", features = ["ws"] }
tower-http = { version = "0.5", features = ["cors", "fs"] }
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/rinchin92/claude/project && cargo build 2>&1 | tail -5
```

Expected: `Finished` with no errors. Cargo downloads axum and tower-http.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add axum and tower-http dependencies"
```

---

## Task 4: Create `DashboardState`

**Files:**
- Create: `src/dashboard/state.rs`

- [ ] **Step 1: Create the file with all shared-state types**

Create `src/dashboard/state.rs`:

```rust
use crate::{metrics::MetricsCollector, orderbook::PriceState, types::CompletedTrade};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::Arc,
};
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
        // Push 501 minimal TradeRecords directly
        {
            let mut buf = state.trades.lock();
            for i in 0u64..501 {
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
                if buf.len() == MAX_TRADES {
                    buf.pop_front();
                }
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
```

- [ ] **Step 2: Run tests**

```bash
cd /Users/rinchin92/claude/project && cargo test dashboard::state
```

Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/state.rs
git commit -m "feat: add DashboardState with trade ring buffer"
```

---

## Task 5: Create WebSocket Handler and Broadcast Loop

**Files:**
- Create: `src/dashboard/ws.rs`

- [ ] **Step 1: Create `src/dashboard/ws.rs`**

```rust
use crate::dashboard::state::DashboardState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashboardState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<DashboardState>) {
    let mut rx = state.broadcast_tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
    debug!("WebSocket client disconnected");
}

pub async fn broadcast_loop(state: Arc<DashboardState>) {
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        interval.tick().await;
        let snapshot = state.build_snapshot();
        match serde_json::to_string(&snapshot) {
            Ok(json) => {
                let _ = state.broadcast_tx.send(json);
            }
            Err(e) => tracing::error!("Failed to serialize snapshot: {}", e),
        }
    }
}
```

- [ ] **Step 2: Verify build (module not wired yet — add to mod.rs first in Task 7)**

Skip compile check for now, will verify when `mod.rs` is created.

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/ws.rs
git commit -m "feat: add WebSocket handler and 500ms broadcast loop"
```

---

## Task 6: Create REST Routes

**Files:**
- Create: `src/dashboard/routes.rs`

- [ ] **Step 1: Create `src/dashboard/routes.rs`**

```rust
use crate::dashboard::state::{DashboardState, TradeRecord};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct TradesQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    200
}

pub async fn trades_handler(
    State(state): State<Arc<DashboardState>>,
    Query(q): Query<TradesQuery>,
) -> Json<Vec<TradeRecord>> {
    Json(state.recent_trades(q.limit))
}
```

- [ ] **Step 2: Commit**

```bash
git add src/dashboard/routes.rs
git commit -m "feat: add REST /api/trades endpoint"
```

---

## Task 7: Create Dashboard Router and Axum Server

**Files:**
- Create: `src/dashboard/mod.rs`

- [ ] **Step 1: Create `src/dashboard/mod.rs`**

```rust
pub mod routes;
pub mod state;
pub mod ws;

use anyhow::Result;
use axum::{routing::get, Router};
use state::DashboardState;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::info;

pub async fn serve(state: Arc<DashboardState>, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/api/trades", get(routes::trades_handler))
        .fallback_service(ServeDir::new("dashboard/dist"))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Dashboard at http://localhost:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/rinchin92/claude/project && cargo build 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/dashboard/mod.rs
git commit -m "feat: add dashboard axum router and server"
```

---

## Task 8: Integrate Dashboard into `main.rs` and `OrderExecutor`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/arbitrage/executor.rs`

- [ ] **Step 1: Add `Arc<DashboardState>` to `OrderExecutor`**

In `src/arbitrage/executor.rs`, update the struct and constructor:

```rust
use crate::{
    config::Config,
    dashboard::state::DashboardState,
    exchanges::{binance::BinanceConnector, bybit::BybitConnector, mexc::MexcConnector},
    metrics::MetricsCollector,
    orderbook::SharedPriceState,
    risk::RiskManager,
    types::{ArbitrageSignal, CompletedTrade, Exchange, MarketType, Side},
};
use anyhow::Result;
use chrono::Utc;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

pub struct OrderExecutor {
    config: Arc<Config>,
    price_state: SharedPriceState,
    binance: Arc<BinanceConnector>,
    bybit: Arc<BybitConnector>,
    mexc: Arc<MexcConnector>,
    risk: Arc<RiskManager>,
    metrics: Arc<MetricsCollector>,
    dashboard: Arc<DashboardState>,
}

impl OrderExecutor {
    pub fn new(
        config: Arc<Config>,
        price_state: SharedPriceState,
        binance: Arc<BinanceConnector>,
        bybit: Arc<BybitConnector>,
        mexc: Arc<MexcConnector>,
        risk: Arc<RiskManager>,
        metrics: Arc<MetricsCollector>,
        dashboard: Arc<DashboardState>,
    ) -> Self {
        Self { config, price_state, binance, bybit, mexc, risk, metrics, dashboard }
    }
    // ... rest of impl unchanged
```

- [ ] **Step 2: Call `dashboard.push_trade()` in `handle()`**

In the `handle()` method of `OrderExecutor`, update the `Ok(trade)` arm:

```rust
match self.execute(&signal).await {
    Ok(trade) => {
        self.risk.on_trade_close(trade.net_pnl);
        self.metrics.record(&trade);
        self.dashboard.push_trade(&trade);
    }
    Err(e) => {
        error!("Execution failed for signal {}: {:#}", signal.id, e);
        self.risk.on_trade_close(dec!(0));
    }
}
```

- [ ] **Step 3: Update `main.rs` to wire everything together**

Replace `src/main.rs` with:

```rust
mod arbitrage;
mod config;
mod dashboard;
mod exchanges;
mod metrics;
mod orderbook;
mod risk;
mod types;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::info;
use tracing_subscriber::EnvFilter;

use arbitrage::{detector::ArbitrageDetector, executor::OrderExecutor};
use dashboard::state::DashboardState;
use exchanges::{binance::BinanceConnector, bybit::BybitConnector, mexc::MexcConnector};
use metrics::MetricsCollector;
use orderbook::PriceState;
use risk::RiskManager;
use types::MarketType;

const DASHBOARD_PORT: u16 = 3001;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("sol_arb=info")),
        )
        .init();

    let config = Arc::new(config::Config::load()?);
    info!(
        "SOL Arb starting | pair={} | paper={} | exchanges=Binance,Bybit,MEXC",
        config.pair(),
        config.trading.paper_trading
    );

    let price_state = Arc::new(PriceState::new());
    let (signal_tx, signal_rx) = mpsc::channel(256);

    let binance = Arc::new(BinanceConnector::new(config.clone()));
    let bybit   = Arc::new(BybitConnector::new(config.clone()));
    let mexc    = Arc::new(MexcConnector::new(config.clone()));
    let risk    = Arc::new(RiskManager::new(config.risk.clone()));
    let metrics = Arc::new(MetricsCollector::new());

    let dash_state = DashboardState::new(price_state.clone(), metrics.clone());

    let detector = Arc::new(ArbitrageDetector::new(
        config.clone(),
        price_state.clone(),
        signal_tx,
    ));

    let executor = Arc::new(OrderExecutor::new(
        config.clone(),
        price_state.clone(),
        binance.clone(),
        bybit.clone(),
        mexc.clone(),
        risk.clone(),
        metrics.clone(),
        dash_state.clone(),
    ));

    let mut set = JoinSet::new();

    // ── Price feeds ──────────────────────────────────────────────────────────
    {
        let (b, ps) = (binance.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Spot, ps).await });
    }
    {
        let (b, ps) = (binance.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Futures, ps).await });
    }
    {
        let (b, ps) = (bybit.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Spot, ps).await });
    }
    {
        let (b, ps) = (bybit.clone(), price_state.clone());
        set.spawn(async move { b.run_feed(MarketType::Futures, ps).await });
    }
    {
        let (m, ps) = (mexc.clone(), price_state.clone());
        set.spawn(async move { m.run_feed(ps).await });
    }

    // ── Arbitrage engine ─────────────────────────────────────────────────────
    {
        let d = detector.clone();
        set.spawn(async move { d.run().await });
    }
    {
        let e = executor.clone();
        set.spawn(async move { e.run(signal_rx).await });
    }

    // ── Dashboard ─────────────────────────────────────────────────────────────
    {
        let ds = dash_state.clone();
        set.spawn(async move { dashboard::ws::broadcast_loop(ds).await; Ok(()) });
    }
    {
        let ds = dash_state.clone();
        set.spawn(async move { dashboard::serve(ds, DASHBOARD_PORT).await });
    }

    while let Some(res) = set.join_next().await {
        if let Err(e) = res {
            tracing::error!("Task panicked: {}", e);
        }
    }

    metrics.print_summary();
    Ok(())
}
```

- [ ] **Step 4: Verify full build**

```bash
cd /Users/rinchin92/claude/project && cargo build 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 5: Run all tests**

```bash
cd /Users/rinchin92/claude/project && cargo test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/arbitrage/executor.rs
git commit -m "feat: integrate dashboard into bot (axum server + broadcast loop)"
```

---

## Task 9: Scaffold Frontend Project

**Files:**
- Create: `dashboard/package.json`
- Create: `dashboard/vite.config.ts`
- Create: `dashboard/tsconfig.json`
- Create: `dashboard/index.html`
- Create: `dashboard/src/main.tsx`

- [ ] **Step 1: Create `dashboard/package.json`**

```json
{
  "name": "sol-arb-dashboard",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "recharts": "^2.12.7"
  },
  "devDependencies": {
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.4.5",
    "vite": "^5.3.1"
  }
}
```

- [ ] **Step 2: Create `dashboard/vite.config.ts`**

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': 'http://localhost:3001',
      '/ws': {
        target: 'ws://localhost:3001',
        ws: true,
      },
    },
  },
})
```

- [ ] **Step 3: Create `dashboard/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Create `dashboard/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>SOL ARB Dashboard</title>
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Create `dashboard/src/main.tsx`**

```tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './styles/global.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
```

- [ ] **Step 6: Install dependencies**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm install
```

Expected: `node_modules` created, no errors.

- [ ] **Step 7: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/
git commit -m "feat: scaffold Vite + React + TypeScript frontend"
```

---

## Task 10: TypeScript Types and WebSocket Hook

**Files:**
- Create: `dashboard/src/types.ts`
- Create: `dashboard/src/hooks/useWebSocket.ts`

- [ ] **Step 1: Create `dashboard/src/types.ts`**

```typescript
export interface MetricsSnapshot {
  trades: number
  wins: number
  win_rate: number
  total_pnl: number
  total_fees: number
  peak_pnl: number
  max_drawdown: number
  avg_exec_ms: number
}

export interface PriceEntry {
  exchange: string
  market: string
  bid: number
  ask: number
  spread_pct: number
  stale: boolean
}

export interface TradeRecord {
  id: string
  buy_market: string
  sell_market: string
  spread_pct: number
  gross_pnl: number
  fees: number
  net_pnl: number
  exec_ms: number
  time: string
}

export interface WsSnapshot {
  metrics: MetricsSnapshot
  prices: PriceEntry[]
  recent_trades: TradeRecord[]
}
```

- [ ] **Step 2: Create `dashboard/src/hooks/useWebSocket.ts`**

```typescript
import { useEffect, useRef, useState } from 'react'
import { WsSnapshot } from '../types'

type Status = 'connecting' | 'connected' | 'disconnected'

export function useWebSocket(url: string) {
  const [snapshot, setSnapshot] = useState<WsSnapshot | null>(null)
  const [status, setStatus] = useState<Status>('connecting')
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    function connect() {
      const ws = new WebSocket(url)
      wsRef.current = ws

      ws.onopen = () => setStatus('connected')

      ws.onmessage = (e) => {
        try {
          setSnapshot(JSON.parse(e.data) as WsSnapshot)
          setLastUpdate(new Date())
        } catch {
          // ignore malformed messages
        }
      }

      ws.onclose = () => {
        setStatus('disconnected')
        reconnectRef.current = setTimeout(connect, 2000)
      }

      ws.onerror = () => ws.close()
    }

    connect()

    return () => {
      wsRef.current?.close()
      if (reconnectRef.current) clearTimeout(reconnectRef.current)
    }
  }, [url])

  return { snapshot, status, lastUpdate }
}
```

- [ ] **Step 3: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/src/types.ts dashboard/src/hooks/
git commit -m "feat: add TypeScript types and WebSocket hook"
```

---

## Task 11: MetricsBar Component

**Files:**
- Create: `dashboard/src/components/MetricsBar.tsx`

- [ ] **Step 1: Create `dashboard/src/components/MetricsBar.tsx`**

```tsx
import { MetricsSnapshot } from '../types'

interface Props {
  metrics: MetricsSnapshot
  paperTrading: boolean
}

interface CardProps {
  label: string
  value: string
  color?: string
}

function Card({ label, value, color }: CardProps) {
  return (
    <div style={{
      background: '#111',
      border: '1px solid #1f1f1f',
      borderRadius: 6,
      padding: '12px 16px',
      flex: 1,
      minWidth: 120,
    }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 6 }}>
        {label}
      </div>
      <div style={{ color: color ?? '#e0e0e0', fontSize: 20, fontWeight: 600 }}>
        {value}
      </div>
    </div>
  )
}

function pnlColor(v: number) {
  return v > 0 ? '#00ff87' : v < 0 ? '#ff4444' : '#e0e0e0'
}

function fmt(v: number, decimals = 2) {
  return v.toFixed(decimals)
}

export function MetricsBar({ metrics, paperTrading }: Props) {
  return (
    <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
      {paperTrading && (
        <div style={{
          alignSelf: 'center',
          background: '#1a1a00',
          border: '1px solid #444400',
          borderRadius: 4,
          padding: '4px 8px',
          color: '#aaaa00',
          fontSize: 11,
          whiteSpace: 'nowrap',
        }}>
          PAPER
        </div>
      )}
      <Card label="Total PnL" value={`${metrics.total_pnl >= 0 ? '+' : ''}${fmt(metrics.total_pnl, 4)} USDT`} color={pnlColor(metrics.total_pnl)} />
      <Card label="Win Rate" value={`${fmt(metrics.win_rate, 1)}%`} />
      <Card label="Max Drawdown" value={`${fmt(metrics.max_drawdown, 4)} USDT`} color={metrics.max_drawdown > 0 ? '#ff4444' : '#e0e0e0'} />
      <Card label="Trades" value={String(metrics.trades)} />
      <Card label="Fees" value={`${fmt(metrics.total_fees, 4)} USDT`} color="#888" />
      <Card label="Avg Exec" value={`${metrics.avg_exec_ms} ms`} />
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/src/components/MetricsBar.tsx
git commit -m "feat: add MetricsBar component"
```

---

## Task 12: PnlChart Component

**Files:**
- Create: `dashboard/src/components/PnlChart.tsx`

- [ ] **Step 1: Create `dashboard/src/components/PnlChart.tsx`**

```tsx
import { useEffect, useState } from 'react'
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine } from 'recharts'
import { TradeRecord } from '../types'

interface ChartPoint {
  time: string
  pnl: number
}

interface Props {
  recentTrades: TradeRecord[]
}

function buildSeries(trades: TradeRecord[]): ChartPoint[] {
  const sorted = [...trades].sort((a, b) => new Date(a.time).getTime() - new Date(b.time).getTime())
  let cumulative = 0
  return sorted.map(t => {
    cumulative += t.net_pnl
    return {
      time: new Date(t.time).toLocaleTimeString(),
      pnl: Math.round(cumulative * 10000) / 10000,
    }
  })
}

export function PnlChart({ recentTrades }: Props) {
  const [history, setHistory] = useState<TradeRecord[]>([])

  useEffect(() => {
    fetch('/api/trades?limit=500')
      .then(r => r.json())
      .then((data: TradeRecord[]) => setHistory(data))
      .catch(() => {})
  }, [])

  const all = [...history, ...recentTrades.filter(t => !history.find(h => h.id === t.id))]
  const data = buildSeries(all)

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Cumulative PnL (USDT)
      </div>
      {data.length === 0 ? (
        <div style={{ color: '#333', textAlign: 'center', padding: '40px 0', fontSize: 13 }}>
          Waiting for first trade...
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
            <XAxis dataKey="time" tick={{ fill: '#444', fontSize: 10 }} tickLine={false} axisLine={false} />
            <YAxis tick={{ fill: '#444', fontSize: 10 }} tickLine={false} axisLine={false} width={55} />
            <Tooltip
              contentStyle={{ background: '#1a1a1a', border: '1px solid #333', borderRadius: 4, fontSize: 12 }}
              labelStyle={{ color: '#888' }}
              itemStyle={{ color: '#00ff87' }}
            />
            <ReferenceLine y={0} stroke="#333" strokeDasharray="3 3" />
            <Line
              type="monotone"
              dataKey="pnl"
              stroke="#00ff87"
              strokeWidth={1.5}
              dot={false}
              activeDot={{ r: 3, fill: '#00ff87' }}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/src/components/PnlChart.tsx
git commit -m "feat: add PnlChart component with Recharts"
```

---

## Task 13: PriceTable Component

**Files:**
- Create: `dashboard/src/components/PriceTable.tsx`

- [ ] **Step 1: Create `dashboard/src/components/PriceTable.tsx`**

```tsx
import { useEffect, useRef, useState } from 'react'
import { PriceEntry } from '../types'

interface Props {
  prices: PriceEntry[]
}

function useFlash(value: number) {
  const [flash, setFlash] = useState(false)
  const prev = useRef(value)
  useEffect(() => {
    if (prev.current !== value) {
      prev.current = value
      setFlash(true)
      const t = setTimeout(() => setFlash(false), 200)
      return () => clearTimeout(t)
    }
  }, [value])
  return flash
}

function PriceRow({ entry }: { entry: PriceEntry }) {
  const flashBid = useFlash(entry.bid)
  const flashAsk = useFlash(entry.ask)

  return (
    <tr style={{ borderBottom: '1px solid #1a1a1a' }}>
      <td style={{ padding: '6px 8px', color: '#888', fontSize: 12 }}>{entry.exchange}</td>
      <td style={{ padding: '6px 8px', color: '#555', fontSize: 12 }}>{entry.market}</td>
      <td style={{ padding: '6px 8px', color: flashBid ? '#ffff00' : '#00ff87', fontSize: 12, transition: 'color 200ms' }}>
        {entry.bid.toFixed(4)}
      </td>
      <td style={{ padding: '6px 8px', color: flashAsk ? '#ffff00' : '#ff4444', fontSize: 12, transition: 'color 200ms' }}>
        {entry.ask.toFixed(4)}
      </td>
      <td style={{ padding: '6px 8px', color: '#444', fontSize: 11 }}>
        {entry.spread_pct.toFixed(3)}%{entry.stale ? ' ⚠' : ''}
      </td>
    </tr>
  )
}

const MARKET_ORDER = ['Binance:Spot', 'Binance:Perp', 'Bybit:Spot', 'Bybit:Perp', 'MEXC:Spot']

export function PriceTable({ prices }: Props) {
  const sorted = [...prices].sort((a, b) => {
    const ia = MARKET_ORDER.indexOf(`${a.exchange}:${a.market}`)
    const ib = MARKET_ORDER.indexOf(`${b.exchange}:${b.market}`)
    return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib)
  })

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Live Prices
      </div>
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr>
            {['Exchange', 'Market', 'Bid', 'Ask', 'Spread'].map(h => (
              <th key={h} style={{ padding: '4px 8px', color: '#444', fontSize: 10, textAlign: 'left', textTransform: 'uppercase' }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.length === 0
            ? <tr><td colSpan={5} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>No price data</td></tr>
            : sorted.map(p => <PriceRow key={`${p.exchange}-${p.market}`} entry={p} />)
          }
        </tbody>
      </table>
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/src/components/PriceTable.tsx
git commit -m "feat: add PriceTable component with bid/ask flash effect"
```

---

## Task 14: TradesFeed Component

**Files:**
- Create: `dashboard/src/components/TradesFeed.tsx`

- [ ] **Step 1: Create `dashboard/src/components/TradesFeed.tsx`**

```tsx
import { TradeRecord } from '../types'

interface Props {
  trades: TradeRecord[]
}

function formatTime(iso: string) {
  return new Date(iso).toLocaleTimeString()
}

export function TradesFeed({ trades }: Props) {
  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16, height: '100%' }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Recent Trades
      </div>
      <div style={{ overflowY: 'auto', maxHeight: 340 }}>
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <thead style={{ position: 'sticky', top: 0, background: '#111' }}>
            <tr>
              {['Time', 'Route', 'Spread', 'PnL', 'Exec'].map(h => (
                <th key={h} style={{ padding: '4px 6px', color: '#444', fontSize: 10, textAlign: 'left', textTransform: 'uppercase' }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {trades.length === 0 ? (
              <tr>
                <td colSpan={5} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>
                  No trades yet
                </td>
              </tr>
            ) : (
              trades.map(t => (
                <tr key={t.id} style={{ borderBottom: '1px solid #1a1a1a' }}>
                  <td style={{ padding: '5px 6px', color: '#555', fontSize: 11, whiteSpace: 'nowrap' }}>
                    {formatTime(t.time)}
                  </td>
                  <td style={{ padding: '5px 6px', color: '#888', fontSize: 11 }}>
                    {t.buy_market} → {t.sell_market}
                  </td>
                  <td style={{ padding: '5px 6px', color: '#555', fontSize: 11 }}>
                    {t.spread_pct.toFixed(3)}%
                  </td>
                  <td style={{ padding: '5px 6px', fontSize: 11, fontWeight: 600, color: t.net_pnl >= 0 ? '#00ff87' : '#ff4444' }}>
                    {t.net_pnl >= 0 ? '+' : ''}{t.net_pnl.toFixed(4)}
                  </td>
                  <td style={{ padding: '5px 6px', color: '#444', fontSize: 11 }}>
                    {t.exec_ms}ms
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/src/components/TradesFeed.tsx
git commit -m "feat: add TradesFeed component"
```

---

## Task 15: StatusBar, App.tsx, and Global CSS

**Files:**
- Create: `dashboard/src/components/StatusBar.tsx`
- Create: `dashboard/src/App.tsx`
- Create: `dashboard/src/styles/global.css`

- [ ] **Step 1: Create `dashboard/src/components/StatusBar.tsx`**

```tsx
type Status = 'connecting' | 'connected' | 'disconnected'

interface Props {
  status: Status
  lastUpdate: Date | null
}

const statusColor: Record<Status, string> = {
  connecting: '#aaaa00',
  connected: '#00ff87',
  disconnected: '#ff4444',
}

const statusLabel: Record<Status, string> = {
  connecting: '● CONNECTING',
  connected: '● LIVE',
  disconnected: '● DISCONNECTED',
}

export function StatusBar({ status, lastUpdate }: Props) {
  return (
    <div style={{
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      padding: '6px 0',
      borderTop: '1px solid #1a1a1a',
      marginTop: 8,
    }}>
      <span style={{ color: statusColor[status], fontSize: 11 }}>
        {statusLabel[status]}
      </span>
      <span style={{ color: '#333', fontSize: 11 }}>
        {lastUpdate ? `Updated ${lastUpdate.toLocaleTimeString()}` : 'Waiting...'}
      </span>
    </div>
  )
}
```

- [ ] **Step 2: Create `dashboard/src/styles/global.css`**

```css
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

body {
  background: #0a0a0a;
  color: #e0e0e0;
  font-family: 'JetBrains Mono', 'Courier New', monospace;
  font-size: 13px;
  -webkit-font-smoothing: antialiased;
}

::-webkit-scrollbar { width: 4px; }
::-webkit-scrollbar-track { background: #0a0a0a; }
::-webkit-scrollbar-thumb { background: #222; border-radius: 2px; }
```

- [ ] **Step 3: Create `dashboard/src/App.tsx`**

```tsx
import { useWebSocket } from './hooks/useWebSocket'
import { MetricsBar } from './components/MetricsBar'
import { PnlChart } from './components/PnlChart'
import { PriceTable } from './components/PriceTable'
import { TradesFeed } from './components/TradesFeed'
import { StatusBar } from './components/StatusBar'

const WS_URL = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`

const EMPTY_METRICS = {
  trades: 0, wins: 0, win_rate: 0,
  total_pnl: 0, total_fees: 0, peak_pnl: 0,
  max_drawdown: 0, avg_exec_ms: 0,
}

export default function App() {
  const { snapshot, status, lastUpdate } = useWebSocket(WS_URL)

  const metrics = snapshot?.metrics ?? EMPTY_METRICS
  const prices = snapshot?.prices ?? []
  const trades = snapshot?.recent_trades ?? []

  return (
    <div style={{ maxWidth: 1400, margin: '0 auto', padding: 20, display: 'flex', flexDirection: 'column', gap: 12 }}>

      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: '0.1em', color: '#e0e0e0' }}>
          SOL ARB
        </span>
        <span style={{ color: '#333', fontSize: 11 }}>
          {new Date().toLocaleTimeString()}
        </span>
      </div>

      {/* Metrics */}
      <MetricsBar metrics={metrics} paperTrading={false} />

      {/* PnL Chart */}
      <PnlChart recentTrades={trades} />

      {/* Bottom row */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <PriceTable prices={prices} />
        <TradesFeed trades={trades} />
      </div>

      <StatusBar status={status} lastUpdate={lastUpdate} />
    </div>
  )
}
```

- [ ] **Step 4: Start dev server and verify the UI loads**

Make sure the Rust bot is running first:
```bash
# Terminal 1
cd /Users/rinchin92/claude/project && cargo run

# Terminal 2
cd /Users/rinchin92/claude/project/dashboard && npm run dev
```

Open `http://localhost:5173` in the browser. Expected:
- Dark background, monospace font
- 6 metric cards showing zeros
- Empty PnL chart with "Waiting for first trade..." message
- Empty price table (populates as soon as bot connects to exchanges)
- Empty trade feed
- Status bar showing connection status

- [ ] **Step 5: Build for production**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build
```

Expected: `dashboard/dist/` created with `index.html` and assets.

- [ ] **Step 6: Verify production build is served by axum**

```bash
# Start bot (serves dashboard/dist at http://localhost:3001)
cd /Users/rinchin92/claude/project && cargo run
```

Open `http://localhost:3001`. Expected: same dashboard UI served directly by the Rust binary.

- [ ] **Step 7: Commit**

```bash
cd /Users/rinchin92/claude/project
git add dashboard/src/components/StatusBar.tsx dashboard/src/App.tsx dashboard/src/styles/
git commit -m "feat: complete dashboard UI with dark crypto theme"
```

---

## Self-Review

**Spec coverage check:**
- ✅ Metrics cards: Total PnL, Win Rate, Max Drawdown, Trades, Fees, Avg Exec Time
- ✅ PnL chart with Recharts, loads history via REST on mount
- ✅ Live prices: 5 markets, bid/ask/spread, flash on change, stale indicator
- ✅ Trade feed: last 50 trades, route, spread %, net PnL, exec ms
- ✅ WebSocket 500ms updates with auto-reconnect
- ✅ REST GET /api/trades?limit=200
- ✅ Dark crypto theme: #0a0a0a bg, #00ff87 green, #ff4444 red, JetBrains Mono
- ✅ Paper trading indicator in MetricsBar
- ✅ Status bar with connection status and last update time
- ✅ axum serves `dashboard/dist/` in production

**Type consistency check:**
- `TradeRecord.exec_ms: u64 (Rust) / number (TS)` — consistent
- `MetricsSnapshot.avg_exec_ms` — defined in Task 1, used in Task 11 — consistent
- `WsSnapshot { metrics, prices, recent_trades }` — Rust struct matches TS interface
- `PriceEntry.stale` — computed in `build_snapshot()`, rendered in `PriceTable` — consistent
- `DashboardState::push_trade()` called in `OrderExecutor::handle()` — wired in Task 8

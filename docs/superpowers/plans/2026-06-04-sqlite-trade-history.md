# SQLite Trade History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist every completed trade to SQLite, restore metrics counters on restart, and add a "История" tab in the dashboard with a filterable trade table and cumulative P&L chart.

**Architecture:** New `src/trade_store.rs` module wraps `rusqlite::Connection` behind `Arc<Mutex<_>>` and exposes async methods via `spawn_blocking`. The executor writes every trade to the DB; on startup `main.rs` loads stored stats and seeds `MetricsCollector`. A new `/api/trades` endpoint serves paginated, filtered rows to a new `TradesHistory` React component.

**Tech Stack:** `rusqlite 0.31` (bundled feature), Rust `tokio::task::spawn_blocking`, `axum` (existing), `recharts` (existing), React (existing).

---

## File Map

| Action  | Path |
|---------|------|
| Create  | `src/trade_store.rs` |
| Modify  | `Cargo.toml` |
| Modify  | `src/main.rs` |
| Modify  | `src/metrics.rs` |
| Modify  | `src/arbitrage/executor.rs` |
| Modify  | `src/dashboard/state.rs` |
| Modify  | `src/dashboard/mod.rs` |
| Modify  | `src/dashboard/routes.rs` |
| Create  | `dashboard/src/components/TradesHistory.tsx` |
| Modify  | `dashboard/src/App.tsx` |

---

### Task 1: Add rusqlite dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add dependency**

In `Cargo.toml` under `[dependencies]`, add:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

- [ ] **Step 2: Verify it compiles**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error|Compiling rusqlite"
```
Expected: line `Compiling rusqlite v0.31.*` then no errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add rusqlite bundled"
```

---

### Task 2: Create TradeStore — schema + open()

**Files:**
- Create: `src/trade_store.rs`
- Modify: `src/main.rs` (add `mod trade_store;`)

- [ ] **Step 1: Write the failing test**

Create `src/trade_store.rs` with the test first:

```rust
use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::Arc;

pub struct TradeStore {
    conn: Arc<Mutex<Connection>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_table() {
        let store = TradeStore::open(":memory:").unwrap();
        let conn = store.conn.lock();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trades", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
~/.cargo/bin/cargo test trade_store::tests::open_creates_table 2>&1 | tail -5
```
Expected: `FAILED` — `open` not defined yet.

- [ ] **Step 3: Implement TradeStore::open()**

Add to `src/trade_store.rs` (before the `#[cfg(test)]` block):

```rust
pub struct StoredStats {
    pub trade_count: usize,
    pub wins:        usize,
    pub total_pnl:   f64,
    pub total_fees:  f64,
    pub total_gross: f64,
    pub total_exec_ms: u64,
    pub peak_pnl:    f64,
}

impl TradeStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS trades (
                id               TEXT PRIMARY KEY,
                symbol           TEXT    NOT NULL,
                buy_exchange     TEXT    NOT NULL,
                buy_market_type  TEXT    NOT NULL,
                sell_exchange    TEXT    NOT NULL,
                sell_market_type TEXT    NOT NULL,
                buy_ask          REAL    NOT NULL,
                sell_bid         REAL    NOT NULL,
                spread_pct       REAL    NOT NULL,
                quantity         REAL    NOT NULL,
                gross_pnl        REAL    NOT NULL,
                fees             REAL    NOT NULL,
                net_pnl          REAL    NOT NULL,
                exec_ms          INTEGER NOT NULL,
                completed_at     TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_trades_time   ON trades(completed_at);
            CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
        ")?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }
}
```

- [ ] **Step 4: Add `mod trade_store;` to `src/main.rs`**

Add near the top of `src/main.rs` with other `mod` declarations:
```rust
mod trade_store;
```

- [ ] **Step 5: Run test to verify it passes**

```bash
~/.cargo/bin/cargo test trade_store::tests::open_creates_table 2>&1 | tail -5
```
Expected: `test trade_store::tests::open_creates_table ... ok`

- [ ] **Step 6: Commit**

```bash
git add src/trade_store.rs src/main.rs
git commit -m "feat: TradeStore::open() with schema"
```

---

### Task 3: Implement insert()

**Files:**
- Modify: `src/trade_store.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/trade_store.rs`:

```rust
#[test]
fn insert_stores_row() {
    use chrono::Utc;
    let store = TradeStore::open(":memory:").unwrap();
    store.insert_sync("uuid-1", "BTCUSDT", "Binance", "Spot", "OKX", "Spot",
        65000.0, 65100.0, 0.15, 0.001, 0.10, 0.02, 0.08, 42, "2026-06-04T10:00:00Z"
    ).unwrap();
    let conn = store.conn.lock();
    let net: f64 = conn.query_row(
        "SELECT net_pnl FROM trades WHERE id = 'uuid-1'", [], |r| r.get(0)
    ).unwrap();
    assert!((net - 0.08).abs() < 1e-9);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
~/.cargo/bin/cargo test trade_store::tests::insert_stores_row 2>&1 | tail -5
```
Expected: `FAILED` — `insert_sync` not defined.

- [ ] **Step 3: Add required imports and insert_sync (sync helper for tests) + async insert()**

Add imports at the top of `src/trade_store.rs`:
```rust
use crate::types::CompletedTrade;
use tokio::task;
```

Add methods to `impl TradeStore`:

```rust
// Sync helper used by tests and by the async wrapper
fn insert_sync(&self,
    id: &str, symbol: &str,
    buy_exchange: &str, buy_market_type: &str,
    sell_exchange: &str, sell_market_type: &str,
    buy_ask: f64, sell_bid: f64, spread_pct: f64, quantity: f64,
    gross_pnl: f64, fees: f64, net_pnl: f64,
    exec_ms: u64, completed_at: &str,
) -> Result<()> {
    let conn = self.conn.lock();
    conn.execute(
        "INSERT OR IGNORE INTO trades (
            id, symbol, buy_exchange, buy_market_type,
            sell_exchange, sell_market_type,
            buy_ask, sell_bid, spread_pct, quantity,
            gross_pnl, fees, net_pnl, exec_ms, completed_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        rusqlite::params![
            id, symbol, buy_exchange, buy_market_type,
            sell_exchange, sell_market_type,
            buy_ask, sell_bid, spread_pct, quantity,
            gross_pnl, fees, net_pnl, exec_ms as i64, completed_at
        ],
    )?;
    Ok(())
}

pub async fn insert(&self, trade: &CompletedTrade) -> Result<()> {
    let id          = trade.id.to_string();
    let symbol      = trade.signal.symbol.clone();
    let buy_ex      = trade.signal.buy_market.exchange.to_string();
    let buy_mt      = trade.signal.buy_market.market_type.to_string();
    let sell_ex     = trade.signal.sell_market.exchange.to_string();
    let sell_mt     = trade.signal.sell_market.market_type.to_string();
    let buy_ask: f64  = trade.signal.buy_ask.to_string().parse().unwrap_or(0.0);
    let sell_bid: f64 = trade.signal.sell_bid.to_string().parse().unwrap_or(0.0);
    let spread: f64   = trade.signal.spread_pct.to_string().parse().unwrap_or(0.0);
    let qty: f64      = trade.signal.quantity.to_string().parse().unwrap_or(0.0);
    let gross: f64    = trade.gross_pnl.to_string().parse().unwrap_or(0.0);
    let fees: f64     = trade.fees.to_string().parse().unwrap_or(0.0);
    let net: f64      = trade.net_pnl.to_string().parse().unwrap_or(0.0);
    let exec_ms       = trade.exec_ms;
    let completed_at  = trade.completed_at.to_rfc3339();
    let store = self.conn.clone();
    task::spawn_blocking(move || {
        let conn = store.lock();
        conn.execute(
            "INSERT OR IGNORE INTO trades (
                id, symbol, buy_exchange, buy_market_type,
                sell_exchange, sell_market_type,
                buy_ask, sell_bid, spread_pct, quantity,
                gross_pnl, fees, net_pnl, exec_ms, completed_at
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            rusqlite::params![
                id, symbol, buy_ex, buy_mt, sell_ex, sell_mt,
                buy_ask, sell_bid, spread, qty, gross, fees, net,
                exec_ms as i64, completed_at
            ],
        )?;
        Ok::<_, anyhow::Error>(())
    }).await??;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
~/.cargo/bin/cargo test trade_store::tests::insert_stores_row 2>&1 | tail -5
```
Expected: `test trade_store::tests::insert_stores_row ... ok`

- [ ] **Step 5: Commit**

```bash
git add src/trade_store.rs
git commit -m "feat: TradeStore::insert()"
```

---

### Task 4: Implement load_stats()

**Files:**
- Modify: `src/trade_store.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn load_stats_sums_correctly() {
    let store = TradeStore::open(":memory:").unwrap();
    store.insert_sync("a","BTCUSDT","Binance","Spot","OKX","Spot",
        1.0,1.1,0.1,1.0, 0.10,0.02,0.08, 40,"2026-06-04T10:00:00Z").unwrap();
    store.insert_sync("b","ETHUSDT","Binance","Spot","OKX","Spot",
        1.0,1.1,0.1,1.0, 0.20,0.02,0.18, 60,"2026-06-04T10:01:00Z").unwrap();
    let stats = store.load_stats_sync().unwrap();
    assert_eq!(stats.trade_count, 2);
    assert_eq!(stats.wins, 2);
    assert!((stats.total_pnl  - 0.26).abs() < 1e-9);
    assert!((stats.total_fees - 0.04).abs() < 1e-9);
    assert!((stats.peak_pnl   - 0.26).abs() < 1e-9);
    assert_eq!(stats.total_exec_ms, 100);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
~/.cargo/bin/cargo test trade_store::tests::load_stats_sums_correctly 2>&1 | tail -5
```
Expected: `FAILED`.

- [ ] **Step 3: Implement load_stats_sync() + async load_stats()**

Add to `impl TradeStore`:

```rust
fn load_stats_sync(&self) -> Result<StoredStats> {
    let conn = self.conn.lock();
    // trade_count, wins, total_pnl, total_fees, total_gross, total_exec_ms
    let (trade_count, wins, total_pnl, total_fees, total_gross, total_exec_ms): (i64, i64, f64, f64, f64, i64) =
        conn.query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN net_pnl > 0 THEN 1 ELSE 0 END),
                    COALESCE(SUM(net_pnl),   0),
                    COALESCE(SUM(fees),       0),
                    COALESCE(SUM(gross_pnl),  0),
                    COALESCE(SUM(exec_ms),    0)
             FROM trades",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )?;

    // Peak = max of running cumulative P&L
    let peak_pnl: f64 = conn.query_row(
        "WITH running AS (
            SELECT SUM(net_pnl) OVER (ORDER BY completed_at ROWS UNBOUNDED PRECEDING) AS cum
            FROM trades
         )
         SELECT COALESCE(MAX(cum), 0) FROM running",
        [],
        |r| r.get(0),
    )?;

    Ok(StoredStats {
        trade_count: trade_count as usize,
        wins:        wins as usize,
        total_pnl,
        total_fees,
        total_gross,
        total_exec_ms: total_exec_ms as u64,
        peak_pnl,
    })
}

pub async fn load_stats(&self) -> Result<StoredStats> {
    let store = self.conn.clone();
    task::spawn_blocking(move || {
        let tmp = TradeStore { conn: store };
        tmp.load_stats_sync()
    }).await?
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
~/.cargo/bin/cargo test trade_store::tests::load_stats_sums_correctly 2>&1 | tail -5
```
Expected: `ok`

- [ ] **Step 5: Commit**

```bash
git add src/trade_store.rs
git commit -m "feat: TradeStore::load_stats()"
```

---

### Task 5: Implement query()

**Files:**
- Modify: `src/trade_store.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn query_filters_by_symbol() {
    let store = TradeStore::open(":memory:").unwrap();
    store.insert_sync("a","BTCUSDT","Binance","Spot","OKX","Spot",
        1.0,1.1,0.1,1.0,0.10,0.02,0.08,40,"2026-06-04T10:00:00Z").unwrap();
    store.insert_sync("b","ETHUSDT","Binance","Spot","OKX","Spot",
        1.0,1.1,0.1,1.0,0.10,0.02,0.08,40,"2026-06-04T10:01:00Z").unwrap();
    let filter = TradeFilter { symbol: Some("ETH".into()), ..Default::default() };
    let page = store.query_sync(filter).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0].symbol, "ETHUSDT");
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
~/.cargo/bin/cargo test trade_store::tests::query_filters_by_symbol 2>&1 | tail -5
```
Expected: `FAILED`.

- [ ] **Step 3: Define TradeFilter, TradeRow, TradesPage structs**

Add before `impl TradeStore`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
pub struct TradeFilter {
    pub symbol:       Option<String>,
    pub buy_exchange: Option<String>,
    pub sell_exchange:Option<String>,
    pub from:         Option<String>,  // ISO 8601 string "2026-06-01T00:00:00Z"
    pub to:           Option<String>,
    pub min_spread:   Option<f64>,
    pub max_spread:   Option<f64>,
    pub page:         Option<u32>,     // 0-indexed, 50 rows per page
}

#[derive(Debug, Serialize)]
pub struct TradeRow {
    pub id:               String,
    pub symbol:           String,
    pub buy_exchange:     String,
    pub buy_market_type:  String,
    pub sell_exchange:    String,
    pub sell_market_type: String,
    pub buy_ask:          f64,
    pub sell_bid:         f64,
    pub spread_pct:       f64,
    pub quantity:         f64,
    pub gross_pnl:        f64,
    pub fees:             f64,
    pub net_pnl:          f64,
    pub exec_ms:          i64,
    pub completed_at:     String,
}

#[derive(Debug, Serialize)]
pub struct TradesPage {
    pub total: i64,
    pub page:  u32,
    pub rows:  Vec<TradeRow>,
}
```

- [ ] **Step 4: Implement query_sync() + async query()**

Add to `impl TradeStore`:

```rust
fn query_sync(&self, f: TradeFilter) -> Result<TradesPage> {
    const PAGE_SIZE: i64 = 50;
    let page = f.page.unwrap_or(0) as i64;
    let offset = page * PAGE_SIZE;

    // Build WHERE clause dynamically
    let mut conditions: Vec<String> = Vec::new();
    if f.symbol.is_some()        { conditions.push("symbol LIKE ?".into()); }
    if f.buy_exchange.is_some()  { conditions.push("buy_exchange = ?".into()); }
    if f.sell_exchange.is_some() { conditions.push("sell_exchange = ?".into()); }
    if f.from.is_some()          { conditions.push("completed_at >= ?".into()); }
    if f.to.is_some()            { conditions.push("completed_at <= ?".into()); }
    if f.min_spread.is_some()    { conditions.push("spread_pct >= ?".into()); }
    if f.max_spread.is_some()    { conditions.push("spread_pct <= ?".into()); }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    // Build params list (order must match condition order above)
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = &f.symbol       { params.push(Box::new(format!("%{}%", s))); }
    if let Some(s) = &f.buy_exchange { params.push(Box::new(s.clone())); }
    if let Some(s) = &f.sell_exchange{ params.push(Box::new(s.clone())); }
    if let Some(s) = &f.from        { params.push(Box::new(s.clone())); }
    if let Some(s) = &f.to          { params.push(Box::new(s.clone())); }
    if let Some(v) = f.min_spread   { params.push(Box::new(v)); }
    if let Some(v) = f.max_spread   { params.push(Box::new(v)); }

    let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let conn = self.conn.lock();

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM trades {}", where_clause),
        params_ref.as_slice(),
        |r| r.get(0),
    )?;

    // Add LIMIT/OFFSET params
    let mut params2: Vec<Box<dyn rusqlite::ToSql>> = params;
    params2.push(Box::new(PAGE_SIZE));
    params2.push(Box::new(offset));
    let params2_ref: Vec<&dyn rusqlite::ToSql> = params2.iter().map(|p| p.as_ref()).collect();

    let sql = format!(
        "SELECT id, symbol, buy_exchange, buy_market_type,
                sell_exchange, sell_market_type,
                buy_ask, sell_bid, spread_pct, quantity,
                gross_pnl, fees, net_pnl, exec_ms, completed_at
         FROM trades {}
         ORDER BY completed_at DESC
         LIMIT ? OFFSET ?",
        where_clause
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params2_ref.as_slice(), |r| {
        Ok(TradeRow {
            id:               r.get(0)?,
            symbol:           r.get(1)?,
            buy_exchange:     r.get(2)?,
            buy_market_type:  r.get(3)?,
            sell_exchange:    r.get(4)?,
            sell_market_type: r.get(5)?,
            buy_ask:          r.get(6)?,
            sell_bid:         r.get(7)?,
            spread_pct:       r.get(8)?,
            quantity:         r.get(9)?,
            gross_pnl:        r.get(10)?,
            fees:             r.get(11)?,
            net_pnl:          r.get(12)?,
            exec_ms:          r.get(13)?,
            completed_at:     r.get(14)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    Ok(TradesPage { total, page: page as u32, rows })
}

pub async fn query(&self, f: TradeFilter) -> Result<TradesPage> {
    let store = self.conn.clone();
    task::spawn_blocking(move || {
        let tmp = TradeStore { conn: store };
        tmp.query_sync(f)
    }).await?
}
```

- [ ] **Step 5: Run all trade_store tests**

```bash
~/.cargo/bin/cargo test trade_store 2>&1 | tail -10
```
Expected: 3 tests pass, 0 fail.

- [ ] **Step 6: Commit**

```bash
git add src/trade_store.rs
git commit -m "feat: TradeStore::query() with filters and pagination"
```

---

### Task 6: Restore MetricsCollector from StoredStats

**Files:**
- Modify: `src/metrics.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `src/metrics.rs`:

```rust
#[test]
fn with_initial_seeds_counters() {
    use crate::trade_store::StoredStats;
    let stats = StoredStats {
        trade_count: 100, wins: 98,
        total_pnl: 50.0, total_fees: 5.0, total_gross: 55.0,
        total_exec_ms: 5000, peak_pnl: 52.0,
    };
    let mc = MetricsCollector::with_initial(stats);
    let snap = mc.snapshot();
    assert_eq!(snap.trades, 100);
    assert!((snap.total_pnl - 50.0).abs() < 1e-6);
    assert!((snap.peak_pnl  - 52.0).abs() < 1e-6);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
~/.cargo/bin/cargo test metrics::tests::with_initial_seeds_counters 2>&1 | tail -5
```
Expected: `FAILED`.

- [ ] **Step 3: Add with_initial() to MetricsCollector**

Add these imports at the top of `src/metrics.rs` if not already present:
```rust
use crate::trade_store::StoredStats;
use rust_decimal::prelude::FromStr;
```

Add after `pub fn new()` in `impl MetricsCollector`:

```rust
pub fn with_initial(s: StoredStats) -> Self {
    let to_dec = |v: f64| Decimal::from_str(&format!("{:.8}", v)).unwrap_or(dec!(0));
    Self {
        inner: Mutex::new(Inner {
            trades:         s.trade_count,
            wins:           s.wins,
            total_pnl:      to_dec(s.total_pnl),
            total_fees:     to_dec(s.total_fees),
            total_gross_pnl:to_dec(s.total_gross),
            peak_pnl:       to_dec(s.peak_pnl),
            max_drawdown:   dec!(0),
            total_exec_ms:  s.total_exec_ms,
            signals_sent:   0,
            rejected_cooldown: 0,
            rejected_risk:  0,
        }),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
~/.cargo/bin/cargo test metrics::tests::with_initial_seeds_counters 2>&1 | tail -5
```
Expected: `ok`.

- [ ] **Step 5: Commit**

```bash
git add src/metrics.rs
git commit -m "feat: MetricsCollector::with_initial() for startup restore"
```

---

### Task 7: Wire TradeStore into main.rs — open DB + restore metrics

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add TradeStore to main.rs**

In `src/main.rs`, locate where `MetricsCollector::new()` is called (search for `MetricsCollector::new`). Replace the startup section. Add these lines before the `MetricsCollector` line:

```rust
use trade_store::TradeStore;

// Open trade DB and restore metrics
let trade_store = Arc::new(
    TradeStore::open("trades.db").expect("Failed to open trades.db")
);
let stored_stats = trade_store.load_stats().await
    .unwrap_or_else(|e| {
        tracing::warn!("Could not load stored stats: {}", e);
        trade_store::StoredStats {
            trade_count: 0, wins: 0,
            total_pnl: 0.0, total_fees: 0.0, total_gross: 0.0,
            total_exec_ms: 0, peak_pnl: 0.0,
        }
    });
```

Then replace `let metrics = Arc::new(MetricsCollector::new());` with:
```rust
let metrics = Arc::new(MetricsCollector::with_initial(stored_stats));
```

- [ ] **Step 2: Verify it compiles**

```bash
~/.cargo/bin/cargo build --release 2>&1 | grep -E "^error|Finished"
```
Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: restore metrics from trades.db on startup"
```

---

### Task 8: Wire insert() into executor.rs

**Files:**
- Modify: `src/arbitrage/executor.rs`
- Modify: `src/main.rs` (pass trade_store to OrderExecutor)

- [ ] **Step 1: Add trade_store field to OrderExecutor**

In `src/arbitrage/executor.rs`, locate `pub struct OrderExecutor`. Add the field:

```rust
pub struct OrderExecutor {
    // ... existing fields ...
    trade_store: Arc<crate::trade_store::TradeStore>,
}
```

Update `pub fn new(...)` signature by adding `trade_store: Arc<crate::trade_store::TradeStore>` as the last parameter, and add it to the `Self { ... }` body.

- [ ] **Step 2: Call insert() after metrics.record()**

In `executor.rs`, in the `handle()` method, replace:

```rust
Ok(trade) => {
    self.risk.on_trade_close(trade.net_pnl);
    self.metrics.record(&trade);
    self.dashboard.push_trade(&trade);
}
```

with:

```rust
Ok(trade) => {
    self.risk.on_trade_close(trade.net_pnl);
    self.metrics.record(&trade);
    self.dashboard.push_trade(&trade);
    if let Err(e) = self.trade_store.insert(&trade).await {
        tracing::warn!("Failed to persist trade {}: {}", trade.id, e);
    }
}
```

- [ ] **Step 3: Update main.rs to pass trade_store to OrderExecutor**

In `src/main.rs`, find where `OrderExecutor::new(...)` is called. Add `trade_store.clone()` as the last argument.

- [ ] **Step 4: Verify it compiles**

```bash
~/.cargo/bin/cargo build --release 2>&1 | grep -E "^error|Finished"
```
Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
git add src/arbitrage/executor.rs src/main.rs
git commit -m "feat: persist every trade to SQLite via executor"
```

---

### Task 9: Add /api/trades endpoint

**Files:**
- Modify: `src/dashboard/state.rs`
- Modify: `src/dashboard/mod.rs`
- Modify: `src/dashboard/routes.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add trade_store to DashboardState**

In `src/dashboard/state.rs`, add field to `DashboardState`:

```rust
pub struct DashboardState {
    // ... existing fields ...
    pub trade_store: Arc<crate::trade_store::TradeStore>,
}
```

Update `DashboardState::new(...)` signature by adding:
```rust
trade_store: Arc<crate::trade_store::TradeStore>,
```
and add `trade_store` to the `Arc::new(Self { ... })` body.

- [ ] **Step 2: Pass trade_store in main.rs**

In `src/main.rs`, find `DashboardState::new(...)` call and add `trade_store.clone()` as the last argument.

- [ ] **Step 3: Register /api/trades route**

In `src/dashboard/mod.rs`, find the `.route(...)` chain and add:
```rust
.route("/api/trades", get(routes::trades_handler))
```

- [ ] **Step 4: Implement trades_handler**

In `src/dashboard/routes.rs`, add:

```rust
use crate::trade_store::{TradeFilter, TradesPage};

pub async fn trades_handler(
    State(state): State<Arc<DashboardState>>,
    axum::extract::Query(filter): axum::extract::Query<TradeFilter>,
) -> Result<Json<TradesPage>, (axum::http::StatusCode, String)> {
    state.trade_store.query(filter).await
        .map(Json)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
```

- [ ] **Step 5: Verify it compiles**

```bash
~/.cargo/bin/cargo build --release 2>&1 | grep -E "^error|Finished"
```
Expected: `Finished`.

- [ ] **Step 6: Quick smoke test**

Start the bot and hit the endpoint:
```bash
./target/release/sol-arb &
sleep 3
curl -s "http://localhost:3001/api/trades?page=0" | python3 -c "import json,sys; d=json.load(sys.stdin); print('total:', d['total'], 'rows:', len(d['rows']))"
```
Expected: `total: <N> rows: 0..50` (N = number of trades in trades.db)

- [ ] **Step 7: Kill test bot, commit**

```bash
pkill -f sol-arb
git add src/dashboard/state.rs src/dashboard/mod.rs src/dashboard/routes.rs src/main.rs
git commit -m "feat: GET /api/trades with filters and pagination"
```

---

### Task 10: Build TradesHistory React component

**Files:**
- Create: `dashboard/src/components/TradesHistory.tsx`

- [ ] **Step 1: Create the component**

Create `dashboard/src/components/TradesHistory.tsx`:

```tsx
import { useEffect, useState, useCallback } from 'react'
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

interface TradeRow {
  id: string; symbol: string
  buy_exchange: string; buy_market_type: string
  sell_exchange: string; sell_market_type: string
  buy_ask: number; sell_bid: number
  spread_pct: number; quantity: number
  gross_pnl: number; fees: number; net_pnl: number
  exec_ms: number; completed_at: string
}

interface TradesPage { total: number; page: number; rows: TradeRow[] }

const EXCHANGES = ['Binance','Bybit','OKX','BingX','Bitget','KuCoin','Gate']

function fmt(v: number, d = 4) { return v.toFixed(d) }
function fmtTime(s: string) {
  const d = new Date(s)
  return `${d.toLocaleDateString('ru')} ${d.toLocaleTimeString('ru', { hour: '2-digit', minute: '2-digit', second: '2-digit' })}`
}

export function TradesHistory() {
  const [page, setPage]           = useState(0)
  const [data, setData]           = useState<TradesPage | null>(null)
  const [symbol, setSymbol]       = useState('')
  const [buyEx, setBuyEx]         = useState('')
  const [sellEx, setSellEx]       = useState('')
  const [from, setFrom]           = useState('')
  const [to, setTo]               = useState('')
  const [minSpread, setMinSpread] = useState('')
  const [maxSpread, setMaxSpread] = useState('')
  const [loading, setLoading]     = useState(false)

  const load = useCallback(() => {
    setLoading(true)
    const p = new URLSearchParams()
    p.set('page', String(page))
    if (symbol)    p.set('symbol',       symbol.toUpperCase())
    if (buyEx)     p.set('buy_exchange',  buyEx)
    if (sellEx)    p.set('sell_exchange', sellEx)
    if (from)      p.set('from', new Date(from).toISOString())
    if (to)        p.set('to',   new Date(to + 'T23:59:59').toISOString())
    if (minSpread) p.set('min_spread', minSpread)
    if (maxSpread) p.set('max_spread', maxSpread)
    fetch(`/api/trades?${p}`)
      .then(r => r.json())
      .then((d: TradesPage) => { setData(d); setLoading(false) })
      .catch(() => setLoading(false))
  }, [page, symbol, buyEx, sellEx, from, to, minSpread, maxSpread])

  useEffect(() => { load() }, [load])

  function reset() {
    setSymbol(''); setBuyEx(''); setSellEx('')
    setFrom(''); setTo(''); setMinSpread(''); setMaxSpread('')
    setPage(0)
  }

  // Build cumulative P&L chart data from current page rows (newest-first → reverse)
  const chartData = (data?.rows ?? []).slice().reverse().map((r, i, arr) => ({
    t: fmtTime(r.completed_at),
    cum: arr.slice(0, i + 1).reduce((s, x) => s + x.net_pnl, 0),
  }))

  const totalPages = data ? Math.ceil(data.total / 50) : 0

  const inputStyle: React.CSSProperties = {
    background: '#0a0a0a', border: '1px solid #2a2a2a', borderRadius: 4,
    color: '#e0e0e0', padding: '4px 8px', fontSize: 11, fontFamily: 'inherit',
    outline: 'none', width: 100,
  }
  const selectStyle: React.CSSProperties = { ...inputStyle, width: 90 }

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      {/* Filter bar */}
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 12, alignItems: 'center' }}>
        <input value={symbol} onChange={e => { setSymbol(e.target.value); setPage(0) }}
          placeholder="Символ" style={inputStyle} />
        <select value={buyEx} onChange={e => { setBuyEx(e.target.value); setPage(0) }} style={selectStyle}>
          <option value="">Купил (все)</option>
          {EXCHANGES.map(e => <option key={e} value={e}>{e}</option>)}
        </select>
        <select value={sellEx} onChange={e => { setSellEx(e.target.value); setPage(0) }} style={selectStyle}>
          <option value="">Продал (все)</option>
          {EXCHANGES.map(e => <option key={e} value={e}>{e}</option>)}
        </select>
        <input type="date" value={from} onChange={e => { setFrom(e.target.value); setPage(0) }}
          style={{ ...inputStyle, width: 120 }} />
        <input type="date" value={to} onChange={e => { setTo(e.target.value); setPage(0) }}
          style={{ ...inputStyle, width: 120 }} />
        <input value={minSpread} onChange={e => { setMinSpread(e.target.value); setPage(0) }}
          placeholder="Спред от %" style={{ ...inputStyle, width: 80 }} />
        <input value={maxSpread} onChange={e => { setMaxSpread(e.target.value); setPage(0) }}
          placeholder="до %" style={{ ...inputStyle, width: 60 }} />
        <button onClick={reset} style={{
          background: '#1a1a1a', border: '1px solid #333', borderRadius: 4,
          color: '#666', padding: '4px 10px', fontSize: 11, cursor: 'pointer',
        }}>Сбросить</button>
        <span style={{ color: '#444', fontSize: 11, marginLeft: 'auto' }}>
          {loading ? 'Загрузка...' : `Всего: ${data?.total ?? 0}`}
        </span>
      </div>

      {/* Table */}
      <div style={{ overflowX: 'auto', overflowY: 'auto', maxHeight: 380 }}>
        <table style={{ borderCollapse: 'collapse', fontSize: 11, whiteSpace: 'nowrap', width: '100%' }}>
          <thead style={{ position: 'sticky', top: 0, background: '#111', zIndex: 1 }}>
            <tr style={{ borderBottom: '1px solid #222' }}>
              {['Время','Пара','Купил','Продал','Спред%','Кол-во','Gross','Комиссии','Net P&L','Exec ms'].map(h => (
                <th key={h} style={{ padding: '5px 8px', textAlign: 'right', color: '#555',
                  fontSize: 10, textTransform: 'uppercase', fontWeight: 400,
                  ...(h === 'Время' || h === 'Пара' ? { textAlign: 'left' } : {}) }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {(data?.rows ?? []).map(r => (
              <tr key={r.id} style={{ borderBottom: '1px solid #1a1a1a' }}>
                <td style={{ padding: '4px 8px', color: '#555', fontFamily: 'monospace', fontSize: 10 }}>
                  {fmtTime(r.completed_at)}
                </td>
                <td style={{ padding: '4px 8px', color: '#ccc', fontWeight: 600 }}>
                  {r.symbol.replace('USDT', '')}<span style={{ color: '#333' }}>/U</span>
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#4488ff', fontFamily: 'monospace' }}>
                  {r.buy_exchange}<span style={{ color: '#444' }}>:{r.buy_market_type}</span>
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#00ff87', fontFamily: 'monospace' }}>
                  {r.sell_exchange}<span style={{ color: '#444' }}>:{r.sell_market_type}</span>
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#aaaa00', fontFamily: 'monospace' }}>
                  {fmt(r.spread_pct, 3)}%
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#555', fontFamily: 'monospace' }}>
                  {fmt(r.quantity, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#666', fontFamily: 'monospace' }}>
                  {fmt(r.gross_pnl, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#444', fontFamily: 'monospace' }}>
                  {fmt(r.fees, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', fontFamily: 'monospace',
                  color: r.net_pnl >= 0 ? '#00ff87' : '#ff4444', fontWeight: 600 }}>
                  {r.net_pnl >= 0 ? '+' : ''}{fmt(r.net_pnl, 4)}
                </td>
                <td style={{ padding: '4px 8px', textAlign: 'right', color: '#444', fontFamily: 'monospace' }}>
                  {r.exec_ms}
                </td>
              </tr>
            ))}
            {(data?.rows ?? []).length === 0 && !loading && (
              <tr><td colSpan={10} style={{ padding: 20, color: '#333', textAlign: 'center' }}>
                Нет данных
              </td></tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 8, alignItems: 'center' }}>
        <button onClick={() => setPage(p => Math.max(0, p - 1))} disabled={page === 0}
          style={{ background: '#1a1a1a', border: '1px solid #333', borderRadius: 4,
            color: page === 0 ? '#333' : '#666', padding: '3px 10px', fontSize: 11, cursor: 'pointer' }}>
          ←
        </button>
        <span style={{ color: '#444', fontSize: 11 }}>
          {page + 1} / {totalPages || 1}
        </span>
        <button onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
          disabled={page >= totalPages - 1}
          style={{ background: '#1a1a1a', border: '1px solid #333', borderRadius: 4,
            color: page >= totalPages - 1 ? '#333' : '#666', padding: '3px 10px', fontSize: 11, cursor: 'pointer' }}>
          →
        </button>
      </div>

      {/* Cumulative P&L Chart */}
      {chartData.length > 1 && (
        <div style={{ marginTop: 16 }}>
          <div style={{ color: '#444', fontSize: 10, textTransform: 'uppercase',
            letterSpacing: '0.05em', marginBottom: 6 }}>
            Кумулятивный P&L (текущая страница)
          </div>
          <ResponsiveContainer width="100%" height={120}>
            <LineChart data={chartData}>
              <XAxis dataKey="t" hide />
              <YAxis width={55} tick={{ fill: '#444', fontSize: 10 }}
                tickFormatter={v => `$${v.toFixed(2)}`} />
              <Tooltip
                contentStyle={{ background: '#1a1a1a', border: '1px solid #333', fontSize: 11 }}
                formatter={(v: number) => [`$${v.toFixed(4)}`, 'P&L']}
                labelStyle={{ color: '#666' }}
              />
              <Line type="monotone" dataKey="cum" stroke="#00ff87"
                dot={false} strokeWidth={1.5} />
            </LineChart>
          </ResponsiveContainer>
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
git add dashboard/src/components/TradesHistory.tsx
git commit -m "feat: TradesHistory component with table, filters, and P&L chart"
```

---

### Task 11: Wire TradesHistory tab into App.tsx

**Files:**
- Modify: `dashboard/src/App.tsx`

- [ ] **Step 1: Add import and tab type**

In `dashboard/src/App.tsx`:

1. Add import at the top:
```tsx
import { TradesHistory } from './components/TradesHistory'
```

2. Change the Tab type from:
```tsx
type Tab = 'dashboard' | 'settings'
```
to:
```tsx
type Tab = 'dashboard' | 'history' | 'settings'
```

- [ ] **Step 2: Add nav tab button**

Find where the existing `<NavTab>` buttons are rendered (around line 55-56). Add a new tab between Дашборд and Настройки:
```tsx
<NavTab label="История" active={tab === 'history'} onClick={() => setTab('history')} />
```

- [ ] **Step 3: Render TradesHistory panel**

Find the conditional rendering block (around line 63). Add a branch for the history tab:
```tsx
{tab === 'history' ? (
  <TradesHistory />
) : tab === 'settings' ? (
  <SettingsPage />
) : (
  // existing dashboard JSX
)}
```

- [ ] **Step 4: Build frontend**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -5
```
Expected: `built in Xs` with no errors.

- [ ] **Step 5: Start bot and verify tab in browser**

```bash
cd /Users/rinchin92/claude/project && ./target/release/sol-arb &
sleep 2
open http://localhost:3001
```
Verify: "История" tab appears, clicking it shows the table, filter bar, and pagination. If `trades.db` has rows, they load immediately.

- [ ] **Step 6: Commit**

```bash
git add dashboard/src/App.tsx
git commit -m "feat: История tab wired into dashboard"
```

---

## Self-Review

**Spec coverage check:**
- ✅ SQLite with rusqlite bundled — Task 1
- ✅ Schema with all required columns + indexes — Task 2
- ✅ `insert()` with INSERT OR IGNORE — Task 3
- ✅ `load_stats()` with peak_pnl via window function — Task 4
- ✅ `query()` with all 7 filter fields + pagination — Task 5
- ✅ `MetricsCollector::with_initial()` — Task 6
- ✅ Startup restoration in main.rs — Task 7
- ✅ insert() called from executor, WARN on error — Task 8
- ✅ `/api/trades` endpoint — Task 9
- ✅ Trade table with all specified columns — Task 10
- ✅ Filters: symbol, buy/sell exchange, date range, spread % — Task 10
- ✅ Pagination 50 rows/page — Task 10
- ✅ Cumulative P&L chart (recharts) — Task 10
- ✅ New tab in App.tsx — Task 11

**Placeholder scan:** No TBDs, all code blocks complete.

**Type consistency:** `StoredStats` defined in Task 2, used in Tasks 4, 6, 7. `TradeFilter`/`TradesPage`/`TradeRow` defined in Task 5, used in Task 9. `with_initial(StoredStats)` defined in Task 6, called in Task 7. All consistent.

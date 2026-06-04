# SQLite Trade History — Design Spec

## Goal

Persist every completed trade to SQLite so that:
1. Metrics (total P&L, trade count, drawdown) survive bot restarts.
2. A new "История" tab in the dashboard shows a filterable trade table and a cumulative P&L chart.

## Database

**File:** `trades.db` in the bot's working directory.  
**Library:** `rusqlite` with the `bundled` feature (SQLite compiled into the binary, no system dependency).  
**Async integration:** all DB calls wrapped in `tokio::task::spawn_blocking`.

### Schema

```sql
CREATE TABLE IF NOT EXISTS trades (
    id              TEXT PRIMARY KEY,
    symbol          TEXT    NOT NULL,
    buy_exchange    TEXT    NOT NULL,
    buy_market_type TEXT    NOT NULL,
    sell_exchange   TEXT    NOT NULL,
    sell_market_type TEXT   NOT NULL,
    buy_ask         REAL    NOT NULL,
    sell_bid        REAL    NOT NULL,
    spread_pct      REAL    NOT NULL,
    quantity        REAL    NOT NULL,
    gross_pnl       REAL    NOT NULL,
    fees            REAL    NOT NULL,
    net_pnl         REAL    NOT NULL,
    exec_ms         INTEGER NOT NULL,
    completed_at    TEXT    NOT NULL   -- ISO 8601, e.g. "2026-06-04T18:00:00Z"
);

CREATE INDEX IF NOT EXISTS idx_trades_time   ON trades(completed_at);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);
```

## Module: `src/trade_store.rs`

```
TradeStore
  Arc<Mutex<rusqlite::Connection>>

  pub fn open(path: &str) -> Result<Self>
      Opens (or creates) the DB file, runs CREATE TABLE IF NOT EXISTS.

  pub async fn insert(&self, trade: &CompletedTrade) -> Result<()>
      Serialises CompletedTrade fields → INSERT OR IGNORE INTO trades.
      Runs in spawn_blocking.

  pub async fn load_stats(&self) -> Result<StoredStats>
      SELECT COUNT(*), SUM(net_pnl), SUM(fees), MAX(net_pnl), AVG(exec_ms) FROM trades.
      Returns StoredStats { trade_count, total_pnl, total_fees, peak_pnl, avg_exec_ms }.
      Runs in spawn_blocking.

  pub async fn query(&self, f: TradeFilter) -> Result<Vec<TradeRow>>
      Builds a parameterised SELECT with optional WHERE clauses.
      Supports: symbol LIKE, buy_exchange =, sell_exchange =,
                completed_at BETWEEN from AND to,
                spread_pct BETWEEN min AND max.
      ORDER BY completed_at DESC.
      LIMIT / OFFSET for pagination (page_size=50).
      Runs in spawn_blocking.
```

`TradeFilter` fields (all optional):
- `symbol: Option<String>`
- `buy_exchange: Option<String>`
- `sell_exchange: Option<String>`
- `from: Option<DateTime<Utc>>`
- `to: Option<DateTime<Utc>>`
- `min_spread: Option<f64>`
- `max_spread: Option<f64>`
- `page: Option<u32>` (0-indexed)

## Integration points

### `src/main.rs`
- Open `TradeStore::open("trades.db")` on startup.
- Pass `Arc<TradeStore>` to `MetricsCollector` and `OrderExecutor`.

### `src/metrics.rs`
- Add `MetricsCollector::with_initial(stats: StoredStats)` constructor.
- Seeds `total_pnl`, `trades`, `peak_pnl`, `avg_exec_ms` from `StoredStats` so counters continue from where they left off.

### `src/arbitrage/executor.rs`
- After `metrics.record(&trade)`, call `trade_store.insert(&trade).await`.
- Log a WARN on insert error (non-fatal — don't drop the trade).

### `src/dashboard/routes.rs`
- New handler: `GET /api/trades`
- Query params map to `TradeFilter`.
- Response: `{ total: u64, page: u32, rows: Vec<TradeRow> }`.

`TradeRow` (JSON-serialisable):
```
id, symbol, buy_exchange, buy_market_type, sell_exchange, sell_market_type,
buy_ask, sell_bid, spread_pct, quantity, gross_pnl, fees, net_pnl,
exec_ms, completed_at
```

## Frontend: `dashboard/src/components/TradesHistory.tsx`

New tab "История" added to `App.tsx`.

**Filter bar (top):**
- Symbol text input
- Buy exchange dropdown (all 7 + "Any")
- Sell exchange dropdown (all 7 + "Any")
- Date range: two `<input type="date">` fields
- Spread % range: min / max numeric inputs
- "Сбросить" button

**Table:**
- Columns: Время | Пара | Купил | Продал | Спред% | P&L | Комиссии | Exec ms
- Sorted by time DESC by default, all columns clickable to sort
- Pagination: 50 rows per page, prev/next buttons

**Chart:**
- Cumulative P&L line chart (recharts `LineChart`) below the table
- X-axis: date (day granularity), Y-axis: USDT
- Computed client-side from all returned rows (no extra endpoint needed)

## Dependency addition

`Cargo.toml`:
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

## Error handling

- DB open failure → fatal, bot exits with error.
- Insert failure → WARN log, trade still recorded in memory metrics, bot continues.
- Query failure → API returns HTTP 500 with error message.

## What is NOT in scope

- Migrations (schema is created once; ALTER TABLE out of scope for now).
- Trade deletion or editing.
- Export to CSV (can be done externally with `sqlite3 trades.db .csv`).

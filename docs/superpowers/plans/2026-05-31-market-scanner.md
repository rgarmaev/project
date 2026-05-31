# Market Scanner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a sortable cross-exchange spread table (Binance vs Bybit) for 50 USDT pairs, shown above the PnlChart on the main dashboard.

**Architecture:** A new Rust `MarketScanner` module polls Binance and Bybit REST APIs every 2 seconds, stores results in an in-memory `RwLock<Vec<MarketRow>>`, and exposes them via `GET /api/market`. A new React `MarketScanner` component fetches this endpoint every 3 seconds and renders a sortable table.

**Tech Stack:** Rust (reqwest, parking_lot, axum), React 18, TypeScript

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Create | `src/market_scanner/mod.rs` | Poll loop, MarketRow type, fetch Binance+Bybit |
| Modify | `src/main.rs` | Add `mod market_scanner`, create scanner, spawn task, pass to DashboardState |
| Modify | `src/dashboard/state.rs` | Add `scanner` field to DashboardState, add `market_snapshot()` |
| Modify | `src/dashboard/routes.rs` | Add `market_handler` |
| Modify | `src/dashboard/mod.rs` | Register `GET /api/market` |
| Modify | `dashboard/src/types.ts` | Add `MarketRow` interface |
| Create | `dashboard/src/components/MarketScanner.tsx` | Sortable table, polls /api/market every 3s |
| Modify | `dashboard/src/App.tsx` | Mount `<MarketScanner />` above PnlChart/ChartsRow grid |

---

### Task 1: Create MarketScanner Rust module

**Files:**
- Create: `src/market_scanner/mod.rs`

- [ ] **Step 1: Create the file**

```rust
use parking_lot::RwLock;
use reqwest::Client;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

const TICKERS: &[&str] = &[
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT",
    "ADAUSDT", "DOGEUSDT", "AVAXUSDT", "DOTUSDT", "MATICUSDT",
    "LINKUSDT", "LTCUSDT", "UNIUSDT", "ATOMUSDT", "BCHUSDT",
    "ICPUSDT", "APTUSDT", "ARBUSDT", "OPUSDT", "FILUSDT",
    "NEARUSDT", "SANDUSDT", "MANAUSDT", "AXSUSDT", "ALGOUSDT",
    "VETUSDT", "FTMUSDT", "HBARUSDT", "ETCUSDT", "XLMUSDT",
    "TRXUSDT", "SUIUSDT", "SEIUSDT", "INJUSDT", "TIAUSDT",
    "JUPUSDT", "WIFUSDT", "BONKUSDT", "PEPEUSDT", "SHIBUSDT",
    "NOTUSDT", "TONUSDT", "STXUSDT", "RUNEUSDT", "RENDERUSDT",
    "WLDUSDT", "ENAUSDT", "ZKUSDT", "THETAUSDT", "FLOKIUSDT",
];

#[derive(Serialize, Clone)]
pub struct MarketRow {
    pub symbol: String,
    pub binance_ask: f64,
    pub binance_bid: f64,
    pub bybit_ask: f64,
    pub bybit_bid: f64,
    pub spread_ab: f64,  // (bybit_bid - binance_ask) / binance_ask * 100
    pub spread_ba: f64,  // (binance_bid - bybit_ask) / bybit_ask * 100
}

pub struct MarketScanner {
    state: RwLock<Vec<MarketRow>>,
    http: Client,
}

impl MarketScanner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: RwLock::new(Vec::new()),
            http: Client::new(),
        })
    }

    pub fn snapshot(&self) -> Vec<MarketRow> {
        self.state.read().clone()
    }

    pub async fn run(self: Arc<Self>) {
        info!("MarketScanner started — polling Binance+Bybit every 2s for {} pairs", TICKERS.len());
        let mut tick = interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            if let Err(e) = self.poll_once().await {
                warn!("MarketScanner poll failed: {e}");
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let (binance_res, bybit_res) = tokio::join!(
            self.fetch_binance(),
            self.fetch_bybit(),
        );

        let binance_map = binance_res.unwrap_or_default();
        let bybit_map   = bybit_res.unwrap_or_default();

        if binance_map.is_empty() && bybit_map.is_empty() {
            return Ok(());
        }

        let rows: Vec<MarketRow> = TICKERS.iter().filter_map(|&sym| {
            let &(b_bid, b_ask) = binance_map.get(sym)?;
            let &(y_bid, y_ask) = bybit_map.get(sym)?;

            let spread_ab = if b_ask > 0.0 { (y_bid - b_ask) / b_ask * 100.0 } else { 0.0 };
            let spread_ba = if y_ask > 0.0 { (b_bid - y_ask) / y_ask * 100.0 } else { 0.0 };

            Some(MarketRow {
                symbol:      sym.to_string(),
                binance_ask: b_ask,
                binance_bid: b_bid,
                bybit_ask:   y_ask,
                bybit_bid:   y_bid,
                spread_ab,
                spread_ba,
            })
        }).collect();

        if !rows.is_empty() {
            *self.state.write() = rows;
        }
        Ok(())
    }

    async fn fetch_binance(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: Vec<serde_json::Value> = self.http
            .get("https://api.binance.com/api/v3/ticker/bookTicker")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;

        let set: HashSet<&str> = TICKERS.iter().copied().collect();
        let mut map = HashMap::new();
        for item in resp {
            let sym = item["symbol"].as_str().unwrap_or("").to_string();
            if set.contains(sym.as_str()) {
                let bid: f64 = item["bidPrice"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let ask: f64 = item["askPrice"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                map.insert(sym, (bid, ask));
            }
        }
        Ok(map)
    }

    async fn fetch_bybit(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: serde_json::Value = self.http
            .get("https://api.bybit.com/v5/market/tickers?category=spot")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;

        let set: HashSet<&str> = TICKERS.iter().copied().collect();
        let mut map = HashMap::new();
        if let Some(list) = resp["result"]["list"].as_array() {
            for item in list {
                let sym = item["symbol"].as_str().unwrap_or("").to_string();
                if set.contains(sym.as_str()) {
                    let bid: f64 = item["bid1Price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let ask: f64 = item["ask1Price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    map.insert(sym, (bid, ask));
                }
            }
        }
        Ok(map)
    }
}
```

- [ ] **Step 2: Verify it compiles (no wiring yet)**

```bash
cd /Users/rinchin92/claude/project
echo 'mod market_scanner;' >> src/main.rs
~/.cargo/bin/cargo check 2>&1 | grep "^error" | head -10
```

Expected: errors about unused import or `mod` not found — that's fine. If `src/market_scanner/mod.rs` is created correctly, there should be no parse errors. Revert the temp change to main.rs:

```bash
git checkout src/main.rs
```

- [ ] **Step 3: Commit**

```bash
git add src/market_scanner/mod.rs
git commit -m "feat: add MarketScanner module with Binance+Bybit polling"
```

---

### Task 2: Wire MarketScanner into backend

**Files:**
- Modify: `src/main.rs`
- Modify: `src/dashboard/state.rs`
- Modify: `src/dashboard/routes.rs`
- Modify: `src/dashboard/mod.rs`

- [ ] **Step 1: Add `mod market_scanner` to main.rs and update imports**

At the top of `src/main.rs`, add after the existing mod declarations:

```rust
mod market_scanner;
```

Add to the use block:

```rust
use market_scanner::MarketScanner;
```

- [ ] **Step 2: Create scanner and wire into main.rs**

After `let metrics = Arc::new(MetricsCollector::new());`, add:

```rust
let scanner = MarketScanner::new();
```

Update the `DashboardState::new` call (currently line 54):

```rust
let dash_state = DashboardState::new(price_state.clone(), metrics.clone(), config.clone(), scanner.clone());
```

Add scanner task in the `// ── Dashboard ──` section, before the broadcast_loop spawn:

```rust
{
    let s = scanner.clone();
    set.spawn(async move { s.run().await });
}
```

- [ ] **Step 3: Update DashboardState to hold scanner**

In `src/dashboard/state.rs`, add import at the top:

```rust
use crate::market_scanner::{MarketRow, MarketScanner};
```

Add `scanner` field to `DashboardState` struct:

```rust
pub struct DashboardState {
    trades: Mutex<VecDeque<TradeRecord>>,
    pub price_state: Arc<PriceState>,
    pub metrics: Arc<MetricsCollector>,
    pub broadcast_tx: broadcast::Sender<String>,
    pub config: Arc<Config>,
    pub scanner: Arc<MarketScanner>,
}
```

Update `DashboardState::new` signature and body:

```rust
pub fn new(
    price_state: Arc<PriceState>,
    metrics: Arc<MetricsCollector>,
    config: Arc<Config>,
    scanner: Arc<MarketScanner>,
) -> Arc<Self> {
    let (broadcast_tx, _) = broadcast::channel(64);
    Arc::new(Self {
        trades: Mutex::new(VecDeque::with_capacity(MAX_TRADES)),
        price_state,
        metrics,
        broadcast_tx,
        config,
        scanner,
    })
}
```

Add `market_snapshot` method to `DashboardState`:

```rust
pub fn market_snapshot(&self) -> Vec<MarketRow> {
    self.scanner.snapshot()
}
```

- [ ] **Step 4: Fix the test `make_state` helper**

In the `#[cfg(test)]` module at the bottom of `state.rs`, the `make_state()` helper must pass a scanner. Update it:

```rust
fn make_state() -> Arc<DashboardState> {
    let config = Arc::new(Config::load().unwrap());
    let scanner = crate::market_scanner::MarketScanner::new();
    DashboardState::new(Arc::new(PriceState::new()), Arc::new(MetricsCollector::new()), config, scanner)
}
```

- [ ] **Step 5: Add market_handler to routes.rs**

In `src/dashboard/routes.rs`, add import:

```rust
use crate::market_scanner::MarketRow;
```

Add handler at the end of the file:

```rust
pub async fn market_handler(
    State(state): State<Arc<DashboardState>>,
) -> Json<Vec<MarketRow>> {
    Json(state.market_snapshot())
}
```

- [ ] **Step 6: Register route in mod.rs**

In `src/dashboard/mod.rs`, add to the router:

```rust
.route("/api/market", get(routes::market_handler))
```

The full router after the change:

```rust
let app = Router::new()
    .route("/ws", get(ws::ws_handler))
    .route("/api/trades", get(routes::trades_handler))
    .route("/api/config", get(config_api::get_config).post(config_api::post_config))
    .route("/api/restart", axum::routing::post(routes::restart_handler))
    .route("/api/market", get(routes::market_handler))
    .fallback_service(ServeDir::new("dashboard/dist"))
    .with_state(state)
    .layer(CorsLayer::permissive());
```

- [ ] **Step 7: Run tests and build**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo test 2>&1 | tail -5
```

Expected: all tests pass.

```bash
~/.cargo/bin/cargo build --release 2>&1 | tail -3
```

Expected: `Finished release profile`.

- [ ] **Step 8: Smoke test the endpoint**

```bash
pkill -f "sol-arb" 2>/dev/null; lsof -ti:3001 | xargs kill -9 2>/dev/null; sleep 1
RUST_LOG=sol_arb=info ./target/release/sol-arb > /tmp/sol-arb.log 2>&1 &
sleep 6 && curl -s http://localhost:3001/api/market | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'{len(d)} rows, first:', d[0]['symbol'] if d else 'empty')"
```

Expected: `N rows, first: BTCUSDT` (or similar).

- [ ] **Step 9: Commit**

```bash
git add src/main.rs src/dashboard/state.rs src/dashboard/routes.rs src/dashboard/mod.rs
git commit -m "feat: wire MarketScanner into dashboard — /api/market endpoint"
```

---

### Task 3: Add MarketRow TypeScript type

**Files:**
- Modify: `dashboard/src/types.ts`

- [ ] **Step 1: Add MarketRow interface**

In `dashboard/src/types.ts`, add at the end of the file:

```typescript
export interface MarketRow {
  symbol: string
  binance_ask: number
  binance_bid: number
  bybit_ask: number
  bybit_bid: number
  spread_ab: number
  spread_ba: number
}
```

- [ ] **Step 2: Commit**

```bash
git add dashboard/src/types.ts
git commit -m "feat: add MarketRow TypeScript type"
```

---

### Task 4: Create MarketScanner React component

**Files:**
- Create: `dashboard/src/components/MarketScanner.tsx`

- [ ] **Step 1: Create the file**

```typescript
import { useEffect, useState } from 'react'
import { MarketRow } from '../types'

type SortKey = keyof Pick<MarketRow,
  'symbol' | 'binance_ask' | 'bybit_bid' | 'spread_ab' | 'bybit_ask' | 'binance_bid' | 'spread_ba'
>

function fmt(v: number): string {
  if (v === 0) return '—'
  if (v >= 1000) return v.toLocaleString('en', { maximumSignificantDigits: 6 })
  return v.toPrecision(6)
}

function fmtSpread(v: number): string {
  return `${v >= 0 ? '+' : ''}${v.toFixed(4)}%`
}

const COL_HEADER: React.CSSProperties = {
  padding: '6px 10px',
  color: '#444',
  fontSize: 10,
  textAlign: 'left' as const,
  textTransform: 'uppercase' as const,
  cursor: 'pointer',
  userSelect: 'none' as const,
  whiteSpace: 'nowrap' as const,
}

export function MarketScanner() {
  const [rows, setRows] = useState<MarketRow[]>([])
  const [sortKey, setSortKey] = useState<SortKey>('spread_ab')
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('desc')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    function load() {
      fetch('/api/market')
        .then(r => r.json())
        .then((data: MarketRow[]) => { setRows(data); setLoading(false) })
        .catch(() => {})
    }
    load()
    const id = setInterval(load, 3000)
    return () => clearInterval(id)
  }, [])

  function toggleSort(key: SortKey) {
    if (key === sortKey) {
      setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    } else {
      setSortKey(key)
      setSortDir('desc')
    }
  }

  const sorted = [...rows].sort((a, b) => {
    const av = a[sortKey]
    const bv = b[sortKey]
    const cmp = typeof av === 'string'
      ? av.localeCompare(bv as string)
      : (av as number) - (bv as number)
    return sortDir === 'asc' ? cmp : -cmp
  })

  function arrow(key: SortKey) {
    if (key !== sortKey) return ' '
    return sortDir === 'asc' ? ' ▲' : ' ▼'
  }

  const columns: { label: string; key: SortKey; align?: 'right' }[] = [
    { label: 'Пара',        key: 'symbol' },
    { label: 'Bin Ask',     key: 'binance_ask',  align: 'right' },
    { label: 'Byb Bid',     key: 'bybit_bid',    align: 'right' },
    { label: 'B→Y Спред',   key: 'spread_ab',    align: 'right' },
    { label: 'Byb Ask',     key: 'bybit_ask',    align: 'right' },
    { label: 'Bin Bid',     key: 'binance_bid',  align: 'right' },
    { label: 'Y→B Спред',   key: 'spread_ba',    align: 'right' },
  ]

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 12 }}>
        Спред по рынку · Binance vs Bybit
      </div>

      {loading ? (
        <div style={{ color: '#333', textAlign: 'center', padding: '20px 0', fontSize: 13 }}>
          Загрузка...
        </div>
      ) : (
        <div style={{ overflowY: 'auto', maxHeight: 400 }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead style={{ position: 'sticky', top: 0, background: '#111' }}>
              <tr>
                {columns.map(col => (
                  <th
                    key={col.key}
                    style={{ ...COL_HEADER, textAlign: col.align ?? 'left' }}
                    onClick={() => toggleSort(col.key)}
                  >
                    {col.label}{arrow(col.key)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {sorted.map(row => (
                <tr key={row.symbol} style={{ borderBottom: '1px solid #1a1a1a' }}>
                  <td style={{ padding: '5px 10px', color: '#888', fontSize: 12 }}>
                    {row.symbol.replace('USDT', '')}<span style={{ color: '#444' }}>/USDT</span>
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.binance_ask)}
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.bybit_bid)}
                  </td>
                  <td style={{ padding: '5px 10px', fontSize: 11, textAlign: 'right', fontWeight: 600,
                    color: row.spread_ab > 0 ? '#00ff87' : '#555' }}>
                    {fmtSpread(row.spread_ab)}
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.bybit_ask)}
                  </td>
                  <td style={{ padding: '5px 10px', color: '#666', fontSize: 11, textAlign: 'right' }}>
                    {fmt(row.binance_bid)}
                  </td>
                  <td style={{ padding: '5px 10px', fontSize: 11, textAlign: 'right', fontWeight: 600,
                    color: row.spread_ba > 0 ? '#00ff87' : '#555' }}>
                    {fmtSpread(row.spread_ba)}
                  </td>
                </tr>
              ))}
              {sorted.length === 0 && (
                <tr>
                  <td colSpan={7} style={{ padding: 16, color: '#333', textAlign: 'center', fontSize: 12 }}>
                    Нет данных
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Commit**

```bash
git add dashboard/src/components/MarketScanner.tsx
git commit -m "feat: add MarketScanner sortable spread table component"
```

---

### Task 5: Mount MarketScanner in App.tsx

**Files:**
- Modify: `dashboard/src/App.tsx`

- [ ] **Step 1: Add import**

At the top of `dashboard/src/App.tsx`, add:

```typescript
import { MarketScanner } from './components/MarketScanner'
```

- [ ] **Step 2: Insert `<MarketScanner />` above the PnlChart/ChartsRow grid**

Find the dashboard tab JSX. Currently it looks like:

```tsx
<MetricsBar ... />
{pendingSymbol && <div ...>...</div>}
<div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, alignItems: 'start' }}>
  <PnlChart recentTrades={trades} />
  <ChartsRow prices={prices} />
</div>
```

Insert `<MarketScanner />` between the pending banner and the PnlChart grid:

```tsx
<MetricsBar metrics={metrics} paperTrading={false} effectiveMinSpreadPct={effectiveMinSpreadPct} />

{pendingSymbol && (
  <div style={{ ... }}>...</div>
)}

<MarketScanner />

<div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, alignItems: 'start' }}>
  <PnlChart recentTrades={trades} />
  <ChartsRow prices={prices} />
</div>

<div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
  <PriceTable prices={prices} />
  <TradesFeed trades={trades} />
</div>
```

- [ ] **Step 3: Build dashboard**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -3
```

Expected: `✓ built in Xs` with no TypeScript errors.

- [ ] **Step 4: Commit**

```bash
git add dashboard/src/App.tsx
git commit -m "feat: mount MarketScanner above PnlChart in App"
```

---

### Task 6: Build release and push

- [ ] **Step 1: Build Rust release binary**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo build --release 2>&1 | tail -3
```

Expected: `Finished release profile`.

- [ ] **Step 2: Restart bot and verify**

```bash
pkill -f "sol-arb" 2>/dev/null; lsof -ti:3001 | xargs kill -9 2>/dev/null; sleep 1
RUST_LOG=sol_arb=info ./target/release/sol-arb > /tmp/sol-arb.log 2>&1 &
sleep 8 && curl -s http://localhost:3001/api/market | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'{len(d)} pairs loaded')
if d:
    best = max(d, key=lambda r: r['spread_ab'])
    print(f'Best B→Y spread: {best[\"symbol\"]} {best[\"spread_ab\"]:.4f}%')
"
```

Expected: `N pairs loaded` (ideally 40-50) and a best spread line.

- [ ] **Step 3: Push**

```bash
git push origin master
```

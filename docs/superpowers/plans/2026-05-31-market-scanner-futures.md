# Market Scanner Futures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend MarketScanner to fetch and display futures (perp) pairs from Binance USDT-M and Bybit Linear, with a Spot/Perp toggle in the frontend table.

**Architecture:** Add `perp_state` to MarketScanner alongside existing spot data, fetch 4 endpoints in parallel in `poll_once`, return `MarketSnapshot { spot, perp }` from `/api/market`. Frontend stores both arrays and switches with a tab button.

**Tech Stack:** Rust (reqwest, parking_lot), React 18, TypeScript

---

## File Map

| Action | Path | Change |
|---|---|---|
| Modify | `src/market_scanner/mod.rs` | Add `perp_state`, `MarketSnapshot`, `fetch_binance_futures`, `fetch_bybit_futures`, extend `poll_once` |
| Modify | `src/dashboard/state.rs` | `market_snapshot()` returns `MarketSnapshot` |
| Modify | `src/dashboard/routes.rs` | `market_handler` returns `Json<MarketSnapshot>` |
| Modify | `dashboard/src/types.ts` | Add `MarketSnapshot` interface |
| Modify | `dashboard/src/components/MarketScanner.tsx` | Tab toggle, use `MarketSnapshot` |

---

### Task 1: Extend MarketScanner with futures data

**Files:**
- Modify: `src/market_scanner/mod.rs`

- [ ] **Step 1: Add `MarketSnapshot` struct and rename `state` to `spot_state`**

Replace the entire file with this updated version:

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
    pub spread_ab: f64,
    pub spread_ba: f64,
}

#[derive(Serialize, Clone)]
pub struct MarketSnapshot {
    pub spot: Vec<MarketRow>,
    pub perp: Vec<MarketRow>,
}

pub struct MarketScanner {
    spot_state: RwLock<Vec<MarketRow>>,
    perp_state: RwLock<Vec<MarketRow>>,
    http: Client,
}

impl MarketScanner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            spot_state: RwLock::new(Vec::new()),
            perp_state: RwLock::new(Vec::new()),
            http: Client::new(),
        })
    }

    pub fn full_snapshot(&self) -> MarketSnapshot {
        MarketSnapshot {
            spot: self.spot_state.read().clone(),
            perp: self.perp_state.read().clone(),
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!("MarketScanner started — polling Binance+Bybit spot+perp every 2s for {} pairs", TICKERS.len());
        let mut tick = interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            if let Err(e) = self.poll_once().await {
                warn!("MarketScanner poll failed: {e}");
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let (spot_binance_res, spot_bybit_res, perp_binance_res, perp_bybit_res) = tokio::join!(
            self.fetch_binance_spot(),
            self.fetch_bybit_spot(),
            self.fetch_binance_futures(),
            self.fetch_bybit_futures(),
        );

        // Spot
        let sb = match spot_binance_res {
            Ok(m) => m,
            Err(e) => { warn!("Binance spot fetch failed: {e}"); HashMap::new() }
        };
        let sy = match spot_bybit_res {
            Ok(m) => m,
            Err(e) => { warn!("Bybit spot fetch failed: {e}"); HashMap::new() }
        };
        if !sb.is_empty() && !sy.is_empty() {
            let rows = build_rows(&sb, &sy);
            if !rows.is_empty() { *self.spot_state.write() = rows; }
        }

        // Perp
        let pb = match perp_binance_res {
            Ok(m) => m,
            Err(e) => { warn!("Binance futures fetch failed: {e}"); HashMap::new() }
        };
        let py = match perp_bybit_res {
            Ok(m) => m,
            Err(e) => { warn!("Bybit futures fetch failed: {e}"); HashMap::new() }
        };
        if !pb.is_empty() && !py.is_empty() {
            let rows = build_rows(&pb, &py);
            if !rows.is_empty() { *self.perp_state.write() = rows; }
        }

        Ok(())
    }

    async fn fetch_binance_spot(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: Vec<serde_json::Value> = self.http
            .get("https://api.binance.com/api/v3/ticker/bookTicker")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_binance_book_ticker(resp)
    }

    async fn fetch_binance_futures(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: Vec<serde_json::Value> = self.http
            .get("https://fapi.binance.com/fapi/v1/ticker/bookTicker")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_binance_book_ticker(resp)
    }

    async fn fetch_bybit_spot(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: serde_json::Value = self.http
            .get("https://api.bybit.com/v5/market/tickers?category=spot")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_bybit_tickers(resp)
    }

    async fn fetch_bybit_futures(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: serde_json::Value = self.http
            .get("https://api.bybit.com/v5/market/tickers?category=linear")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_bybit_tickers(resp)
    }
}

fn build_rows(
    binance_map: &HashMap<String, (f64, f64)>,
    bybit_map: &HashMap<String, (f64, f64)>,
) -> Vec<MarketRow> {
    TICKERS.iter().filter_map(|&sym| {
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
    }).collect()
}

fn parse_binance_book_ticker(resp: Vec<serde_json::Value>) -> anyhow::Result<HashMap<String, (f64, f64)>> {
    let set: HashSet<&str> = TICKERS.iter().copied().collect();
    let mut map = HashMap::new();
    for item in resp {
        let sym = item["symbol"].as_str().unwrap_or("").to_string();
        if set.contains(sym.as_str()) {
            if let (Some(bid), Some(ask)) = (
                item["bidPrice"].as_str().and_then(|s| s.parse::<f64>().ok()),
                item["askPrice"].as_str().and_then(|s| s.parse::<f64>().ok()),
            ) {
                if bid > 0.0 && ask > 0.0 { map.insert(sym, (bid, ask)); }
            }
        }
    }
    Ok(map)
}

fn parse_bybit_tickers(resp: serde_json::Value) -> anyhow::Result<HashMap<String, (f64, f64)>> {
    let set: HashSet<&str> = TICKERS.iter().copied().collect();
    let mut map = HashMap::new();
    if let Some(list) = resp["result"]["list"].as_array() {
        for item in list {
            let sym = item["symbol"].as_str().unwrap_or("").to_string();
            if set.contains(sym.as_str()) {
                if let (Some(bid), Some(ask)) = (
                    item["bid1Price"].as_str().and_then(|s| s.parse::<f64>().ok()),
                    item["ask1Price"].as_str().and_then(|s| s.parse::<f64>().ok()),
                ) {
                    if bid > 0.0 && ask > 0.0 { map.insert(sym, (bid, ask)); }
                }
            }
        }
    }
    Ok(map)
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo check 2>&1 | grep "^error" | head -10
```

Expected: errors about `snapshot()` no longer existing (it was renamed to `full_snapshot()`). These will be fixed in Task 2. Zero parse errors in market_scanner itself.

- [ ] **Step 3: Commit**

```bash
git add src/market_scanner/mod.rs
git commit -m "feat: add perp fetching to MarketScanner, extract parse helpers"
```

---

### Task 2: Update DashboardState and routes to use MarketSnapshot

**Files:**
- Modify: `src/dashboard/state.rs`
- Modify: `src/dashboard/routes.rs`

- [ ] **Step 1: Update import in state.rs**

In `src/dashboard/state.rs`, update the market_scanner import (MarketRow no longer needed directly — MarketSnapshot wraps it):

```rust
use crate::market_scanner::{MarketScanner, MarketSnapshot};
```

- [ ] **Step 2: Update `market_snapshot` method in DashboardState**

Find the `market_snapshot` method in `DashboardState` and change its return type and body:

```rust
pub fn market_snapshot(&self) -> MarketSnapshot {
    self.scanner.full_snapshot()
}
```

- [ ] **Step 3: Update import in routes.rs**

In `src/dashboard/routes.rs`, replace the existing `MarketRow` import with `MarketSnapshot`:

```rust
use crate::market_scanner::MarketSnapshot;
```

- [ ] **Step 4: Update market_handler return type**

```rust
pub async fn market_handler(
    State(state): State<Arc<DashboardState>>,
) -> Json<MarketSnapshot> {
    Json(state.market_snapshot())
}
```

- [ ] **Step 5: Run tests and build**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo test 2>&1 | tail -5
```

Expected: all 4 tests pass.

```bash
~/.cargo/bin/cargo build --release 2>&1 | tail -3
```

Expected: `Finished release profile`.

- [ ] **Step 6: Commit**

```bash
git add src/dashboard/state.rs src/dashboard/routes.rs
git commit -m "feat: market_snapshot returns MarketSnapshot with spot+perp"
```

---

### Task 3: Add MarketSnapshot TypeScript type

**Files:**
- Modify: `dashboard/src/types.ts`

- [ ] **Step 1: Add MarketSnapshot interface**

In `dashboard/src/types.ts`, add after the `MarketRow` interface:

```typescript
export interface MarketSnapshot {
  spot: MarketRow[]
  perp: MarketRow[]
}
```

- [ ] **Step 2: Commit**

```bash
git add dashboard/src/types.ts
git commit -m "feat: add MarketSnapshot TypeScript type"
```

---

### Task 4: Update MarketScanner component with tab toggle

**Files:**
- Modify: `dashboard/src/components/MarketScanner.tsx`

- [ ] **Step 1: Replace the file with the updated version**

```typescript
import { useEffect, useState } from 'react'
import { MarketRow, MarketSnapshot } from '../types'

type SortKey = keyof Pick<MarketRow,
  'symbol' | 'binance_ask' | 'bybit_bid' | 'spread_ab' | 'bybit_ask' | 'binance_bid' | 'spread_ba'
>

const EMPTY_SNAPSHOT: MarketSnapshot = { spot: [], perp: [] }

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
  const [data, setData] = useState<MarketSnapshot>(EMPTY_SNAPSHOT)
  const [activeTab, setActiveTab] = useState<'spot' | 'perp'>('spot')
  const [sortKey, setSortKey] = useState<SortKey>('spread_ab')
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('desc')
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    function load() {
      fetch('/api/market')
        .then(r => r.json())
        .then((d: MarketSnapshot) => { setData(d); setLoading(false) })
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

  const rows = data[activeTab]
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
    { label: 'Пара',       key: 'symbol' },
    { label: 'Bin Ask',    key: 'binance_ask',  align: 'right' },
    { label: 'Byb Bid',    key: 'bybit_bid',    align: 'right' },
    { label: 'B→Y Спред',  key: 'spread_ab',    align: 'right' },
    { label: 'Byb Ask',    key: 'bybit_ask',    align: 'right' },
    { label: 'Bin Bid',    key: 'binance_bid',  align: 'right' },
    { label: 'Y→B Спред',  key: 'spread_ba',    align: 'right' },
  ]

  function tabBtn(label: string, key: 'spot' | 'perp') {
    const active = activeTab === key
    return (
      <button
        key={key}
        onClick={() => setActiveTab(key)}
        style={{
          background: 'none',
          border: `1px solid ${active ? '#00ff87' : '#2a2a2a'}`,
          borderRadius: 4,
          color: active ? '#00ff87' : '#444',
          cursor: 'pointer',
          padding: '3px 12px',
          fontSize: 11,
          fontFamily: 'inherit',
          fontWeight: active ? 600 : 400,
        }}
      >
        {label}
      </button>
    )
  }

  return (
    <div style={{ background: '#111', border: '1px solid #1f1f1f', borderRadius: 6, padding: 16 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 12 }}>
        <div style={{ color: '#666', fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
          Спред по рынку · Binance vs Bybit
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          {tabBtn('Spot', 'spot')}
          {tabBtn('Perp', 'perp')}
        </div>
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

- [ ] **Step 2: Build dashboard**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -3
```

Expected: `✓ built in Xs` with no TypeScript errors.

- [ ] **Step 3: Commit**

```bash
git add dashboard/src/components/MarketScanner.tsx
git commit -m "feat: add Spot/Perp toggle to MarketScanner table"
```

---

### Task 5: Build release and push

- [ ] **Step 1: Rebuild Rust release binary**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo build --release 2>&1 | tail -3
```

Expected: `Finished release profile`.

- [ ] **Step 2: Restart and smoke test**

```bash
pkill -f "sol-arb" 2>/dev/null; lsof -ti:3001 | xargs kill -9 2>/dev/null; sleep 1
RUST_LOG=sol_arb=info ./target/release/sol-arb > /tmp/sol-arb.log 2>&1 &
sleep 8 && curl -s http://localhost:3001/api/market | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'spot: {len(d[\"spot\"])} pairs, perp: {len(d[\"perp\"])} pairs')
if d['perp']:
    best = max(d['perp'], key=lambda r: r['spread_ab'])
    print(f'Best perp B→Y: {best[\"symbol\"]} {best[\"spread_ab\"]:.4f}%')
"
```

Expected: `spot: N pairs, perp: M pairs` (both > 0).

- [ ] **Step 3: Push**

```bash
git push origin master
```

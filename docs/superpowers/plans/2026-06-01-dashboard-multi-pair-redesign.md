# Dashboard Multi-Pair Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the single-pair `TickerSelector` from the header and replace `PriceTable` with a live **Top-10 Opportunities** widget showing the best gross spreads across all 50 pairs, updated every 500ms via WebSocket.

**Architecture:** Add `OpportunityRow` + `top_opportunities` to the backend `WsSnapshot` (computed from `self.multi_feed` in `build_snapshot()`), extend the TypeScript types, create a new `TopOpportunities` React component, and clean up `App.tsx` to remove all single-pair UI state.

**Tech Stack:** Rust (Axum/Serde for backend), React + TypeScript (no new libraries needed)

---

## File Map

| Action | File |
|--------|------|
| Modify | `src/dashboard/state.rs` — add `OpportunityRow`, update `WsSnapshot` + `build_snapshot()` |
| Modify | `dashboard/src/types.ts` — add `OpportunityRow`, update `WsSnapshot` |
| Create | `dashboard/src/components/TopOpportunities.tsx` |
| Modify | `dashboard/src/App.tsx` — remove TickerSelector + pendingSymbol, swap PriceTable → TopOpportunities |

---

## Task 1: Backend — OpportunityRow + WsSnapshot + build_snapshot

**Files:**
- Modify: `src/dashboard/state.rs`

- [ ] **Step 1: Add `OpportunityRow` struct**

In `src/dashboard/state.rs`, add this struct after the `PriceEntry` struct (around line 65):

```rust
#[derive(Serialize, Clone)]
pub struct OpportunityRow {
    pub symbol:      String,
    pub buy_market:  String,  // e.g. "Binance:Spot"
    pub sell_market: String,  // e.g. "Bybit:Spot"
    pub spread_pct:  f64,     // gross spread %  (sell_bid - buy_ask) / buy_ask * 100
    pub ask:         f64,     // buy-side ask
    pub bid:         f64,     // sell-side bid
}
```

- [ ] **Step 2: Add `top_opportunities` field to `WsSnapshot`**

In `WsSnapshot` (around line 67), add one field at the end:

```rust
#[derive(Serialize, Clone)]
pub struct WsSnapshot {
    pub metrics: crate::metrics::MetricsSnapshot,
    pub prices: Vec<PriceEntry>,
    pub recent_trades: Vec<TradeRecord>,
    pub effective_min_spread_pct: f64,
    pub symbol: String,
    pub top_opportunities: Vec<OpportunityRow>,
}
```

- [ ] **Step 3: Add `use std::time::Duration;` import**

In the import block at the top of `src/dashboard/state.rs`, add to the `std` use:

```rust
use std::{collections::VecDeque, sync::Arc, time::Duration};
```

- [ ] **Step 4: Compute top_opportunities in `build_snapshot()`**

In `build_snapshot()`, before the final `WsSnapshot { ... }` return, add:

```rust
// ── Top-10 cross-market opportunities from live WebSocket feed ───────────────
let stale = Duration::from_millis(500);
let mut opps: Vec<OpportunityRow> = Vec::new();

for entry in self.multi_feed.iter() {
    let sym  = entry.key();
    let tick = entry.value();
    if tick.updated_at.elapsed() > stale { continue; }

    // Collect available fresh quotes for this symbol
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

    // Find best (buy, sell) combo for this symbol
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
opps.truncate(10);
```

- [ ] **Step 5: Add `top_opportunities` to the return value**

Update the `WsSnapshot { ... }` at the end of `build_snapshot()`:

```rust
WsSnapshot {
    metrics: self.metrics.snapshot(),
    prices,
    recent_trades: self.recent_trades(50),
    effective_min_spread_pct,
    symbol: self.config.pair(),
    top_opportunities: opps,
}
```

- [ ] **Step 6: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add src/dashboard/state.rs
git commit -m "feat: add top_opportunities to WsSnapshot from live multi-feed state"
```

---

## Task 2: Frontend types

**Files:**
- Modify: `dashboard/src/types.ts`

- [ ] **Step 1: Add `OpportunityRow` interface and update `WsSnapshot`**

Replace the entire `dashboard/src/types.ts` with:

```typescript
export interface MetricsSnapshot {
  trades: number
  wins: number
  win_rate: number
  total_pnl: number
  total_fees: number
  total_gross_pnl: number
  peak_pnl: number
  max_drawdown: number
  avg_exec_ms: number
  fee_ratio: number   // fees / gross — portion eaten by fees (0..1)
}

export interface PriceEntry {
  exchange: string
  market: string
  bid: number
  ask: number
  microprice: number    // Stoikov 2018: volume-weighted fair price
  spread_pct: number
  imbalance: number     // (bid_qty - ask_qty) / (bid_qty + ask_qty) ∈ [-1, 1]
  sigma_pct: number     // EWMA σ in %, e.g. 0.05 means 0.05%/tick
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
  buy_ask: number
  sell_bid: number
}

export interface OpportunityRow {
  symbol: string
  buy_market: string
  sell_market: string
  spread_pct: number   // gross spread %, e.g. 0.127 means 0.127%
  ask: number
  bid: number
}

export interface WsSnapshot {
  metrics: MetricsSnapshot
  prices: PriceEntry[]
  recent_trades: TradeRecord[]
  effective_min_spread_pct: number  // AS-2008 vol-adjusted threshold
  symbol: string
  top_opportunities: OpportunityRow[]
}

export interface MarketRow {
  symbol: string
  binance_ask: number
  binance_bid: number
  bybit_ask: number
  bybit_bid: number
  spread_ab: number
  spread_ba: number
}

export interface MarketSnapshot {
  spot: MarketRow[]
  perp: MarketRow[]
}
```

- [ ] **Step 2: Verify TypeScript (build check)**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -10
```

Expected: successful build or only pre-existing warnings (no new type errors).

- [ ] **Step 3: Commit**

```bash
git add dashboard/src/types.ts
git commit -m "feat: add OpportunityRow type and top_opportunities to WsSnapshot"
```

---

## Task 3: Create TopOpportunities component

**Files:**
- Create: `dashboard/src/components/TopOpportunities.tsx`

- [ ] **Step 1: Create the file**

```typescript
import { OpportunityRow } from '../types'

interface Props {
  data: OpportunityRow[]
}

function fmt(n: number) {
  const sign = n >= 0 ? '+' : ''
  return `${sign}${n.toFixed(3)}%`
}

function fmtPrice(n: number) {
  if (n >= 1000) return n.toFixed(2)
  if (n >= 1)    return n.toFixed(4)
  return n.toFixed(6)
}

export function TopOpportunities({ data }: Props) {
  return (
    <div style={{
      background: '#0d0d0d', border: '1px solid #1a1a1a',
      borderRadius: 6, padding: '12px 16px',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
        <span style={{ fontSize: 11, color: '#555', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
          Top Opportunities
        </span>
        <span style={{ fontSize: 10, color: '#333' }}>live ●</span>
      </div>

      {data.length === 0 ? (
        <div style={{ color: '#333', fontSize: 12, padding: '20px 0', textAlign: 'center' }}>
          Waiting for data…
        </div>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead>
            <tr style={{ color: '#444' }}>
              <th style={{ textAlign: 'left',  padding: '4px 8px 4px 0', fontWeight: 400 }}>Symbol</th>
              <th style={{ textAlign: 'left',  padding: '4px 8px', fontWeight: 400 }}>Buy</th>
              <th style={{ textAlign: 'left',  padding: '4px 8px', fontWeight: 400 }}>Sell</th>
              <th style={{ textAlign: 'right', padding: '4px 0 4px 8px', fontWeight: 400 }}>Ask</th>
              <th style={{ textAlign: 'right', padding: '4px 0 4px 8px', fontWeight: 400 }}>Bid</th>
              <th style={{ textAlign: 'right', padding: '4px 0 4px 8px', fontWeight: 400 }}>Spread</th>
            </tr>
          </thead>
          <tbody>
            {data.map((row, i) => {
              const positive = row.spread_pct > 0
              const spreadColor = positive ? '#00ff87' : '#555'
              return (
                <tr key={i} style={{ borderTop: '1px solid #111' }}>
                  <td style={{ padding: '5px 8px 5px 0', color: '#e0e0e0', fontWeight: 600 }}>
                    {row.symbol.replace('USDT', '')}<span style={{ color: '#444' }}>/USDT</span>
                  </td>
                  <td style={{ padding: '5px 8px', color: '#888' }}>{row.buy_market}</td>
                  <td style={{ padding: '5px 8px', color: '#888' }}>{row.sell_market}</td>
                  <td style={{ padding: '5px 0 5px 8px', color: '#666', textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>
                    {fmtPrice(row.ask)}
                  </td>
                  <td style={{ padding: '5px 0 5px 8px', color: '#666', textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>
                    {fmtPrice(row.bid)}
                  </td>
                  <td style={{ padding: '5px 0 5px 8px', color: spreadColor, textAlign: 'right', fontWeight: positive ? 600 : 400, fontVariantNumeric: 'tabular-nums' }}>
                    {fmt(row.spread_pct)}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -10
```

Expected: successful build.

- [ ] **Step 3: Commit**

```bash
git add dashboard/src/components/TopOpportunities.tsx
git commit -m "feat: add TopOpportunities component showing live top-10 cross-market spreads"
```

---

## Task 4: Update App.tsx

**Files:**
- Modify: `dashboard/src/App.tsx`

Remove: `TickerSelector` import + usage, `pendingSymbol` state, `changeTicker` function, `restart` function, pending symbol banner, `symbol`/`base` variables. Replace `<PriceTable>` with `<TopOpportunities>`. Update header title to "MULTI ARB".

- [ ] **Step 1: Replace App.tsx**

```typescript
import { useState } from 'react'
import { useWebSocket } from './hooks/useWebSocket'
import { MetricsBar } from './components/MetricsBar'
import { PnlChart } from './components/PnlChart'
import { TopOpportunities } from './components/TopOpportunities'
import { TradesFeed } from './components/TradesFeed'
import { StatusBar } from './components/StatusBar'
import { SettingsPage } from './components/SettingsPage'
import { ChartsRow } from './components/ChartsRow'
import { MarketScanner } from './components/MarketScanner'

const WS_URL = `${window.location.protocol === 'https:' ? 'wss' : 'ws'}://${window.location.host}/ws`

const EMPTY_METRICS = {
  trades: 0, wins: 0, win_rate: 0,
  total_pnl: 0, total_fees: 0, total_gross_pnl: 0,
  peak_pnl: 0, max_drawdown: 0, avg_exec_ms: 0,
  fee_ratio: 0,
}

type Tab = 'dashboard' | 'settings'

function NavTab({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button onClick={onClick} style={{
      background: 'none', border: 'none', cursor: 'pointer', padding: '6px 14px',
      color: active ? '#e0e0e0' : '#444', fontSize: 12, fontFamily: 'inherit',
      borderBottom: active ? '2px solid #00ff87' : '2px solid transparent',
    }}>
      {label}
    </button>
  )
}

export default function App() {
  const [tab, setTab] = useState<Tab>('dashboard')
  const { snapshot, status, lastUpdate } = useWebSocket(WS_URL)

  const metrics = snapshot?.metrics ?? EMPTY_METRICS
  const prices = snapshot?.prices ?? []
  const trades = snapshot?.recent_trades ?? []
  const effectiveMinSpreadPct = snapshot?.effective_min_spread_pct ?? 0
  const opportunities = snapshot?.top_opportunities ?? []

  return (
    <div style={{ maxWidth: 1400, margin: '0 auto', padding: 20, display: 'flex', flexDirection: 'column', gap: 12 }}>

      {/* Header + Nav */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 0 }}>
          <span style={{ fontSize: 14, fontWeight: 600, letterSpacing: '0.1em', color: '#e0e0e0', marginRight: 12 }}>
            MULTI ARB
          </span>
          <NavTab label="Дашборд" active={tab === 'dashboard'} onClick={() => setTab('dashboard')} />
          <NavTab label="Настройки" active={tab === 'settings'} onClick={() => setTab('settings')} />
        </div>
        <span style={{ color: '#333', fontSize: 11 }}>
          {new Date().toLocaleTimeString()}
        </span>
      </div>

      {tab === 'settings' ? (
        <SettingsPage />
      ) : (
        <>
          <MetricsBar metrics={metrics} paperTrading={false} effectiveMinSpreadPct={effectiveMinSpreadPct} />
          <MarketScanner />
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, alignItems: 'start' }}>
            <PnlChart recentTrades={trades} />
            <ChartsRow prices={prices} />
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
            <TopOpportunities data={opportunities} />
            <TradesFeed trades={trades} />
          </div>
          <StatusBar status={status} lastUpdate={lastUpdate} />
        </>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify build**

```bash
cd /Users/rinchin92/claude/project/dashboard && npm run build 2>&1 | tail -15
```

Expected: successful build, no TypeScript errors.

- [ ] **Step 3: Run backend + open browser**

```bash
pkill -f sol-arb 2>/dev/null
RUST_LOG=sol_arb=info ~/.cargo/bin/cargo run &
sleep 8
```

Open `http://localhost:3001` and verify:
- Header shows "MULTI ARB" with no dropdown
- Top-left section shows Top Opportunities table (10 rows)
- Right side still shows TradesFeed
- MarketScanner still works

- [ ] **Step 4: Commit**

```bash
git add dashboard/src/App.tsx
git commit -m "feat: replace TickerSelector+PriceTable with TopOpportunities in dashboard"
```

---

## Verification

**Backend check:**
```bash
curl -s http://localhost:3001/ws  # WebSocket — check via browser devtools Network tab
```

In the browser console, run:
```js
// Connect to WS and inspect first message
const ws = new WebSocket(`ws://${location.host}/ws`)
ws.onmessage = e => { const d = JSON.parse(e.data); console.log('top_opps:', d.top_opportunities?.length, d.top_opportunities?.[0]) }
```

Expected: `top_opps: 10` and first row has `symbol`, `buy_market`, `sell_market`, `spread_pct`, `ask`, `bid` fields.

**Visual check:**
- Top Opportunities table shows ≤10 rows, sorted by spread descending
- Positive spread rows have green `+0.xxx%`, negative rows are grey `-0.xxx%`
- Table updates live (values change every ~500ms)
- No TickerSelector dropdown in header
- "MULTI ARB" title in header

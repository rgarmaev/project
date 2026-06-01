# Dashboard Multi-Pair Redesign

**Date:** 2026-06-01
**Status:** Approved

## Goal

Remove the single-pair `TickerSelector` from the dashboard header and replace the single-pair `PriceTable` with a live **Top-10 Opportunities** widget that shows the best cross-exchange spreads across all 50 pairs in real-time.

## Context

The bot now uses `MultiPairDetector` — it automatically finds and trades the best pair. The `TickerSelector` and single-pair `PriceTable` are no longer meaningful. `DashboardState` already stores `multi_feed: MultiPairState` (the live Arc<DashMap> of 50×4 market quotes), so the data is available with zero additional plumbing.

## Architecture

```
MultiPairState (DashMap, updated ~realtime via WS)
    │
    ▼
build_snapshot() — compute top-10 every 500ms
    │
    ▼
WsSnapshot.top_opportunities: Vec<OpportunityRow>
    │
    ▼ (existing WS broadcast)
TopOpportunities.tsx — table rendered in browser
```

## Backend Changes — `src/dashboard/state.rs`

### New type: `OpportunityRow`

```rust
#[derive(Serialize, Clone)]
pub struct OpportunityRow {
    pub symbol:      String,
    pub buy_market:  String,  // e.g. "Binance:Spot"
    pub sell_market: String,  // e.g. "Bybit:Spot"
    pub spread_pct:  f64,     // gross spread as %, e.g. 0.127
    pub ask:         f64,     // buy-side ask price
    pub bid:         f64,     // sell-side bid price
}
```

### Updated `WsSnapshot`

Add one field at the end:

```rust
pub struct WsSnapshot {
    pub metrics: crate::metrics::MetricsSnapshot,
    pub prices: Vec<PriceEntry>,
    pub recent_trades: Vec<TradeRecord>,
    pub effective_min_spread_pct: f64,
    pub symbol: String,
    pub top_opportunities: Vec<OpportunityRow>,  // NEW
}
```

### Updated `build_snapshot()`

After building the existing fields, compute top opportunities from `self.multi_feed`:

```rust
// Compute top-10 opportunities from live multi-feed state
let stale = std::time::Duration::from_millis(500);
let mut opps: Vec<OpportunityRow> = Vec::new();

for entry in self.multi_feed.iter() {
    let sym = entry.key();
    let tick = entry.value();
    if tick.updated_at.elapsed() > stale { continue; }

    // Get all available quotes as (market_name, bid, ask, bid_qty, ask_qty)
    let quotes: Vec<(&str, f64, f64)> = [
        ("Binance:Spot", tick.spot_binance.as_ref()),
        ("Binance:Perp", tick.perp_binance.as_ref()),
        ("Bybit:Spot",   tick.spot_bybit.as_ref()),
        ("Bybit:Perp",   tick.perp_bybit.as_ref()),
    ]
    .iter()
    .filter_map(|(name, q)| q.map(|q| (*name, q.bid, q.ask)))
    .collect();

    // Find best (buy_market, sell_market) combo for this symbol
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

The `WsSnapshot` return value includes `top_opportunities: opps`.

## Frontend Changes

### `dashboard/src/types.ts`

Add `OpportunityRow` interface and `top_opportunities` field to `WsSnapshot`:

```typescript
export interface OpportunityRow {
  symbol: string
  buy_market: string
  sell_market: string
  spread_pct: number
  ask: number
  bid: number
}

// In WsSnapshot, add:
top_opportunities: OpportunityRow[]
```

### New `dashboard/src/components/TopOpportunities.tsx`

Table with columns: Symbol | Buy | Sell | Spread. Rows sorted by spread descending (already sorted by server). Positive spread rows highlighted green (`#00ff87`), negative/zero rows shown in muted grey (`#555`). Updates automatically via existing WS subscription.

```
┌─────────────────────────────────────────────────────┐
│  Top Opportunities                          live ●  │
├──────────────┬──────────────┬──────────────┬────────┤
│ Symbol       │ Buy          │ Sell         │ Spread │
├──────────────┼──────────────┼──────────────┼────────┤
│ NOTUSDT      │ Binance:Spot │ Bybit:Spot   │+0.127% │  ← green
│ PEPEUSDT     │ Bybit:Perp  │ Binance:Spot  │+0.089% │  ← green
│ WIFUSDT      │ Binance:Perp│ Bybit:Spot    │-0.012% │  ← grey
│ ...          │             │              │        │
└──────────────┴──────────────┴──────────────┴────────┘
```

### `dashboard/src/App.tsx`

- Remove `import { TickerSelector }` and its usage from the header
- Remove the "pending symbol" notification banner
- Replace `<PriceTable ... />` with `<TopOpportunities data={snapshot?.top_opportunities ?? []} />`
- Header simplifies to: logo/title + connection status dot only

`PriceTable.tsx` and `TickerSelector.tsx` remain on disk but are no longer imported.

## Files Changed

| Action | File |
|--------|------|
| Modify | `src/dashboard/state.rs` — add OpportunityRow, update WsSnapshot + build_snapshot |
| Modify | `dashboard/src/types.ts` — add OpportunityRow + field |
| Create | `dashboard/src/components/TopOpportunities.tsx` |
| Modify | `dashboard/src/App.tsx` — remove TickerSelector, swap PriceTable → TopOpportunities |

## Scope

- No new API endpoints
- No new backend tasks
- `TickerSelector.tsx` and `PriceTable.tsx` kept but unused
- Spread shown is gross (before fees) — for opportunity discovery, not execution threshold
- No changes to MarketScanner, TradesFeed, MetricsBar, ChartsRow, SettingsPage

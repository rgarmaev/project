# Dashboard Charts: Buy/Sell Prices + Live Bid/Ask

**Date:** 2026-05-31  
**Status:** Approved

## Goal

Add 4 charts to the dashboard in a 2×2 grid between the PnlChart and the price table:
1. Buy/Sell prices across all trade routes over time
2. Live Binance bid/ask (Spot + Futures)
3. Live Bybit bid/ask (Spot + Futures)
4. Live MEXC bid/ask (Spot only)

## New Files

### `dashboard/src/hooks/usePriceHistory.ts`

Custom hook that accumulates incoming `PriceEntry[]` snapshots into a rolling buffer.

- Input: `prices: PriceEntry[]` (from WS snapshot), `maxPoints = 300`
- Output: `Map<string, { time: string; bid: number; ask: number }[]>`
  - Key format: `"Binance:Spot"`, `"Binance:Perp"`, `"Bybit:Spot"`, `"Bybit:Perp"`, `"MEXC:Spot"`
- Each new snapshot appends a timestamped entry per market; oldest entries are dropped when buffer exceeds `maxPoints`
- Buffer lives in `useRef` to avoid re-renders on every tick; exposed via `useState` updated on each new snapshot

### `dashboard/src/components/ChartsRow.tsx`

Container for all 4 charts. Props:

```ts
interface Props {
  prices: PriceEntry[]
  trades: TradeRecord[]
}
```

Calls `usePriceHistory(prices)` internally. Renders a 2×2 CSS grid, each cell height 180px.

**Chart 1 — Buy / Sell Prices**
- Source: `TradeRecord[]` (same merge logic as PnlChart: `/api/trades` history + live WS trades)
- X-axis: trade timestamp
- Two lines:
  - Buy ask — `#ff4444` (red)
  - Sell bid — `#00ff87` (green)
- Recharts `LineChart`, `dot={false}`, `type="monotone"`

**Chart 2 — Binance Bid/Ask**
- Source: `history.get("Binance:Spot")` and `history.get("Binance:Perp")`
- 4 lines: Spot bid (green solid), Spot ask (red solid), Futures bid (green dashed), Futures ask (red dashed)

**Chart 3 — Bybit Bid/Ask**
- Same structure as Chart 2, using `"Bybit:Spot"` and `"Bybit:Perp"`

**Chart 4 — MEXC Bid/Ask**
- Source: `history.get("MEXC:Spot")`
- 2 lines: bid (green solid), ask (red solid)

## Changes to Existing Files

### `dashboard/src/App.tsx`

Add `<ChartsRow prices={prices} trades={trades} />` between `<PnlChart>` and the 2-column grid row containing `<PriceTable>` and `<TradesFeed>`.

No other files are changed.

## Styling

All charts follow the existing dark theme:
- Background: `#111`, border: `1px solid #1f1f1f`, border-radius: 6px, padding: 16px
- Axis ticks: `#444`, font-size 10px, no axis lines
- Tooltip: background `#1a1a1a`, border `#333`
- Section label: `#666`, uppercase, letter-spacing `0.05em`

## Data Flow

```
WS snapshot → App.tsx
  ├─ prices → ChartsRow → usePriceHistory → Charts 2/3/4
  └─ trades → ChartsRow → Chart 1
```

Chart 1 also fetches `/api/trades?limit=500` on mount (same as PnlChart) to populate historical buy/sell prices.

## Constraints

- No backend changes required — all data already available in `WsSnapshot` and `/api/trades`
- Buffer size 300 points ≈ 5 minutes at 1 WebSocket update/second
- Recharts already installed (`recharts ^2.12.7`)

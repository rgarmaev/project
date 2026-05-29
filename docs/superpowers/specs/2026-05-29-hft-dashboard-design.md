# HFT Arbitrage Dashboard — Design Spec
Date: 2026-05-29

## Overview

Real-time web dashboard for the SOL arbitrage bot. Displays live metrics, prices from all exchanges, PnL chart, and trade feed. Built as a separate React frontend communicating with an embedded axum HTTP/WebSocket server inside the existing Rust bot.

---

## Architecture

### Backend — `src/dashboard/`

New tokio task spawned in `main.rs` alongside existing price feeds and arbitrage engine.

**New shared state `DashboardState` (`Arc<RwLock<>>`):**
- Snapshot of `MetricsCollector` data
- Ring buffer of last 500 `CompletedTrade` records
- Current prices from `PriceState` (all 5 markets)

**Modules:**
- `src/dashboard/mod.rs` — axum router, state initialization
- `src/dashboard/ws.rs` — WebSocket handler, 500ms broadcast loop
- `src/dashboard/routes.rs` — REST endpoints
- `src/dashboard/state.rs` — `DashboardState` struct

**Endpoints:**
- `GET /api/trades?limit=200` — historical trades for initial chart load
- `WS /ws` — real-time snapshot every 500ms

**WebSocket message format (JSON):**
```json
{
  "metrics": {
    "trades": 42,
    "wins": 30,
    "win_rate": 71.4,
    "total_pnl": 12.34,
    "total_fees": 1.20,
    "peak_pnl": 15.00,
    "max_drawdown": 0.50,
    "avg_exec_ms": 87
  },
  "prices": [
    { "exchange": "Binance", "market": "Spot", "bid": 148.50, "ask": 148.52, "spread_pct": 0.013 },
    { "exchange": "Binance", "market": "Perp", "bid": 148.48, "ask": 148.51, "spread_pct": 0.020 },
    { "exchange": "Bybit",   "market": "Spot", "bid": 148.55, "ask": 148.57, "spread_pct": 0.013 },
    { "exchange": "Bybit",   "market": "Perp", "bid": 148.53, "ask": 148.56, "spread_pct": 0.020 },
    { "exchange": "MEXC",    "market": "Spot", "bid": 148.49, "ask": 148.52, "spread_pct": 0.020 }
  ],
  "recent_trades": [
    {
      "id": "uuid",
      "buy_market": "Binance:Spot",
      "sell_market": "Bybit:Perp",
      "spread_pct": 0.12,
      "gross_pnl": 0.45,
      "fees": 0.14,
      "net_pnl": 0.31,
      "exec_ms": 82,
      "time": "2026-05-29T10:23:01Z"
    }
  ]
}
```

### Execution Time Tracking

`CompletedTrade` gets a new field `exec_ms: u64` — milliseconds between `signal.detected_at` and `sell_order.timestamp`.

`MetricsCollector` tracks cumulative execution time to compute rolling average.

### Frontend — `dashboard/`

Vite + React + TypeScript project in `dashboard/` directory at repo root.

**Stack:**
- Vite 5, React 18, TypeScript
- Recharts — PnL line chart
- Native WebSocket API — custom `useWebSocket` hook
- No UI framework — custom components, inline styles / CSS modules

**Dev proxy:** Vite proxies `/api` and `/ws` to `localhost:3001` (axum server port).

**Build:** `vite build` outputs to `dashboard/dist/` — axum serves static files from there in production via `tower-http ServeDir`.

---

## UI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  SOL ARB  [● LIVE]     paper_trading: true          23:14:05    │
├──────────┬──────────┬──────────┬──────────┬──────────┬──────────┤
│ Total PnL│ Win Rate │Max Drawn │  Trades  │   Fees   │ Avg Exec │
│ +12.34 U │  71.4%   │  0.50 U  │    42    │  1.20 U  │  87 ms   │
├──────────┴──────────┴──────────┴──────────┴──────────┴──────────┤
│                    PnL over time (Recharts line chart)           │
│                                                                  │
├──────────────────────────┬──────────────────────────────────────┤
│  Live Prices             │  Recent Trades                        │
│                          │                                       │
│  Binance Spot  148.50/52 │  time  buy→sell  spread  pnl  exec   │
│  Binance Perp  148.48/51 │  ...                                  │
│  Bybit Spot    148.55/57 │  ...                                  │
│  Bybit Perp    148.53/56 │  ...                                  │
│  MEXC Spot     148.49/52 │  ...                                  │
├──────────────────────────┴──────────────────────────────────────┤
│  WS: connected  |  last update: 500ms ago                        │
└─────────────────────────────────────────────────────────────────┘
```

**Visual style — dark crypto aesthetic:**
- Background: `#0a0a0a`
- Card background: `#111111`
- Borders: `#1f1f1f`
- Green (profit): `#00ff87`
- Red (loss): `#ff4444`
- Text: `#e0e0e0`
- Muted: `#666666`
- Font: JetBrains Mono (monospace)
- Price flash on change: yellow highlight 200ms fade

---

## Components

- `App.tsx` — WebSocket connection, top-level state
- `MetricsBar.tsx` — 6 stat cards
- `PnlChart.tsx` — Recharts LineChart, loads history via REST on mount
- `PriceTable.tsx` — 5 rows, flashes on price change
- `TradesFeed.tsx` — scrollable table, last 50 trades
- `StatusBar.tsx` — connection status + last update time
- `useWebSocket.ts` — custom hook, auto-reconnect on disconnect

---

## Rust Dependencies to Add

```toml
axum = { version = "0.7", features = ["ws"] }
tower-http = { version = "0.5", features = ["cors", "fs"] }
tokio-tungstenite = "0.21"
serde_json = "1"
```

---

## Error Handling

- WebSocket disconnect: frontend auto-reconnects every 2 seconds, shows "reconnecting..." in status bar
- No trades yet: PnL chart shows empty state, metrics show zeros
- Exchange feed down: price row shows last known value with "stale" indicator if >5s old

---

## Out of Scope

- Authentication / access control
- Persistent storage of trade history (in-memory only)
- Multiple trading pairs
- Mobile layout

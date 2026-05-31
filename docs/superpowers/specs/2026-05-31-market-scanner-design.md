# Market Scanner — Cross-Exchange Spread Table

**Date:** 2026-05-31
**Status:** Approved

## Goal

Add a sortable table to the main dashboard showing the cross-exchange bid-ask spread for all 50 USDT pairs (from the existing TICKERS list) between Binance and Bybit. Data comes from a backend scanner that polls both exchanges every 2 seconds.

## Architecture

```
MarketScanner (Rust)
  └── polls every 2s:
        Binance GET /api/v3/ticker/bookTicker   → all symbols, one request
        Bybit   GET /v5/market/tickers?category=spot → all symbols, one request
  └── filters 50 TICKERS, computes spreads
  └── stores Arc<RwLock<Vec<MarketRow>>>
  └── exposed via GET /api/market

MarketScanner.tsx (React)
  └── polls /api/market every 3s
  └── renders sortable table
  └── mounted in App.tsx below the price/trades grid
```

## Backend

### New files

**`src/market_scanner/mod.rs`**

```
pub struct MarketRow {
    pub symbol: String,          // e.g. "BTCUSDT"
    pub binance_ask: f64,        // best ask on Binance spot
    pub binance_bid: f64,        // best bid on Binance spot
    pub bybit_ask: f64,          // best ask on Bybit spot
    pub bybit_bid: f64,          // best bid on Bybit spot
    pub spread_ab: f64,          // Buy Binance→Sell Bybit: (bybit_bid - binance_ask) / binance_ask * 100
    pub spread_ba: f64,          // Buy Bybit→Sell Binance: (binance_bid - bybit_ask) / bybit_ask * 100
}

pub struct MarketScanner {
    state: Arc<RwLock<Vec<MarketRow>>>,
    http: reqwest::Client,
}

impl MarketScanner {
    pub fn new() -> Arc<Self>
    pub async fn run(self: Arc<Self>)   // poll loop every 2s
    pub fn snapshot(&self) -> Vec<MarketRow>
}
```

Poll loop fetches Binance and Bybit in parallel (`tokio::join!`), parses JSON, filters to the 50 known TICKERS, computes spreads, writes to state.

**TICKERS list** — same 50 symbols as in `dashboard/src/components/TickerSelector.tsx`:
`BTCUSDT, ETHUSDT, BNBUSDT, SOLUSDT, XRPUSDT, ADAUSDT, DOGEUSDT, AVAXUSDT, DOTUSDT, MATICUSDT, LINKUSDT, LTCUSDT, UNIUSDT, ATOMUSDT, BCHUSDT, ICPUSDT, APTUSDT, ARBUSDT, OPUSDT, FILUSDT, NEARUSDT, SANDUSDT, MANAUSDT, AXSUSDT, ALGOUSDT, VETUSDT, FTMUSDT, HBARUSDT, ETCUSDT, XLMUSDT, TRXUSDT, SUIUSDT, SEIUSDT, INJUSDT, TIAUSDT, JUPUSDT, WIFUSDT, BONKUSDT, PEPEUSDT, SHIBUSDT, NOTUSDT, TONUSDT, STXUSDT, RUNEUSDT, RENDERUSDT, WLDUSDT, ENAUSDT, ZKUSDT, THETAUSDT, FLOKIUSDT`

### Existing files modified

**`src/main.rs`** — create `MarketScanner`, spawn `scanner.run()` in the `JoinSet`, pass scanner ref to `DashboardState::new`.

**`src/dashboard/state.rs`** — add `scanner: Arc<MarketScanner>` field to `DashboardState`; add `market_snapshot(&self) -> Vec<MarketRow>` method.

**`src/dashboard/routes.rs`** — add `market_handler` that calls `state.market_snapshot()` and returns JSON.

**`src/dashboard/mod.rs`** — register route `GET /api/market → market_handler`.

## API

```
GET /api/market
Response: [
  {
    "symbol": "BTCUSDT",
    "binance_ask": 67500.12,
    "binance_bid": 67499.88,
    "bybit_ask": 67501.00,
    "bybit_bid": 67500.50,
    "spread_ab": 0.0006,   // (67500.50 - 67500.12) / 67500.12 * 100
    "spread_ba": -0.0002
  },
  ...
]
```

If a symbol is missing from either exchange response (e.g. NOTUSDT not on Bybit), it is omitted from the result.

## Frontend

### New file: `dashboard/src/components/MarketScanner.tsx`

- Fetches `/api/market` on mount and every 3000ms via `setInterval`
- Local state: `rows: MarketRow[]`, `sortKey: keyof MarketRow`, `sortDir: 'asc'|'desc'`
- Default sort: `spread_ab` descending (best arbitrage opportunity first)
- Clicking a column header toggles sort direction, switches sort key
- Table columns:

| Column | Key | Format |
|---|---|---|
| Пара | symbol | `BTC/USDT` |
| Binance Ask | binance_ask | 6 sig digits |
| Bybit Bid | bybit_bid | 6 sig digits |
| B→Y Спред | spread_ab | `+0.0123%` color green if >0, grey if ≤0 |
| Bybit Ask | bybit_ask | 6 sig digits |
| Binance Bid | binance_bid | 6 sig digits |
| Y→B Спред | spread_ba | `+0.0045%` color green if >0, grey if ≤0 |

- Header: sortable, shows ▲ or ▼ on active column
- Max height 400px with overflow-y scroll
- Dark theme: matches existing panels (background `#111`, border `#1f1f1f`)

### TypeScript type

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

Add to `dashboard/src/types.ts`.

### Modified: `dashboard/src/App.tsx`

Import and mount `MarketScanner` **above** the PnlChart/ChartsRow grid:

```tsx
<MetricsBar ... />
{pendingSymbol && <banner />}

<MarketScanner />   ← new, full width, ABOVE charts

<div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, alignItems: 'start' }}>
  <PnlChart recentTrades={trades} />
  <ChartsRow prices={prices} />
</div>

<div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
  <PriceTable prices={prices} />
  <TradesFeed trades={trades} />
</div>
```

## Error handling

- If either REST call fails, the scanner logs a warning and keeps the previous snapshot (stale data is acceptable for a dashboard)
- If a symbol appears on Binance but not Bybit (or vice versa), it is silently excluded from results
- Frontend shows "Загрузка..." until first response arrives

## Scope

No changes to the existing arbitrage bot logic, WebSocket feeds, or config. The scanner runs independently alongside the bot.

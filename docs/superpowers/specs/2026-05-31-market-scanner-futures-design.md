# Market Scanner — Add Futures Pairs

**Date:** 2026-05-31
**Status:** Approved

## Goal

Extend the existing MarketScanner to include USDT-M futures pairs (Binance Perp + Bybit Linear), with a Spot/Perp toggle above the table in the frontend.

## Backend changes

### `src/market_scanner/mod.rs`

Add two new fetch methods:
- `fetch_binance_futures()` → `GET https://fapi.binance.com/fapi/v1/ticker/bookTicker` — parses `bidPrice`/`askPrice`
- `fetch_bybit_futures()` → `GET https://api.bybit.com/v5/market/tickers?category=linear` — parses `bid1Price`/`ask1Price`

Store separate `perp_rows: RwLock<Vec<MarketRow>>` alongside the existing `spot_rows`.

Add `perp_snapshot(&self) -> Vec<MarketRow>` method.

`poll_once` fetches all 4 endpoints in parallel via `tokio::join!`, populates both `spot_rows` and `perp_rows`.

### Response shape

Change `/api/market` from `Vec<MarketRow>` to:
```json
{ "spot": [...MarketRow], "perp": [...MarketRow] }
```

Add `MarketSnapshot` wrapper struct (Rust):
```rust
#[derive(Serialize)]
pub struct MarketSnapshot {
    pub spot: Vec<MarketRow>,
    pub perp: Vec<MarketRow>,
}
```

`market_snapshot()` in `DashboardState` returns `MarketSnapshot`.
`market_handler` returns `Json<MarketSnapshot>`.

## Frontend changes

### `dashboard/src/types.ts`

Replace `MarketRow[]` return type assumption for `/api/market` with a new wrapper:
```typescript
export interface MarketSnapshot {
  spot: MarketRow[]
  perp: MarketRow[]
}
```

### `dashboard/src/components/MarketScanner.tsx`

- State: `activeTab: 'spot' | 'perp'` (default: `'spot'`)
- Fetch `/api/market` returning `MarketSnapshot`, store whole object
- Display `data[activeTab]` in the table
- Add toggle above table:
  ```
  [ Spot ]  [ Perp ]
  ```
  Active tab: border `#00ff87`, color `#00ff87`. Inactive: border `#2a2a2a`, color `#444`.

No changes to table columns, sort logic, or formatting.

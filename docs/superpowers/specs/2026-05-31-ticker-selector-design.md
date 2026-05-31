# Ticker Selector Implementation Plan

**Date:** 2026-05-31
**Status:** Approved

## Goal

Add a ticker dropdown to the dashboard header so the user can switch the trading pair (e.g. SOLUSDT → BTCUSDT) without editing config.toml manually. Selecting a ticker saves the new symbol to config.toml and shows a "Restart required" banner with a restart button.

## Interaction Flow

1. Dashboard header shows current symbol (e.g. `SOLUSDT`)
2. User clicks it → dropdown with search opens, shows 50 top USDT pairs
3. User selects a different ticker
4. Frontend POSTs `/api/config` with updated `symbol`
5. Banner appears: `Тикер изменён на BTCUSDT — ↺ Перезапустить`
6. User clicks restart → bot restarts, page reloads, new symbol active

## Static Ticker List (50 pairs)

```
BTCUSDT ETHUSDT BNBUSDT SOLUSDT XRPUSDT ADAUSDT DOGEUSDT AVAXUSDT
DOTUSDT MATICUSDT LINKUSDT LTCUSDT UNIUSDT ATOMUSDT BCHUSDT ICPUSDT
APTUSDT ARBUSDT OPUSDT FILUSDT NEARUSDT SANDUSDT MANAUSDT AXSUSDT
ALGOUSDT VETUSDT FTMUSDT HBARUSDT ETCUSDT XLMUSDT TRXUSDT SUIUSDT
SEIUSDT INJUSDT TIAUSDT JUPUSDT WIFUSDT BONKUSDT PEPEUSDT SHIBUSDT
NOTUSDT TONUSDT STXUSDT RUNEUSDT RENDERUSDT WLDUSDT ENAUSDT ZKUSDT
THETAUSDT FLOKIUSDT
```

## Files

| Action | Path | Change |
|---|---|---|
| Create | `dashboard/src/components/TickerSelector.tsx` | Dropdown with search, static list |
| Modify | `dashboard/src/App.tsx` | Add TickerSelector to header, restart banner |
| Modify | `dashboard/src/types.ts` | Add `symbol: string` to `WsSnapshot` |
| Modify | `src/dashboard/state.rs` | Add `symbol` to `WsSnapshot`, read from config |
| Modify | `src/dashboard/config_api.rs` | Add `symbol` to `ConfigPayload`, write to config.toml |

## Component: TickerSelector

Props: `current: string`, `onChange: (symbol: string) => void`

- Input field for search (filters list as user types)
- Dropdown list of 50 pairs, filtered by search string
- Closes on outside click or Escape
- Shows base asset only in header: `SOL/USDT` not `SOLUSDT`
- Styled consistent with dark theme (background `#111`, border `#1f1f1f`)

## Backend: symbol in ConfigPayload

Add `symbol: String` to `ConfigPayload` (Rust) and `ConfigPayload` (TypeScript).

`get_config` reads `symbol` + `quote` from `config.toml`, returns `symbol` as `"SOLUSDT"`.

`post_config` writes `symbol` back to config.toml by splitting at `USDT`: `symbol = "BTC"`, `quote = "USDT"`. All pairs in the list end with `USDT` so the split is safe.

## Backend: symbol in WsSnapshot

Add `symbol: String` to `WsSnapshot` in `src/dashboard/state.rs`. Populated from `self.config.pair()`. This lets the header always know the current active symbol even after restart.

## App.tsx changes

- Import `TickerSelector`
- Read `symbol` from `snapshot?.symbol ?? 'SOLUSDT'`
- Pass to `TickerSelector` as `current`
- On `onChange`: POST `/api/config` with updated symbol, set `pendingRestart = true`
- When `pendingRestart`: show banner below MetricsBar with restart button (reuses existing `restart()` function)

## Styling

Banner: yellow warning style — `background: #1a1a00`, `border: #444400`, `color: #aaaa00`, same as PAPER badge pattern.

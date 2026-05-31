# Multi-Pair WebSocket Feed — Phase 1

**Date:** 2026-05-31
**Status:** Approved

## Goal

Build a new `src/multi_feed/` module that maintains real-time bid/ask prices for all 50 USDT pairs across four markets (Binance Spot, Binance Perp, Bybit Spot, Bybit Linear) via four parallel WebSocket connections. This shared price state will be consumed by the multi-pair detector in Phase 2.

## Architecture

```
Binance Spot  wss://stream.binance.com:9443/ws/!bookTicker   ─┐
Binance Perp  wss://fstream.binance.com/ws/!bookTicker        ├──→ Arc<DashMap<String, MultiPairTick>>
Bybit Spot    wss://stream.bybit.com/v5/public/spot            ├──→ (keyed by symbol e.g. "BTCUSDT")
Bybit Linear  wss://stream.bybit.com/v5/public/linear         ─┘
```

Each connection runs in its own tokio task. Updates are written directly into the shared DashMap — no channels needed.

## Data Types

```rust
pub struct MultiPairTick {
    pub spot_binance: Option<(f64, f64)>,  // (bid, ask)
    pub perp_binance: Option<(f64, f64)>,
    pub spot_bybit:   Option<(f64, f64)>,
    pub perp_bybit:   Option<(f64, f64)>,
    pub updated_at:   Instant,
}

pub type MultiPairState = Arc<DashMap<String, MultiPairTick>>;
```

Only tickers from the 50-element TICKERS constant are stored; all others are discarded.

## New File: `src/multi_feed/mod.rs`

### Public API

```rust
pub fn new_state() -> MultiPairState

pub async fn run_binance_spot(state: MultiPairState)
pub async fn run_binance_perp(state: MultiPairState)
pub async fn run_bybit_spot(state: MultiPairState)
pub async fn run_bybit_linear(state: MultiPairState)
```

### Binance feeds (spot + perp)

URL: `wss://stream.binance.com:9443/ws/!bookTicker` (spot) and `wss://fstream.binance.com/ws/!bookTicker` (perp).

No subscription message needed — stream starts pushing immediately on connect.

Each message format:
```json
{"u":123,"s":"BTCUSDT","b":"67500.00","B":"0.5","a":"67501.00","A":"0.3"}
```
Fields: `s` = symbol, `b` = bid price, `a` = ask price.

On each message: if symbol in TICKERS, update `spot_binance` (or `perp_binance`) field of the DashMap entry.

### Bybit feeds (spot + linear)

URL: `wss://stream.bybit.com/v5/public/spot` (spot) and `wss://stream.bybit.com/v5/public/linear` (perp).

Send one subscription message after connect:
```json
{"op":"subscribe","args":["tickers.BTCUSDT","tickers.ETHUSDT",...all 50...]}
```

Each message format:
```json
{"topic":"tickers.BTCUSDT","data":{"bid1Price":"67500","ask1Price":"67501",...}}
```
Parse `bid1Price` and `ask1Price` from `data`.

### Reconnect on disconnect

Each run_* function loops forever: connect → stream → on error/close, log warn and reconnect after 1s sleep.

### DashMap update pattern

```rust
state.entry(symbol.to_string())
    .and_modify(|t| { t.spot_binance = Some((bid, ask)); t.updated_at = Instant::now(); })
    .or_insert_with(|| MultiPairTick {
        spot_binance: Some((bid, ask)),
        perp_binance: None,
        spot_bybit:   None,
        perp_bybit:   None,
        updated_at:   Instant::now(),
    });
```

## main.rs Changes

Add `mod multi_feed;` and spawn 4 tasks in the JoinSet:

```rust
let multi_state = multi_feed::new_state();

set.spawn(multi_feed::run_binance_spot(multi_state.clone()));
set.spawn(multi_feed::run_binance_perp(multi_state.clone()));
set.spawn(multi_feed::run_bybit_spot(multi_state.clone()));
set.spawn(multi_feed::run_bybit_linear(multi_state.clone()));
```

`multi_state` is also passed to `DashboardState` for future use by Phase 2 detector.

## Stale data handling

`updated_at: Instant` allows Phase 2 to skip ticks older than 500ms. No stale-data logic in this module.

## Scope

This phase builds only the feeds and shared price state. No trading logic. No UI changes. The existing single-pair bot continues to run unchanged alongside the new feeds.

## TICKERS constant

The `TICKERS` const slice is currently private in `src/market_scanner/mod.rs`. Move it to a new `src/tickers.rs` file and make it `pub`. Both `market_scanner` and `multi_feed` import it from there:

```rust
// src/tickers.rs
pub const TICKERS: &[&str] = &[ /* same 50 symbols */ ];
```

Add `mod tickers;` to `main.rs` and `use crate::tickers::TICKERS;` in both modules.

## Dependencies

`dashmap` is already in Cargo.toml. `tokio-tungstenite` is already in Cargo.toml.

# Multi-Pair Arbitrage Detector — Phase 2

**Date:** 2026-06-01
**Status:** Approved

## Goal

Replace the existing single-pair `ArbitrageDetector` with a `MultiPairDetector` that scans all 50 USDT pairs across four markets (Binance Spot, Binance Perp, Bybit Spot, Bybit Linear) simultaneously and executes trades on whichever symbol has the best opportunity at any given moment.

## Context

Phase 1 built `src/multi_feed/` — four parallel WebSocket connections writing real-time bid/ask into `Arc<DashMap<String, MultiPairTick>>`. Phase 2 consumes that state to detect and execute cross-pair arbitrage.

## Architecture

```
MultiPairState (Arc<DashMap<String, MultiPairTick>>)
    │
    ▼
MultiPairDetector  ──→  mpsc::Sender<ArbitrageSignal>  ──→  OrderExecutor
    │                   (signal includes symbol)               │
    │                                                          ▼
    └── EWMA vol per (symbol, market)              place_order(symbol, ...)
```

The existing single-pair `ArbitrageDetector` is removed from `main.rs`. The five single-pair price feeds (Binance/Bybit/MEXC) continue running — they power the dashboard PriceTable and executor paper_fill.

## Data Layer Changes — `src/multi_feed/mod.rs`

Add `MarketQuote` struct carrying full order book top-of-book data:

```rust
#[derive(Debug, Clone)]
pub struct MarketQuote {
    pub bid:     f64,
    pub ask:     f64,
    pub bid_qty: f64,
    pub ask_qty: f64,
}
```

Replace `Option<(f64, f64)>` fields in `MultiPairTick` with `Option<MarketQuote>`:

```rust
#[derive(Debug, Clone)]
pub struct MultiPairTick {
    pub spot_binance: Option<MarketQuote>,
    pub perp_binance: Option<MarketQuote>,
    pub spot_bybit:   Option<MarketQuote>,
    pub perp_bybit:   Option<MarketQuote>,
    pub updated_at:   Instant,
}
```

**Binance parser update:** `B` field → `bid_qty`, `A` field → `ask_qty` (already present in `!bookTicker` stream, currently discarded).

**Bybit parser update:** `bid1Size` → `bid_qty`, `ask1Size` → `ask_qty` (already present in `tickers` stream, currently discarded).

DashMap update pattern changes from `Some((bid, ask))` to `Some(MarketQuote { bid, ask, bid_qty, ask_qty })` throughout.

## Signal Changes — `src/types.rs`

Add `symbol` field to `ArbitrageSignal`:

```rust
pub struct ArbitrageSignal {
    pub id:                 Uuid,
    pub symbol:             String,   // e.g. "BTCUSDT" — NEW
    pub buy_market:         MarketId,
    pub sell_market:        MarketId,
    pub buy_ask:            Decimal,
    pub sell_bid:           Decimal,
    pub spread_pct:         Decimal,
    pub quantity:           Decimal,
    pub expected_pnl_usdt:  Decimal,
    pub detected_at:        DateTime<Utc>,
}
```

The existing `ArbitrageDetector` populates `symbol: config.pair()` to maintain compatibility until it is removed.

## MultiPairDetector — `src/arbitrage/multi_detector.rs`

```rust
pub struct MultiPairDetector {
    config:    Arc<Config>,
    state:     MultiPairState,
    signal_tx: mpsc::Sender<ArbitrageSignal>,
    vol_map:   DashMap<(String, u8), EwmaVolatility>,  // (symbol, market_idx) → σ
    last_mids: DashMap<(String, u8), f64>,
}
```

`market_idx`: 0 = spot_binance, 1 = perp_binance, 2 = spot_bybit, 3 = perp_bybit.

### Detection Loop

Runs every 1ms (same cadence as old detector):

```
for each (symbol, tick) in MultiPairState:
    skip if tick.updated_at > 500ms ago
    update EWMA vol for each available market
    for each ordered pair (buy_market, sell_market) of available markets:
        evaluate(symbol, buy_quote, sell_quote, variances)
        if signal → try_send to channel
```

12 directed combinations per symbol (4 markets × 3 others). 50 symbols = 600 evaluations per tick — well within 1ms budget.

### Filters (identical to existing detector)

1. **Stale check** — skip if `updated_at > 500ms`
2. **Microprice** — `microprice(bid, bid_qty, ask, ask_qty)` for fair-value comparison; skip if `mp_spread_pct < -0.5%`
3. **Imbalance** — `(bid_qty - ask_qty) / (bid_qty + ask_qty)`; skip if adverse beyond `imbalance_threshold`
4. **Fee-adjusted spread** — `spread_pct = (net_recv - net_cost) / net_cost`
5. **Vol-adjusted minimum** — `spread_pct ≥ min_spread_pct + γ·σ²·τ`

Fee rates: copy the `fee_rate()` function directly into `multi_detector.rs`. Do not move it from `detector.rs` — that file stays untouched except for the `symbol` field addition.

### EWMA Volatility

One `EwmaVolatility` tracker per `(symbol, market_idx)` pair — 50 × 4 = 200 trackers maximum. Updated on mid price change, same logic as existing detector. `DashMap` avoids locking the entire map on each update.

## Exchange Connector Changes

`place_order` on both `BinanceConnector` and `BybitConnector` gains a `symbol: &str` parameter:

```rust
pub async fn place_order(
    &self,
    symbol: &str,       // NEW — replaces self.config.pair()
    market: MarketType,
    side: Side,
    quantity: Decimal,
    price_state: &SharedPriceState,
) -> Result<OrderResult>
```

All internal uses of `self.config.pair()` inside `place_order` are replaced with `symbol`.

`MexcConnector::place_order` is unchanged — MEXC is not part of multi-feed and not called by `MultiPairDetector`.

## Executor Changes — `src/arbitrage/executor.rs`

All calls to `place_order` pass `&signal.symbol`:

```rust
// Было:
binance.place_order(market, side, qty, &price_state).await
// Стало:
binance.place_order(&signal.symbol, market, side, qty, &price_state).await
```

No other executor logic changes.

## `main.rs` Changes

Replace detector construction:

```rust
// Remove:
let detector = Arc::new(ArbitrageDetector::new(config.clone(), price_state.clone(), signal_tx));

// Add:
let detector = Arc::new(MultiPairDetector::new(config.clone(), multi_state.clone(), signal_tx));
```

The `use arbitrage::detector::ArbitrageDetector` import is removed; `use arbitrage::multi_detector::MultiPairDetector` is added.

## Files Changed

| Action | File |
|--------|------|
| Modify | `src/multi_feed/mod.rs` — add `MarketQuote`, update parsers |
| Modify | `src/types.rs` — add `symbol` to `ArbitrageSignal` |
| Create | `src/arbitrage/multi_detector.rs` |
| Modify | `src/arbitrage/mod.rs` — add `pub mod multi_detector` |
| Modify | `src/exchanges/binance.rs` — add `symbol` param to `place_order` |
| Modify | `src/exchanges/bybit.rs` — add `symbol` param to `place_order` |
| Modify | `src/arbitrage/executor.rs` — pass `signal.symbol` to `place_order` |
| Modify | `src/main.rs` — swap detector |

## Scope

- No UI changes
- No new config fields
- MEXC excluded from multi-pair detection (not in multi_feed)
- `ArbitrageDetector` file remains on disk but is no longer instantiated (can be deleted later)
- Stale data policy (500ms) unchanged from Phase 1 spec

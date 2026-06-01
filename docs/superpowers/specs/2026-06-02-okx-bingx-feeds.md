# OKX + BingX Multi-Feed — Phase 2

**Date:** 2026-06-02
**Status:** Approved

## Goal

Add OKX (spot + swap) and BingX (spot + swap) to the live multi-pair feed, expanding coverage from 4 markets to 8 markets across all 199 USDT pairs. Split `src/multi_feed/mod.rs` into per-exchange files for maintainability.

## Architecture

```
Binance Spot   wss://stream.binance.com:9443/ws/!bookTicker       ─┐
Binance Perp   wss://fstream.binance.com/ws/!bookTicker            │
Bybit Spot     wss://stream.bybit.com/v5/public/spot               │
Bybit Linear   wss://stream.bybit.com/v5/public/linear             ├──→ Arc<DashMap<String, MultiPairTick>>
OKX Spot       wss://ws.okx.com:8443/ws/v5/public                  │
OKX Swap       wss://ws.okx.com:8443/ws/v5/public                  │
BingX Spot     wss://open-api-ws.bingx.com/market                  │
BingX Swap     wss://open-api-swap.bingx.com/swap-market           ─┘
```

## File Structure

```
src/multi_feed/
├── mod.rs      — MarketQuote, MultiPairTick, MultiPairState, new_state(), pub re-exports
├── binance.rs  — run_binance_spot, run_binance_perp (moved from old mod.rs)
├── bybit.rs    — run_bybit_spot, run_bybit_linear (moved from old mod.rs)
├── okx.rs      — run_okx_spot, run_okx_swap (new)
└── bingx.rs    — run_bingx_spot, run_bingx_swap (new)
```

## Data Types — `src/multi_feed/mod.rs`

### Expanded `MultiPairTick`

```rust
#[derive(Debug, Clone)]
pub struct MultiPairTick {
    pub spot_binance: Option<MarketQuote>,
    pub perp_binance: Option<MarketQuote>,
    pub spot_bybit:   Option<MarketQuote>,
    pub perp_bybit:   Option<MarketQuote>,
    pub spot_okx:     Option<MarketQuote>,
    pub perp_okx:     Option<MarketQuote>,
    pub spot_bingx:   Option<MarketQuote>,
    pub perp_bingx:   Option<MarketQuote>,
    pub updated_at:   Instant,
}
```

All existing fields keep their names. New fields initialize to `None` in `or_insert_with` closures in all existing feed files.

### Symbol conversion helper (in `mod.rs`)

```rust
/// "BTCUSDT" → "BTC-USDT"  (for OKX and BingX)
pub fn to_dashed(ticker: &str) -> String {
    format!("{}-{}", &ticker[..ticker.len()-4], &ticker[ticker.len()-4..])
}
```

This assumes all TICKERS end with "USDT" (4 chars). Panics if input is too short — acceptable since TICKERS is hardcoded.

## OKX Feed — `src/multi_feed/okx.rs`

### Connections

Two independent connections, one per market type:
- Spot: `wss://ws.okx.com:8443/ws/v5/public`
- Swap: `wss://ws.okx.com:8443/ws/v5/public` (same endpoint, different `instId` suffix)

Both connections can hold up to 240 subscriptions. With 199 pairs per connection, both are within limits.

### Subscription message

Spot (sent once after connect):
```json
{"op":"subscribe","args":[
  {"channel":"tickers","instId":"BTC-USDT"},
  {"channel":"tickers","instId":"ETH-USDT"},
  ... (199 total)
]}
```

Swap uses `instId: "BTC-USDT-SWAP"` instead.

### Message format

```json
{
  "arg":  {"channel":"tickers","instId":"BTC-USDT"},
  "data": [{"instId":"BTC-USDT","bidPx":"67500","askPx":"67501","bidSz":"1.5","askSz":"0.8"}]
}
```

Parse: build a `HashSet<String>` of OKX instIds from TICKERS at connection start. On each message, extract `arg.instId`, look it up in a reverse map `HashMap<String /*instId*/, String /*TICKERS symbol*/>` built once before the loop. Fields: `bidPx`, `askPx`, `bidSz`, `askSz`.

The reverse map is built like this:
```rust
let inst_map: HashMap<String, String> = TICKERS.iter()
    .map(|&t| (to_dashed(t) + if is_swap { "-SWAP" } else { "" }, t.to_string()))
    .collect();
```

### Keepalive

Every 25s, send the literal string `"ping"` (not JSON). Server replies with `"pong"`. Use `tokio::time::interval(Duration::from_secs(25))`.

### Reconnect

Same pattern as Binance/Bybit: on error or close → `sleep(1s)` → reconnect.

### Public API

```rust
pub async fn run_okx_spot(state: MultiPairState)
pub async fn run_okx_swap(state: MultiPairState)
```

## BingX Feed — `src/multi_feed/bingx.rs`

### Connections

- Spot: `wss://open-api-ws.bingx.com/market`
- Swap: `wss://open-api-swap.bingx.com/swap-market`

### Subscription

BingX requires one subscription message per symbol (not batched). Send 199 messages after connecting:

```json
{"id":"1","dataType":"BTC-USDT@bookTicker"}
{"id":"2","dataType":"ETH-USDT@bookTicker"}
...
```

Symbol format: same `BTC-USDT` dash convention as OKX.

### Message format

```json
{
  "dataType": "BTC-USDT@bookTicker",
  "data": {
    "symbol":   "BTC-USDT",
    "bidPrice": "67500",
    "bidQty":   "1.5",
    "askPrice": "67501",
    "askQty":   "0.8"
  }
}
```

Parse `dataType` to extract symbol: strip `@bookTicker` suffix, convert `BTC-USDT` → `BTCUSDT` for DashMap lookup. Fields: `bidPrice`, `askPrice`, `bidQty`, `askQty`.

### Keepalive

No application-level ping needed — handle WebSocket-protocol `Message::Ping(d)` → reply `Message::Pong(d)`. This is already the pattern in Bybit.

### Public API

```rust
pub async fn run_bingx_spot(state: MultiPairState)
pub async fn run_bingx_swap(state: MultiPairState)
```

## DashMap Update Pattern

All new feeds follow the same pattern as existing ones. In `or_insert_with`, all 8 fields are initialized:

```rust
.or_insert_with(|| MultiPairTick {
    spot_binance: None,
    perp_binance: None,
    spot_bybit:   None,
    perp_bybit:   None,
    spot_okx:     None,
    perp_okx:     None,
    spot_bingx:   None,
    perp_bingx:   None,
    updated_at:   Instant::now(),
})
```

## MultiPairDetector — `src/arbitrage/multi_detector.rs`

### Expanded `Field` enum

```rust
#[derive(Clone, Copy, PartialEq)]
enum Field {
    SpotBinance, PerpBinance,
    SpotBybit,   PerpBybit,
    SpotOkx,     PerpOkx,
    SpotBingx,   PerpBingx,
}

const FIELDS: [Field; 8] = [
    Field::SpotBinance, Field::PerpBinance,
    Field::SpotBybit,   Field::PerpBybit,
    Field::SpotOkx,     Field::PerpOkx,
    Field::SpotBingx,   Field::PerpBingx,
];
```

### Expanded `Field` methods

```rust
fn get<'a>(&self, t: &'a MultiPairTick) -> Option<&'a MarketQuote> {
    match self {
        Self::SpotBinance => t.spot_binance.as_ref(),
        Self::PerpBinance => t.perp_binance.as_ref(),
        Self::SpotBybit   => t.spot_bybit.as_ref(),
        Self::PerpBybit   => t.perp_bybit.as_ref(),
        Self::SpotOkx     => t.spot_okx.as_ref(),
        Self::PerpOkx     => t.perp_okx.as_ref(),
        Self::SpotBingx   => t.spot_bingx.as_ref(),
        Self::PerpBingx   => t.perp_bingx.as_ref(),
    }
}

fn market_id(self) -> MarketId {
    // Binance and Bybit use existing Exchange enum values.
    // OKX and BingX are not in Exchange enum — use a string label in the signal instead.
    // For now: map to closest existing exchange for fee_rate purposes.
    // OKX fees: Spot 0.08% taker, Swap 0.05% taker
    // BingX fees: Spot 0.1% taker, Swap 0.05% taker
    // See fee_rate() below.
}
```

### Exchange enum extension

Add `Okx` and `Bingx` variants to `src/types.rs`:

```rust
pub enum Exchange {
    Binance,
    Bybit,
    Mexc,
    Okx,
    Bingx,
}
```

Update `Display` impl. Update `fee_rate()` in `multi_detector.rs`:

```rust
fn fee_rate(market: &MarketId) -> f64 {
    match (market.exchange, market.market_type) {
        (Exchange::Binance, MarketType::Spot)    => 0.00100,
        (Exchange::Binance, MarketType::Futures) => 0.00050,
        (Exchange::Bybit,   MarketType::Spot)    => 0.00100,
        (Exchange::Bybit,   MarketType::Futures) => 0.00055,
        (Exchange::Okx,     MarketType::Spot)    => 0.00080,
        (Exchange::Okx,     MarketType::Futures) => 0.00050,
        (Exchange::Bingx,   MarketType::Spot)    => 0.00100,
        (Exchange::Bingx,   MarketType::Futures) => 0.00050,
        _                                         => 0.00100,
    }
}
```

`Field::market_id()` maps:
- `SpotOkx` → `MarketId::new(Exchange::Okx, MarketType::Spot)`
- `PerpOkx` → `MarketId::new(Exchange::Okx, MarketType::Futures)`
- `SpotBingx` → `MarketId::new(Exchange::Bingx, MarketType::Spot)`
- `PerpBingx` → `MarketId::new(Exchange::Bingx, MarketType::Futures)`

`Field::idx()` maps new variants to 4, 5, 6, 7.

The startup log updates to: `8 markets × 56 combos per pair`.

## main.rs Changes

Add 4 more spawns after the existing multi-feed spawns:

```rust
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_okx_spot(s).await });
}
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_okx_swap(s).await });
}
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_bingx_spot(s).await });
}
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_bingx_swap(s).await });
}
```

## dashboard/state.rs Changes

In `build_snapshot()`, add OKX and BingX quotes to the per-symbol collection:

```rust
if let Some(q) = &tick.spot_okx  {
    if q.updated_at.elapsed() <= stale { quotes.push(("OKX:Spot",  q.bid, q.ask)); }
}
if let Some(q) = &tick.perp_okx  {
    if q.updated_at.elapsed() <= stale { quotes.push(("OKX:Perp",  q.bid, q.ask)); }
}
if let Some(q) = &tick.spot_bingx {
    if q.updated_at.elapsed() <= stale { quotes.push(("BingX:Spot", q.bid, q.ask)); }
}
if let Some(q) = &tick.perp_bingx {
    if q.updated_at.elapsed() <= stale { quotes.push(("BingX:Perp", q.bid, q.ask)); }
}
```

## Files Changed

| Action | File |
|--------|------|
| Modify | `src/types.rs` — add `Exchange::Okx`, `Exchange::Bingx` variants |
| Modify | `src/multi_feed/mod.rs` — add 4 fields to `MultiPairTick`, add `to_dashed()`, move feed functions to sub-modules, pub re-export all `run_*` |
| Create | `src/multi_feed/binance.rs` — move Binance feed code from old mod.rs |
| Create | `src/multi_feed/bybit.rs` — move Bybit feed code from old mod.rs |
| Create | `src/multi_feed/okx.rs` — OKX spot + swap feeds |
| Create | `src/multi_feed/bingx.rs` — BingX spot + swap feeds |
| Modify | `src/arbitrage/multi_detector.rs` — expand Field enum to 8 variants, add fee rates for OKX/BingX |
| Modify | `src/arbitrage/executor.rs` — add Exchange::Okx/Bingx match arms in `place()` (no-op for now, paper trading only) |
| Modify | `src/main.rs` — spawn 4 more feed tasks |
| Modify | `src/dashboard/state.rs` — add OKX and BingX to opportunity quotes |

## Scope

- No live order execution for OKX/BingX. In `executor.rs` `place()`, add arms:
  ```rust
  Exchange::Okx   => bail!("OKX order execution not implemented"),
  Exchange::Bingx => bail!("BingX order execution not implemented"),
  ```
  Since paper trading is active, `place()` is never called (paper_fill short-circuits). The arms prevent compile error from non-exhaustive match.
- Paper trading PnL continues to use HBAR/BTC prices for fill simulation (pre-existing limitation)
- MarketScanner (REST polling for dashboard table) remains Binance+Bybit only — unchanged

# Multi-Pair Feed Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build four parallel WebSocket connections (Binance Spot, Binance Perp, Bybit Spot, Bybit Linear) that maintain real-time bid/ask prices for all 50 USDT pairs in a shared `DashMap<String, MultiPairTick>`.

**Architecture:** Extract the shared `TICKERS` constant to `src/tickers.rs`, create `src/multi_feed/mod.rs` with data types and four async feed functions, wire four tasks into main.rs. The existing single-pair bot is unchanged.

**Tech Stack:** Rust async, tokio-tungstenite (already in Cargo.toml), dashmap (already in Cargo.toml), serde_json

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Create | `src/tickers.rs` | Shared TICKERS constant (50 USDT pairs) |
| Modify | `src/market_scanner/mod.rs` | Remove local TICKERS, use crate::tickers::TICKERS |
| Create | `src/multi_feed/mod.rs` | MultiPairTick type, MultiPairState, 4 feed functions |
| Modify | `src/main.rs` | mod tickers, mod multi_feed, spawn 4 tasks |

---

### Task 1: Extract TICKERS to src/tickers.rs

**Files:**
- Create: `src/tickers.rs`
- Modify: `src/market_scanner/mod.rs`

- [ ] **Step 1: Create `src/tickers.rs`**

```rust
pub const TICKERS: &[&str] = &[
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT",
    "ADAUSDT", "DOGEUSDT", "AVAXUSDT", "DOTUSDT", "MATICUSDT",
    "LINKUSDT", "LTCUSDT", "UNIUSDT", "ATOMUSDT", "BCHUSDT",
    "ICPUSDT", "APTUSDT", "ARBUSDT", "OPUSDT", "FILUSDT",
    "NEARUSDT", "SANDUSDT", "MANAUSDT", "AXSUSDT", "ALGOUSDT",
    "VETUSDT", "FTMUSDT", "HBARUSDT", "ETCUSDT", "XLMUSDT",
    "TRXUSDT", "SUIUSDT", "SEIUSDT", "INJUSDT", "TIAUSDT",
    "JUPUSDT", "WIFUSDT", "BONKUSDT", "PEPEUSDT", "SHIBUSDT",
    "NOTUSDT", "TONUSDT", "STXUSDT", "RUNEUSDT", "RENDERUSDT",
    "WLDUSDT", "ENAUSDT", "ZKUSDT", "THETAUSDT", "FLOKIUSDT",
];
```

- [ ] **Step 2: Update `src/market_scanner/mod.rs`**

Remove the local `TICKERS` const block (lines 10–21) and replace with:

```rust
use crate::tickers::TICKERS;
```

Add this `use` after the other `use` statements at the top of the file.

- [ ] **Step 3: Add `mod tickers` to main.rs**

In `src/main.rs`, add after `mod types;`:

```rust
mod tickers;
```

- [ ] **Step 4: Verify compilation**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo check 2>&1 | grep "^error"
```

Expected: no output (no errors).

- [ ] **Step 5: Run tests**

```bash
~/.cargo/bin/cargo test 2>&1 | tail -3
```

Expected: all 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tickers.rs src/market_scanner/mod.rs src/main.rs
git commit -m "refactor: extract TICKERS to src/tickers.rs"
```

---

### Task 2: Create src/multi_feed/mod.rs

**Files:**
- Create: `src/multi_feed/mod.rs`

- [ ] **Step 1: Create the file**

```rust
use crate::tickers::TICKERS;
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct MultiPairTick {
    pub spot_binance: Option<(f64, f64)>,  // (bid, ask)
    pub perp_binance: Option<(f64, f64)>,
    pub spot_bybit:   Option<(f64, f64)>,
    pub perp_bybit:   Option<(f64, f64)>,
    pub updated_at:   Instant,
}

impl Default for MultiPairTick {
    fn default() -> Self {
        Self {
            spot_binance: None,
            perp_binance: None,
            spot_bybit:   None,
            perp_bybit:   None,
            updated_at:   Instant::now(),
        }
    }
}

pub type MultiPairState = Arc<DashMap<String, MultiPairTick>>;

pub fn new_state() -> MultiPairState {
    Arc::new(DashMap::new())
}

// ── Binance ──────────────────────────────────────────────────────────────────

pub async fn run_binance_spot(state: MultiPairState) {
    run_binance("wss://stream.binance.com:9443/ws/!bookTicker", state, true).await;
}

pub async fn run_binance_perp(state: MultiPairState) {
    run_binance("wss://fstream.binance.com/ws/!bookTicker", state, false).await;
}

async fn run_binance(url: &str, state: MultiPairState, is_spot: bool) {
    let set: HashSet<&str> = TICKERS.iter().copied().collect();
    let kind = if is_spot { "spot" } else { "perp" };
    loop {
        info!("MultiPairFeed Binance:{} connecting → {}", kind, url);
        match connect_async(url).await {
            Ok((mut ws, _)) => {
                while let Some(msg) = ws.next().await {
                    match msg {
                        Ok(Message::Text(txt)) => {
                            let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
                            let sym = v["s"].as_str().unwrap_or("").to_string();
                            if !set.contains(sym.as_str()) { continue; }
                            let bid: f64 = v["b"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let ask: f64 = v["a"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            if bid <= 0.0 || ask <= 0.0 { continue; }
                            state.entry(sym).and_modify(|t| {
                                if is_spot { t.spot_binance = Some((bid, ask)); }
                                else       { t.perp_binance = Some((bid, ask)); }
                                t.updated_at = Instant::now();
                            }).or_insert_with(|| {
                                let mut tick = MultiPairTick::default();
                                if is_spot { tick.spot_binance = Some((bid, ask)); }
                                else       { tick.perp_binance = Some((bid, ask)); }
                                tick
                            });
                        }
                        Ok(Message::Ping(d)) => { let _ = ws.send(Message::Pong(d)).await; }
                        Err(e) => { warn!("Binance:{} ws error: {e}", kind); break; }
                        _ => {}
                    }
                }
                warn!("Binance:{} stream ended, reconnecting", kind);
            }
            Err(e) => warn!("Binance:{} connect failed: {e}", kind),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

// ── Bybit ────────────────────────────────────────────────────────────────────

pub async fn run_bybit_spot(state: MultiPairState) {
    run_bybit("wss://stream.bybit.com/v5/public/spot", state, true).await;
}

pub async fn run_bybit_linear(state: MultiPairState) {
    run_bybit("wss://stream.bybit.com/v5/public/linear", state, false).await;
}

async fn run_bybit(url: &str, state: MultiPairState, is_spot: bool) {
    let set: HashSet<&str> = TICKERS.iter().copied().collect();
    let kind = if is_spot { "spot" } else { "linear" };
    let args: Vec<String> = TICKERS.iter().map(|s| format!("tickers.{s}")).collect();
    let sub_msg = serde_json::json!({ "op": "subscribe", "args": args }).to_string();

    loop {
        info!("MultiPairFeed Bybit:{} connecting → {}", kind, url);
        match connect_async(url).await {
            Ok((mut ws, _)) => {
                if ws.send(Message::Text(sub_msg.clone())).await.is_err() {
                    warn!("Bybit:{} subscribe send failed", kind);
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                while let Some(msg) = ws.next().await {
                    match msg {
                        Ok(Message::Text(txt)) => {
                            let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) else { continue };
                            let topic = v["topic"].as_str().unwrap_or("");
                            if !topic.starts_with("tickers.") { continue; }
                            let sym = topic.trim_start_matches("tickers.").to_string();
                            if !set.contains(sym.as_str()) { continue; }
                            let data = &v["data"];
                            let bid: f64 = data["bid1Price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let ask: f64 = data["ask1Price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            if bid <= 0.0 || ask <= 0.0 { continue; }
                            state.entry(sym).and_modify(|t| {
                                if is_spot { t.spot_bybit = Some((bid, ask)); }
                                else       { t.perp_bybit = Some((bid, ask)); }
                                t.updated_at = Instant::now();
                            }).or_insert_with(|| {
                                let mut tick = MultiPairTick::default();
                                if is_spot { tick.spot_bybit = Some((bid, ask)); }
                                else       { tick.perp_bybit = Some((bid, ask)); }
                                tick
                            });
                        }
                        Ok(Message::Ping(d)) => { let _ = ws.send(Message::Pong(d)).await; }
                        Err(e) => { warn!("Bybit:{} ws error: {e}", kind); break; }
                        _ => {}
                    }
                }
                warn!("Bybit:{} stream ended, reconnecting", kind);
            }
            Err(e) => warn!("Bybit:{} connect failed: {e}", kind),
        }
        sleep(Duration::from_secs(1)).await;
    }
}
```

- [ ] **Step 2: Verify it compiles (without wiring yet)**

Temporarily add `mod multi_feed;` to main.rs, check, then revert:

```bash
cd /Users/rinchin92/claude/project
echo 'mod multi_feed;' >> src/main.rs
~/.cargo/bin/cargo check 2>&1 | grep "^error" | head -10
git checkout src/main.rs
```

Expected: no errors from multi_feed itself (there will be an error about `mod tickers` missing from the temporary main.rs — that's fine since Task 1 already added it).

Actually, since Task 1 already added `mod tickers;` to main.rs, just run:

```bash
cd /Users/rinchin92/claude/project
~/.cargo/bin/cargo check 2>&1 | grep "^error" | head -10
```

Expected: an error about `multi_feed` not found (module not yet declared) — confirm no errors WITHIN market_scanner or tickers.

- [ ] **Step 3: Commit**

```bash
git add src/multi_feed/mod.rs
git commit -m "feat: add MultiPairFeed with Binance+Bybit spot+perp WebSocket feeds"
```

---

### Task 3: Wire into main.rs and verify

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add `mod multi_feed` declaration**

In `src/main.rs`, add after `mod tickers;`:

```rust
mod multi_feed;
```

- [ ] **Step 2: Create state and spawn 4 tasks**

After `let metrics = Arc::new(MetricsCollector::new());`, add:

```rust
let multi_state = multi_feed::new_state();
```

In the `// ── Price feeds ──` section, add 4 new spawns AFTER the existing 5 single-pair feeds:

```rust
// ── Multi-pair feeds ─────────────────────────────────────────────────────
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_binance_spot(s).await });
}
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_binance_perp(s).await });
}
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_bybit_spot(s).await });
}
{
    let s = multi_state.clone();
    set.spawn(async move { multi_feed::run_bybit_linear(s).await });
}
```

- [ ] **Step 3: Build release**

```bash
cd /Users/rinchin92/claude/project && ~/.cargo/bin/cargo build --release 2>&1 | tail -3
```

Expected: `Finished release profile`.

- [ ] **Step 4: Run and verify feeds connect**

```bash
pkill -f "sol-arb" 2>/dev/null; lsof -ti:3001 | xargs kill -9 2>/dev/null; sleep 1
RUST_LOG=sol_arb=info ./target/release/sol-arb > /tmp/sol-arb.log 2>&1 &
sleep 6 && grep "MultiPairFeed" /tmp/sol-arb.log
```

Expected output (4 lines, one per feed):
```
MultiPairFeed Binance:spot connecting → wss://stream.binance.com:9443/ws/!bookTicker
MultiPairFeed Binance:perp connecting → wss://fstream.binance.com/ws/!bookTicker
MultiPairFeed Bybit:spot connecting → wss://stream.bybit.com/v5/public/spot
MultiPairFeed Bybit:linear connecting → wss://stream.bybit.com/v5/public/linear
```

- [ ] **Step 5: Verify data is flowing**

After 10 seconds, add a quick debug check by looking at the log for any warn messages (should be none if connections are healthy):

```bash
sleep 5 && grep -c "MultiPairFeed" /tmp/sol-arb.log && grep "warn\|error" /tmp/sol-arb.log | grep -i "multi" | head -5
```

Expected: at least 4 MultiPairFeed log lines, no warn/error lines for MultiPair feeds.

- [ ] **Step 6: Commit and push**

```bash
git add src/main.rs
git commit -m "feat: wire MultiPairFeed into main — 4 WebSocket feeds for 50 pairs"
git push origin master
```

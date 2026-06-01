# Multi-Pair Arbitrage Detector — Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-pair `ArbitrageDetector` with a `MultiPairDetector` that scans all 50 USDT pairs across Binance and Bybit spot+perp markets in real-time and executes trades on the best opportunity found.

**Architecture:** Extend `MultiPairTick` with bid/ask qty data (already in WebSocket streams, currently discarded), build `MultiPairDetector` that reads from the shared `DashMap`, applies all existing filters (microprice, imbalance, vol-adjusted spread), and sends signals enriched with a `symbol` field to the existing `OrderExecutor`. Exchange `place_order` methods gain a `symbol` parameter so the executor can trade any pair.

**Tech Stack:** Rust, tokio, dashmap, rust_decimal, existing pricing utilities (microprice, imbalance logic inlined as f64 variants)

---

## File Map

| Action | File |
|--------|------|
| Modify | `src/multi_feed/mod.rs` — add `MarketQuote`, update parsers |
| Modify | `src/types.rs` — add `symbol: String` to `ArbitrageSignal` |
| Modify | `src/arbitrage/detector.rs` — populate `symbol` field |
| Modify | `src/exchanges/binance.rs` — add `symbol` param to `place_order` |
| Modify | `src/exchanges/bybit.rs` — add `symbol` param to `place_order` |
| Modify | `src/arbitrage/executor.rs` — pass `signal.symbol` to `place` |
| Create | `src/arbitrage/multi_detector.rs` |
| Modify | `src/arbitrage/mod.rs` — add `pub mod multi_detector` |
| Modify | `src/main.rs` — swap detector |

---

## Task 1: Extend MultiPairTick with qty fields

**Files:**
- Modify: `src/multi_feed/mod.rs`

The Binance `!bookTicker` stream already sends `B` (bid_qty) and `A` (ask_qty). The Bybit `tickers` stream sends `bid1Size` and `ask1Size`. We currently discard these. This task captures them.

- [ ] **Step 1: Add `MarketQuote` struct and update `MultiPairTick`**

In `src/multi_feed/mod.rs`, after the `use` block and before `MultiPairTick`, add:

```rust
#[derive(Debug, Clone)]
pub struct MarketQuote {
    pub bid:     f64,
    pub ask:     f64,
    pub bid_qty: f64,
    pub ask_qty: f64,
}
```

Replace the `MultiPairTick` struct definition:

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

- [ ] **Step 2: Update `connect_binance_once` to parse qty and use `MarketQuote`**

In `connect_binance_once`, replace the entire `Message::Text` arm:

```rust
Message::Text(text) => {
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => continue,
    };
    let sym = v["s"].as_str().unwrap_or("");
    if !valid.contains(sym) { continue; }
    let bid     = v["b"].as_str().and_then(|s| s.parse::<f64>().ok());
    let ask     = v["a"].as_str().and_then(|s| s.parse::<f64>().ok());
    let bid_qty = v["B"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    let ask_qty = v["A"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    if let (Some(bid), Some(ask)) = (bid, ask) {
        if bid > 0.0 && ask > 0.0 {
            let quote = MarketQuote { bid, ask, bid_qty, ask_qty };
            state.entry(sym.to_string())
                .and_modify(|t| {
                    if is_perp { t.perp_binance = Some(quote.clone()); }
                    else       { t.spot_binance = Some(quote.clone()); }
                    t.updated_at = Instant::now();
                })
                .or_insert_with(|| MultiPairTick {
                    spot_binance: if !is_perp { Some(quote.clone()) } else { None },
                    perp_binance: if  is_perp { Some(quote.clone()) } else { None },
                    spot_bybit:   None,
                    perp_bybit:   None,
                    updated_at:   Instant::now(),
                });
        }
    }
}
```

- [ ] **Step 3: Update `connect_bybit_once` to parse qty and use `MarketQuote`**

In `connect_bybit_once`, replace the block that builds the DashMap update inside `Message::Text`:

```rust
let data = &v["data"];
let bid     = data["bid1Price"].as_str().and_then(|s| s.parse::<f64>().ok());
let ask     = data["ask1Price"].as_str().and_then(|s| s.parse::<f64>().ok());
let bid_qty = data["bid1Size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
let ask_qty = data["ask1Size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
if let (Some(bid), Some(ask)) = (bid, ask) {
    if bid > 0.0 && ask > 0.0 {
        let quote = MarketQuote { bid, ask, bid_qty, ask_qty };
        state.entry(sym.to_string())
            .and_modify(|t| {
                if is_perp { t.perp_bybit = Some(quote.clone()); }
                else       { t.spot_bybit = Some(quote.clone()); }
                t.updated_at = Instant::now();
            })
            .or_insert_with(|| MultiPairTick {
                spot_binance: None,
                perp_binance: None,
                spot_bybit:   if !is_perp { Some(quote.clone()) } else { None },
                perp_bybit:   if  is_perp { Some(quote.clone()) } else { None },
                updated_at:   Instant::now(),
            });
    }
}
```

- [ ] **Step 4: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no `error` lines. Warnings about unused fields are fine.

- [ ] **Step 5: Commit**

```bash
git add src/multi_feed/mod.rs
git commit -m "feat: add MarketQuote with qty fields to MultiPairTick"
```

---

## Task 2: Add `symbol` to `ArbitrageSignal`

**Files:**
- Modify: `src/types.rs`
- Modify: `src/arbitrage/detector.rs`

- [ ] **Step 1: Add `symbol` field to `ArbitrageSignal` in `src/types.rs`**

Replace the `ArbitrageSignal` struct:

```rust
#[derive(Debug, Clone)]
pub struct ArbitrageSignal {
    pub id: Uuid,
    pub symbol: String,
    pub buy_market: MarketId,
    pub sell_market: MarketId,
    /// Price we'll pay on the buy leg (ask + slippage)
    pub buy_ask: Decimal,
    /// Price we'll receive on the sell leg (bid - slippage)
    pub sell_bid: Decimal,
    pub spread_pct: Decimal,
    pub quantity: Decimal,
    pub expected_pnl_usdt: Decimal,
    pub detected_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Update `ArbitrageDetector` to populate `symbol`**

In `src/arbitrage/detector.rs`, in the `evaluate` method, update the `Some(ArbitrageSignal { ... })` block at the bottom to add the symbol field:

```rust
Some(ArbitrageSignal {
    id: Uuid::new_v4(),
    symbol: self.config.pair(),
    buy_market: buy.market,
    sell_market: sell.market,
    buy_ask: eff_buy,
    sell_bid: eff_sell,
    spread_pct,
    quantity,
    expected_pnl_usdt: expected_pnl,
    detected_at: Utc::now(),
})
```

- [ ] **Step 3: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no `error` lines.

- [ ] **Step 4: Commit**

```bash
git add src/types.rs src/arbitrage/detector.rs
git commit -m "feat: add symbol field to ArbitrageSignal"
```

---

## Task 3: Add `symbol` param to `place_order`

**Files:**
- Modify: `src/exchanges/binance.rs`
- Modify: `src/exchanges/bybit.rs`
- Modify: `src/arbitrage/executor.rs`

- [ ] **Step 1: Update `BinanceConnector::place_order` signature in `src/exchanges/binance.rs`**

Replace the `place_order` method signature and the `pair` variable inside it:

```rust
pub async fn place_order(
    &self,
    symbol: &str,
    market: MarketType,
    side: Side,
    quantity: Decimal,
    price_state: &SharedPriceState,
) -> Result<OrderResult> {
    if self.config.trading.paper_trading {
        return Ok(self.paper_fill(market, side, quantity, price_state));
    }

    let ts = now_ms();
    let params = format!(
        "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
        symbol, side, quantity, ts
    );
    // ... rest of method unchanged
```

The only change is: (1) add `symbol: &str` as the first parameter, (2) replace `let pair = self.config.pair();` with `let pair = symbol;` in the params format string (or use `symbol` directly as shown above).

Full updated `place_order` for `BinanceConnector` (replace existing method, lines 123–174):

```rust
pub async fn place_order(
    &self,
    symbol: &str,
    market: MarketType,
    side: Side,
    quantity: Decimal,
    price_state: &SharedPriceState,
) -> Result<OrderResult> {
    if self.config.trading.paper_trading {
        return Ok(self.paper_fill(market, side, quantity, price_state));
    }

    let ts = now_ms();
    let params = format!(
        "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
        symbol, side, quantity, ts
    );
    let sig = sign_hmac_sha256(&self.config.binance.api_secret, &params);
    let body = format!("{}&signature={}", params, sig);

    let path = match market {
        MarketType::Spot    => "/api/v3/order",
        MarketType::Futures => "/fapi/v1/order",
    };
    let resp: serde_json::Value = self.http
        .post(format!("{}{}", self.rest_base(market), path))
        .header("X-MBX-APIKEY", &self.config.binance.api_key)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send().await?
        .json().await?;

    if let Some(code) = resp.get("code") {
        bail!("Binance order error {}: {}", code, resp["msg"].as_str().unwrap_or(""));
    }

    let filled_qty = Decimal::from_str(resp["executedQty"].as_str().unwrap_or("0"))?;
    let quote_qty  = Decimal::from_str(resp["cummulativeQuoteQty"].as_str().unwrap_or("0"))?;
    let avg_price  = if filled_qty > dec!(0) { quote_qty / filled_qty } else { dec!(0) };
    let fee_rate   = if market == MarketType::Futures { dec!(0.0005) } else { dec!(0.001) };

    Ok(OrderResult {
        exchange:    Exchange::Binance,
        market_type: market,
        order_id:    resp["orderId"].as_u64().unwrap_or(0).to_string(),
        side,
        filled_qty,
        avg_price,
        fee_usdt:  filled_qty * avg_price * fee_rate,
        timestamp: Utc::now(),
    })
}
```

- [ ] **Step 2: Update `BybitConnector::place_order` in `src/exchanges/bybit.rs`**

Full updated `place_order` for `BybitConnector` (replace existing method, lines 163–223):

```rust
pub async fn place_order(
    &self,
    symbol: &str,
    market: MarketType,
    side: Side,
    quantity: Decimal,
    price_state: &SharedPriceState,
) -> Result<OrderResult> {
    if self.config.trading.paper_trading {
        return Ok(self.paper_fill(market, side, quantity, price_state));
    }

    let ts = now_ms();
    let recv_window = 5000u64;
    let category = match market {
        MarketType::Spot    => "spot",
        MarketType::Futures => "linear",
    };
    let body = serde_json::json!({
        "category":  category,
        "symbol":    symbol,
        "side":      match side { Side::Buy => "Buy", Side::Sell => "Sell" },
        "orderType": "Market",
        "qty":       quantity.to_string(),
    }).to_string();

    let sign_payload = format!("{}{}{}{}", ts, self.config.bybit.api_key, recv_window, body);
    let sig = sign_hmac_sha256(&self.config.bybit.api_secret, &sign_payload);

    let resp: serde_json::Value = self.http
        .post(format!("{}/v5/order/create", self.rest_base()))
        .header("X-BAPI-API-KEY",      &self.config.bybit.api_key)
        .header("X-BAPI-SIGN",         sig)
        .header("X-BAPI-SIGN-TYPE",    "2")
        .header("X-BAPI-TIMESTAMP",    ts.to_string())
        .header("X-BAPI-RECV-WINDOW",  recv_window.to_string())
        .header("Content-Type",        "application/json")
        .body(body)
        .send().await?
        .json().await?;

    let ret_code = resp["retCode"].as_i64().unwrap_or(-1);
    if ret_code != 0 {
        bail!("Bybit order error {}: {}", ret_code, resp["retMsg"].as_str().unwrap_or(""));
    }

    let result = &resp["result"];
    let filled_qty = Decimal::from_str(result["cumExecQty"].as_str().unwrap_or("0"))?;
    let avg_price  = Decimal::from_str(result["avgPrice"].as_str().unwrap_or("0"))?;
    let fee_rate   = if market == MarketType::Futures { dec!(0.00055) } else { dec!(0.001) };

    Ok(OrderResult {
        exchange:    Exchange::Bybit,
        market_type: market,
        order_id:    result["orderId"].as_str().unwrap_or("").to_string(),
        side,
        filled_qty,
        avg_price,
        fee_usdt:  filled_qty * avg_price * fee_rate,
        timestamp: Utc::now(),
    })
}
```

- [ ] **Step 3: Update `OrderExecutor::place` in `src/arbitrage/executor.rs`**

Update `execute` to pass `&signal.symbol`:

```rust
async fn execute(&self, signal: &ArbitrageSignal) -> Result<CompletedTrade> {
    let qty = signal.quantity;
    let ps  = &self.price_state;

    let (buy_res, sell_res) = tokio::join!(
        self.place(signal.buy_market.exchange,  &signal.symbol, signal.buy_market.market_type,  Side::Buy,  qty, ps),
        self.place(signal.sell_market.exchange, &signal.symbol, signal.sell_market.market_type, Side::Sell, qty, ps),
    );

    let buy_order  = buy_res?;
    let sell_order = sell_res?;

    let gross_pnl = (sell_order.avg_price - buy_order.avg_price) * buy_order.filled_qty;
    let fees      = buy_order.fee_usdt + sell_order.fee_usdt;
    let net_pnl   = gross_pnl - fees;

    let exec_ms = if self.config.trading.paper_trading {
        30 + (signal.id.as_u128() % 120) as u64
    } else {
        sell_order.timestamp
            .signed_duration_since(signal.detected_at)
            .num_milliseconds()
            .max(0) as u64
    };

    info!(
        "TRADE {} {} | buy {}@{:.4} | sell {}@{:.4} | net_pnl={:.4} USDT | exec={}ms",
        signal.symbol, signal.id,
        signal.buy_market, buy_order.avg_price,
        signal.sell_market, sell_order.avg_price,
        net_pnl, exec_ms
    );

    Ok(CompletedTrade {
        id: signal.id,
        signal: signal.clone(),
        buy_order,
        sell_order,
        gross_pnl,
        fees,
        net_pnl,
        exec_ms,
        completed_at: Utc::now(),
    })
}
```

Update the `place` private method signature and body:

```rust
async fn place(
    &self,
    exchange: Exchange,
    symbol: &str,
    market: MarketType,
    side: Side,
    qty: rust_decimal::Decimal,
    ps: &SharedPriceState,
) -> Result<crate::types::OrderResult> {
    match exchange {
        Exchange::Binance => self.binance.place_order(symbol, market, side, qty, ps).await,
        Exchange::Bybit   => self.bybit.place_order(symbol, market, side, qty, ps).await,
        Exchange::Mexc    => self.mexc.place_order(side, qty, ps).await,
    }
}
```

- [ ] **Step 4: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no `error` lines.

- [ ] **Step 5: Run tests**

```bash
~/.cargo/bin/cargo test 2>&1 | tail -10
```

Expected: `4 passed; 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add src/exchanges/binance.rs src/exchanges/bybit.rs src/arbitrage/executor.rs
git commit -m "feat: add symbol param to place_order; executor passes signal.symbol"
```

---

## Task 4: Implement MultiPairDetector

**Files:**
- Create: `src/arbitrage/multi_detector.rs`
- Modify: `src/arbitrage/mod.rs`

- [ ] **Step 1: Add `pub mod multi_detector` to `src/arbitrage/mod.rs`**

```rust
pub mod detector;
pub mod executor;
pub mod multi_detector;
```

- [ ] **Step 2: Create `src/arbitrage/multi_detector.rs`**

```rust
use crate::{
    config::Config,
    multi_feed::{MarketQuote, MultiPairState, MultiPairTick},
    types::{ArbitrageSignal, Exchange, MarketId, MarketType},
};
use chrono::Utc;
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, info};
use uuid::Uuid;

// ── Fee rates (taker, no VIP) ─────────────────────────────────────────────────

fn fee_rate(market: &MarketId) -> f64 {
    match (market.exchange, market.market_type) {
        (Exchange::Binance, MarketType::Spot)    => 0.00100,
        (Exchange::Binance, MarketType::Futures) => 0.00050,
        (Exchange::Bybit,   MarketType::Spot)    => 0.00100,
        (Exchange::Bybit,   MarketType::Futures) => 0.00055,
        _                                         => 0.00100,
    }
}

// ── f64 pricing helpers ───────────────────────────────────────────────────────

fn microprice(bid: f64, bid_qty: f64, ask: f64, ask_qty: f64) -> Option<f64> {
    let total = bid_qty + ask_qty;
    if total == 0.0 { return None; }
    Some((bid * ask_qty + ask * bid_qty) / total)
}

fn imbalance(bid_qty: f64, ask_qty: f64) -> f64 {
    let total = bid_qty + ask_qty;
    if total == 0.0 { return 0.0; }
    (bid_qty - ask_qty) / total
}

fn to_dec(v: f64) -> Decimal {
    v.to_string().parse().unwrap_or(Decimal::ZERO)
}

fn to_f64(d: Decimal) -> f64 {
    d.to_string().parse().unwrap_or(0.0)
}

// ── EWMA volatility (λ=0.94, f64 internals) ──────────────────────────────────

struct F64Ewma {
    lambda:   f64,
    variance: f64,
    last_mid: Option<f64>,
    n:        u32,
}

impl F64Ewma {
    fn new() -> Self { Self { lambda: 0.94, variance: 0.0, last_mid: None, n: 0 } }

    fn update(&mut self, mid: f64) {
        if let Some(prev) = self.last_mid {
            if prev > 0.0 {
                let r = (mid - prev) / prev;
                self.variance = self.lambda * self.variance + (1.0 - self.lambda) * r * r;
                self.n += 1;
            }
        }
        self.last_mid = Some(mid);
    }

    fn variance(&self) -> Option<f64> {
        if self.n >= 2 { Some(self.variance) } else { None }
    }
}

// ── Market field enum ─────────────────────────────────────────────────────────
// Maps a market slot in MultiPairTick to its MarketId and u8 index for vol tracking.

#[derive(Clone, Copy, PartialEq)]
enum Field { SpotBinance, PerpBinance, SpotBybit, PerpBybit }

impl Field {
    fn get<'a>(&self, t: &'a MultiPairTick) -> Option<&'a MarketQuote> {
        match self {
            Self::SpotBinance => t.spot_binance.as_ref(),
            Self::PerpBinance => t.perp_binance.as_ref(),
            Self::SpotBybit   => t.spot_bybit.as_ref(),
            Self::PerpBybit   => t.perp_bybit.as_ref(),
        }
    }

    fn market_id(self) -> MarketId {
        match self {
            Self::SpotBinance => MarketId::new(Exchange::Binance, MarketType::Spot),
            Self::PerpBinance => MarketId::new(Exchange::Binance, MarketType::Futures),
            Self::SpotBybit   => MarketId::new(Exchange::Bybit,   MarketType::Spot),
            Self::PerpBybit   => MarketId::new(Exchange::Bybit,   MarketType::Futures),
        }
    }

    fn idx(self) -> u8 {
        match self {
            Self::SpotBinance => 0,
            Self::PerpBinance => 1,
            Self::SpotBybit   => 2,
            Self::PerpBybit   => 3,
        }
    }
}

const FIELDS: [Field; 4] = [Field::SpotBinance, Field::PerpBinance, Field::SpotBybit, Field::PerpBybit];

// ── Detector ──────────────────────────────────────────────────────────────────

pub struct MultiPairDetector {
    config:    Arc<Config>,
    state:     MultiPairState,
    signal_tx: mpsc::Sender<ArbitrageSignal>,
    vol_map:   DashMap<(String, u8), F64Ewma>,
    last_mids: DashMap<(String, u8), f64>,
}

impl MultiPairDetector {
    pub fn new(
        config: Arc<Config>,
        state: MultiPairState,
        signal_tx: mpsc::Sender<ArbitrageSignal>,
    ) -> Self {
        Self {
            config,
            state,
            signal_tx,
            vol_map:   DashMap::new(),
            last_mids: DashMap::new(),
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!(
            "MultiPairDetector started — 50 pairs × 12 combos (min_spread={:.3}%  γ={}  τ={})",
            to_f64(self.config.trading.min_spread_pct) * 100.0,
            self.config.trading.gamma,
            self.config.trading.tau,
        );
        loop {
            self.scan_once();
            sleep(Duration::from_millis(1)).await;
        }
    }

    fn scan_once(&self) {
        let stale = Duration::from_millis(500);

        // ── Pass 1: update EWMA volatilities, snapshot variances ─────────────
        let mut variances: HashMap<(String, u8), f64> = HashMap::new();
        for entry in self.state.iter() {
            let sym  = entry.key();
            let tick = entry.value();
            for &field in &FIELDS {
                if let Some(q) = field.get(tick) {
                    let mid = (q.bid + q.ask) / 2.0;
                    let key = (sym.clone(), field.idx());
                    let changed = self.last_mids.get(&key)
                        .map(|prev| (*prev - mid).abs() > f64::EPSILON)
                        .unwrap_or(true);
                    if changed {
                        self.last_mids.insert(key.clone(), mid);
                        self.vol_map
                            .entry(key.clone())
                            .and_modify(|e| e.update(mid))
                            .or_insert_with(|| { let mut e = F64Ewma::new(); e.update(mid); e });
                    }
                    if let Some(var) = self.vol_map.get(&key).and_then(|e| e.variance()) {
                        variances.insert(key, var);
                    }
                }
            }
        }

        // ── Pass 2: detect opportunities ─────────────────────────────────────
        for entry in self.state.iter() {
            let sym  = entry.key();
            let tick = entry.value();
            if tick.updated_at.elapsed() > stale { continue; }

            for &buy_field in &FIELDS {
                for &sell_field in &FIELDS {
                    if buy_field == sell_field { continue; }
                    if let (Some(buy_q), Some(sell_q)) = (buy_field.get(tick), sell_field.get(tick)) {
                        let buy_var  = variances.get(&(sym.clone(), buy_field.idx())).copied().unwrap_or(0.0);
                        let sell_var = variances.get(&(sym.clone(), sell_field.idx())).copied().unwrap_or(0.0);
                        let avg_var  = (buy_var + sell_var) / 2.0;
                        if let Some(signal) = self.evaluate(
                            sym,
                            buy_field.market_id(), buy_q,
                            sell_field.market_id(), sell_q,
                            avg_var,
                        ) {
                            if self.signal_tx.try_send(signal).is_err() {
                                debug!("Signal channel full — dropping");
                            }
                        }
                    }
                }
            }
        }
    }

    fn evaluate(
        &self,
        symbol:      &str,
        buy_market:  MarketId,
        buy_q:       &MarketQuote,
        sell_market: MarketId,
        sell_q:      &MarketQuote,
        avg_var:     f64,
    ) -> Option<ArbitrageSignal> {
        let buy_ask  = buy_q.ask;
        let sell_bid = sell_q.bid;
        if buy_ask <= 0.0 || sell_bid <= 0.0 { return None; }

        // ── 1. Microprice ─────────────────────────────────────────────────────
        let buy_mp  = microprice(buy_q.bid,  buy_q.bid_qty,  buy_q.ask,  buy_q.ask_qty)
            .unwrap_or((buy_q.bid  + buy_q.ask)  / 2.0);
        let sell_mp = microprice(sell_q.bid, sell_q.bid_qty, sell_q.ask, sell_q.ask_qty)
            .unwrap_or((sell_q.bid + sell_q.ask) / 2.0);

        let mp_spread = if buy_mp > 0.0 { (sell_mp - buy_mp) / buy_mp } else { 0.0 };
        if mp_spread < -0.005 {
            debug!(
                "{} skipped: microprice disagrees ({:.3}%)",
                symbol, mp_spread * 100.0
            );
            return None;
        }

        // ── 2. Imbalance ──────────────────────────────────────────────────────
        let buy_imb  = imbalance(buy_q.bid_qty,  buy_q.ask_qty);
        let sell_imb = imbalance(sell_q.bid_qty, sell_q.ask_qty);
        let thresh   = self.config.trading.imbalance_threshold;
        if buy_imb > thresh || sell_imb < -thresh {
            debug!(
                "{} skipped: adverse imbalance buy={:.2} sell={:.2}",
                symbol, buy_imb, sell_imb
            );
            return None;
        }

        // ── 3. Fee-adjusted spread ────────────────────────────────────────────
        let slippage = to_f64(self.config.trading.max_slippage_pct);
        let eff_buy  = buy_ask  * (1.0 + slippage);
        let eff_sell = sell_bid * (1.0 - slippage);
        let net_cost = eff_buy  * (1.0 + fee_rate(&buy_market));
        let net_recv = eff_sell * (1.0 - fee_rate(&sell_market));
        let spread   = (net_recv - net_cost) / net_cost;

        // ── 4. Vol-adjusted minimum spread ────────────────────────────────────
        let min_spread  = to_f64(self.config.trading.min_spread_pct);
        let vol_penalty = self.config.trading.gamma * avg_var * self.config.trading.tau;
        let eff_min     = min_spread + vol_penalty;

        if spread < eff_min { return None; }

        let trade_usdt = to_f64(self.config.trading.trade_size_usdt);
        let quantity   = trade_usdt / eff_buy;

        info!(
            "SIGNAL {} buy={} ask={:.4} mp={:.4} imb={:.2} | sell={} bid={:.4} mp={:.4} imb={:.2} | spread={:.4}% ≥ min={:.4}%",
            symbol,
            buy_market,  buy_ask,  buy_mp,  buy_imb,
            sell_market, sell_bid, sell_mp, sell_imb,
            spread * 100.0, eff_min * 100.0,
        );

        Some(ArbitrageSignal {
            id:                Uuid::new_v4(),
            symbol:            symbol.to_string(),
            buy_market,
            sell_market,
            buy_ask:           to_dec(eff_buy),
            sell_bid:          to_dec(eff_sell),
            spread_pct:        to_dec(spread),
            quantity:          to_dec(quantity),
            expected_pnl_usdt: to_dec((net_recv - net_cost) * quantity),
            detected_at:       Utc::now(),
        })
    }
}
```

- [ ] **Step 3: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no `error` lines.

- [ ] **Step 4: Commit**

```bash
git add src/arbitrage/multi_detector.rs src/arbitrage/mod.rs
git commit -m "feat: implement MultiPairDetector for 50-pair cross-exchange arb detection"
```

---

## Task 5: Wire MultiPairDetector into main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update imports in `src/main.rs`**

Replace the arbitrage imports line:

```rust
use arbitrage::{detector::ArbitrageDetector, executor::OrderExecutor};
```

With:

```rust
use arbitrage::{multi_detector::MultiPairDetector, executor::OrderExecutor};
```

- [ ] **Step 2: Replace detector construction**

Find the block:
```rust
let detector = Arc::new(ArbitrageDetector::new(
    config.clone(),
    price_state.clone(),
    signal_tx,
));
```

Replace with:
```rust
let detector = Arc::new(MultiPairDetector::new(
    config.clone(),
    multi_state.clone(),
    signal_tx,
));
```

- [ ] **Step 3: Verify compilation**

```bash
~/.cargo/bin/cargo check 2>&1 | grep -E "^error" | head -20
```

Expected: no errors.

- [ ] **Step 4: Run tests**

```bash
~/.cargo/bin/cargo test 2>&1 | tail -10
```

Expected: `4 passed; 0 failed`.

- [ ] **Step 5: Release build**

```bash
~/.cargo/bin/cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized] target(s)`.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: replace ArbitrageDetector with MultiPairDetector in main"
```

---

## Verification

**Start the bot and check logs:**

```bash
RUST_LOG=sol_arb=info ~/.cargo/bin/cargo run 2>&1 | grep -E "MultiPairDetector|SIGNAL|TRADE"
```

Expected startup line:
```
INFO sol_arb::arbitrage::multi_detector: MultiPairDetector started — 50 pairs × 12 combos (min_spread=0.030%  γ=50  τ=1)
```

When a spread ≥ min is found:
```
INFO sol_arb::arbitrage::multi_detector: SIGNAL BTCUSDT buy=Binance:Spot ask=72800.0000 ... | spread=0.0412% ≥ min=0.0300%
INFO sol_arb::arbitrage::executor: TRADE BTCUSDT <uuid> | buy Binance:Spot@72800.0000 | sell Bybit:Spot@72801.0000 | net_pnl=...
```

**Paper trading note:** Paper fills still use `price_state` (single-pair HBAR data) for fill price simulation. Net PnL numbers in paper mode will be based on HBAR prices, not the actual traded symbol. This is a known limitation for Phase 2 paper mode only — live trading uses actual exchange prices.

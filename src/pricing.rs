/// Pricing utilities from Avellaneda-Stoikov (2008) and Stoikov (2018).
///
/// Adapted from https://github.com/l-arkadiy-l/hft-market-making-2026
/// for cross-exchange arbitrage (fractional price units, not fixed-point).
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ── Microprice (Stoikov 2018) ─────────────────────────────────────────────────
//
//   microprice = (bid · ask_qty + ask · bid_qty) / (bid_qty + ask_qty)
//
// Volume-weighted mid: if bid side is heavier the microprice shifts toward bid
// (price likely to fall), and vice versa. More accurate than simple mid.

pub fn microprice(
    bid: Decimal,
    bid_qty: Decimal,
    ask: Decimal,
    ask_qty: Decimal,
) -> Option<Decimal> {
    let total = bid_qty + ask_qty;
    if total == dec!(0) {
        return None;
    }
    Some((bid * ask_qty + ask * bid_qty) / total)
}

// ── Order book imbalance ──────────────────────────────────────────────────────
//
//   imbalance = (bid_qty − ask_qty) / (bid_qty + ask_qty)  ∈ [−1, 1]
//
// Positive → buying pressure (price likely rising).
// Negative → selling pressure (price likely falling).

pub fn imbalance(bid_qty: Decimal, ask_qty: Decimal) -> f64 {
    let bq: f64 = bid_qty.to_string().parse().unwrap_or(0.0);
    let aq: f64 = ask_qty.to_string().parse().unwrap_or(0.0);
    let total = bq + aq;
    if total == 0.0 {
        return 0.0;
    }
    (bq - aq) / total
}

// ── EWMA volatility — RiskMetrics (1996) ─────────────────────────────────────
//
//   σ²_t = λ · σ²_{t-1} + (1−λ) · r_t²
//   r_t  = (mid_t − mid_{t-1}) / mid_{t-1}
//
// Default λ = 0.94 (half-life of volatility shock ≈ 11 ticks).
// Returns σ² / σ in fractional units (not fp).
// Requires ≥ 50 updates before values are considered reliable.

pub struct EwmaVolatility {
    lambda: f64,
    pub variance: f64,
    last_mid: Option<f64>,
    pub n_updates: u64,
}

impl EwmaVolatility {
    pub fn new() -> Self {
        Self { lambda: 0.94, variance: 0.0, last_mid: None, n_updates: 0 }
    }

    pub fn update(&mut self, mid: Decimal) {
        let m: f64 = mid.to_string().parse().unwrap_or(0.0);
        if let Some(prev) = self.last_mid {
            if prev > 0.0 {
                let r = (m - prev) / prev;
                self.variance = self.lambda * self.variance + (1.0 - self.lambda) * r * r;
                self.n_updates += 1;
            }
        }
        self.last_mid = Some(m);
    }

    /// σ² (fractional, per price change). None until 2+ real price changes.
    pub fn variance(&self) -> Option<f64> {
        if self.n_updates < 2 { None } else { Some(self.variance) }
    }

    /// σ (std dev of fractional returns per price change). None until 2+ changes.
    pub fn sigma(&self) -> Option<f64> {
        self.variance().map(|v| v.sqrt())
    }
}

// ── AS-2008 volatility penalty ────────────────────────────────────────────────
//
//   vol_penalty = γ · σ² · τ    (fractional units)
//
// γ (gamma): risk aversion. Calibrated for fractional prices:
//   γ = 50 gives ~5 bps penalty at σ = 0.001 (0.1%/tick normal vol).
//   γ = 50 gives ~125 bps penalty at σ = 0.005 (0.5%/tick high vol).
//
// This penalty is added to min_spread_pct so the bot becomes more selective
// when volatility rises — directly encoding the AS-2008 intuition.

pub fn as_vol_penalty(variance: f64, gamma: f64, tau: f64) -> f64 {
    gamma * variance * tau
}

// ── AS-2008 optimal half-spread ───────────────────────────────────────────────
//
//   half_spread = (γ·σ²·τ)/2 + (1/γ)·ln(1 + γ/k)
//
// For reference / logging. Not used directly in threshold because the second
// term is calibrated for market-making order placement, not arb thresholds.

pub fn as_half_spread(variance: f64, gamma: f64, tau: f64, k: f64) -> f64 {
    let vol_term = gamma * variance * tau / 2.0;
    let fill_term = (1.0 / gamma) * (1.0 + gamma / k).ln();
    vol_term + fill_term
}

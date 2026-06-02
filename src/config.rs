use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub trading: TradingConfig,
    pub risk: RiskConfig,
    pub binance: ExchangeConfig,
    pub bybit: ExchangeConfig,
    pub mexc: ExchangeConfig,
    #[serde(default)]
    pub okx: OkxConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradingConfig {
    pub symbol: String,
    pub quote: String,
    pub min_spread_pct: Decimal,
    pub trade_size_usdt: Decimal,
    pub max_slippage_pct: Decimal,
    pub paper_trading: bool,

    // ── Avellaneda-Stoikov 2008 ─────────────────────────────────────────────
    // gamma: risk aversion calibrated for fractional prices (not fp units).
    //   50 → ~5 bps penalty at normal vol, ~125 bps at high vol.
    #[serde(default = "default_gamma")]
    pub gamma: f64,
    // k: fill-intensity parameter  lambda(d) = A·exp(−k·d)
    #[serde(default = "default_k")]
    pub k: f64,
    // tau: rolling time-horizon constant (dimensionless)
    #[serde(default = "default_tau")]
    pub tau: f64,
    // Imbalance magnitude above which we skip the signal.
    //   buy_imbalance > threshold → price rising, we overpay.
    //   sell_imbalance < -threshold → price falling, we undersell.
    #[serde(default = "default_imbalance_threshold")]
    pub imbalance_threshold: f64,
}

fn default_gamma() -> f64 { 50.0 }
fn default_k() -> f64 { 1.5 }
fn default_tau() -> f64 { 1.0 }
fn default_imbalance_threshold() -> f64 { 0.8 }

#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    pub max_position_usdt: Decimal,
    pub max_daily_loss_usdt: Decimal,
    pub max_open_trades: usize,
    pub cooldown_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeConfig {
    pub api_key: String,
    pub api_secret: String,
    pub testnet: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OkxConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
    #[serde(default)]
    pub passphrase: String,
    #[serde(default)]
    pub testnet: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let contents = std::fs::read_to_string("config.toml")
            .context("config.toml not found")?;
        let mut cfg: Config = toml::from_str(&contents)
            .context("Failed to parse config.toml")?;

        if let Ok(v) = std::env::var("BINANCE_API_KEY")    { cfg.binance.api_key    = v; }
        if let Ok(v) = std::env::var("BINANCE_API_SECRET") { cfg.binance.api_secret = v; }
        if let Ok(v) = std::env::var("BYBIT_API_KEY")      { cfg.bybit.api_key      = v; }
        if let Ok(v) = std::env::var("BYBIT_API_SECRET")   { cfg.bybit.api_secret   = v; }
        if let Ok(v) = std::env::var("MEXC_API_KEY")       { cfg.mexc.api_key       = v; }
        if let Ok(v) = std::env::var("MEXC_API_SECRET")    { cfg.mexc.api_secret    = v; }
        if let Ok(v) = std::env::var("OKX_API_KEY")        { cfg.okx.api_key        = v; }
        if let Ok(v) = std::env::var("OKX_API_SECRET")     { cfg.okx.api_secret     = v; }
        if let Ok(v) = std::env::var("OKX_PASSPHRASE")     { cfg.okx.passphrase     = v; }

        Ok(cfg)
    }

    pub fn pair(&self) -> String {
        format!("{}{}", self.trading.symbol, self.trading.quote)
    }
}

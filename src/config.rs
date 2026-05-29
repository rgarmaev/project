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
}

#[derive(Debug, Clone, Deserialize)]
pub struct TradingConfig {
    pub symbol: String,
    pub quote: String,
    pub min_spread_pct: Decimal,
    pub trade_size_usdt: Decimal,
    pub max_slippage_pct: Decimal,
    pub paper_trading: bool,
}

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

        Ok(cfg)
    }

    pub fn pair(&self) -> String {
        format!("{}{}", self.trading.symbol, self.trading.quote)
    }
}

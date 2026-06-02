use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::dashboard::state::DashboardState;

#[derive(Serialize, Deserialize, Clone)]
pub struct ExchangeSettings {
    pub api_key: String,
    pub api_secret: String,
    pub testnet: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OkxSettings {
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: String,
    pub testnet: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BingxSettings {
    pub api_key: String,
    pub api_secret: String,
}

#[derive(Serialize, Deserialize)]
pub struct ConfigPayload {
    pub paper_trading: bool,
    pub trade_size_usdt: f64,
    pub min_spread_pct: f64,
    pub symbol: String,
    pub binance: ExchangeSettings,
    pub bybit: ExchangeSettings,
    pub mexc: ExchangeSettings,
    pub okx: OkxSettings,
    pub bingx: BingxSettings,
}

pub async fn get_config(
    State(_state): State<Arc<DashboardState>>,
) -> Json<ConfigPayload> {
    // Read .env if exists, mask secrets
    let env_path = ".env";
    let env_vars = read_env_file(env_path);

    let binance_key    = env_vars.get("BINANCE_API_KEY").cloned().unwrap_or_default();
    let binance_secret = env_vars.get("BINANCE_API_SECRET").cloned().unwrap_or_default();
    let bybit_key      = env_vars.get("BYBIT_API_KEY").cloned().unwrap_or_default();
    let bybit_secret   = env_vars.get("BYBIT_API_SECRET").cloned().unwrap_or_default();
    let mexc_key       = env_vars.get("MEXC_API_KEY").cloned().unwrap_or_default();
    let mexc_secret    = env_vars.get("MEXC_API_SECRET").cloned().unwrap_or_default();
    let okx_key        = env_vars.get("OKX_API_KEY").cloned().unwrap_or_default();
    let okx_secret     = env_vars.get("OKX_API_SECRET").cloned().unwrap_or_default();
    let okx_pass       = env_vars.get("OKX_PASSPHRASE").cloned().unwrap_or_default();
    let bingx_key      = env_vars.get("BINGX_API_KEY").cloned().unwrap_or_default();
    let bingx_secret   = env_vars.get("BINGX_API_SECRET").cloned().unwrap_or_default();

    // Read config.toml for trading params
    let (paper, size, spread, bin_testnet, bybit_testnet, symbol) = read_config_toml();

    Json(ConfigPayload {
        paper_trading: paper,
        trade_size_usdt: size,
        min_spread_pct: spread,
        symbol,
        binance: ExchangeSettings {
            api_key:    mask(&binance_key),
            api_secret: mask(&binance_secret),
            testnet:    bin_testnet,
        },
        bybit: ExchangeSettings {
            api_key:    mask(&bybit_key),
            api_secret: mask(&bybit_secret),
            testnet:    bybit_testnet,
        },
        mexc: ExchangeSettings {
            api_key:    mask(&mexc_key),
            api_secret: mask(&mexc_secret),
            testnet:    false,
        },
        okx: OkxSettings {
            api_key:    mask(&okx_key),
            api_secret: mask(&okx_secret),
            passphrase: mask(&okx_pass),
            testnet:    false,
        },
        bingx: BingxSettings {
            api_key:    mask(&bingx_key),
            api_secret: mask(&bingx_secret),
        },
    })
}

pub async fn post_config(
    State(_state): State<Arc<DashboardState>>,
    Json(payload): Json<ConfigPayload>,
) -> Json<serde_json::Value> {
    // Write API keys to .env (only if not masked)
    let existing = read_env_file(".env");

    let binance_key = if payload.binance.api_key.contains('*') {
        existing.get("BINANCE_API_KEY").cloned().unwrap_or_default()
    } else { payload.binance.api_key.clone() };
    let binance_secret = if payload.binance.api_secret.contains('*') {
        existing.get("BINANCE_API_SECRET").cloned().unwrap_or_default()
    } else { payload.binance.api_secret.clone() };
    let bybit_key = if payload.bybit.api_key.contains('*') {
        existing.get("BYBIT_API_KEY").cloned().unwrap_or_default()
    } else { payload.bybit.api_key.clone() };
    let bybit_secret = if payload.bybit.api_secret.contains('*') {
        existing.get("BYBIT_API_SECRET").cloned().unwrap_or_default()
    } else { payload.bybit.api_secret.clone() };
    let mexc_key = if payload.mexc.api_key.contains('*') {
        existing.get("MEXC_API_KEY").cloned().unwrap_or_default()
    } else { payload.mexc.api_key.clone() };
    let mexc_secret = if payload.mexc.api_secret.contains('*') {
        existing.get("MEXC_API_SECRET").cloned().unwrap_or_default()
    } else { payload.mexc.api_secret.clone() };

    let okx_key = if payload.okx.api_key.contains('*') {
        existing.get("OKX_API_KEY").cloned().unwrap_or_default()
    } else { payload.okx.api_key.clone() };
    let okx_secret = if payload.okx.api_secret.contains('*') {
        existing.get("OKX_API_SECRET").cloned().unwrap_or_default()
    } else { payload.okx.api_secret.clone() };
    let okx_pass = if payload.okx.passphrase.contains('*') {
        existing.get("OKX_PASSPHRASE").cloned().unwrap_or_default()
    } else { payload.okx.passphrase.clone() };
    let bingx_key = if payload.bingx.api_key.contains('*') {
        existing.get("BINGX_API_KEY").cloned().unwrap_or_default()
    } else { payload.bingx.api_key.clone() };
    let bingx_secret = if payload.bingx.api_secret.contains('*') {
        existing.get("BINGX_API_SECRET").cloned().unwrap_or_default()
    } else { payload.bingx.api_secret.clone() };

    let env_content = format!(
        "BINANCE_API_KEY={}\nBINANCE_API_SECRET={}\n\nBYBIT_API_KEY={}\nBYBIT_API_SECRET={}\n\nMEXC_API_KEY={}\nMEXC_API_SECRET={}\n\nOKX_API_KEY={}\nOKX_API_SECRET={}\nOKX_PASSPHRASE={}\n\nBINGX_API_KEY={}\nBINGX_API_SECRET={}\n\nRUST_LOG=sol_arb=info\n",
        binance_key, binance_secret,
        bybit_key, bybit_secret,
        mexc_key, mexc_secret,
        okx_key, okx_secret, okx_pass,
        bingx_key, bingx_secret,
    );

    if let Err(e) = std::fs::write(".env", &env_content) {
        return Json(serde_json::json!({"ok": false, "error": e.to_string()}));
    }

    // Update config.toml trading section
    if let Err(e) = update_config_toml(&payload) {
        return Json(serde_json::json!({"ok": false, "error": e.to_string()}));
    }

    Json(serde_json::json!({"ok": true, "message": "Saved. Restart bot to apply API keys."}))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn mask(s: &str) -> String {
    if s.is_empty() { return String::new(); }
    if s.len() <= 4 { return "*".repeat(s.len()); }
    format!("{}{}{}",  &s[..2], "*".repeat(s.len() - 4), &s[s.len()-2..])
}

fn read_env_file(path: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Ok(content) = std::fs::read_to_string(path) else { return map; };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() { continue; }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

fn read_config_toml() -> (bool, f64, f64, bool, bool, String) {
    let Ok(content) = std::fs::read_to_string("config.toml") else {
        return (true, 200.0, 0.00005, false, false, "SOLUSDT".to_string());
    };
    let paper = content.contains("paper_trading = true");
    let size   = extract_f64(&content, "trade_size_usdt").unwrap_or(200.0);
    let spread = extract_f64(&content, "min_spread_pct").unwrap_or(0.00005);
    let bin_testnet   = section_bool(&content, "[binance]", "testnet = true");
    let bybit_testnet = section_bool(&content, "[bybit]",   "testnet = true");
    let sym   = extract_str_in_section(&content, "[trading]", "symbol").unwrap_or("SOL".to_string());
    let quote = extract_str_in_section(&content, "[trading]", "quote").unwrap_or("USDT".to_string());
    (paper, size, spread, bin_testnet, bybit_testnet, format!("{}{}", sym, quote))
}

fn extract_f64(content: &str, key: &str) -> Option<f64> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with(key) {
            if let Some(val) = line.split('=').nth(1) {
                return val.trim().trim_matches('"').parse().ok();
            }
        }
    }
    None
}

fn extract_str_in_section(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == section;
            continue;
        }
        if in_section && line.starts_with(key) && line.contains('=') {
            if let Some(val) = line.split('=').nth(1) {
                return Some(val.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn section_bool(content: &str, section: &str, key: &str) -> bool {
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') { in_section = line == section; }
        if in_section && line.contains(key) { return true; }
    }
    false
}

fn update_config_toml(payload: &ConfigPayload) -> anyhow::Result<()> {
    let content = std::fs::read_to_string("config.toml")?;
    let mut lines: Vec<String> = content.lines().map(String::from).collect();

    let (sym, quote) = payload.symbol.strip_suffix("USDT")
        .map(|s| (s.to_string(), "USDT".to_string()))
        .ok_or_else(|| anyhow::anyhow!("unrecognised quote currency in '{}'", payload.symbol))?;

    let mut current_section = String::new();
    for line in &mut lines {
        let t = line.trim();
        if t.starts_with('[') {
            current_section = t.to_string();
        }
        if t.starts_with("paper_trading") {
            *line = format!("paper_trading = {}", payload.paper_trading);
        } else if t.starts_with("trade_size_usdt") {
            *line = format!("trade_size_usdt  = \"{}\"", payload.trade_size_usdt);
        } else if t.starts_with("min_spread_pct") {
            *line = format!("min_spread_pct   = \"{}\"", payload.min_spread_pct);
        } else if current_section == "[trading]" && t.starts_with("symbol") {
            *line = format!("symbol = \"{}\"", sym);
        } else if current_section == "[trading]" && t.starts_with("quote") {
            *line = format!("quote  = \"{}\"", quote);
        }
    }

    // Update testnet flags per section
    let mut section = String::new();
    for line in &mut lines {
        let t = line.trim();
        if t.starts_with('[') { section = t.to_string(); }
        if t.starts_with("testnet") {
            let val = match section.as_str() {
                "[binance]" => payload.binance.testnet,
                "[bybit]"   => payload.bybit.testnet,
                _           => false,
            };
            *line = format!("testnet    = {}", val);
        }
    }

    std::fs::write("config.toml", lines.join("\n") + "\n")?;
    Ok(())
}

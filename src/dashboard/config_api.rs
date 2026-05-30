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

#[derive(Serialize, Deserialize)]
pub struct ConfigPayload {
    pub paper_trading: bool,
    pub trade_size_usdt: f64,
    pub min_spread_pct: f64,
    pub binance: ExchangeSettings,
    pub bybit: ExchangeSettings,
    pub mexc: ExchangeSettings,
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

    // Read config.toml for trading params
    let (paper, size, spread, bin_testnet, bybit_testnet) = read_config_toml();

    Json(ConfigPayload {
        paper_trading: paper,
        trade_size_usdt: size,
        min_spread_pct: spread,
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

    let env_content = format!(
        "BINANCE_API_KEY={}\nBINANCE_API_SECRET={}\n\nBYBIT_API_KEY={}\nBYBIT_API_SECRET={}\n\nMEXC_API_KEY={}\nMEXC_API_SECRET={}\n\nRUST_LOG=sol_arb=info\n",
        binance_key, binance_secret,
        bybit_key, bybit_secret,
        mexc_key, mexc_secret,
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

fn read_config_toml() -> (bool, f64, f64, bool, bool) {
    let Ok(content) = std::fs::read_to_string("config.toml") else {
        return (true, 200.0, 0.00005, false, false);
    };
    let paper = content.contains("paper_trading = true");
    let size = extract_f64(&content, "trade_size_usdt").unwrap_or(200.0);
    let spread = extract_f64(&content, "min_spread_pct").unwrap_or(0.00005);
    // testnet lines: look in [binance] section
    let bin_testnet = section_bool(&content, "[binance]", "testnet = true");
    let bybit_testnet = section_bool(&content, "[bybit]", "testnet = true");
    (paper, size, spread, bin_testnet, bybit_testnet)
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

    for line in &mut lines {
        let t = line.trim();
        if t.starts_with("paper_trading") {
            *line = format!("paper_trading = {}", payload.paper_trading);
        } else if t.starts_with("trade_size_usdt") {
            *line = format!("trade_size_usdt  = \"{}\"", payload.trade_size_usdt);
        } else if t.starts_with("min_spread_pct") {
            *line = format!("min_spread_pct   = \"{}\"", payload.min_spread_pct);
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

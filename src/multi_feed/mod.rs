use dashmap::DashMap;
use futures_util::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::tickers::TICKERS;

pub struct MultiPairTick {
    pub spot_binance: Option<(f64, f64)>,
    pub perp_binance: Option<(f64, f64)>,
    pub spot_bybit:   Option<(f64, f64)>,
    pub perp_bybit:   Option<(f64, f64)>,
    pub updated_at:   Instant,
}

pub type MultiPairState = Arc<DashMap<String, MultiPairTick>>;

pub fn new_state() -> MultiPairState {
    Arc::new(DashMap::new())
}

// ── Binance feeds ─────────────────────────────────────────────────────────────

pub async fn run_binance_spot(state: MultiPairState) {
    run_binance(state, "wss://stream.binance.com:9443/ws/!bookTicker", false).await;
}

pub async fn run_binance_perp(state: MultiPairState) {
    run_binance(state, "wss://fstream.binance.com/ws/!bookTicker", true).await;
}

async fn run_binance(state: MultiPairState, url: &'static str, is_perp: bool) {
    let label = if is_perp { "perp" } else { "spot" };
    let valid: HashSet<&str> = TICKERS.iter().copied().collect();
    loop {
        match connect_binance_once(&state, url, is_perp, &valid).await {
            Ok(())  => warn!("multi_feed: Binance {} closed",        label),
            Err(e)  => warn!("multi_feed: Binance {} error: {:#}",   label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn connect_binance_once(
    state: &MultiPairState,
    url: &str,
    is_perp: bool,
    valid: &HashSet<&str>,
) -> anyhow::Result<()> {
    let (ws, _) = connect_async(url).await?;
    let (_, mut read) = ws.split();
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let sym = v["s"].as_str().unwrap_or("");
                if !valid.contains(sym) { continue; }
                let bid = v["b"].as_str().and_then(|s| s.parse::<f64>().ok());
                let ask = v["a"].as_str().and_then(|s| s.parse::<f64>().ok());
                if let (Some(bid), Some(ask)) = (bid, ask) {
                    if bid > 0.0 && ask > 0.0 {
                        state.entry(sym.to_string())
                            .and_modify(|t| {
                                if is_perp { t.perp_binance = Some((bid, ask)); }
                                else       { t.spot_binance = Some((bid, ask)); }
                                t.updated_at = Instant::now();
                            })
                            .or_insert_with(|| MultiPairTick {
                                spot_binance: if !is_perp { Some((bid, ask)) } else { None },
                                perp_binance: if  is_perp { Some((bid, ask)) } else { None },
                                spot_bybit:   None,
                                perp_bybit:   None,
                                updated_at:   Instant::now(),
                            });
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    Ok(())
}

// ── Bybit feeds (stubs — implemented in Task 4) ───────────────────────────────

pub async fn run_bybit_spot(_state: MultiPairState)   { todo!() }
pub async fn run_bybit_linear(_state: MultiPairState) { todo!() }

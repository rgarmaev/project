use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::tickers::TICKERS;
use super::{MarketQuote, MultiPairState, MultiPairTick};

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
    let (mut write, mut read) = ws.split();
    while let Some(msg) = read.next().await {
        match msg? {
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
                        let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
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
                                spot_okx:     None,
                                perp_okx:     None,
                                spot_bingx:   None,
                                perp_bingx:   None,
                                updated_at:   Instant::now(),
                            });
                    }
                }
            }
            Message::Ping(d) => { let _ = write.send(Message::Pong(d)).await; }
            Message::Close(_) => break,
            _ => {}
        }
    }
    Ok(())
}

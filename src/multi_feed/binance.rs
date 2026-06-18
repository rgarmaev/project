use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use crate::tickers::TICKERS;
use super::{MarketQuote, MultiPairState, MultiPairTick};

pub async fn run_binance_spot(state: MultiPairState) {
    loop {
        match connect_binance_spot_once(&state).await {
            Ok(())  => warn!("multi_feed: Binance spot closed"),
            Err(e)  => warn!("multi_feed: Binance spot error: {:#}", e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub async fn run_binance_perp(state: MultiPairState) {
    run_binance_stream(state, "wss://fstream.binance.com/ws/!bookTicker", true).await;
}

// Spot: use combined stream + SUBSCRIBE — !bookTicker on port 443 connects but sends nothing
async fn connect_binance_spot_once(state: &MultiPairState) -> anyhow::Result<()> {
    let url = "wss://stream.binance.com/stream";
    info!("multi_feed: Binance spot connecting → {}", url);
    let (ws, _) = timeout(Duration::from_secs(10), connect_async(url)).await
        .map_err(|_| anyhow::anyhow!("connect timeout"))??;
    info!("multi_feed: Binance spot connected");
    let (mut write, mut read) = ws.split();

    let params: Vec<String> = TICKERS.iter()
        .map(|s| format!("{}@bookTicker", s.to_lowercase()))
        .collect();
    for (i, chunk) in params.chunks(200).enumerate() {
        let sub = serde_json::json!({"method":"SUBSCRIBE","params":chunk,"id":i+1});
        write.send(Message::Text(sub.to_string())).await?;
    }

    let valid: HashSet<&str> = TICKERS.iter().copied().collect();
    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                // Combined stream wraps payload: {"stream":"...","data":{...}}
                let data = if v["stream"].is_string() { &v["data"] } else { &v };
                let sym = data["s"].as_str().unwrap_or("");
                if !valid.contains(sym) { continue; }
                let bid     = data["b"].as_str().and_then(|s| s.parse::<f64>().ok());
                let ask     = data["a"].as_str().and_then(|s| s.parse::<f64>().ok());
                let bid_qty = data["B"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let ask_qty = data["A"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                if let (Some(bid), Some(ask)) = (bid, ask) {
                    if bid > 0.0 && ask > 0.0 {
                        let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
                        state.entry(sym.to_string())
                            .and_modify(|t| { t.spot_binance = Some(quote.clone()); t.updated_at = Instant::now(); })
                            .or_insert_with(|| { let mut t = super::blank_tick(); t.spot_binance = Some(quote.clone()); t });
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

// Perp: !bookTicker push stream works on fstream.binance.com
async fn run_binance_stream(state: MultiPairState, url: &'static str, is_perp: bool) {
    let valid: HashSet<&str> = TICKERS.iter().copied().collect();
    loop {
        match connect_binance_once(&state, url, is_perp, &valid).await {
            Ok(())  => warn!("multi_feed: Binance perp closed"),
            Err(e)  => warn!("multi_feed: Binance perp error: {:#}", e),
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
    info!("multi_feed: Binance perp connecting → {}", url);
    let (ws, _) = timeout(Duration::from_secs(10), connect_async(url)).await
        .map_err(|_| anyhow::anyhow!("connect timeout"))??;
    info!("multi_feed: Binance perp connected");
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
                            .or_insert_with(|| {
                                let mut t = super::blank_tick();
                                if is_perp { t.perp_binance = Some(quote.clone()); }
                                else       { t.spot_binance = Some(quote.clone()); }
                                t
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

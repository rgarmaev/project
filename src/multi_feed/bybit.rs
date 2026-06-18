use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn};

use crate::tickers::TICKERS;
use super::{MarketQuote, MultiPairState, MultiPairTick};

pub async fn run_bybit_spot(state: MultiPairState) {
    run_bybit(state, "wss://stream.bybit.com/v5/public/spot", false).await;
}

pub async fn run_bybit_linear(state: MultiPairState) {
    run_bybit(state, "wss://stream.bybit.com/v5/public/linear", true).await;
}

async fn run_bybit(state: MultiPairState, url: &'static str, is_perp: bool) {
    let label = if is_perp { "linear" } else { "spot" };
    let valid: HashSet<&str> = TICKERS.iter().copied().collect();
    loop {
        match connect_bybit_once(&state, url, is_perp, &valid).await {
            Ok(())  => warn!("multi_feed: Bybit {} closed",      label),
            Err(e)  => warn!("multi_feed: Bybit {} error: {:#}", label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn connect_bybit_once(
    state: &MultiPairState,
    url: &str,
    is_perp: bool,
    valid: &HashSet<&str>,
) -> anyhow::Result<()> {
    info!("multi_feed: Bybit {} connecting → {}", if is_perp { "linear" } else { "spot" }, url);
    let (ws, _) = timeout(Duration::from_secs(10), connect_async(url)).await
        .map_err(|_| anyhow::anyhow!("connect timeout"))??;
    info!("multi_feed: Bybit {} connected", if is_perp { "linear" } else { "spot" });
    let (mut write, mut read) = ws.split();

    // Spot: orderbook.1 sends immediate snapshots (tickers only sends deltas, no initial snapshot)
    // Linear: tickers sends snapshot + delta with bid1Price/ask1Price
    let topic_prefix = if is_perp { "tickers." } else { "orderbook.1." };
    let args: Vec<String> = TICKERS.iter().map(|s| format!("{}{}", topic_prefix, s)).collect();
    for chunk in args.chunks(10) {
        let sub = serde_json::json!({ "op": "subscribe", "args": chunk });
        write.send(Message::Text(sub.to_string())).await?;
    }

    let mut ping_tick = interval(Duration::from_secs(20));
    ping_tick.tick().await;

    loop {
        select! {
            msg = read.next() => {
                let msg = match msg {
                    Some(Ok(m))  => m,
                    Some(Err(e)) => return Err(e.into()),
                    None         => break,
                };
                match msg {
                    Message::Text(text) => {
                        let v: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let topic = v["topic"].as_str().unwrap_or("");
                        if !topic.starts_with(topic_prefix) { continue; }
                        let sym = &topic[topic_prefix.len()..];
                        if !valid.contains(sym) { continue; }
                        let (bid, ask, bid_qty, ask_qty) = if is_perp {
                            // tickers: bid1Price/ask1Price
                            let data = &v["data"];
                            let bid     = data["bid1Price"].as_str().and_then(|s| s.parse::<f64>().ok());
                            let ask     = data["ask1Price"].as_str().and_then(|s| s.parse::<f64>().ok());
                            let bid_qty = data["bid1Size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            let ask_qty = data["ask1Size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            (bid, ask, bid_qty, ask_qty)
                        } else {
                            // orderbook.1: data.b[0] = [price, qty], data.a[0] = [price, qty]
                            let data = &v["data"];
                            let bid     = data["b"][0][0].as_str().and_then(|s| s.parse::<f64>().ok());
                            let ask     = data["a"][0][0].as_str().and_then(|s| s.parse::<f64>().ok());
                            let bid_qty = data["b"][0][1].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            let ask_qty = data["a"][0][1].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            (bid, ask, bid_qty, ask_qty)
                        };
                        if let (Some(bid), Some(ask)) = (bid, ask) {
                            if bid > 0.0 && ask > 0.0 {
                                let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
                                state.entry(sym.to_string())
                                    .and_modify(|t| {
                                        if is_perp { t.perp_bybit = Some(quote.clone()); }
                                        else       { t.spot_bybit = Some(quote.clone()); }
                                        t.updated_at = Instant::now();
                                    })
                                    .or_insert_with(|| {
                                        let mut t = super::blank_tick();
                                        if is_perp { t.perp_bybit = Some(quote.clone()); }
                                        else       { t.spot_bybit = Some(quote.clone()); }
                                        t
                                    });
                            }
                        }
                    }
                    Message::Ping(d) => { write.send(Message::Pong(d)).await?; }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = ping_tick.tick() => {
                write.send(Message::Text(r#"{"op":"ping"}"#.to_string())).await?;
            }
        }
    }
    Ok(())
}

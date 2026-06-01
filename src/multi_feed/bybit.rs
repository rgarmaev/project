use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

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
    let (ws, _) = connect_async(url).await?;
    let (mut write, mut read) = ws.split();

    let args: Vec<String> = TICKERS.iter().map(|s| format!("tickers.{}", s)).collect();
    let sub = serde_json::json!({ "op": "subscribe", "args": args });
    write.send(Message::Text(sub.to_string())).await?;

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
                        if !topic.starts_with("tickers.") { continue; }
                        let sym = &topic["tickers.".len()..];
                        if !valid.contains(sym) { continue; }
                        let data = &v["data"];
                        let bid     = data["bid1Price"].as_str().and_then(|s| s.parse::<f64>().ok());
                        let ask     = data["ask1Price"].as_str().and_then(|s| s.parse::<f64>().ok());
                        let bid_qty = data["bid1Size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        let ask_qty = data["ask1Size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        if let (Some(bid), Some(ask)) = (bid, ask) {
                            if bid > 0.0 && ask > 0.0 {
                                let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
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
                                        spot_okx:     None,
                                        perp_okx:     None,
                                        spot_bingx:   None,
                                        perp_bingx:   None,
                                        updated_at:   Instant::now(),
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

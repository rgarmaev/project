use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::tickers::TICKERS;
use super::{to_underscored, MarketQuote, MultiPairState};

const GATE_SPOT_WS: &str    = "wss://api.gateio.ws/ws/v4/";
const GATE_FUTURES_WS: &str = "wss://fx-ws.gateio.ws/v4/ws/usdt";

pub async fn run_gate_spot(state: MultiPairState) {
    run_gate(state, false).await;
}

pub async fn run_gate_futures(state: MultiPairState) {
    run_gate(state, true).await;
}

async fn run_gate(state: MultiPairState, is_futures: bool) {
    let label = if is_futures { "futures" } else { "spot" };
    let url   = if is_futures { GATE_FUTURES_WS } else { GATE_SPOT_WS };

    let sym_map: HashMap<String, String> = TICKERS.iter()
        .map(|&t| (to_underscored(t), t.to_string()))
        .collect();

    loop {
        match connect_gate_once(&state, url, is_futures, &sym_map).await {
            Ok(())  => warn!("multi_feed: Gate.io {} closed",      label),
            Err(e)  => warn!("multi_feed: Gate.io {} error: {:#}", label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn connect_gate_once(
    state: &MultiPairState,
    url: &str,
    is_futures: bool,
    sym_map: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let (ws, _) = timeout(Duration::from_secs(10), connect_async(url)).await
        .map_err(|_| anyhow::anyhow!("connect timeout"))??;
    let (mut write, mut read) = ws.split();

    // Build payload list of "BTC_USDT", "ETH_USDT", ...
    let payload: Vec<String> = TICKERS.iter().map(|&t| to_underscored(t)).collect();
    let channel = if is_futures { "futures.book_ticker" } else { "spot.book_ticker" };
    let sub = serde_json::json!({
        "time":    Utc::now().timestamp() as u64,
        "channel": channel,
        "event":   "subscribe",
        "payload": payload,
    });
    write.send(Message::Text(sub.to_string())).await?;

    let ping_channel = if is_futures { "futures.ping" } else { "spot.ping" };
    let mut ping_tick = interval(Duration::from_secs(10));
    ping_tick.tick().await; // consume the immediate tick

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
                            Ok(v)  => v,
                            Err(_) => continue,
                        };
                        // Only process update events on the book_ticker channel
                        if v["event"].as_str().unwrap_or("") != "update" { continue; }
                        let result = &v["result"];
                        let gate_sym = result["s"].as_str().unwrap_or("");
                        if gate_sym.is_empty() { continue; }
                        let sym = match sym_map.get(gate_sym) {
                            Some(s) => s.clone(),
                            None    => continue,
                        };
                        let bid     = result["b"].as_str().and_then(|s| s.parse::<f64>().ok());
                        let ask     = result["a"].as_str().and_then(|s| s.parse::<f64>().ok());
                        let bid_qty = result["B"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        let ask_qty = result["A"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        if let (Some(bid), Some(ask)) = (bid, ask) {
                            if bid > 0.0 && ask > 0.0 {
                                let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
                                state.entry(sym)
                                    .and_modify(|t| {
                                        if is_futures { t.perp_gate = Some(quote.clone()); }
                                        else          { t.spot_gate = Some(quote.clone()); }
                                        t.updated_at = Instant::now();
                                    })
                                    .or_insert_with(|| {
                                        let mut t = super::blank_tick();
                                        if is_futures { t.perp_gate = Some(quote.clone()); }
                                        else          { t.spot_gate = Some(quote.clone()); }
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
                let ping = serde_json::json!({
                    "time":    Utc::now().timestamp() as u64,
                    "channel": ping_channel,
                    "event":   "",
                });
                write.send(Message::Text(ping.to_string())).await?;
            }
        }
    }
    Ok(())
}

use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::tickers::TICKERS;
use super::{to_dashed, MarketQuote, MultiPairState, MultiPairTick};

const OKX_WS: &str = "wss://ws.okx.com:8443/ws/v5/public";

pub async fn run_okx_spot(state: MultiPairState) {
    run_okx(state, false).await;
}

pub async fn run_okx_swap(state: MultiPairState) {
    run_okx(state, true).await;
}

async fn run_okx(state: MultiPairState, is_swap: bool) {
    let label = if is_swap { "swap" } else { "spot" };
    let inst_map: HashMap<String, String> = TICKERS.iter()
        .map(|&t| {
            let inst_id = if is_swap {
                to_dashed(t) + "-SWAP"
            } else {
                to_dashed(t)
            };
            (inst_id, t.to_string())
        })
        .collect();

    let args: Vec<serde_json::Value> = inst_map.keys()
        .map(|id| serde_json::json!({"channel": "tickers", "instId": id}))
        .collect();
    let sub_msg = serde_json::json!({"op": "subscribe", "args": args}).to_string();

    loop {
        match connect_okx_once(&state, &sub_msg, is_swap, &inst_map).await {
            Ok(())  => warn!("multi_feed: OKX {} closed",      label),
            Err(e)  => warn!("multi_feed: OKX {} error: {:#}", label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn connect_okx_once(
    state: &MultiPairState,
    sub_msg: &str,
    is_swap: bool,
    inst_map: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let (ws, _) = timeout(Duration::from_secs(10), connect_async(OKX_WS)).await
        .map_err(|_| anyhow::anyhow!("connect timeout"))??;
    let (mut write, mut read) = ws.split();

    write.send(Message::Text(sub_msg.to_string())).await?;

    let mut ping_tick = interval(Duration::from_secs(25));
    ping_tick.tick().await; // consume immediate tick

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
                        if text == "pong" { continue; }
                        let v: serde_json::Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        let inst_id = v["arg"]["instId"].as_str().unwrap_or("");
                        let sym = match inst_map.get(inst_id) {
                            Some(s) => s.clone(),
                            None    => continue,
                        };
                        let data = match v["data"].as_array().and_then(|a| a.first()) {
                            Some(d) => d,
                            None    => continue,
                        };
                        let bid     = data["bidPx"].as_str().and_then(|s| s.parse::<f64>().ok());
                        let ask     = data["askPx"].as_str().and_then(|s| s.parse::<f64>().ok());
                        let bid_qty = data["bidSz"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        let ask_qty = data["askSz"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                        if let (Some(bid), Some(ask)) = (bid, ask) {
                            if bid > 0.0 && ask > 0.0 {
                                let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
                                state.entry(sym)
                                    .and_modify(|t| {
                                        if is_swap { t.perp_okx = Some(quote.clone()); }
                                        else       { t.spot_okx = Some(quote.clone()); }
                                        t.updated_at = Instant::now();
                                    })
                                    .or_insert_with(|| {
                                        let mut t = super::blank_tick();
                                        if is_swap { t.perp_okx = Some(quote.clone()); }
                                        else       { t.spot_okx = Some(quote.clone()); }
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
                write.send(Message::Text("ping".to_string())).await?;
            }
        }
    }
    Ok(())
}

use futures_util::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::tickers::TICKERS;
use super::{MarketQuote, MultiPairState};

const BITGET_WS: &str = "wss://ws.bitget.com/v2/ws/public";

pub async fn run_bitget_spot(state: MultiPairState) {
    run_bitget(state, false).await;
}

pub async fn run_bitget_futures(state: MultiPairState) {
    run_bitget(state, true).await;
}

async fn run_bitget(state: MultiPairState, is_futures: bool) {
    let label = if is_futures { "futures" } else { "spot" };
    let inst_type = if is_futures { "USDT-FUTURES" } else { "SPOT" };

    let sym_set: HashSet<&str> = TICKERS.iter().copied().collect();

    let args: Vec<serde_json::Value> = TICKERS.iter()
        .map(|&t| serde_json::json!({"instType": inst_type, "channel": "ticker", "instId": t}))
        .collect();
    let sub_msg = serde_json::json!({"op": "subscribe", "args": args}).to_string();

    loop {
        match connect_bitget_once(&state, &sub_msg, is_futures, &sym_set).await {
            Ok(())  => warn!("multi_feed: Bitget {} closed",      label),
            Err(e)  => warn!("multi_feed: Bitget {} error: {:#}", label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn connect_bitget_once(
    state: &MultiPairState,
    sub_msg: &str,
    is_futures: bool,
    sym_set: &HashSet<&str>,
) -> anyhow::Result<()> {
    let (ws, _) = connect_async(BITGET_WS).await?;
    let (mut write, mut read) = ws.split();

    write.send(Message::Text(sub_msg.to_string())).await?;

    let mut ping_tick = interval(Duration::from_secs(30));
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
                        let sym = v["arg"]["instId"].as_str().unwrap_or("");
                        if !sym_set.contains(sym) { continue; }
                        let sym = sym.to_string();

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
                                        if is_futures { t.perp_bitget = Some(quote.clone()); }
                                        else          { t.spot_bitget = Some(quote.clone()); }
                                        t.updated_at = Instant::now();
                                    })
                                    .or_insert_with(|| {
                                        let mut t = super::blank_tick();
                                        if is_futures { t.perp_bitget = Some(quote.clone()); }
                                        else          { t.spot_bitget = Some(quote.clone()); }
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

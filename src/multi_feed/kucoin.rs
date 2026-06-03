use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;
use uuid::Uuid;

use crate::tickers::TICKERS;
use super::{to_dashed, to_kucoin_futures, MarketQuote, MultiPairState};

const SPOT_TOKEN_URL:    &str = "https://api.kucoin.com/api/v1/bullet-public";
const FUTURES_TOKEN_URL: &str = "https://api-futures.kucoin.com/api/v1/bullet-public";

pub async fn run_kucoin_spot(state: MultiPairState) {
    run_kucoin(state, false).await;
}

pub async fn run_kucoin_futures(state: MultiPairState) {
    run_kucoin(state, true).await;
}

async fn run_kucoin(state: MultiPairState, is_futures: bool) {
    let label = if is_futures { "futures" } else { "spot" };

    // Build reverse lookup maps once
    let spot_map: HashMap<String, String> = TICKERS.iter()
        .map(|&t| (to_dashed(t), t.to_string()))
        .collect();
    let fut_map: HashMap<String, String> = TICKERS.iter()
        .map(|&t| (to_kucoin_futures(t), t.to_string()))
        .collect();

    loop {
        let (endpoint, token) = match get_ws_token(is_futures).await {
            Ok(v)  => v,
            Err(e) => {
                warn!("multi_feed: KuCoin {} token error: {:#}", label, e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let connect_id = Uuid::new_v4().to_string();
        let url = format!("{}?token={}&connectId={}", endpoint, token, connect_id);

        let sym_map = if is_futures { &fut_map } else { &spot_map };
        match connect_kucoin_once(&state, &url, is_futures, sym_map).await {
            Ok(())  => warn!("multi_feed: KuCoin {} closed",      label),
            Err(e)  => warn!("multi_feed: KuCoin {} error: {:#}", label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn get_ws_token(is_futures: bool) -> anyhow::Result<(String, String)> {
    let url = if is_futures { FUTURES_TOKEN_URL } else { SPOT_TOKEN_URL };
    let resp: serde_json::Value = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?
        .post(url)
        .header("Content-Type", "application/json")
        .body("{}")
        .send().await?
        .json().await?;

    let token = resp["data"]["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("KuCoin: missing token in bullet-public response"))?
        .to_string();

    let endpoint = resp["data"]["instanceServers"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|s| s["endpoint"].as_str())
        .ok_or_else(|| anyhow::anyhow!("KuCoin: missing instanceServers endpoint"))?
        .to_string();

    Ok((endpoint, token))
}

async fn connect_kucoin_once(
    state: &MultiPairState,
    url: &str,
    is_futures: bool,
    sym_map: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let (ws, _) = timeout(Duration::from_secs(10), connect_async(url)).await
        .map_err(|_| anyhow::anyhow!("KuCoin WS connect timeout"))??;
    let (mut write, mut read) = ws.split();

    // KuCoin topic string limit — send in batches of 50 symbols
    let symbols: Vec<String> = sym_map.keys().cloned().collect();
    let prefix = if is_futures { "/contractMarket/ticker" } else { "/market/ticker" };
    for chunk in symbols.chunks(50) {
        let topic = format!("{}:{}", prefix, chunk.join(","));
        let sub_msg = serde_json::json!({
            "id": Uuid::new_v4().to_string(),
            "type": "subscribe",
            "topic": topic,
            "privateChannel": false,
            "response": true
        }).to_string();
        write.send(Message::Text(sub_msg)).await?;
    }

    // KuCoin requires a ping every ~18-20 s
    let mut ping_tick = interval(Duration::from_secs(20));
    ping_tick.tick().await; // consume immediate tick

    // Topic prefix for stripping
    let strip_prefix = if is_futures { "/contractMarket/ticker:" } else { "/market/ticker:" };

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

                        let msg_type = v["type"].as_str().unwrap_or("");

                        // Ignore pong / ack / welcome
                        if msg_type != "message" { continue; }

                        let topic_str = v["topic"].as_str().unwrap_or("");
                        let kucoin_sym = match topic_str.strip_prefix(strip_prefix) {
                            Some(s) => s,
                            None    => continue,
                        };

                        let sym = match sym_map.get(kucoin_sym) {
                            Some(s) => s.clone(),
                            None    => continue,
                        };

                        let data = &v["data"];

                        let (bid, ask, bid_qty, ask_qty) = if is_futures {
                            let bid     = data["bestBidPrice"].as_str().and_then(|s| s.parse::<f64>().ok());
                            let ask     = data["bestAskPrice"].as_str().and_then(|s| s.parse::<f64>().ok());
                            let bid_qty = data["bestBidSize"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            let ask_qty = data["bestAskSize"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            (bid, ask, bid_qty, ask_qty)
                        } else {
                            let bid     = data["bestBid"].as_str().and_then(|s| s.parse::<f64>().ok());
                            let ask     = data["bestAsk"].as_str().and_then(|s| s.parse::<f64>().ok());
                            let bid_qty = data["bestBidSize"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            let ask_qty = data["bestAskSize"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            (bid, ask, bid_qty, ask_qty)
                        };

                        if let (Some(bid), Some(ask)) = (bid, ask) {
                            if bid > 0.0 && ask > 0.0 {
                                let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
                                state.entry(sym)
                                    .and_modify(|t| {
                                        if is_futures { t.perp_kucoin = Some(quote.clone()); }
                                        else          { t.spot_kucoin = Some(quote.clone()); }
                                        t.updated_at = Instant::now();
                                    })
                                    .or_insert_with(|| {
                                        let mut t = super::blank_tick();
                                        if is_futures { t.perp_kucoin = Some(quote.clone()); }
                                        else          { t.spot_kucoin = Some(quote.clone()); }
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
                    "id": Uuid::new_v4().to_string(),
                    "type": "ping"
                })
                .to_string();
                write.send(Message::Text(ping)).await?;
            }
        }
    }
    Ok(())
}

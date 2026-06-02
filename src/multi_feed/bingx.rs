use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::warn;

use crate::tickers::TICKERS;
use super::{to_dashed, MarketQuote, MultiPairState, MultiPairTick};

const BINGX_SPOT_WS: &str = "wss://open-api-ws.bingx.com/market";
const BINGX_SWAP_WS: &str = "wss://open-api-swap.bingx.com/swap-market";

pub async fn run_bingx_spot(state: MultiPairState) {
    run_bingx(state, false).await;
}

pub async fn run_bingx_swap(state: MultiPairState) {
    run_bingx(state, true).await;
}

async fn run_bingx(state: MultiPairState, is_swap: bool) {
    let label = if is_swap { "swap" } else { "spot" };
    let url   = if is_swap { BINGX_SWAP_WS } else { BINGX_SPOT_WS };
    let sym_map: HashMap<String, String> = TICKERS.iter()
        .map(|&t| (to_dashed(t), t.to_string()))
        .collect();
    loop {
        match connect_bingx_once(&state, url, is_swap, &sym_map).await {
            Ok(())  => warn!("multi_feed: BingX {} closed",      label),
            Err(e)  => warn!("multi_feed: BingX {} error: {:#}", label, e),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

async fn connect_bingx_once(
    state: &MultiPairState,
    url: &str,
    is_swap: bool,
    sym_map: &HashMap<String, String>,
) -> anyhow::Result<()> {
    let (ws, _) = connect_async(url).await?;
    let (mut write, mut read) = ws.split();

    for (idx, ticker) in TICKERS.iter().enumerate() {
        let data_type = format!("{}@bookTicker", to_dashed(ticker));
        let msg = serde_json::json!({
            "id": (idx + 1).to_string(),
            "dataType": data_type
        });
        write.send(Message::Text(msg.to_string())).await?;
    }

    while let Some(msg) = read.next().await {
        match msg? {
            Message::Text(text) => {
                let v: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let data_type = v["dataType"].as_str().unwrap_or("");
                let dashed = data_type.strip_suffix("@bookTicker").unwrap_or("");
                if dashed.is_empty() { continue; }
                let sym = match sym_map.get(dashed) {
                    Some(s) => s.clone(),
                    None    => continue,
                };
                let data = &v["data"];
                let bid     = data["bidPrice"].as_str().and_then(|s| s.parse::<f64>().ok());
                let ask     = data["askPrice"].as_str().and_then(|s| s.parse::<f64>().ok());
                let bid_qty = data["bidQty"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let ask_qty = data["askQty"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                if let (Some(bid), Some(ask)) = (bid, ask) {
                    if bid > 0.0 && ask > 0.0 {
                        let quote = MarketQuote { bid, ask, bid_qty, ask_qty, updated_at: Instant::now() };
                        state.entry(sym)
                            .and_modify(|t| {
                                if is_swap { t.perp_bingx = Some(quote.clone()); }
                                else       { t.spot_bingx = Some(quote.clone()); }
                                t.updated_at = Instant::now();
                            })
                            .or_insert_with(|| {
                                let mut t = super::blank_tick();
                                if is_swap { t.perp_bingx = Some(quote.clone()); }
                                else       { t.spot_bingx = Some(quote.clone()); }
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
    Ok(())
}

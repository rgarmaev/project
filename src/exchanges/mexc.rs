use anyhow::{bail, Result};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    config::Config,
    exchanges::{now_ms, sign_hmac_sha256},
    orderbook::SharedPriceState,
    types::{BookTicker, Exchange, MarketId, MarketType, OrderResult, Side},
};

// ── WebSocket shapes ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MexcWsMsg {
    c: Option<String>,
    d: Option<MexcBookTicker>,
}

#[derive(Deserialize)]
struct MexcBookTicker {
    #[serde(rename = "b")]
    bid: Option<String>,
    #[serde(rename = "B")]
    bid_qty: Option<String>,
    #[serde(rename = "a")]
    ask: Option<String>,
    #[serde(rename = "A")]
    ask_qty: Option<String>,
}

// ── Connector ────────────────────────────────────────────────────────────────

pub struct MexcConnector {
    config: Arc<Config>,
    http: Client,
}

impl MexcConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    async fn connect_once(&self, price_state: &SharedPriceState) -> Result<()> {
        let url = "wss://wbs.mexc.com/ws";
        let market_id = MarketId::new(Exchange::Mexc, MarketType::Spot);
        let pair = self.config.pair();

        info!("{} connecting → {}", market_id, url);
        let (ws, _) = connect_async(url).await?;
        let (mut write, mut read) = ws.split();

        let sub = serde_json::json!({
            "method": "SUBSCRIPTION",
            "params": [format!("spot@public.bookTicker.v3.api@{}", pair)]
        });
        write.send(Message::Text(sub.to_string())).await?;

        // MEXC requires a PING every 30 seconds
        let mut ping_tick = interval(Duration::from_secs(30));
        ping_tick.tick().await;

        loop {
            select! {
                msg = read.next() => {
                    let msg = match msg {
                        Some(Ok(m))  => m,
                        Some(Err(e)) => return Err(e.into()),
                        None => break,
                    };
                    match msg {
                        Message::Text(text) => {
                            if text.contains("\"msg\":\"PONG\"") { continue; }
                            let env: MexcWsMsg = match serde_json::from_str(&text) {
                                Ok(e) => e,
                                Err(_) => continue,
                            };
                            let Some(d) = env.d else { continue };
                            let bid = d.bid.as_deref().and_then(|s| Decimal::from_str(s).ok()).unwrap_or(dec!(0));
                            let ask = d.ask.as_deref().and_then(|s| Decimal::from_str(s).ok()).unwrap_or(dec!(0));
                            if bid > dec!(0) && ask > dec!(0) {
                                debug!("{} bid={} ask={}", market_id, bid, ask);
                                price_state.update(BookTicker {
                                    market: market_id,
                                    bid_price: bid,
                                    bid_qty:   d.bid_qty.as_deref().and_then(|s| Decimal::from_str(s).ok()).unwrap_or(dec!(0)),
                                    ask_price: ask,
                                    ask_qty:   d.ask_qty.as_deref().and_then(|s| Decimal::from_str(s).ok()).unwrap_or(dec!(0)),
                                    updated_at: Utc::now(),
                                });
                            }
                        }
                        Message::Ping(d) => { write.send(Message::Pong(d)).await?; }
                        Message::Close(_) => break,
                        _ => {}
                    }
                }
                _ = ping_tick.tick() => {
                    write.send(Message::Text(r#"{"method":"PING"}"#.to_string())).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn run_feed(self: Arc<Self>, price_state: SharedPriceState) {
        let mut delay = Duration::from_millis(1_000);
        loop {
            if let Err(e) = self.connect_once(&price_state).await {
                warn!("MEXC:Spot feed error: {:#}", e);
            } else {
                warn!("MEXC:Spot feed closed");
            }
            sleep(delay).await;
            delay = (delay * 2).min(Duration::from_secs(30));
        }
    }

    pub async fn place_order(
        &self,
        side: Side,
        quantity: Decimal,
        price_state: &SharedPriceState,
    ) -> Result<OrderResult> {
        if self.config.trading.paper_trading {
            return Ok(self.paper_fill(side, quantity, price_state));
        }

        let ts = now_ms();
        let pair = self.config.pair();
        let params = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
            pair, side, quantity, ts
        );
        let sig = sign_hmac_sha256(&self.config.mexc.api_secret, &params);
        let url = format!("https://api.mexc.com/api/v3/order?{}&signature={}", params, sig);

        let resp: serde_json::Value = self.http
            .post(&url)
            .header("X-MEXC-APIKEY", &self.config.mexc.api_key)
            .send().await?
            .json().await?;

        if let Some(code) = resp.get("code") {
            bail!("MEXC order error {}: {}", code, resp["msg"].as_str().unwrap_or(""));
        }

        let filled_qty = Decimal::from_str(resp["executedQty"].as_str().unwrap_or("0"))?;
        let quote_qty  = Decimal::from_str(resp["cummulativeQuoteQty"].as_str().unwrap_or("0"))?;
        let avg_price  = if filled_qty > dec!(0) { quote_qty / filled_qty } else { dec!(0) };

        Ok(OrderResult {
            exchange:    Exchange::Mexc,
            market_type: MarketType::Spot,
            order_id:    resp["orderId"].as_str().unwrap_or("").to_string(),
            side,
            filled_qty,
            avg_price,
            fee_usdt:  filled_qty * avg_price * dec!(0.002),
            timestamp: Utc::now(),
        })
    }

    fn paper_fill(&self, side: Side, quantity: Decimal, price_state: &SharedPriceState) -> OrderResult {
        let mid = MarketId::new(Exchange::Mexc, MarketType::Spot);
        let price = price_state.get(&mid)
            .map(|t| match side { Side::Buy => t.ask_price, Side::Sell => t.bid_price })
            .unwrap_or(dec!(150));
        OrderResult {
            exchange:    Exchange::Mexc,
            market_type: MarketType::Spot,
            order_id:    Uuid::new_v4().to_string(),
            side,
            filled_qty:  quantity,
            avg_price:   price,
            fee_usdt:    quantity * price * dec!(0.002),
            timestamp:   Utc::now(),
        }
    }
}

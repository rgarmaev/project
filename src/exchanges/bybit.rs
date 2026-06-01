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
use tokio::time::{interval, sleep};
use tokio::select;
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
struct WsEnvelope {
    topic: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    data: Option<WsBookData>,
    op: Option<String>,
}

#[derive(Deserialize)]
struct WsBookData {
    #[serde(default)]
    b: Vec<[String; 2]>,
    #[serde(default)]
    a: Vec<[String; 2]>,
}

// ── Connector ────────────────────────────────────────────────────────────────

pub struct BybitConnector {
    config: Arc<Config>,
    http: Client,
}

impl BybitConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    fn ws_url(&self, market: MarketType) -> &'static str {
        match (market, self.config.bybit.testnet) {
            (MarketType::Spot,    false) => "wss://stream.bybit.com/v5/public/spot",
            (MarketType::Spot,    true)  => "wss://stream-testnet.bybit.com/v5/public/spot",
            (MarketType::Futures, false) => "wss://stream.bybit.com/v5/public/linear",
            (MarketType::Futures, true)  => "wss://stream-testnet.bybit.com/v5/public/linear",
        }
    }

    fn rest_base(&self) -> &'static str {
        if self.config.bybit.testnet {
            "https://api-testnet.bybit.com"
        } else {
            "https://api.bybit.com"
        }
    }

    async fn connect_once(&self, market: MarketType, price_state: &SharedPriceState) -> Result<()> {
        let url = self.ws_url(market);
        let market_id = MarketId::new(Exchange::Bybit, market);
        let pair = self.config.pair();

        info!("{} connecting → {}", market_id, url);
        let (ws, _) = connect_async(url).await?;
        let (mut write, mut read) = ws.split();

        let sub = serde_json::json!({
            "op": "subscribe",
            "args": [format!("orderbook.1.{}", pair)]
        });
        write.send(Message::Text(sub.to_string())).await?;

        // Bybit requires an application-level ping every 20 s
        let mut ping_tick = interval(Duration::from_secs(20));
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
                            let env: WsEnvelope = match serde_json::from_str(&text) {
                                Ok(e) => e,
                                Err(_) => continue,
                            };
                            // Application-level ping from server
                            if env.op.as_deref() == Some("ping") {
                                write.send(Message::Text(r#"{"op":"pong"}"#.to_string())).await?;
                                continue;
                            }
                            let Some(data) = env.data else { continue };
                            let bid = data.b.first()
                                .and_then(|row| Decimal::from_str(&row[0]).ok())
                                .unwrap_or(dec!(0));
                            let bid_qty = data.b.first()
                                .and_then(|row| Decimal::from_str(&row[1]).ok())
                                .unwrap_or(dec!(0));
                            let ask = data.a.first()
                                .and_then(|row| Decimal::from_str(&row[0]).ok())
                                .unwrap_or(dec!(0));
                            let ask_qty = data.a.first()
                                .and_then(|row| Decimal::from_str(&row[1]).ok())
                                .unwrap_or(dec!(0));

                            if bid > dec!(0) && ask > dec!(0) {
                                debug!("{} bid={} ask={}", market_id, bid, ask);
                                price_state.update(BookTicker {
                                    market: market_id,
                                    bid_price: bid,
                                    bid_qty,
                                    ask_price: ask,
                                    ask_qty,
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
                    write.send(Message::Text(r#"{"op":"ping"}"#.to_string())).await?;
                }
            }
        }
        Ok(())
    }

    pub async fn run_feed(self: Arc<Self>, market: MarketType, price_state: SharedPriceState) {
        let name = format!("Bybit:{}", market);
        let mut delay = Duration::from_millis(1_000);
        loop {
            if let Err(e) = self.connect_once(market, &price_state).await {
                warn!("{} feed error: {:#}", name, e);
            } else {
                warn!("{} feed closed", name);
            }
            sleep(delay).await;
            delay = (delay * 2).min(Duration::from_secs(30));
        }
    }

    pub async fn place_order(
        &self,
        symbol: &str,
        market: MarketType,
        side: Side,
        quantity: Decimal,
        price_state: &SharedPriceState,
    ) -> Result<OrderResult> {
        if self.config.trading.paper_trading {
            return Ok(self.paper_fill(market, side, quantity, price_state));
        }

        let ts = now_ms();
        let recv_window = 5000u64;
        let category = match market {
            MarketType::Spot    => "spot",
            MarketType::Futures => "linear",
        };
        let body = serde_json::json!({
            "category":  category,
            "symbol":    symbol,
            "side":      match side { Side::Buy => "Buy", Side::Sell => "Sell" },
            "orderType": "Market",
            "qty":       quantity.to_string(),
        }).to_string();

        let sign_payload = format!("{}{}{}{}", ts, self.config.bybit.api_key, recv_window, body);
        let sig = sign_hmac_sha256(&self.config.bybit.api_secret, &sign_payload);

        let resp: serde_json::Value = self.http
            .post(format!("{}/v5/order/create", self.rest_base()))
            .header("X-BAPI-API-KEY",      &self.config.bybit.api_key)
            .header("X-BAPI-SIGN",         sig)
            .header("X-BAPI-SIGN-TYPE",    "2")
            .header("X-BAPI-TIMESTAMP",    ts.to_string())
            .header("X-BAPI-RECV-WINDOW",  recv_window.to_string())
            .header("Content-Type",        "application/json")
            .body(body)
            .send().await?
            .json().await?;

        let ret_code = resp["retCode"].as_i64().unwrap_or(-1);
        if ret_code != 0 {
            bail!("Bybit order error {}: {}", ret_code, resp["retMsg"].as_str().unwrap_or(""));
        }

        let result = &resp["result"];
        let filled_qty = Decimal::from_str(result["cumExecQty"].as_str().unwrap_or("0"))?;
        let avg_price  = Decimal::from_str(result["avgPrice"].as_str().unwrap_or("0"))?;
        let fee_rate   = if market == MarketType::Futures { dec!(0.00055) } else { dec!(0.001) };

        Ok(OrderResult {
            exchange:    Exchange::Bybit,
            market_type: market,
            order_id:    result["orderId"].as_str().unwrap_or("").to_string(),
            side,
            filled_qty,
            avg_price,
            fee_usdt:  filled_qty * avg_price * fee_rate,
            timestamp: Utc::now(),
        })
    }

    fn paper_fill(
        &self,
        market: MarketType,
        side: Side,
        quantity: Decimal,
        price_state: &SharedPriceState,
    ) -> OrderResult {
        let mid = MarketId::new(Exchange::Bybit, market);
        let base_price = price_state.get(&mid)
            .map(|t| match side { Side::Buy => t.ask_price, Side::Sell => t.bid_price })
            .unwrap_or(dec!(150));
        let order_id = Uuid::new_v4();
        let bits = order_id.as_u128();
        let total_bp = ((bits % 3) as i64 - 1) as i32;
        let price = base_price * (dec!(1) + Decimal::from(total_bp) / dec!(10000));
        let fee_rate = if market == MarketType::Futures { dec!(0.00055) } else { dec!(0.001) };
        OrderResult {
            exchange:    Exchange::Bybit,
            market_type: market,
            order_id:    order_id.to_string(),
            side,
            filled_qty:  quantity,
            avg_price:   price,
            fee_usdt:    quantity * price * fee_rate,
            timestamp:   Utc::now(),
        }
    }
}

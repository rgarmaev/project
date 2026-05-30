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
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    config::Config,
    exchanges::{now_ms, sign_hmac_sha256},
    orderbook::SharedPriceState,
    types::{BookTicker, Exchange, MarketId, MarketType, OrderResult, Side},
};

// ── WebSocket message shapes ────────────────────────────────────────────────

#[derive(Deserialize)]
struct WsBookTicker {
    #[serde(rename = "b")]
    bid: String,
    #[serde(rename = "B")]
    bid_qty: String,
    #[serde(rename = "a")]
    ask: String,
    #[serde(rename = "A")]
    ask_qty: String,
}

// ── REST response shapes ─────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderResponse {
    order_id: u64,
    executed_qty: String,
    #[serde(default)]
    cummulative_quote_qty: String,
}

// ── Connector ────────────────────────────────────────────────────────────────

pub struct BinanceConnector {
    config: Arc<Config>,
    http: Client,
}

impl BinanceConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    fn ws_url(&self, market: MarketType) -> String {
        let pair = self.config.pair().to_lowercase();
        match market {
            MarketType::Spot    => format!("wss://stream.binance.com:9443/ws/{}@bookTicker", pair),
            MarketType::Futures => format!("wss://fstream.binance.com/ws/{}@bookTicker", pair),
        }
    }

    fn rest_base(&self, market: MarketType) -> &'static str {
        match (market, self.config.binance.testnet) {
            (MarketType::Spot,    false) => "https://api.binance.com",
            (MarketType::Spot,    true)  => "https://testnet.binance.vision",
            (MarketType::Futures, false) => "https://fapi.binance.com",
            (MarketType::Futures, true)  => "https://testnet.binancefuture.com",
        }
    }

    async fn connect_once(&self, market: MarketType, price_state: &SharedPriceState) -> Result<()> {
        let url = self.ws_url(market);
        let market_id = MarketId::new(Exchange::Binance, market);

        info!("{} connecting → {}", market_id, url);
        let (ws, _) = connect_async(&url).await?;
        let (_, mut read) = ws.split();

        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    if let Ok(t) = serde_json::from_str::<WsBookTicker>(&text) {
                        let ticker = BookTicker {
                            market: market_id,
                            bid_price: Decimal::from_str(&t.bid).unwrap_or(dec!(0)),
                            bid_qty:   Decimal::from_str(&t.bid_qty).unwrap_or(dec!(0)),
                            ask_price: Decimal::from_str(&t.ask).unwrap_or(dec!(0)),
                            ask_qty:   Decimal::from_str(&t.ask_qty).unwrap_or(dec!(0)),
                            updated_at: Utc::now(),
                        };
                        debug!("{} bid={} ask={}", market_id, ticker.bid_price, ticker.ask_price);
                        price_state.update(ticker);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok(())
    }

    /// Runs the WebSocket feed for the given market with auto-reconnect.
    pub async fn run_feed(self: Arc<Self>, market: MarketType, price_state: SharedPriceState) {
        let name = format!("Binance:{}", market);
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
        market: MarketType,
        side: Side,
        quantity: Decimal,
        price_state: &SharedPriceState,
    ) -> Result<OrderResult> {
        if self.config.trading.paper_trading {
            return Ok(self.paper_fill(market, side, quantity, price_state));
        }

        let ts = now_ms();
        let pair = self.config.pair();
        let params = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
            pair, side, quantity, ts
        );
        let sig = sign_hmac_sha256(&self.config.binance.api_secret, &params);
        let body = format!("{}&signature={}", params, sig);

        let path = match market {
            MarketType::Spot    => "/api/v3/order",
            MarketType::Futures => "/fapi/v1/order",
        };
        let resp: serde_json::Value = self.http
            .post(format!("{}{}", self.rest_base(market), path))
            .header("X-MBX-APIKEY", &self.config.binance.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send().await?
            .json().await?;

        if let Some(code) = resp.get("code") {
            bail!("Binance order error {}: {}", code, resp["msg"].as_str().unwrap_or(""));
        }

        let filled_qty = Decimal::from_str(resp["executedQty"].as_str().unwrap_or("0"))?;
        let quote_qty  = Decimal::from_str(resp["cummulativeQuoteQty"].as_str().unwrap_or("0"))?;
        let avg_price  = if filled_qty > dec!(0) { quote_qty / filled_qty } else { dec!(0) };
        let fee_rate   = if market == MarketType::Futures { dec!(0.0002) } else { dec!(0.001) };

        Ok(OrderResult {
            exchange:    Exchange::Binance,
            market_type: market,
            order_id:    resp["orderId"].as_u64().unwrap_or(0).to_string(),
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
        let mid = MarketId::new(Exchange::Binance, market);
        let base_price = price_state.get(&mid)
            .map(|t| match side { Side::Buy => t.ask_price, Side::Sell => t.bid_price })
            .unwrap_or(dec!(150));
        let order_id = Uuid::new_v4();
        // ±1 bp tick noise — realistic for limit order at quoted price
        let bits = order_id.as_u128();
        let total_bp = ((bits % 3) as i64 - 1) as i32;
        let price = base_price * (dec!(1) + Decimal::from(total_bp) / dec!(10000));
        let fee_rate = if market == MarketType::Futures { dec!(0.00005) } else { dec!(0.00010) };
        OrderResult {
            exchange:    Exchange::Binance,
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

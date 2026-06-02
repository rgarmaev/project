use anyhow::{bail, Result};
use chrono::Utc;
use reqwest::Client;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;
use uuid::Uuid;

use crate::{
    config::Config,
    multi_feed::to_underscored,
    orderbook::SharedPriceState,
    types::{Exchange, MarketType, OrderResult, Side},
};

const BASE_URL: &str = "https://api.gateio.ws/api/v4";

pub struct GateConnector {
    config: Arc<Config>,
    http:   Client,
}

impl GateConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    /// Gate.io signing: HMAC-SHA512
    /// sign_str = METHOD\nPATH\nQUERY\nSHA512(body)\nTIMESTAMP
    fn gate_sign(secret: &str, method: &str, path: &str, query: &str, body: &str, ts: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::{Digest, Sha512};

        let body_hash = hex::encode(Sha512::digest(body.as_bytes()));
        let sign_str  = format!("{}\n{}\n{}\n{}\n{}", method, path, query, body_hash, ts);
        let mut mac   = Hmac::<Sha512>::new_from_slice(secret.as_bytes())
            .expect("HMAC accepts any key size");
        mac.update(sign_str.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    pub async fn place_order(
        &self,
        symbol: &str,
        market: MarketType,
        side: Side,
        quantity: Decimal,
        price_hint: Decimal,
        _price_state: &SharedPriceState,
    ) -> Result<OrderResult> {
        if self.config.trading.paper_trading {
            return Ok(self.paper_fill(market, side, quantity, price_hint));
        }

        if self.config.gate.api_key.is_empty() {
            bail!("Gate.io API key not configured");
        }

        match market {
            MarketType::Spot    => self.place_spot_order(symbol, side, quantity, price_hint).await,
            MarketType::Futures => self.place_futures_order(symbol, side, quantity, price_hint).await,
        }
    }

    async fn place_spot_order(
        &self,
        symbol: &str,
        side: Side,
        quantity: Decimal,
        price_hint: Decimal,
    ) -> Result<OrderResult> {
        let pair     = to_underscored(symbol);
        let side_str = match side { Side::Buy => "buy", Side::Sell => "sell" };

        // Gate spot market buy: amount = quote (USDT) to spend
        // Gate spot market sell: amount = base qty to sell
        let amount = match side {
            Side::Buy  => (quantity * price_hint).to_string(),
            Side::Sell => quantity.to_string(),
        };

        let body = serde_json::json!({
            "currency_pair": pair,
            "type":          "market",
            "side":          side_str,
            "amount":        amount,
            "time_in_force": "ioc",
        })
        .to_string();

        let path = "/spot/orders";
        let ts   = Utc::now().timestamp().to_string();
        let sig  = Self::gate_sign(&self.config.gate.api_secret, "POST", path, "", &body, &ts);

        let resp: serde_json::Value = self.http
            .post(format!("{}{}", BASE_URL, path))
            .header("KEY",          &self.config.gate.api_key)
            .header("SIGN",         sig)
            .header("Timestamp",    &ts)
            .header("Content-Type", "application/json")
            .body(body)
            .send().await?
            .json().await?;

        if let Some(label) = resp["label"].as_str() {
            bail!("Gate.io spot order error: {} — {}", label, resp["message"].as_str().unwrap_or(""));
        }

        // Brief wait for fill details to settle
        sleep(Duration::from_millis(300)).await;

        let order_id  = resp["id"].as_str().unwrap_or("").to_string();
        let avg_price = Decimal::from_str(resp["avg_deal_price"].as_str().unwrap_or("0"))
            .unwrap_or(price_hint);
        let filled_qty = Decimal::from_str(resp["amount"].as_str().unwrap_or("0"))
            .unwrap_or(quantity);
        let fee_str  = resp["fee"].as_str().unwrap_or("0");
        let fee_parsed = Decimal::from_str(fee_str).unwrap_or(dec!(0));
        let fee_rate = dec!(0.00100);
        let fee_usdt = if fee_parsed > dec!(0) {
            fee_parsed
        } else {
            filled_qty * avg_price * fee_rate
        };

        Ok(OrderResult {
            exchange:    Exchange::Gate,
            market_type: MarketType::Spot,
            order_id,
            side,
            filled_qty,
            avg_price,
            fee_usdt,
            timestamp:   Utc::now(),
        })
    }

    async fn place_futures_order(
        &self,
        symbol: &str,
        side: Side,
        quantity: Decimal,
        price_hint: Decimal,
    ) -> Result<OrderResult> {
        let contract = to_underscored(symbol);

        // Gate futures size is in contracts (integers); positive = long, negative = short.
        // Use quantity as contract count rounded to nearest integer.
        let size: i64 = match side {
            Side::Buy  =>  quantity.to_string().parse::<f64>().unwrap_or(1.0).round() as i64,
            Side::Sell => -(quantity.to_string().parse::<f64>().unwrap_or(1.0).round() as i64),
        };

        let body = serde_json::json!({
            "contract": contract,
            "size":     size,
            "price":    "0",    // market order
            "tif":      "ioc",
        })
        .to_string();

        let path = "/futures/usdt/orders";
        let ts   = Utc::now().timestamp().to_string();
        let sig  = Self::gate_sign(&self.config.gate.api_secret, "POST", path, "", &body, &ts);

        let resp: serde_json::Value = self.http
            .post(format!("{}{}", BASE_URL, path))
            .header("KEY",          &self.config.gate.api_key)
            .header("SIGN",         sig)
            .header("Timestamp",    &ts)
            .header("Content-Type", "application/json")
            .body(body)
            .send().await?
            .json().await?;

        if let Some(label) = resp["label"].as_str() {
            bail!("Gate.io futures order error: {} — {}", label, resp["message"].as_str().unwrap_or(""));
        }

        sleep(Duration::from_millis(300)).await;

        let order_id   = resp["id"].as_i64().map(|i| i.to_string()).unwrap_or_default();
        let fill_price = Decimal::from_str(resp["fill_price"].as_str().unwrap_or("0"))
            .unwrap_or(price_hint);
        let avg_price  = if fill_price > dec!(0) { fill_price } else { price_hint };
        let filled_qty = quantity;
        let fee_rate   = dec!(0.00050);

        if resp["status"].as_str().unwrap_or("") != "finished" {
            warn!("Gate.io futures order {} status={}", order_id, resp["status"].as_str().unwrap_or("unknown"));
        }

        Ok(OrderResult {
            exchange:    Exchange::Gate,
            market_type: MarketType::Futures,
            order_id,
            side,
            filled_qty,
            avg_price,
            fee_usdt:    filled_qty * avg_price * fee_rate,
            timestamp:   Utc::now(),
        })
    }

    fn paper_fill(
        &self,
        market: MarketType,
        side: Side,
        qty: Decimal,
        price_hint: Decimal,
    ) -> OrderResult {
        let fee_rate = match market {
            MarketType::Spot    => dec!(0.00100),
            MarketType::Futures => dec!(0.00050),
        };
        OrderResult {
            exchange:    Exchange::Gate,
            market_type: market,
            order_id:    Uuid::new_v4().to_string(),
            side,
            filled_qty:  qty,
            avg_price:   price_hint,
            fee_usdt:    qty * price_hint * fee_rate,
            timestamp:   Utc::now(),
        }
    }
}

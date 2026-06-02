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
    exchanges::sign_hmac_sha256_b64,
    orderbook::SharedPriceState,
    types::{Exchange, MarketType, OrderResult, Side},
};

const BASE_URL: &str = "https://api.bitget.com";

pub struct BitgetConnector {
    config: Arc<Config>,
    http:   Client,
}

impl BitgetConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    fn timestamp() -> String {
        Utc::now().timestamp().to_string()
    }

    fn sign(&self, ts: &str, method: &str, path: &str, body: &str) -> String {
        let prehash = format!("{}{}{}{}", ts, method, path, body);
        sign_hmac_sha256_b64(&self.config.bitget.api_secret, &prehash)
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

        if self.config.bitget.api_key.is_empty() {
            bail!("Bitget API key not configured");
        }

        let side_str = match side {
            Side::Buy  => "buy",
            Side::Sell => "sell",
        };

        match market {
            MarketType::Spot => {
                self.place_spot_order(symbol, side_str, quantity, price_hint, side, market).await
            }
            MarketType::Futures => {
                self.place_futures_order(symbol, side_str, quantity, price_hint, side, market).await
            }
        }
    }

    async fn place_spot_order(
        &self,
        symbol: &str,
        side_str: &str,
        quantity: Decimal,
        price_hint: Decimal,
        side: Side,
        market: MarketType,
    ) -> Result<OrderResult> {
        let path = "/api/v2/spot/trade/place-order";
        let body = serde_json::json!({
            "symbol":    symbol,
            "side":      side_str,
            "orderType": "market",
            "force":     "gtc",
            "size":      quantity.to_string(),
        })
        .to_string();

        let ts  = Self::timestamp();
        let sig = self.sign(&ts, "POST", path, &body);

        let resp: serde_json::Value = self.http
            .post(format!("{}{}", BASE_URL, path))
            .header("ACCESS-KEY",        &self.config.bitget.api_key)
            .header("ACCESS-SIGN",       sig)
            .header("ACCESS-TIMESTAMP",  &ts)
            .header("ACCESS-PASSPHRASE", &self.config.bitget.passphrase)
            .header("Content-Type",      "application/json")
            .body(body)
            .send().await?
            .json().await?;

        if resp["code"].as_str().unwrap_or("") != "00000" {
            bail!("Bitget spot place_order error: {}", resp["msg"].as_str().unwrap_or("unknown"));
        }

        let order_id = resp["data"]["orderId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Bitget: missing orderId in spot response"))?
            .to_string();

        sleep(Duration::from_millis(300)).await;
        self.fetch_spot_fill(&order_id, symbol, market, side, quantity, price_hint).await
    }

    async fn fetch_spot_fill(
        &self,
        order_id: &str,
        symbol:   &str,
        market:   MarketType,
        side:     Side,
        quantity: Decimal,
        price_hint: Decimal,
    ) -> Result<OrderResult> {
        let path = format!("/api/v2/spot/trade/orderInfo?orderId={}&symbol={}", order_id, symbol);
        let ts  = Self::timestamp();
        let sig = self.sign(&ts, "GET", &path, "");

        let resp: serde_json::Value = self.http
            .get(format!("{}{}", BASE_URL, path))
            .header("ACCESS-KEY",        &self.config.bitget.api_key)
            .header("ACCESS-SIGN",       sig)
            .header("ACCESS-TIMESTAMP",  &ts)
            .header("ACCESS-PASSPHRASE", &self.config.bitget.passphrase)
            .send().await?
            .json().await?;

        if resp["code"].as_str().unwrap_or("") != "00000" {
            bail!("Bitget spot fetch_fill error: {}", resp["msg"].as_str().unwrap_or("unknown"));
        }

        let data = match resp["data"]["orderList"].as_array().and_then(|a| a.first()) {
            Some(d) => d.clone(),
            None    => {
                warn!("Bitget spot: empty orderList for {}, using fallback", order_id);
                return Ok(self.paper_fill(market, side, quantity, price_hint));
            }
        };

        let filled_qty = Decimal::from_str(data["baseVolume"].as_str().unwrap_or("0"))
            .unwrap_or(quantity);
        let quote_vol  = Decimal::from_str(data["quoteVolume"].as_str().unwrap_or("0"))
            .unwrap_or(dec!(0));
        let avg_price  = if filled_qty > dec!(0) { quote_vol / filled_qty } else { price_hint };
        let fee_str    = data["fee"].as_str().unwrap_or("0");
        let fee_abs    = Decimal::from_str(fee_str.trim_start_matches('-')).unwrap_or(dec!(0));

        let fee_rate = dec!(0.00100);
        Ok(OrderResult {
            exchange:    Exchange::Bitget,
            market_type: market,
            order_id:    order_id.to_string(),
            side,
            filled_qty,
            avg_price,
            fee_usdt:    if fee_abs > dec!(0) { fee_abs } else { filled_qty * avg_price * fee_rate },
            timestamp:   Utc::now(),
        })
    }

    async fn place_futures_order(
        &self,
        symbol: &str,
        side_str: &str,
        quantity: Decimal,
        price_hint: Decimal,
        side: Side,
        market: MarketType,
    ) -> Result<OrderResult> {
        let path = "/api/v2/mix/order/place-order";
        let body = serde_json::json!({
            "symbol":      symbol,
            "productType": "USDT-FUTURES",
            "marginMode":  "crossed",
            "marginCoin":  "USDT",
            "size":        quantity.to_string(),
            "side":        side_str,
            "tradeSide":   "open",
            "orderType":   "market",
        })
        .to_string();

        let ts  = Self::timestamp();
        let sig = self.sign(&ts, "POST", path, &body);

        let resp: serde_json::Value = self.http
            .post(format!("{}{}", BASE_URL, path))
            .header("ACCESS-KEY",        &self.config.bitget.api_key)
            .header("ACCESS-SIGN",       sig)
            .header("ACCESS-TIMESTAMP",  &ts)
            .header("ACCESS-PASSPHRASE", &self.config.bitget.passphrase)
            .header("Content-Type",      "application/json")
            .body(body)
            .send().await?
            .json().await?;

        if resp["code"].as_str().unwrap_or("") != "00000" {
            bail!("Bitget futures place_order error: {}", resp["msg"].as_str().unwrap_or("unknown"));
        }

        let order_id = resp["data"]["orderId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Bitget: missing orderId in futures response"))?
            .to_string();

        sleep(Duration::from_millis(300)).await;
        self.fetch_futures_fill(&order_id, symbol, market, side, quantity, price_hint).await
    }

    async fn fetch_futures_fill(
        &self,
        order_id: &str,
        symbol:   &str,
        market:   MarketType,
        side:     Side,
        quantity: Decimal,
        price_hint: Decimal,
    ) -> Result<OrderResult> {
        let path = format!(
            "/api/v2/mix/order/detail?symbol={}&orderId={}&productType=USDT-FUTURES",
            symbol, order_id
        );
        let ts  = Self::timestamp();
        let sig = self.sign(&ts, "GET", &path, "");

        let resp: serde_json::Value = self.http
            .get(format!("{}{}", BASE_URL, path))
            .header("ACCESS-KEY",        &self.config.bitget.api_key)
            .header("ACCESS-SIGN",       sig)
            .header("ACCESS-TIMESTAMP",  &ts)
            .header("ACCESS-PASSPHRASE", &self.config.bitget.passphrase)
            .send().await?
            .json().await?;

        if resp["code"].as_str().unwrap_or("") != "00000" {
            bail!("Bitget futures fetch_fill error: {}", resp["msg"].as_str().unwrap_or("unknown"));
        }

        let data = &resp["data"];
        let filled_qty = Decimal::from_str(data["baseVolume"].as_str().unwrap_or("0"))
            .unwrap_or(quantity);
        let quote_vol  = Decimal::from_str(data["quoteVolume"].as_str().unwrap_or("0"))
            .unwrap_or(dec!(0));
        let avg_price  = if filled_qty > dec!(0) { quote_vol / filled_qty } else { price_hint };
        let fee_str    = data["fee"].as_str().unwrap_or("0");
        let fee_abs    = Decimal::from_str(fee_str.trim_start_matches('-')).unwrap_or(dec!(0));

        let fee_rate = dec!(0.00060);
        Ok(OrderResult {
            exchange:    Exchange::Bitget,
            market_type: market,
            order_id:    order_id.to_string(),
            side,
            filled_qty,
            avg_price,
            fee_usdt:    if fee_abs > dec!(0) { fee_abs } else { filled_qty * avg_price * fee_rate },
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
            MarketType::Futures => dec!(0.00060),
        };
        OrderResult {
            exchange:    Exchange::Bitget,
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

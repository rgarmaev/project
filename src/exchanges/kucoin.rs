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

const SPOT_BASE:    &str = "https://api.kucoin.com";
const FUTURES_BASE: &str = "https://api-futures.kucoin.com";

pub struct KucoinConnector {
    config: Arc<Config>,
    http:   Client,
}

impl KucoinConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    /// "BTCUSDT" → "BTC-USDT" (spot) or "XBTUSDTM" (futures)
    fn kucoin_symbol(symbol: &str, market: MarketType) -> String {
        let base = &symbol[..symbol.len() - 4]; // strip "USDT"
        match market {
            MarketType::Spot    => format!("{}-USDT", base),
            MarketType::Futures => {
                let kucoin_base = if base == "BTC" { "XBT" } else { base };
                format!("{}USDTM", kucoin_base)
            }
        }
    }

    fn timestamp_ms() -> String {
        Utc::now().timestamp_millis().to_string()
    }

    /// Build KC-API-SIGN: base64(HMAC-SHA256(secret, ts + method + path + body))
    fn sign(&self, ts: &str, method: &str, path: &str, body: &str) -> String {
        let prehash = format!("{}{}{}{}", ts, method, path, body);
        sign_hmac_sha256_b64(&self.config.kucoin.api_secret, &prehash)
    }

    /// KC-API-PASSPHRASE for API v2: base64(HMAC-SHA256(secret, passphrase))
    fn signed_passphrase(&self) -> String {
        sign_hmac_sha256_b64(&self.config.kucoin.api_secret, &self.config.kucoin.passphrase)
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

        if self.config.kucoin.api_key.is_empty() {
            bail!("KuCoin API key not configured");
        }

        let kucoin_sym = Self::kucoin_symbol(symbol, market);
        let side_str = match side {
            Side::Buy  => "buy",
            Side::Sell => "sell",
        };

        let base_url = match market {
            MarketType::Spot    => SPOT_BASE,
            MarketType::Futures => FUTURES_BASE,
        };
        let path = "/api/v1/orders";

        let body = match market {
            MarketType::Spot => serde_json::json!({
                "clientOid": Uuid::new_v4().to_string(),
                "side":      side_str,
                "symbol":    kucoin_sym,
                "type":      "market",
                "size":      quantity.to_string(),
            }),
            MarketType::Futures => serde_json::json!({
                "clientOid": Uuid::new_v4().to_string(),
                "side":      side_str,
                "symbol":    kucoin_sym,
                "type":      "market",
                "size":      quantity.to_string(),
                "leverage":  "1",
            }),
        }
        .to_string();

        let ts  = Self::timestamp_ms();
        let sig = self.sign(&ts, "POST", path, &body);
        let pp  = self.signed_passphrase();

        let resp: serde_json::Value = self.http
            .post(format!("{}{}", base_url, path))
            .header("KC-API-KEY",         &self.config.kucoin.api_key)
            .header("KC-API-SIGN",        sig)
            .header("KC-API-TIMESTAMP",   &ts)
            .header("KC-API-PASSPHRASE",  pp)
            .header("KC-API-KEY-VERSION", "2")
            .header("Content-Type",       "application/json")
            .body(body)
            .send().await?
            .json().await?;

        let code = resp["code"].as_str().unwrap_or("");
        if code != "200000" {
            bail!("KuCoin place_order error {}: {}", code, resp["msg"].as_str().unwrap_or("unknown"));
        }

        let order_id = resp["data"]["orderId"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("KuCoin: missing orderId in response"))?
            .to_string();

        // Brief wait then fetch fill
        sleep(Duration::from_millis(300)).await;
        self.fetch_fill(base_url, &order_id, market, side, quantity).await
    }

    async fn fetch_fill(
        &self,
        base_url: &str,
        order_id: &str,
        market:   MarketType,
        side:     Side,
        quantity: Decimal,
    ) -> Result<OrderResult> {
        let path = format!("/api/v1/orders/{}", order_id);
        let ts   = Self::timestamp_ms();
        let sig  = self.sign(&ts, "GET", &path, "");
        let pp   = self.signed_passphrase();

        let resp: serde_json::Value = self.http
            .get(format!("{}{}", base_url, path))
            .header("KC-API-KEY",         &self.config.kucoin.api_key)
            .header("KC-API-SIGN",        sig)
            .header("KC-API-TIMESTAMP",   &ts)
            .header("KC-API-PASSPHRASE",  pp)
            .header("KC-API-KEY-VERSION", "2")
            .send().await?
            .json().await?;

        let code = resp["code"].as_str().unwrap_or("");
        if code != "200000" {
            bail!("KuCoin fetch_fill error {}: {}", code, resp["msg"].as_str().unwrap_or("unknown"));
        }

        let data = &resp["data"];

        // dealSize = filled base qty, dealFunds = filled quote amount
        let filled_qty = Decimal::from_str(data["dealSize"].as_str().unwrap_or("0"))
            .unwrap_or(quantity);
        let deal_funds = Decimal::from_str(data["dealFunds"].as_str().unwrap_or("0"))
            .unwrap_or(dec!(0));
        let fee = Decimal::from_str(data["fee"].as_str().unwrap_or("0"))
            .unwrap_or(dec!(0));

        let avg_price = if filled_qty > dec!(0) {
            deal_funds / filled_qty
        } else {
            dec!(0)
        };

        let fee_rate = match market {
            MarketType::Spot    => dec!(0.00100),
            MarketType::Futures => dec!(0.00060),
        };

        // Use actual fee if available, otherwise estimate
        let fee_usdt = if fee > dec!(0) {
            fee
        } else {
            filled_qty * avg_price * fee_rate
        };

        if data["isActive"].as_bool().unwrap_or(true) {
            warn!("KuCoin order {} may still be active — using available fill data", order_id);
        }

        Ok(OrderResult {
            exchange:    Exchange::Kucoin,
            market_type: market,
            order_id:    order_id.to_string(),
            side,
            filled_qty,
            avg_price,
            fee_usdt,
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
            exchange:    Exchange::Kucoin,
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

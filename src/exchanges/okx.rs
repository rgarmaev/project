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

const BASE_URL: &str = "https://www.okx.com";

pub struct OkxConnector {
    config: Arc<Config>,
    http:   Client,
}

impl OkxConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    /// "BTCUSDT" → "BTC-USDT" (spot) or "BTC-USDT-SWAP" (futures)
    fn inst_id(symbol: &str, market: MarketType) -> String {
        let base = &symbol[..symbol.len() - 4]; // strip "USDT"
        match market {
            MarketType::Spot    => format!("{}-USDT", base),
            MarketType::Futures => format!("{}-USDT-SWAP", base),
        }
    }

    fn okx_timestamp() -> String {
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }

    fn sign(&self, ts: &str, method: &str, path: &str, body: &str) -> String {
        let prehash = format!("{}{}{}{}", ts, method, path, body);
        sign_hmac_sha256_b64(&self.config.okx.api_secret, &prehash)
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

        if self.config.okx.api_key.is_empty() {
            bail!("OKX API key not configured");
        }

        let inst_id = Self::inst_id(symbol, market);
        let td_mode = match market {
            MarketType::Spot    => "cash",
            MarketType::Futures => "cross",
        };
        let side_str = match side {
            Side::Buy  => "buy",
            Side::Sell => "sell",
        };

        let body = serde_json::json!({
            "instId":  inst_id,
            "tdMode":  td_mode,
            "side":    side_str,
            "ordType": "market",
            "sz":      quantity.to_string(),
        })
        .to_string();

        let path = "/api/v5/trade/order";
        let ts   = Self::okx_timestamp();
        let sig  = self.sign(&ts, "POST", path, &body);

        let resp: serde_json::Value = self.http
            .post(format!("{}{}", BASE_URL, path))
            .header("OK-ACCESS-KEY",        &self.config.okx.api_key)
            .header("OK-ACCESS-SIGN",       sig)
            .header("OK-ACCESS-TIMESTAMP",  &ts)
            .header("OK-ACCESS-PASSPHRASE", &self.config.okx.passphrase)
            .header("Content-Type",         "application/json")
            .body(body)
            .send().await?
            .json().await?;

        if resp["code"].as_str().unwrap_or("") != "0" {
            bail!("OKX place_order error: {}", resp["msg"].as_str().unwrap_or("unknown"));
        }

        let ord_id = resp["data"][0]["ordId"].as_str()
            .ok_or_else(|| anyhow::anyhow!("OKX: missing ordId in response"))?
            .to_string();

        // Brief wait then fetch fill details
        sleep(Duration::from_millis(300)).await;
        self.fetch_fill(&inst_id, &ord_id, market, side, quantity).await
    }

    async fn fetch_fill(
        &self,
        inst_id: &str,
        ord_id:  &str,
        market:  MarketType,
        side:    Side,
        quantity: Decimal,
    ) -> Result<OrderResult> {
        let path = format!("/api/v5/trade/order?instId={}&ordId={}", inst_id, ord_id);
        let ts   = Self::okx_timestamp();
        let sig  = self.sign(&ts, "GET", &path, "");

        let resp: serde_json::Value = self.http
            .get(format!("{}{}", BASE_URL, path))
            .header("OK-ACCESS-KEY",        &self.config.okx.api_key)
            .header("OK-ACCESS-SIGN",       sig)
            .header("OK-ACCESS-TIMESTAMP",  &ts)
            .header("OK-ACCESS-PASSPHRASE", &self.config.okx.passphrase)
            .send().await?
            .json().await?;

        if resp["code"].as_str().unwrap_or("") != "0" {
            bail!("OKX fetch_fill error: {}", resp["msg"].as_str().unwrap_or("unknown"));
        }

        let data      = &resp["data"][0];
        let state     = data["state"].as_str().unwrap_or("");
        if state != "filled" && state != "partially_filled" {
            warn!("OKX order {} state={} — using quantity as fallback", ord_id, state);
        }

        let filled_qty = Decimal::from_str(data["accFillSz"].as_str().unwrap_or("0"))
            .unwrap_or(quantity);
        let avg_price  = Decimal::from_str(data["avgPx"].as_str().unwrap_or("0"))
            .unwrap_or(dec!(0));

        // fee is negative in OKX (deduction), abs it
        let fee_str = data["fee"].as_str().unwrap_or("0");
        let fee_abs = Decimal::from_str(fee_str.trim_start_matches('-')).unwrap_or(dec!(0));

        let fee_rate = match market {
            MarketType::Spot    => dec!(0.00080),
            MarketType::Futures => dec!(0.00050),
        };

        Ok(OrderResult {
            exchange:    Exchange::Okx,
            market_type: market,
            order_id:    ord_id.to_string(),
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
        quantity: Decimal,
        _price_state: &SharedPriceState,
    ) -> OrderResult {
        // Recover implied price: quantity = trade_size_usdt / price → price = trade_size_usdt / quantity
        let price = if quantity > dec!(0) {
            self.config.trading.trade_size_usdt / quantity
        } else {
            dec!(0)
        };
        let order_id = Uuid::new_v4();
        let fee_rate = match market {
            MarketType::Spot    => dec!(0.00080),
            MarketType::Futures => dec!(0.00050),
        };
        OrderResult {
            exchange:    Exchange::Okx,
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

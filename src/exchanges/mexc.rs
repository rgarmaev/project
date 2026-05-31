use anyhow::{bail, Result};
use chrono::Utc;
use reqwest::Client;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
    config::Config,
    exchanges::{now_ms, sign_hmac_sha256},
    orderbook::SharedPriceState,
    types::{BookTicker, Exchange, MarketId, MarketType, OrderResult, Side},
};

pub struct MexcConnector {
    config: Arc<Config>,
    http: Client,
}

impl MexcConnector {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config, http: Client::new() }
    }

    /// REST polling: MEXC public WebSocket channels are blocked without API keys.
    /// Poll bookTicker REST endpoint every 500ms — sufficient for paper trading.
    pub async fn run_feed(self: Arc<Self>, price_state: SharedPriceState) {
        let market_id = MarketId::new(Exchange::Mexc, MarketType::Spot);
        let url = format!(
            "https://api.mexc.com/api/v3/ticker/bookTicker?symbol={}",
            self.config.pair()
        );
        info!("MEXC:Spot polling → {}", url);

        let mut tick = interval(Duration::from_millis(500));
        loop {
            tick.tick().await;
            match self.http.get(&url).send().await {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(data) => {
                        let bid = data["bidPrice"].as_str()
                            .and_then(|s| Decimal::from_str(s).ok())
                            .unwrap_or(dec!(0));
                        let ask = data["askPrice"].as_str()
                            .and_then(|s| Decimal::from_str(s).ok())
                            .unwrap_or(dec!(0));
                        let bid_qty = data["bidQty"].as_str()
                            .and_then(|s| Decimal::from_str(s).ok())
                            .unwrap_or(dec!(0));
                        let ask_qty = data["askQty"].as_str()
                            .and_then(|s| Decimal::from_str(s).ok())
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
                    Err(e) => warn!("MEXC:Spot parse error: {}", e),
                },
                Err(e) => warn!("MEXC:Spot fetch error: {}", e),
            }
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
            fee_usdt:  filled_qty * avg_price * dec!(0.0005),
            timestamp: Utc::now(),
        })
    }

    fn paper_fill(&self, side: Side, quantity: Decimal, price_state: &SharedPriceState) -> OrderResult {
        let mid = MarketId::new(Exchange::Mexc, MarketType::Spot);
        let base_price = price_state.get(&mid)
            .map(|t| match side { Side::Buy => t.ask_price, Side::Sell => t.bid_price })
            .unwrap_or(dec!(150));
        let order_id = Uuid::new_v4();
        let bits = order_id.as_u128();
        let total_bp = ((bits % 3) as i64 - 1) as i32;
        let price = base_price * (dec!(1) + Decimal::from(total_bp) / dec!(10000));
        OrderResult {
            exchange:    Exchange::Mexc,
            market_type: MarketType::Spot,
            order_id:    order_id.to_string(),
            side,
            filled_qty:  quantity,
            avg_price:   price,
            fee_usdt:    quantity * price * dec!(0.00050),
            timestamp:   Utc::now(),
        }
    }
}

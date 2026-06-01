use crate::tickers::TICKERS;
use parking_lot::RwLock;
use reqwest::Client;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

#[derive(Serialize, Clone)]
pub struct MarketRow {
    pub symbol: String,
    pub binance_ask: f64,
    pub binance_bid: f64,
    pub bybit_ask: f64,
    pub bybit_bid: f64,
    pub spread_ab: f64,
    pub spread_ba: f64,
}

#[derive(Serialize, Clone)]
pub struct MarketSnapshot {
    pub spot: Vec<MarketRow>,
    pub perp: Vec<MarketRow>,
}

pub struct MarketScanner {
    spot_state: RwLock<Vec<MarketRow>>,
    perp_state: RwLock<Vec<MarketRow>>,
    http: Client,
}

impl MarketScanner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            spot_state: RwLock::new(Vec::new()),
            perp_state: RwLock::new(Vec::new()),
            http: Client::new(),
        })
    }

    pub fn full_snapshot(&self) -> MarketSnapshot {
        MarketSnapshot {
            spot: self.spot_state.read().clone(),
            perp: self.perp_state.read().clone(),
        }
    }

    pub async fn run(self: Arc<Self>) {
        info!("MarketScanner started — polling Binance+Bybit spot+perp every 2s for {} pairs", TICKERS.len());
        let mut tick = interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            if let Err(e) = self.poll_once().await {
                warn!("MarketScanner poll failed: {e}");
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let (spot_binance_res, spot_bybit_res, perp_binance_res, perp_bybit_res) = tokio::join!(
            self.fetch_binance_spot(),
            self.fetch_bybit_spot(),
            self.fetch_binance_futures(),
            self.fetch_bybit_futures(),
        );

        // Spot
        let sb = match spot_binance_res {
            Ok(m) => m,
            Err(e) => { warn!("Binance spot fetch failed: {e}"); HashMap::new() }
        };
        let sy = match spot_bybit_res {
            Ok(m) => m,
            Err(e) => { warn!("Bybit spot fetch failed: {e}"); HashMap::new() }
        };
        if !sb.is_empty() && !sy.is_empty() {
            let rows = build_rows(&sb, &sy);
            if !rows.is_empty() { *self.spot_state.write() = rows; }
        }

        // Perp
        let pb = match perp_binance_res {
            Ok(m) => m,
            Err(e) => { warn!("Binance futures fetch failed: {e}"); HashMap::new() }
        };
        let py = match perp_bybit_res {
            Ok(m) => m,
            Err(e) => { warn!("Bybit futures fetch failed: {e}"); HashMap::new() }
        };
        if !pb.is_empty() && !py.is_empty() {
            let rows = build_rows(&pb, &py);
            if !rows.is_empty() { *self.perp_state.write() = rows; }
        }

        Ok(())
    }

    async fn fetch_binance_spot(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: Vec<serde_json::Value> = self.http
            .get("https://api.binance.com/api/v3/ticker/bookTicker")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_binance_book_ticker(resp)
    }

    async fn fetch_binance_futures(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: Vec<serde_json::Value> = self.http
            .get("https://fapi.binance.com/fapi/v1/ticker/bookTicker")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_binance_book_ticker(resp)
    }

    async fn fetch_bybit_spot(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: serde_json::Value = self.http
            .get("https://api.bybit.com/v5/market/tickers?category=spot")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_bybit_tickers(resp)
    }

    async fn fetch_bybit_futures(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: serde_json::Value = self.http
            .get("https://api.bybit.com/v5/market/tickers?category=linear")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;
        parse_bybit_tickers(resp)
    }
}

fn build_rows(
    binance_map: &HashMap<String, (f64, f64)>,
    bybit_map: &HashMap<String, (f64, f64)>,
) -> Vec<MarketRow> {
    TICKERS.iter().filter_map(|&sym| {
        let &(b_bid, b_ask) = binance_map.get(sym)?;
        let &(y_bid, y_ask) = bybit_map.get(sym)?;
        let spread_ab = if b_ask > 0.0 { (y_bid - b_ask) / b_ask * 100.0 } else { 0.0 };
        let spread_ba = if y_ask > 0.0 { (b_bid - y_ask) / y_ask * 100.0 } else { 0.0 };
        Some(MarketRow {
            symbol:      sym.to_string(),
            binance_ask: b_ask,
            binance_bid: b_bid,
            bybit_ask:   y_ask,
            bybit_bid:   y_bid,
            spread_ab,
            spread_ba,
        })
    }).collect()
}

fn parse_binance_book_ticker(resp: Vec<serde_json::Value>) -> anyhow::Result<HashMap<String, (f64, f64)>> {
    let set: HashSet<&str> = TICKERS.iter().copied().collect();
    let mut map = HashMap::new();
    for item in resp {
        let sym = item["symbol"].as_str().unwrap_or("").to_string();
        if set.contains(sym.as_str()) {
            if let (Some(bid), Some(ask)) = (
                item["bidPrice"].as_str().and_then(|s| s.parse::<f64>().ok()),
                item["askPrice"].as_str().and_then(|s| s.parse::<f64>().ok()),
            ) {
                if bid > 0.0 && ask > 0.0 { map.insert(sym, (bid, ask)); }
            }
        }
    }
    Ok(map)
}

fn parse_bybit_tickers(resp: serde_json::Value) -> anyhow::Result<HashMap<String, (f64, f64)>> {
    let set: HashSet<&str> = TICKERS.iter().copied().collect();
    let mut map = HashMap::new();
    if let Some(list) = resp["result"]["list"].as_array() {
        for item in list {
            let sym = item["symbol"].as_str().unwrap_or("").to_string();
            if set.contains(sym.as_str()) {
                if let (Some(bid), Some(ask)) = (
                    item["bid1Price"].as_str().and_then(|s| s.parse::<f64>().ok()),
                    item["ask1Price"].as_str().and_then(|s| s.parse::<f64>().ok()),
                ) {
                    if bid > 0.0 && ask > 0.0 { map.insert(sym, (bid, ask)); }
                }
            }
        }
    }
    Ok(map)
}

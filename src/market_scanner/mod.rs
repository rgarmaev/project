use parking_lot::RwLock;
use reqwest::Client;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

const TICKERS: &[&str] = &[
    "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT",
    "ADAUSDT", "DOGEUSDT", "AVAXUSDT", "DOTUSDT", "MATICUSDT",
    "LINKUSDT", "LTCUSDT", "UNIUSDT", "ATOMUSDT", "BCHUSDT",
    "ICPUSDT", "APTUSDT", "ARBUSDT", "OPUSDT", "FILUSDT",
    "NEARUSDT", "SANDUSDT", "MANAUSDT", "AXSUSDT", "ALGOUSDT",
    "VETUSDT", "FTMUSDT", "HBARUSDT", "ETCUSDT", "XLMUSDT",
    "TRXUSDT", "SUIUSDT", "SEIUSDT", "INJUSDT", "TIAUSDT",
    "JUPUSDT", "WIFUSDT", "BONKUSDT", "PEPEUSDT", "SHIBUSDT",
    "NOTUSDT", "TONUSDT", "STXUSDT", "RUNEUSDT", "RENDERUSDT",
    "WLDUSDT", "ENAUSDT", "ZKUSDT", "THETAUSDT", "FLOKIUSDT",
];

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

pub struct MarketScanner {
    state: RwLock<Vec<MarketRow>>,
    http: Client,
}

impl MarketScanner {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: RwLock::new(Vec::new()),
            http: Client::new(),
        })
    }

    pub fn snapshot(&self) -> Vec<MarketRow> {
        self.state.read().clone()
    }

    pub async fn run(self: Arc<Self>) {
        info!("MarketScanner started — polling Binance+Bybit every 2s for {} pairs", TICKERS.len());
        let mut tick = interval(Duration::from_secs(2));
        loop {
            tick.tick().await;
            if let Err(e) = self.poll_once().await {
                warn!("MarketScanner poll failed: {e}");
            }
        }
    }

    async fn poll_once(&self) -> anyhow::Result<()> {
        let (binance_res, bybit_res) = tokio::join!(
            self.fetch_binance(),
            self.fetch_bybit(),
        );

        let binance_map = binance_res.unwrap_or_default();
        let bybit_map   = bybit_res.unwrap_or_default();

        if binance_map.is_empty() && bybit_map.is_empty() {
            return Ok(());
        }

        let rows: Vec<MarketRow> = TICKERS.iter().filter_map(|&sym| {
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
        }).collect();

        if !rows.is_empty() {
            *self.state.write() = rows;
        }
        Ok(())
    }

    async fn fetch_binance(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: Vec<serde_json::Value> = self.http
            .get("https://api.binance.com/api/v3/ticker/bookTicker")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;

        let set: HashSet<&str> = TICKERS.iter().copied().collect();
        let mut map = HashMap::new();
        for item in resp {
            let sym = item["symbol"].as_str().unwrap_or("").to_string();
            if set.contains(sym.as_str()) {
                let bid: f64 = item["bidPrice"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let ask: f64 = item["askPrice"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                map.insert(sym, (bid, ask));
            }
        }
        Ok(map)
    }

    async fn fetch_bybit(&self) -> anyhow::Result<HashMap<String, (f64, f64)>> {
        let resp: serde_json::Value = self.http
            .get("https://api.bybit.com/v5/market/tickers?category=spot")
            .timeout(Duration::from_secs(5))
            .send().await?
            .json().await?;

        let set: HashSet<&str> = TICKERS.iter().copied().collect();
        let mut map = HashMap::new();
        if let Some(list) = resp["result"]["list"].as_array() {
            for item in list {
                let sym = item["symbol"].as_str().unwrap_or("").to_string();
                if set.contains(sym.as_str()) {
                    let bid: f64 = item["bid1Price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let ask: f64 = item["ask1Price"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    map.insert(sym, (bid, ask));
                }
            }
        }
        Ok(map)
    }
}

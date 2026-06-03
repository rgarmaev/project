use crate::dashboard::state::{DashboardState, TradeRecord};
use crate::market_scanner::MarketSnapshot;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize)]
pub struct TradesQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    200
}

pub async fn trades_handler(
    State(state): State<Arc<DashboardState>>,
    Query(q): Query<TradesQuery>,
) -> Json<Vec<TradeRecord>> {
    Json(state.recent_trades(q.limit))
}

pub async fn restart_handler() -> Json<serde_json::Value> {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let exe = std::env::current_exe().unwrap();
        let dir = std::env::current_dir().unwrap();
        std::process::Command::new(exe).current_dir(dir).spawn().unwrap();
        std::process::exit(0);
    });
    Json(serde_json::json!({ "message": "Перезапуск..." }))
}

pub async fn market_handler(
    State(state): State<Arc<DashboardState>>,
) -> Json<MarketSnapshot> {
    Json(state.market_snapshot())
}

#[derive(Serialize)]
pub struct ExchangeQuote {
    pub bid: f64,
    pub ask: f64,
}

#[derive(Serialize)]
pub struct MultiSymbolRow {
    pub symbol:        String,
    pub binance_spot:  Option<ExchangeQuote>,
    pub bybit_spot:    Option<ExchangeQuote>,
    pub okx_spot:      Option<ExchangeQuote>,
    pub bingx_spot:    Option<ExchangeQuote>,
    pub bitget_spot:   Option<ExchangeQuote>,
    pub kucoin_spot:   Option<ExchangeQuote>,
    pub gate_spot:     Option<ExchangeQuote>,
    /// Best cross-exchange spot spread: (best_bid - best_ask) / best_ask * 100
    pub best_spread_pct: f64,
    pub best_buy:      String,  // exchange with cheapest ask
    pub best_sell:     String,  // exchange with highest bid
}

pub async fn multi_snapshot_handler(
    State(state): State<Arc<DashboardState>>,
) -> Json<Vec<MultiSymbolRow>> {
    let stale = Duration::from_secs(30);
    let mut rows: Vec<MultiSymbolRow> = Vec::new();

    for entry in state.multi_feed.iter() {
        let sym  = entry.key().clone();
        let tick = entry.value();

        // Returns spot quote if fresh, falls back to perp as indicative price
        let q = |spot: &Option<crate::multi_feed::MarketQuote>,
                  perp: &Option<crate::multi_feed::MarketQuote>| {
            let pick = |opt: &Option<crate::multi_feed::MarketQuote>| {
                opt.as_ref().filter(|q| q.updated_at.elapsed() <= stale)
                   .map(|q| ExchangeQuote { bid: q.bid, ask: q.ask })
            };
            pick(spot).or_else(|| pick(perp))
        };

        let binance = q(&tick.spot_binance, &tick.perp_binance);
        let bybit   = q(&tick.spot_bybit,   &tick.perp_bybit);
        let okx     = q(&tick.spot_okx,     &tick.perp_okx);
        let bingx   = q(&tick.spot_bingx,   &tick.perp_bingx);
        let bitget  = q(&tick.spot_bitget,  &tick.perp_bitget);
        let kucoin  = q(&tick.spot_kucoin,  &tick.perp_kucoin);
        let gate    = q(&tick.spot_gate,    &tick.perp_gate);

        // Find best cross-exchange spread
        let markets: &[(&str, &Option<ExchangeQuote>)] = &[
            ("Binance", &binance), ("Bybit", &bybit), ("OKX", &okx),
            ("BingX", &bingx),    ("Bitget", &bitget), ("KuCoin", &kucoin),
            ("Gate", &gate),
        ];

        let mut best_spread = f64::NEG_INFINITY;
        let mut best_buy    = String::new();
        let mut best_sell   = String::new();

        for (buy_name, buy_q) in markets.iter() {
            for (sell_name, sell_q) in markets.iter() {
                if buy_name == sell_name { continue; }
                if let (Some(bq), Some(sq)) = (buy_q, sell_q) {
                    if bq.ask > 0.0 && sq.bid > 0.0 {
                        // Price ratio sanity: reject if >2x apart (different contract units)
                        let ratio = sq.bid / bq.ask;
                        if ratio > 2.0 || ratio < 0.5 { continue; }
                        let spread = (sq.bid - bq.ask) / bq.ask * 100.0;
                        if spread > best_spread {
                            best_spread = spread;
                            best_buy    = buy_name.to_string();
                            best_sell   = sell_name.to_string();
                        }
                    }
                }
            }
        }

        // Only include symbols with at least 2 exchanges active
        let active = [&binance, &bybit, &okx, &bingx, &bitget, &kucoin, &gate]
            .iter().filter(|q| q.is_some()).count();
        if active < 2 { continue; }

        rows.push(MultiSymbolRow {
            symbol: sym,
            binance_spot: binance,
            bybit_spot:   bybit,
            okx_spot:     okx,
            bingx_spot:   bingx,
            bitget_spot:  bitget,
            kucoin_spot:  kucoin,
            gate_spot:    gate,
            best_spread_pct: if best_spread == f64::NEG_INFINITY { 0.0 } else { best_spread },
            best_buy,
            best_sell,
        });
    }

    rows.sort_by(|a, b| b.best_spread_pct.partial_cmp(&a.best_spread_pct).unwrap_or(std::cmp::Ordering::Equal));
    Json(rows)
}

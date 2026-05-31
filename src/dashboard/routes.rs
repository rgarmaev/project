use crate::dashboard::state::{DashboardState, TradeRecord};
use crate::market_scanner::MarketRow;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
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
) -> Json<Vec<MarketRow>> {
    Json(state.market_snapshot())
}

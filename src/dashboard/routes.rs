use crate::dashboard::state::{DashboardState, TradeRecord};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

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

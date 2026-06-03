pub mod config_api;
pub mod routes;
pub mod state;
pub mod ws;

use anyhow::Result;
use axum::{routing::get, Router};
use state::DashboardState;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::info;

pub async fn serve(state: Arc<DashboardState>, port: u16) -> Result<()> {
    let app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/api/trades", get(routes::trades_handler))
        .route("/api/config", get(config_api::get_config).post(config_api::post_config))
        .route("/api/restart", axum::routing::post(routes::restart_handler))
        .route("/api/market", get(routes::market_handler))
        .route("/api/multi-snapshot", get(routes::multi_snapshot_handler))
        .fallback_service(ServeDir::new("dashboard/dist"))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Dashboard at http://localhost:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}

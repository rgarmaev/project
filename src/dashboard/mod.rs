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
        .fallback_service(ServeDir::new("dashboard/dist"))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Dashboard at http://localhost:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}

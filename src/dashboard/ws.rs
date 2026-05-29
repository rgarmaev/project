use crate::dashboard::state::DashboardState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<DashboardState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<DashboardState>) {
    let mut rx = state.broadcast_tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
    debug!("WebSocket client disconnected");
}

pub async fn broadcast_loop(state: Arc<DashboardState>) {
    let mut interval = tokio::time::interval(Duration::from_millis(500));
    loop {
        interval.tick().await;
        let snapshot = state.build_snapshot();
        match serde_json::to_string(&snapshot) {
            Ok(json) => {
                let _ = state.broadcast_tx.send(json);
            }
            Err(e) => tracing::error!("Failed to serialize snapshot: {}", e),
        }
    }
}

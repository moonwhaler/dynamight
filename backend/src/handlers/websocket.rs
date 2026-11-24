use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;

use crate::extractors::extract_token_from_headers;
use crate::middleware::check_session;
use crate::AppState;

type WsReject = (StatusCode, Json<serde_json::Value>);

fn unauthorized(msg: &str) -> WsReject {
    (StatusCode::UNAUTHORIZED, Json(json!({ "error": msg })))
}

/// Authenticate a WebSocket handshake with the session cookie, so the token
/// never has to be exposed to JavaScript or put into a URL. The cookie is
/// `SameSite=Strict`, so browsers do not send it on cross-site handshakes
/// (no cross-site WebSocket hijacking).
async fn authenticate_ws(headers: &HeaderMap, state: &AppState) -> Result<(), WsReject> {
    let token = extract_token_from_headers(headers).ok_or_else(|| unauthorized("Not authenticated"))?;
    let claims = state
        .auth_service
        .validate_token(&token)
        .map_err(|_| unauthorized("Invalid or expired token"))?;
    check_session(state, &claims).await.map_err(|_| unauthorized("Session expired"))
}

pub async fn ws_logs_handler(
    ws: WebSocketUpgrade,
    Path(run_id): Path<i64>,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Response, WsReject> {
    authenticate_ws(&headers, &state).await?;
    Ok(ws.on_upgrade(move |socket| handle_logs_socket(socket, run_id, state)))
}

async fn handle_logs_socket(socket: WebSocket, run_id: i64, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to log broadcasts
    let mut log_rx = state.log_tx.subscribe();

    // Spawn task to forward logs to client
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = log_rx.recv().await {
            if msg.run_id == run_id {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Handle incoming messages (ping/pong, close)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(data) => {
                    // Pong is handled automatically by axum
                    let _ = data;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

pub async fn ws_status_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> Result<Response, WsReject> {
    authenticate_ws(&headers, &state).await?;
    Ok(ws.on_upgrade(move |socket| handle_status_socket(socket, state)))
}

async fn handle_status_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to log broadcasts for status updates
    let mut log_rx = state.log_tx.subscribe();

    // Track which runs we've sent start/complete for
    let mut active_runs = std::collections::HashSet::new();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = log_rx.recv().await {
            // Send status updates for job start/complete
            let status_update = if msg.message.contains("Starting backup job") {
                active_runs.insert(msg.run_id);
                Some(serde_json::json!({
                    "type": "job_started",
                    "run_id": msg.run_id,
                    "timestamp": msg.timestamp
                }))
            } else if msg.message.contains("Backup complete") && active_runs.contains(&msg.run_id) {
                active_runs.remove(&msg.run_id);
                Some(serde_json::json!({
                    "type": "job_completed",
                    "run_id": msg.run_id,
                    "timestamp": msg.timestamp
                }))
            } else {
                None
            };

            if let Some(update) = status_update {
                if let Ok(json) = serde_json::to_string(&update) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}


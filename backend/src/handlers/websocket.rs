use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
    response::Response,
    Json,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct WsAuthQuery {
    token: Option<String>,
}

/// Validate a WebSocket token and check session validity (including password change invalidation).
async fn validate_ws_token(
    token: &str,
    state: &AppState,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let claims = state.auth_service.validate_token(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or expired token"})),
        )
    })?;

    // Check if token was issued before password change (session invalidation)
    let password_changed_at: Option<(Option<chrono::DateTime<chrono::Utc>>,)> =
        sqlx::query_as("SELECT password_changed_at FROM users WHERE id = ?")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    if let Some((Some(changed_at),)) = password_changed_at {
        let token_issued_at = claims.iat as i64;
        if token_issued_at < changed_at.timestamp() {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Session expired"})),
            ));
        }
    }

    Ok(())
}

pub async fn ws_logs_handler(
    ws: WebSocketUpgrade,
    Path(run_id): Path<i64>,
    Query(query): Query<WsAuthQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Validate token from query parameter
    let token = query.token.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Missing token parameter"})),
        )
    })?;

    validate_ws_token(&token, &state).await?;

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
    Query(query): Query<WsAuthQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Validate token from query parameter
    let token = query.token.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Missing token parameter"})),
        )
    })?;

    validate_ws_token(&token, &state).await?;

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

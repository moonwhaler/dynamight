use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub max_runs_per_job: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub max_runs_per_job: Option<u32>,
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Get max_runs_per_job from database, fall back to config (env), then default to 5
    let db_value: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'max_runs_per_job'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let max_runs_per_job = match db_value {
        Some((value,)) => value.parse::<u32>().ok(),
        None => state.config.max_runs_per_job.or(Some(5)), // Default to 5 if not configured
    };

    Json(AppSettings { max_runs_per_job })
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
    // Verify authentication
    let token = headers
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let mut parts = c.trim().splitn(2, '=');
                if parts.next() == Some("token") {
                    parts.next().map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let token = match token {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Not authenticated"}))),
    };

    if state.auth_service.validate_token(&token).is_err() {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"})));
    }

    // Update or insert max_runs_per_job setting
    match req.max_runs_per_job {
        Some(value) => {
            let result = sqlx::query(
                r#"
                INSERT INTO app_settings (key, value, updated_at)
                VALUES ('max_runs_per_job', ?, CURRENT_TIMESTAMP)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
                "#
            )
            .bind(value.to_string())
            .execute(&state.db)
            .await;

            if let Err(e) = result {
                tracing::error!("Failed to update max_runs_per_job setting: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
            }

            // Update the backup service's max_runs_per_job value
            state.backup_service.set_max_runs_per_job(Some(value)).await;
        }
        None => {
            // Delete the setting to fall back to env config or default
            let _ = sqlx::query("DELETE FROM app_settings WHERE key = 'max_runs_per_job'")
                .execute(&state.db)
                .await;

            // Reset to env config value or default (5)
            state.backup_service.set_max_runs_per_job(state.config.max_runs_per_job.or(Some(5))).await;
        }
    }

    (StatusCode::OK, Json(json!({"success": true})))
}

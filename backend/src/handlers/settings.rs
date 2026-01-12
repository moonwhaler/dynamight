use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::extractors::AuthClaims;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppSettings {
    pub max_runs_per_job: Option<u32>,
    pub delete_verification_window_minutes: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub max_runs_per_job: Option<u32>,
    pub delete_verification_window_minutes: Option<u32>,
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Get max_runs_per_job from database, default to 5 if not set
    let max_runs_db: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'max_runs_per_job'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let max_runs_per_job = max_runs_db
        .and_then(|(value,)| value.parse::<u32>().ok())
        .or(Some(5));

    // Get delete_verification_window_minutes from database, default to 5 if not set
    let delete_window_db: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'delete_verification_window_minutes'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let delete_verification_window_minutes = delete_window_db
        .and_then(|(value,)| value.parse::<u32>().ok())
        .or(Some(5));

    Json(AppSettings {
        max_runs_per_job,
        delete_verification_window_minutes,
    })
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    AuthClaims(_claims): AuthClaims,
    Json(req): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
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
            // Delete the setting to fall back to default
            let _ = sqlx::query("DELETE FROM app_settings WHERE key = 'max_runs_per_job'")
                .execute(&state.db)
                .await;

            // Reset to default (5)
            state.backup_service.set_max_runs_per_job(Some(5)).await;
        }
    }

    // Update or insert delete_verification_window_minutes setting
    match req.delete_verification_window_minutes {
        Some(value) => {
            // Clamp value between 1 and 60 minutes
            let clamped_value = value.clamp(1, 60);

            let result = sqlx::query(
                r#"
                INSERT INTO app_settings (key, value, updated_at)
                VALUES ('delete_verification_window_minutes', ?, CURRENT_TIMESTAMP)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
                "#
            )
            .bind(clamped_value.to_string())
            .execute(&state.db)
            .await;

            if let Err(e) = result {
                tracing::error!("Failed to update delete_verification_window_minutes setting: {}", e);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
            }
        }
        None => {
            // Delete the setting to fall back to default (5 minutes)
            let _ = sqlx::query("DELETE FROM app_settings WHERE key = 'delete_verification_window_minutes'")
                .execute(&state.db)
                .await;
        }
    }

    (StatusCode::OK, Json(json!({"success": true})))
}

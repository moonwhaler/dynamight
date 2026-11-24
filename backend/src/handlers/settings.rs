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
    pub search_timeout_seconds: Option<u32>,
    pub show_directory_sizes: Option<bool>,
    /// Days run logs are kept; `None` keeps them forever
    pub log_retention_days: Option<u32>,
    /// Days items stay in the file manager's trash; `None` keeps them
    pub trash_retention_days: Option<u32>,
}

/// Stored value of `max_runs_per_job` that means "keep every run"
const UNLIMITED: &str = "unlimited";
/// Runs kept per job when nothing is configured
pub const DEFAULT_MAX_RUNS_PER_JOB: u32 = 5;
/// Days run logs are kept when nothing is configured
pub const DEFAULT_LOG_RETENTION_DAYS: u32 = 90;
/// Days items stay in the trash when nothing is configured
pub const DEFAULT_TRASH_RETENTION_DAYS: u32 = 30;

/// Read a stored limit: missing means `default`, "unlimited" means no
/// limit (None).
fn limit_from_setting(value: Option<&str>, default: u32) -> Option<u32> {
    match value {
        None => Some(default),
        Some(UNLIMITED) => None,
        Some(v) => Some(v.parse::<u32>().ok().filter(|n| *n > 0).unwrap_or(default)),
    }
}

/// Read the stored `max_runs_per_job` setting
pub fn max_runs_from_setting(value: Option<&str>) -> Option<u32> {
    limit_from_setting(value, DEFAULT_MAX_RUNS_PER_JOB)
}

/// Read the stored `log_retention_days` setting
pub fn log_days_from_setting(value: Option<&str>) -> Option<u32> {
    limit_from_setting(value, DEFAULT_LOG_RETENTION_DAYS)
}

/// Read the stored `trash_retention_days` setting
pub fn trash_days_from_setting(value: Option<&str>) -> Option<u32> {
    limit_from_setting(value, DEFAULT_TRASH_RETENTION_DAYS)
}

pub async fn stored_value(db: &sqlx::SqlitePool, key: &str) -> Option<String> {
    sqlx::query_as::<_, (String,)>("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|(v,)| v)
}

/// Store a limit (`None` = unlimited)
async fn store_limit(db: &sqlx::SqlitePool, key: &str, limit: Option<u32>) -> Result<(), sqlx::Error> {
    let stored = limit.map(|n| n.to_string()).unwrap_or_else(|| UNLIMITED.to_string());
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(key)
    .bind(stored)
    .execute(db)
    .await
    .map(|_| ())
}

/// For fields where "absent" and `null` differ: absent stays `None`
/// (keep the stored value), `null` becomes `Some(None)`.
fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Only the fields present in the request are changed.
#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    /// A number of runs to keep, or `null` to keep every run
    #[serde(default, deserialize_with = "present")]
    pub max_runs_per_job: Option<Option<u32>>,
    /// Days to keep run logs, or `null` to keep them forever
    #[serde(default, deserialize_with = "present")]
    pub log_retention_days: Option<Option<u32>>,
    /// Days to keep trashed files, or `null` to keep them until emptied
    #[serde(default, deserialize_with = "present")]
    pub trash_retention_days: Option<Option<u32>>,
    pub delete_verification_window_minutes: Option<u32>,
    pub search_timeout_seconds: Option<u32>,
    pub show_directory_sizes: Option<bool>,
}

pub async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Get max_runs_per_job from database, default to 5 if not set
    let max_runs_db: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'max_runs_per_job'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let max_runs_per_job = max_runs_from_setting(max_runs_db.as_ref().map(|(v,)| v.as_str()));

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

    // Get search_timeout_seconds from database, default to 10 if not set
    let search_timeout_db: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'search_timeout_seconds'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let search_timeout_seconds = search_timeout_db
        .and_then(|(value,)| value.parse::<u32>().ok())
        .or(Some(10));

    // Get show_directory_sizes from database, default to true if not set
    let show_dir_sizes_db: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'show_directory_sizes'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let show_directory_sizes = Some(
        show_dir_sizes_db
            .map(|(value,)| value == "true")
            .unwrap_or(true),
    );

    let log_retention_days = log_days_from_setting(stored_value(&state.db, "log_retention_days").await.as_deref());
    let trash_retention_days =
        trash_days_from_setting(stored_value(&state.db, "trash_retention_days").await.as_deref());

    Json(AppSettings {
        log_retention_days,
        trash_retention_days,
        max_runs_per_job,
        delete_verification_window_minutes,
        search_timeout_seconds,
        show_directory_sizes,
    })
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    AuthClaims(_claims): AuthClaims,
    Json(req): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
    // max_runs_per_job: absent = unchanged, null = keep every run
    if let Some(limit) = req.max_runs_per_job {
        let limit = limit.map(|n| n.max(1));
        if let Err(e) = store_limit(&state.db, "max_runs_per_job", limit).await {
            tracing::error!("Failed to update max_runs_per_job setting: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
        }

        state.backup_service.set_max_runs_per_job(limit).await;
    }

    // log_retention_days: absent = unchanged, null = keep every log
    if let Some(days) = req.log_retention_days {
        let days = days.map(|n| n.max(1));
        if let Err(e) = store_limit(&state.db, "log_retention_days", days).await {
            tracing::error!("Failed to update log_retention_days setting: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
        }
        state.backup_service.set_log_retention_days(days).await;
        // Apply a shorter limit now instead of at the next maintenance
        let service = Arc::clone(&state.backup_service);
        tokio::spawn(async move { service.maintain_logs().await });
    }

    // trash_retention_days: absent = unchanged, null = keep until emptied
    if let Some(days) = req.trash_retention_days {
        let days = days.map(|n| n.max(1));
        if let Err(e) = store_limit(&state.db, "trash_retention_days", days).await {
            tracing::error!("Failed to update trash_retention_days setting: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
        }
        // Apply a shorter limit now instead of at the next hourly cleanup
        let db = state.db.clone();
        let allowed = state.config.allowed_browse_paths.clone();
        tokio::spawn(async move { crate::handlers::files::purge_expired_trash(&db, &allowed).await });
    }

    // Update delete_verification_window_minutes when present
    if let Some(value) = req.delete_verification_window_minutes {
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

    // Update or insert search_timeout_seconds setting
    if let Some(value) = req.search_timeout_seconds {
        // Clamp value between 3 and 60 seconds
        let clamped_value = value.clamp(3, 60);

        let result = sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES ('search_timeout_seconds', ?, CURRENT_TIMESTAMP)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(clamped_value.to_string())
        .execute(&state.db)
        .await;

        if let Err(e) = result {
            tracing::error!("Failed to update search_timeout_seconds setting: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
        }

        // Update in-memory value
        *state.search_timeout_seconds.write().await = clamped_value;
    }

    // Update or insert show_directory_sizes setting
    if let Some(value) = req.show_directory_sizes {
        let result = sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES ('show_directory_sizes', ?, CURRENT_TIMESTAMP)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
            "#
        )
        .bind(value.to_string())
        .execute(&state.db)
        .await;

        if let Err(e) = result {
            tracing::error!("Failed to update show_directory_sizes setting: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to save setting"})));
        }

        // Update in-memory value
        *state.show_directory_sizes.write().await = value;
    }

    (StatusCode::OK, Json(json!({"success": true})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_limit_is_read_back() {
        assert_eq!(max_runs_from_setting(None), Some(DEFAULT_MAX_RUNS_PER_JOB));
        assert_eq!(max_runs_from_setting(Some("unlimited")), None);
        assert_eq!(max_runs_from_setting(Some("20")), Some(20));
        assert_eq!(max_runs_from_setting(Some("0")), Some(DEFAULT_MAX_RUNS_PER_JOB));
        assert_eq!(max_runs_from_setting(Some("x")), Some(DEFAULT_MAX_RUNS_PER_JOB));
    }

    #[test]
    fn absent_and_null_differ() {
        let absent: UpdateSettingsRequest = serde_json::from_str(r#"{"search_timeout_seconds": 5}"#).unwrap();
        assert_eq!(absent.max_runs_per_job, None);
        let null: UpdateSettingsRequest = serde_json::from_str(r#"{"max_runs_per_job": null}"#).unwrap();
        assert_eq!(null.max_runs_per_job, Some(None));
        let n: UpdateSettingsRequest = serde_json::from_str(r#"{"max_runs_per_job": 12}"#).unwrap();
        assert_eq!(n.max_runs_per_job, Some(Some(12)));
    }
}

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::models::{JobRun, JobRunRow, LogEntry, LogEntryRow};
use crate::AppState;

#[derive(Deserialize)]
pub struct ListRunsQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<i64>,
    Query(query): Query<ListRunsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let runs: Vec<JobRunRow> = sqlx::query_as(
        "SELECT * FROM job_runs WHERE job_id = ? ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(job_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let response: Vec<JobRun> = runs.into_iter().map(JobRun::from).collect();

    Json(response)
}

pub async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let run: Option<JobRunRow> = sqlx::query_as("SELECT * FROM job_runs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    match run {
        Some(r) => Json(JobRun::from(r)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Run not found"}))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct GetLogsQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub level: Option<String>,
}

pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<i64>,
    Query(query): Query<GetLogsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(1000);
    let offset = query.offset.unwrap_or(0);

    let logs: Vec<LogEntryRow> = if let Some(level) = &query.level {
        sqlx::query_as(
            "SELECT * FROM log_entries WHERE job_run_id = ? AND level = ? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(run_id)
        .bind(level)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT * FROM log_entries WHERE job_run_id = ? ORDER BY id LIMIT ? OFFSET ?",
        )
        .bind(run_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let response: Vec<LogEntry> = logs.into_iter().map(LogEntry::from).collect();

    Json(response)
}

/// Delete a single run and its logs
pub async fn delete_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<i64>,
) -> impl IntoResponse {
    // Log entries are deleted via CASCADE
    let result = sqlx::query("DELETE FROM job_runs WHERE id = ?")
        .bind(run_id)
        .execute(&state.db)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            Json(json!({"success": true})).into_response()
        }
        Ok(_) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "Run not found"}))).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete run {}: {}", run_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to delete run"}))).into_response()
        }
    }
}

/// Delete all runs for a specific job
pub async fn delete_job_runs(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<i64>,
) -> impl IntoResponse {
    // Log entries are deleted via CASCADE
    let result = sqlx::query("DELETE FROM job_runs WHERE job_id = ?")
        .bind(job_id)
        .execute(&state.db)
        .await;

    match result {
        Ok(r) => {
            Json(json!({"success": true, "deleted": r.rows_affected()})).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete runs for job {}: {}", job_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to delete runs"}))).into_response()
        }
    }
}

/// Delete all runs (purge all history)
pub async fn purge_all_runs(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Log entries are deleted via CASCADE
    let result = sqlx::query("DELETE FROM job_runs")
        .execute(&state.db)
        .await;

    match result {
        Ok(r) => {
            Json(json!({"success": true, "deleted": r.rows_affected()})).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to purge all runs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to purge runs"}))).into_response()
        }
    }
}

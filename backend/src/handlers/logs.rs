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

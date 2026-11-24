use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{QueryBuilder, Sqlite};
use std::sync::Arc;

use crate::errors::ApiError;
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

#[derive(Deserialize, Default)]
pub struct RecentRunsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Only runs with a smaller id (cursor for "load more" that is not
    /// shifted by runs started or deleted in between)
    pub before: Option<i64>,
    pub job_id: Option<i64>,
    /// Comma-separated statuses, e.g. `failed,cancelled`
    pub status: Option<String>,
    /// Only runs started at or after this time
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    /// Only runs started at or before this time
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

const RUN_STATUSES: [&str; 5] = ["pending", "running", "completed", "failed", "cancelled"];

/// Runs across all jobs, newest first, with optional filters
/// (dashboard "recent runs" and the paged history).
pub async fn list_recent_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecentRunsQuery>,
) -> impl IntoResponse {
    Json(query_runs(&state.db, &query).await.unwrap_or_default())
}

async fn query_runs(db: &sqlx::SqlitePool, query: &RecentRunsQuery) -> Result<Vec<JobRun>, sqlx::Error> {
    // A negative LIMIT means "no limit" in SQLite
    let limit = query.limit.unwrap_or(10).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut qb = sqlx::QueryBuilder::new("SELECT * FROM job_runs WHERE 1 = 1");
    if let Some(before) = query.before {
        qb.push(" AND id < ").push_bind(before);
    }
    if let Some(job_id) = query.job_id {
        qb.push(" AND job_id = ").push_bind(job_id);
    }
    if let Some(status) = &query.status {
        // Unknown names are dropped; if none is left the filter matches nothing
        let statuses: Vec<&str> = status
            .split(',')
            .map(str::trim)
            .filter(|s| RUN_STATUSES.contains(s))
            .collect();
        qb.push(" AND status IN (");
        if statuses.is_empty() {
            qb.push("NULL");
        }
        let mut list = qb.separated(", ");
        for s in statuses {
            list.push_bind(s);
        }
        qb.push(")");
    }
    // julianday() parses the stored RFC 3339 text, so differing precision
    // of the fractional seconds does not affect the comparison
    if let Some(from) = query.from {
        qb.push(" AND julianday(started_at) >= julianday(").push_bind(from.to_rfc3339()).push(")");
    }
    if let Some(to) = query.to {
        qb.push(" AND julianday(started_at) <= julianday(").push_bind(to.to_rfc3339()).push(")");
    }
    qb.push(" ORDER BY id DESC LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);

    let rows: Vec<JobRunRow> = qb.build_query_as().fetch_all(db).await?;
    Ok(rows.into_iter().map(JobRun::from).collect())
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
        None => ApiError::run_not_found().into_response(),
    }
}

#[derive(Deserialize)]
pub struct GetLogsQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    /// One level or a comma-separated list ("warning,error")
    pub level: Option<String>,
    /// Only lines whose message contains this text (case-insensitive)
    pub q: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedLogsResponse {
    pub entries: Vec<LogEntry>,
    pub total: i64,
    pub offset: i32,
    pub limit: i32,
    pub has_more: bool,
}

/// Append the filters of a log query to a statement that already selects one run
fn push_log_filters<'a>(qb: &mut QueryBuilder<'a, Sqlite>, levels: &'a [String], q: Option<&'a str>) {
    if !levels.is_empty() {
        qb.push(" AND level IN (");
        let mut sep = qb.separated(", ");
        for level in levels {
            sep.push_bind(level);
        }
        qb.push(")");
    }
    if let Some(q) = q {
        // LIKE with the wildcards in the search text escaped
        let pattern = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        qb.push(" AND message LIKE ").push_bind(pattern).push(" ESCAPE '\\'");
    }
}

pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<i64>,
    Query(query): Query<GetLogsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(500).clamp(1, 5000);
    let offset = query.offset.unwrap_or(0).max(0);
    let levels: Vec<String> = query
        .level
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty())
        .collect();
    let q = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());

    let mut count = QueryBuilder::new("SELECT COUNT(*) FROM log_entries WHERE job_run_id = ");
    count.push_bind(run_id);
    push_log_filters(&mut count, &levels, q);
    let total: i64 = count
        .build_query_scalar()
        .fetch_one(&state.logs_db)
        .await
        .unwrap_or(0);

    let mut select = QueryBuilder::new("SELECT * FROM log_entries WHERE job_run_id = ");
    select.push_bind(run_id);
    push_log_filters(&mut select, &levels, q);
    select.push(" ORDER BY id LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    let logs: Vec<LogEntryRow> = select
        .build_query_as()
        .fetch_all(&state.logs_db)
        .await
        .unwrap_or_default();

    let entries: Vec<LogEntry> = logs.into_iter().map(LogEntry::from).collect();
    let fetched = entries.len() as i32;
    let has_more = (offset + fetched) < total as i32;

    Json(PaginatedLogsResponse {
        entries,
        total,
        offset,
        limit,
        has_more,
    })
}

/// Delete a single run and its logs
pub async fn delete_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<i64>,
) -> impl IntoResponse {
    // Delete log entries from the separate logs database first
    if let Err(e) = state.backup_service.delete_logs_for_run(run_id).await {
        tracing::error!("Failed to delete logs for run {}: {}", run_id, e);
    }

    // Then delete the run from main database
    let result = sqlx::query("DELETE FROM job_runs WHERE id = ?")
        .bind(run_id)
        .execute(&state.db)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            Json(json!({"success": true})).into_response()
        }
        Ok(_) => ApiError::run_not_found().into_response(),
        Err(e) => {
            tracing::error!("Failed to delete run {}: {}", run_id, e);
            ApiError::internal_error().into_response()
        }
    }
}

/// Delete all runs for a specific job
pub async fn delete_job_runs(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<i64>,
) -> impl IntoResponse {
    // Delete log entries from the separate logs database first
    if let Err(e) = state.backup_service.delete_logs_for_job(job_id).await {
        tracing::error!("Failed to delete logs for job {}: {}", job_id, e);
    }

    // Then delete the runs from main database
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
            ApiError::internal_error().into_response()
        }
    }
}

/// Delete all runs (purge all history)
pub async fn purge_all_runs(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Delete all log entries from the separate logs database first
    if let Err(e) = state.backup_service.delete_all_logs().await {
        tracing::error!("Failed to purge all logs: {}", e);
    }

    // Then delete all runs from main database
    let result = sqlx::query("DELETE FROM job_runs")
        .execute(&state.db)
        .await;

    match result {
        Ok(r) => {
            Json(json!({"success": true, "deleted": r.rows_affected()})).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to purge all runs: {}", e);
            ApiError::internal_error().into_response()
        }
    }
}

#[cfg(test)]
mod recent_runs_tests {
    use super::*;

    async fn insert_run(db: &sqlx::SqlitePool, job_id: i64, status: &str, started: Option<&str>) {
        sqlx::query("INSERT INTO job_runs (job_id, status, started_at) VALUES (?, ?, ?)")
            .bind(job_id)
            .bind(status)
            .bind(started)
            .execute(db)
            .await
            .unwrap();
    }

    async fn setup() -> sqlx::SqlitePool {
        let db = crate::test_helpers::test_db().await;
        for name in ["a", "b"] {
            sqlx::query(
                "INSERT INTO jobs (name, enabled, mount_point, source_dirs, destination_type, destination_config) \
                 VALUES (?, 1, '', '[]', 'local', '{}')",
            )
            .bind(name)
            .execute(&db)
            .await
            .unwrap();
        }
        insert_run(&db, 1, "completed", Some("2026-01-01T10:00:00.123456789+00:00")).await;
        insert_run(&db, 2, "failed", Some("2026-01-02T10:00:00+00:00")).await;
        insert_run(&db, 1, "failed", Some("2026-01-03T10:00:00.5+00:00")).await;
        insert_run(&db, 2, "pending", None).await;
        db
    }

    fn ids(runs: &[JobRun]) -> Vec<i64> {
        runs.iter().map(|r| r.id).collect()
    }

    fn at(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        Some(s.parse().unwrap())
    }

    #[tokio::test]
    async fn newest_first_with_paging() {
        let db = setup().await;
        let q = RecentRunsQuery { limit: Some(2), ..Default::default() };
        assert_eq!(ids(&query_runs(&db, &q).await.unwrap()), vec![4, 3]);
        let q = RecentRunsQuery { limit: Some(2), offset: Some(2), ..Default::default() };
        assert_eq!(ids(&query_runs(&db, &q).await.unwrap()), vec![2, 1]);
        let q = RecentRunsQuery { limit: Some(2), before: Some(3), ..Default::default() };
        assert_eq!(ids(&query_runs(&db, &q).await.unwrap()), vec![2, 1]);
    }

    #[tokio::test]
    async fn filters_by_job_status_and_date() {
        let db = setup().await;
        let q = RecentRunsQuery { job_id: Some(1), limit: Some(50), ..Default::default() };
        assert_eq!(ids(&query_runs(&db, &q).await.unwrap()), vec![3, 1]);

        let q = RecentRunsQuery { status: Some("failed, bogus".into()), limit: Some(50), ..Default::default() };
        assert_eq!(ids(&query_runs(&db, &q).await.unwrap()), vec![3, 2]);

        let q = RecentRunsQuery { status: Some("bogus".into()), limit: Some(50), ..Default::default() };
        assert!(query_runs(&db, &q).await.unwrap().is_empty());

        // Boundaries include runs whose stored time has more or fewer fraction digits
        let q = RecentRunsQuery {
            from: at("2026-01-01T10:00:00.123456789Z"),
            to: at("2026-01-02T10:00:00Z"),
            limit: Some(50),
            ..Default::default()
        };
        assert_eq!(ids(&query_runs(&db, &q).await.unwrap()), vec![2, 1]);
    }
}

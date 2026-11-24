use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;

use crate::errors::{ApiError, ErrorCode};
use crate::models::{parse_cron, CreateScheduleRequest, Schedule, UpdateScheduleRequest};
use crate::AppState;

pub async fn list_schedules(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<i64>,
) -> impl IntoResponse {
    let schedules: Vec<Schedule> =
        sqlx::query_as("SELECT * FROM schedules WHERE job_id = ? ORDER BY id")
            .bind(job_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    Json(schedules)
}

#[derive(serde::Deserialize)]
pub struct ValidateCronRequest {
    pub cron_expression: String,
}

/// Check a cron expression without saving it, so the job form can keep
/// schedule changes as a draft until the job is saved. Returns the next run
/// time in the server's schedule timezone.
pub async fn validate_cron(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidateCronRequest>,
) -> impl IntoResponse {
    let Ok(cron_schedule) = parse_cron(req.cron_expression.trim()) else {
        return ApiError::invalid_cron().into_response();
    };
    let next_run = cron_schedule
        .upcoming(state.config.timezone)
        .next()
        .map(|dt| dt.with_timezone(&Utc));
    Json(json!({ "next_run_at": next_run })).into_response()
}

pub async fn create_schedule(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<i64>,
    Json(req): Json<CreateScheduleRequest>,
) -> impl IntoResponse {
    // Check if job exists
    let job_exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM jobs WHERE id = ?")
        .bind(job_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    if job_exists.is_none() {
        return ApiError::job_not_found().into_response();
    }

    let cron_expression = req.to_cron_expression();

    // Validate cron expression
    let Ok(cron_schedule) = parse_cron(&cron_expression) else {
        return ApiError::invalid_cron().into_response();
    };

    // Calculate next run
    let tz = state.config.timezone;
    let next_run = cron_schedule
        .upcoming(tz)
        .next()
        .map(|dt| dt.with_timezone(&Utc));

    let result = sqlx::query(
        r#"
        INSERT INTO schedules (
            job_id, enabled, cron_expression,
            schedule_type, time_of_day, day_of_week, day_of_month,
            next_run_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(job_id)
    .bind(req.enabled.unwrap_or(true))
    .bind(&cron_expression)
    .bind(&req.schedule_type)
    .bind(&req.time_of_day)
    .bind(req.day_of_week)
    .bind(req.day_of_month)
    .bind(next_run)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => {
            let id = r.last_insert_rowid();
            let schedule: Option<Schedule> =
                sqlx::query_as("SELECT * FROM schedules WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&state.db)
                    .await
                    .unwrap_or(None);

            match schedule {
                Some(s) => (StatusCode::CREATED, Json(s)).into_response(),
                None => ApiError::new(ErrorCode::ScheduleCreateFailed).into_response(),
            }
        }
        Err(_) => ApiError::new(ErrorCode::ScheduleCreateFailed).into_response(),
    }
}

pub async fn update_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateScheduleRequest>,
) -> impl IntoResponse {
    // Get existing schedule
    let existing: Option<Schedule> = sqlx::query_as("SELECT * FROM schedules WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let existing = match existing {
        Some(s) => s,
        None => {
            return ApiError::schedule_not_found().into_response();
        }
    };

    let enabled = req.enabled.unwrap_or(existing.enabled);
    let schedule_type = req.schedule_type.or(existing.schedule_type);
    let time_of_day = req.time_of_day.or(existing.time_of_day);
    let day_of_week = req.day_of_week.or(existing.day_of_week);
    let day_of_month = req.day_of_month.or(existing.day_of_month);

    // Build cron expression
    let cron_expression = if let Some(cron) = req.cron_expression {
        cron
    } else if schedule_type.is_some() || time_of_day.is_some() {
        let time = time_of_day.as_deref().unwrap_or("00:00");
        let parts: Vec<&str> = time.split(':').collect();
        let hour = parts.first().unwrap_or(&"0");
        let minute = parts.get(1).unwrap_or(&"0");

        match schedule_type.as_deref() {
            Some("daily") => format!("{} {} * * *", minute, hour),
            Some("weekly") => {
                let dow = day_of_week.unwrap_or(0);
                format!("{} {} * * {}", minute, hour, dow)
            }
            Some("monthly") => {
                let dom = day_of_month.unwrap_or(1);
                format!("{} {} {} * *", minute, hour, dom)
            }
            _ => existing.cron_expression.clone(),
        }
    } else {
        existing.cron_expression.clone()
    };

    // Validate cron expression
    let Ok(cron_schedule) = parse_cron(&cron_expression) else {
        return ApiError::invalid_cron().into_response();
    };

    // Calculate next run
    let tz = state.config.timezone;
    let next_run = if enabled {
        cron_schedule
            .upcoming(tz)
            .next()
            .map(|dt| dt.with_timezone(&Utc))
    } else {
        None
    };

    let result = sqlx::query(
        r#"
        UPDATE schedules SET
            enabled = ?, cron_expression = ?,
            schedule_type = ?, time_of_day = ?, day_of_week = ?, day_of_month = ?,
            next_run_at = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(enabled)
    .bind(&cron_expression)
    .bind(&schedule_type)
    .bind(&time_of_day)
    .bind(day_of_week)
    .bind(day_of_month)
    .bind(next_run)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let schedule: Option<Schedule> =
                sqlx::query_as("SELECT * FROM schedules WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&state.db)
                    .await
                    .unwrap_or(None);

            match schedule {
                Some(s) => Json(s).into_response(),
                None => ApiError::new(ErrorCode::ScheduleUpdateFailed).into_response(),
            }
        }
        Err(_) => ApiError::new(ErrorCode::ScheduleUpdateFailed).into_response(),
    }
}

pub async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM schedules WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => Json(json!({"success": true})).into_response(),
        Ok(_) => ApiError::schedule_not_found().into_response(),
        Err(_) => ApiError::new(ErrorCode::ScheduleDeleteFailed).into_response(),
    }
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;

use crate::models::{CreateJobRequest, Job, JobResponse, JobRunStatus, UpdateJobRequest};
use crate::AppState;

pub async fn list_jobs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let jobs: Vec<Job> = sqlx::query_as("SELECT * FROM jobs ORDER BY name")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    // Get last run status for each job
    let last_runs: Vec<(i64, String, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT job_id, status, COALESCE(completed_at, started_at) as run_at
        FROM job_runs jr1
        WHERE id = (
            SELECT id FROM job_runs jr2
            WHERE jr2.job_id = jr1.job_id
            ORDER BY COALESCE(completed_at, started_at) DESC
            LIMIT 1
        )
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let response: Vec<JobResponse> = jobs
        .into_iter()
        .map(|job| {
            let job_id = job.id;
            let run_info = last_runs.iter().find(|(id, _, _)| *id == job_id);
            let (status, run_at) = run_info
                .map(|(_, s, t)| (Some(s.clone()), *t))
                .unwrap_or((None, None));
            JobResponse::from(job).with_run_status(status, run_at)
        })
        .collect();

    Json(response)
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    match job {
        Some(j) => Json(JobResponse::from(j)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"}))).into_response(),
    }
}

pub async fn create_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJobRequest>,
) -> impl IntoResponse {
    // Validate at least one source directory
    if req.source_dirs.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "At least one source directory must be specified"})),
        )
            .into_response();
    }

    // Check for duplicate job name
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM jobs WHERE name = ?")
        .bind(&req.name)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    if existing.is_some() {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "A job with this name already exists"})),
        )
            .into_response();
    }

    let source_dirs_json = serde_json::to_string(&req.source_dirs).unwrap_or_default();
    let excludes_json = req
        .rsync_excludes
        .as_ref()
        .map(|e| serde_json::to_string(e).unwrap_or_default());

    let result = sqlx::query(
        r#"
        INSERT INTO jobs (
            name, description, enabled,
            usb_uuid, mount_point, auto_mount, auto_unmount,
            source_dirs, backup_subdir,
            sync_deletes, rsync_excludes, checksum_mode, compress, dry_run, bandwidth_limit, verbosity
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(req.enabled.unwrap_or(true))
    .bind(&req.usb_uuid)
    .bind(&req.mount_point)
    .bind(req.auto_mount.unwrap_or(true))
    .bind(req.auto_unmount.unwrap_or(true))
    .bind(&source_dirs_json)
    .bind(req.backup_subdir.as_deref().unwrap_or("backups"))
    .bind(req.sync_deletes.unwrap_or(false))
    .bind(&excludes_json)
    .bind(req.checksum_mode.unwrap_or(false))
    .bind(req.compress.unwrap_or(false))
    .bind(req.dry_run.unwrap_or(false))
    .bind(req.bandwidth_limit)
    .bind(req.verbosity.as_deref().unwrap_or("normal"))
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => {
            let id = r.last_insert_rowid();
            let job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
                .bind(id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None);

            match job {
                Some(j) => (StatusCode::CREATED, Json(JobResponse::from(j))).into_response(),
                None => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to fetch created job"})),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create job: {}", e)})),
        )
            .into_response(),
    }
}

pub async fn update_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateJobRequest>,
) -> impl IntoResponse {
    // Check if job exists
    let existing: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    if existing.is_none() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"}))).into_response();
    }

    let existing = existing.unwrap();

    // Validate source directories if provided
    if let Some(ref source_dirs) = req.source_dirs {
        if source_dirs.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "At least one source directory must be specified"})),
            )
                .into_response();
        }
    }

    // Check for duplicate job name (exclude current job)
    if let Some(ref new_name) = req.name {
        let duplicate: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM jobs WHERE name = ? AND id != ?")
                .bind(new_name)
                .bind(id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None);

        if duplicate.is_some() {
            return (
                StatusCode::CONFLICT,
                Json(json!({"error": "A job with this name already exists"})),
            )
                .into_response();
        }
    }

    // Build update with coalesced values
    let name = req.name.unwrap_or(existing.name);
    let description = req.description.or(existing.description);
    let enabled = req.enabled.unwrap_or(existing.enabled);
    let usb_uuid = req.usb_uuid.or(existing.usb_uuid);
    let mount_point = req.mount_point.unwrap_or(existing.mount_point);
    let auto_mount = req.auto_mount.unwrap_or(existing.auto_mount);
    let auto_unmount = req.auto_unmount.unwrap_or(existing.auto_unmount);
    let source_dirs = req
        .source_dirs
        .map(|s| serde_json::to_string(&s).unwrap_or_default())
        .unwrap_or(existing.source_dirs);
    let backup_subdir = req.backup_subdir.unwrap_or(existing.backup_subdir);
    let sync_deletes = req.sync_deletes.unwrap_or(existing.sync_deletes);
    let rsync_excludes = req
        .rsync_excludes
        .map(|e| serde_json::to_string(&e).unwrap_or_default())
        .or(existing.rsync_excludes);
    let checksum_mode = req.checksum_mode.unwrap_or(existing.checksum_mode);
    let compress = req.compress.unwrap_or(existing.compress);
    let dry_run = req.dry_run.unwrap_or(existing.dry_run);
    let bandwidth_limit = req.bandwidth_limit.or(existing.bandwidth_limit);
    let verbosity = req.verbosity.unwrap_or(existing.verbosity);

    let result = sqlx::query(
        r#"
        UPDATE jobs SET
            name = ?, description = ?, enabled = ?,
            usb_uuid = ?, mount_point = ?, auto_mount = ?, auto_unmount = ?,
            source_dirs = ?, backup_subdir = ?,
            sync_deletes = ?, rsync_excludes = ?, checksum_mode = ?, compress = ?, dry_run = ?, bandwidth_limit = ?, verbosity = ?,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(&name)
    .bind(&description)
    .bind(enabled)
    .bind(&usb_uuid)
    .bind(&mount_point)
    .bind(auto_mount)
    .bind(auto_unmount)
    .bind(&source_dirs)
    .bind(&backup_subdir)
    .bind(sync_deletes)
    .bind(&rsync_excludes)
    .bind(checksum_mode)
    .bind(compress)
    .bind(dry_run)
    .bind(bandwidth_limit)
    .bind(&verbosity)
    .bind(id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
                .bind(id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None);

            match job {
                Some(j) => Json(JobResponse::from(j)).into_response(),
                None => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to fetch updated job"})),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update job: {}", e)})),
        )
            .into_response(),
    }
}

pub async fn delete_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let result = sqlx::query("DELETE FROM jobs WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"success": true}))).into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to delete job: {}", e)})),
        )
            .into_response(),
    }
}

pub async fn run_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Get job
    let job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let job = match job {
        Some(j) => j,
        None => {
            return (StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"}))).into_response()
        }
    };

    // Check if already running
    if state.backup_service.is_job_running(id).await {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "Job is already running"})),
        )
            .into_response();
    }

    // Create job run
    let result = sqlx::query("INSERT INTO job_runs (job_id, status) VALUES (?, ?)")
        .bind(id)
        .bind(JobRunStatus::Pending.as_str())
        .execute(&state.db)
        .await;

    let run_id = match result {
        Ok(r) => r.last_insert_rowid(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create job run: {}", e)})),
            )
                .into_response()
        }
    };

    // Spawn job execution
    let backup_service = state.backup_service.clone();
    let db = state.db.clone();

    tokio::spawn(async move {
        // Update status to running
        let _ = sqlx::query("UPDATE job_runs SET status = ?, started_at = ? WHERE id = ?")
            .bind(JobRunStatus::Running.as_str())
            .bind(Utc::now())
            .bind(run_id)
            .execute(&db)
            .await;

        // Execute
        let result = backup_service.execute_job(&job, run_id, None).await;

        // Update completion status (but don't overwrite 'cancelled' status)
        let (exit_code, files, bytes, errors) = match &result {
            Ok(r) => (r.exit_code, r.files_transferred, r.bytes_transferred, r.error_count),
            Err(_) => (1, 0, 0, 1),
        };

        // Check database for current status - if cancelled, preserve it
        let current_status: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM job_runs WHERE id = ?"
        )
        .bind(run_id)
        .fetch_optional(&db)
        .await
        .ok()
        .flatten();

        let was_cancelled = current_status.as_ref().map(|s| s.0.as_str()) == Some("cancelled");

        if was_cancelled {
            // Only update stats, preserve 'cancelled' status
            let _ = sqlx::query(
                r#"
                UPDATE job_runs
                SET exit_code = ?, files_transferred = ?, bytes_transferred = ?, error_count = ?
                WHERE id = ? AND status = 'cancelled'
                "#,
            )
            .bind(exit_code)
            .bind(files)
            .bind(bytes)
            .bind(errors)
            .bind(run_id)
            .execute(&db)
            .await;
        } else {
            let status = match &result {
                Ok(r) => if r.error_count > 0 { JobRunStatus::Failed } else { JobRunStatus::Completed },
                Err(_) => JobRunStatus::Failed,
            };

            let _ = sqlx::query(
                r#"
                UPDATE job_runs
                SET status = ?, completed_at = ?, exit_code = ?,
                    files_transferred = ?, bytes_transferred = ?, error_count = ?
                WHERE id = ?
                "#,
            )
            .bind(status.as_str())
            .bind(Utc::now())
            .bind(exit_code)
            .bind(files)
            .bind(bytes)
            .bind(errors)
            .bind(run_id)
            .execute(&db)
            .await;
        }

        // Cleanup old runs
        backup_service.cleanup_old_runs(id).await;
    });

    (StatusCode::OK, Json(json!({"runId": run_id}))).into_response()
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    tracing::info!("Cancel request received for job_id: {}", id);

    // Find running job run (check for both 'running' and 'cancelled' status - may still have process running)
    let run: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM job_runs WHERE job_id = ? AND status IN ('running', 'cancelled') ORDER BY id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    match run {
        Some((run_id,)) => {
            tracing::info!("Found run_id: {} for job_id: {}", run_id, id);
            match state.backup_service.cancel_job(run_id).await {
                Ok(killed) => {
                    tracing::info!("cancel_job returned: killed={}", killed);
                    (StatusCode::OK, Json(json!({
                        "success": true,
                        "processKilled": killed
                    })))
                },
                Err(e) => {
                    tracing::error!("cancel_job error: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                        "error": format!("Failed to cancel: {}", e)
                    })))
                },
            }
        }
        None => {
            tracing::warn!("No running job found for job_id: {}", id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "No running job found"})),
            )
        },
    }
}

pub async fn clone_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Fetch the job to clone
    let job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let job = match job {
        Some(j) => j,
        None => {
            return (StatusCode::NOT_FOUND, Json(json!({"error": "Job not found"}))).into_response()
        }
    };

    // Generate a unique clone name
    let base_name = job.name.trim_end_matches(|c: char| c.is_ascii_digit() || c == ')' || c == '(' || c == ' ');
    let base_name = base_name.trim_end_matches(" (clone");
    let clone_name = generate_unique_clone_name(&state.db, base_name).await;

    // Insert the cloned job
    let result = sqlx::query(
        r#"
        INSERT INTO jobs (
            name, description, enabled,
            usb_uuid, mount_point, auto_mount, auto_unmount,
            source_dirs, backup_subdir,
            sync_deletes, rsync_excludes, checksum_mode, compress, dry_run, bandwidth_limit, verbosity
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&clone_name)
    .bind(&job.description)
    .bind(job.enabled)
    .bind(&job.usb_uuid)
    .bind(&job.mount_point)
    .bind(job.auto_mount)
    .bind(job.auto_unmount)
    .bind(&job.source_dirs)
    .bind(&job.backup_subdir)
    .bind(job.sync_deletes)
    .bind(&job.rsync_excludes)
    .bind(job.checksum_mode)
    .bind(job.compress)
    .bind(job.dry_run)
    .bind(job.bandwidth_limit)
    .bind(&job.verbosity)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => {
            let new_id = r.last_insert_rowid();
            let new_job: Option<Job> = sqlx::query_as("SELECT * FROM jobs WHERE id = ?")
                .bind(new_id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None);

            match new_job {
                Some(j) => (StatusCode::CREATED, Json(JobResponse::from(j))).into_response(),
                None => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to fetch cloned job"})),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to clone job: {}", e)})),
        )
            .into_response(),
    }
}

async fn generate_unique_clone_name(db: &sqlx::SqlitePool, base_name: &str) -> String {
    let first_attempt = format!("{} (clone)", base_name);

    // Check if the simple "(clone)" name is available
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM jobs WHERE name = ?")
        .bind(&first_attempt)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    if exists.is_none() {
        return first_attempt;
    }

    // Find the next available number
    let mut counter = 2;
    loop {
        let candidate = format!("{} (clone {})", base_name, counter);
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM jobs WHERE name = ?")
            .bind(&candidate)
            .fetch_optional(db)
            .await
            .unwrap_or(None);

        if exists.is_none() {
            return candidate;
        }
        counter += 1;

        // Safety limit
        if counter > 100 {
            return format!("{} (clone {})", base_name, chrono::Utc::now().timestamp());
        }
    }
}

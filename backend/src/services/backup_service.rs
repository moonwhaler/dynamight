use crate::models::{DestinationConfig, Job, JobRunStatus, LogLevel, LogMessage};
use crate::services::compress_service;
use crate::services::credential_service::CredentialService;
use crate::services::providers::{self, RsyncProvider, StorageInfo, SyncContext, ProviderError};
use chrono::Utc;
use sqlx::{QueryBuilder, SqlitePool};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

/// Logged (source "system") when a run starts; the status WebSocket turns it
/// into a `job_started` event.
pub const RUN_STARTED_MESSAGE: &str = "Starting backup job";
/// Prefix of the message `finalize_run` logs (source "system") once the final
/// run status is stored; the status WebSocket turns it into `job_completed`.
pub const RUN_FINISHED_PREFIX: &str = "Run finished";

#[derive(Debug, Default)]
pub struct JobResult {
    pub exit_code: i32,
    pub files_transferred: i64,
    pub bytes_transferred: i64,
    pub error_count: i32,
    /// Storage info captured during sync (before unmounting for USB drives)
    pub storage_info: Option<StorageInfo>,
}

pub struct BackupService {
    db: SqlitePool,
    logs_db: SqlitePool,
    log_tx: broadcast::Sender<LogMessage>,
    running_jobs: Arc<RwLock<std::collections::HashSet<i64>>>,
    /// Tracks cancelled run_ids
    cancelled_runs: Arc<Mutex<std::collections::HashSet<i64>>>,
    max_runs_per_job: Arc<RwLock<Option<u32>>>,
    credential_service: Arc<CredentialService>,
}

impl BackupService {
    pub fn new(
        db: SqlitePool,
        logs_db: SqlitePool,
        log_tx: broadcast::Sender<LogMessage>,
        max_runs_per_job: Option<u32>,
        credential_service: Arc<CredentialService>,
    ) -> Self {
        Self {
            db,
            logs_db,
            log_tx,
            running_jobs: Arc::new(RwLock::new(std::collections::HashSet::new())),
            cancelled_runs: Arc::new(Mutex::new(std::collections::HashSet::new())),
            max_runs_per_job: Arc::new(RwLock::new(max_runs_per_job)),
            credential_service,
        }
    }

    /// Update the max_runs_per_job setting dynamically
    pub async fn set_max_runs_per_job(&self, value: Option<u32>) {
        *self.max_runs_per_job.write().await = value;
        tracing::info!("Updated max_runs_per_job to {:?}", value);
    }

    /// Get the current max_runs_per_job setting
    pub async fn get_max_runs_per_job(&self) -> Option<u32> {
        *self.max_runs_per_job.read().await
    }

    /// Cleanup old job runs, keeping only the most recent `max_runs_per_job` runs.
    pub async fn cleanup_old_runs(&self, job_id: i64) {
        let Some(max_runs) = *self.max_runs_per_job.read().await else {
            return;
        };

        // First, get the run IDs that will be deleted
        let runs_to_delete: Vec<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM job_runs
            WHERE job_id = ? AND id NOT IN (
                SELECT id FROM job_runs
                WHERE job_id = ?
                ORDER BY id DESC
                LIMIT ?
            )
            "#,
        )
        .bind(job_id)
        .bind(job_id)
        .bind(max_runs)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        if runs_to_delete.is_empty() {
            return;
        }

        let run_ids: Vec<i64> = runs_to_delete.into_iter().map(|(id,)| id).collect();

        // Delete log entries from the separate logs database in one batch query
        let mut qb = QueryBuilder::new("DELETE FROM log_entries WHERE job_run_id IN (");
        let mut sep = qb.separated(", ");
        for id in &run_ids {
            sep.push_bind(id);
        }
        qb.push(")");
        let _ = qb.build().execute(&self.logs_db).await;

        // Delete the job runs from main database
        let result = sqlx::query(
            r#"
            DELETE FROM job_runs
            WHERE job_id = ? AND id NOT IN (
                SELECT id FROM job_runs
                WHERE job_id = ?
                ORDER BY id DESC
                LIMIT ?
            )
            "#,
        )
        .bind(job_id)
        .bind(job_id)
        .bind(max_runs)
        .execute(&self.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                tracing::info!(
                    "Cleaned up {} old job runs for job {}",
                    r.rows_affected(),
                    job_id
                );
            }
            Err(e) => {
                tracing::error!("Failed to cleanup old job runs: {}", e);
            }
            _ => {}
        }
    }

    pub async fn is_job_running(&self, job_id: i64) -> bool {
        self.running_jobs.read().await.contains(&job_id)
    }

    /// Mark runs left in 'pending' or 'running' by a crash or restart as failed.
    ///
    /// Must be called on startup before the scheduler or HTTP server start any
    /// run: at that point no run can really be active. Without this, stale runs
    /// show as running forever and block config import (BACKUP_JOBS_RUNNING).
    pub async fn mark_interrupted_runs(&self) -> anyhow::Result<u64> {
        let stale: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM job_runs WHERE status IN ('pending', 'running')")
                .fetch_all(&self.db)
                .await?;

        if stale.is_empty() {
            return Ok(0);
        }

        sqlx::query(
            "UPDATE job_runs SET status = ?, completed_at = ?, error_count = COALESCE(error_count, 0) + 1, \
             summary = 'Interrupted by server restart' WHERE status IN ('pending', 'running')",
        )
        .bind(JobRunStatus::Failed.as_str())
        .bind(Utc::now())
        .execute(&self.db)
        .await?;

        for (run_id,) in &stale {
            self.log(*run_id, LogLevel::Error, "Run was interrupted by a server restart", "system").await;
        }

        Ok(stale.len() as u64)
    }

    /// Atomically mark a job as running. Returns `false` if it already is.
    ///
    /// Callers must reserve the job *before* creating the run record and
    /// spawning the task, so two triggers (double click, manual + scheduled)
    /// cannot both start the same job. [`execute_job`](Self::execute_job)
    /// releases the reservation when it finishes; call
    /// [`release_job`](Self::release_job) if the run is never started.
    pub async fn try_reserve_job(&self, job_id: i64) -> bool {
        self.running_jobs.write().await.insert(job_id)
    }

    /// Release a reservation made with [`try_reserve_job`](Self::try_reserve_job)
    /// for a run that was never handed to `execute_job`.
    pub async fn release_job(&self, job_id: i64) {
        self.running_jobs.write().await.remove(&job_id);
    }

    /// Cancel a running job by marking it as cancelled.
    /// The provider will check this flag and stop execution.
    pub async fn cancel_job(&self, run_id: i64) -> anyhow::Result<bool> {
        tracing::info!("cancel_job called for run_id: {}", run_id);

        // Mark as cancelled in memory (providers will check this)
        self.cancelled_runs.lock().await.insert(run_id);

        // Mark as cancelled in database
        sqlx::query("UPDATE job_runs SET status = ?, completed_at = ? WHERE id = ? AND status IN ('pending', 'running')")
            .bind(JobRunStatus::Cancelled.as_str())
            .bind(Utc::now())
            .bind(run_id)
            .execute(&self.db)
            .await?;

        self.log(run_id, LogLevel::Warning, "Job cancellation requested", "system").await;
        Ok(true)
    }

    /// Check if a job run has been cancelled
    pub fn is_cancelled_sync(&self, run_id: i64) -> bool {
        // Use try_lock to avoid blocking - if we can't get the lock, assume not cancelled
        self.cancelled_runs.try_lock().map(|guard| guard.contains(&run_id)).unwrap_or(false)
    }

    /// Check if a job run has been cancelled (async version)
    pub async fn is_cancelled(&self, run_id: i64) -> bool {
        self.cancelled_runs.lock().await.contains(&run_id)
    }

    pub async fn execute_job(
        &self,
        job: &Job,
        run_id: i64,
        schedule_id: Option<i64>,
    ) -> anyhow::Result<JobResult> {
        use futures::FutureExt;

        // The job was reserved by the caller (see try_reserve_job). Make sure the
        // reservation is set even for callers that did not reserve.
        self.running_jobs.write().await.insert(job.id);

        // Catch panics so a provider bug cannot leave the job marked as running
        // forever or the run stuck in the 'running' state.
        let result = match std::panic::AssertUnwindSafe(self.do_execute(job, run_id, schedule_id))
            .catch_unwind()
            .await
        {
            Ok(r) => r,
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                tracing::error!("Job {} run {} panicked: {}", job.id, run_id, msg);
                self.log(run_id, LogLevel::Error, &format!("Internal error: {}", msg), "system").await;
                Err(anyhow::anyhow!("Job execution panicked: {}", msg))
            }
        };

        // Remove from running jobs
        self.running_jobs.write().await.remove(&job.id);

        // Clean up cancelled tracking
        self.cancelled_runs.lock().await.remove(&run_id);

        result
    }

    async fn do_execute(
        &self,
        job: &Job,
        run_id: i64,
        _schedule_id: Option<i64>,
    ) -> anyhow::Result<JobResult> {
        // Get destination config and sync options from job
        let destination = job.get_destination_config();
        let mut sync_options = job.get_sync_options();
        let source_dirs = job.source_dirs_vec();

        // Load credentials if needed
        let credential = if let Some(cred_id) = job.credential_id {
            // get_fresh renews an expired OAuth access token up front
            match self.credential_service.get_fresh(cred_id).await {
                Ok(cred) => Some(cred),
                Err(e) => {
                    self.log(run_id, LogLevel::Error, &format!("Failed to load credentials: {}", e), "system").await;
                    return Ok(JobResult {
                        exit_code: 1,
                        error_count: 1,
                        ..Default::default()
                    });
                }
            }
        } else {
            None
        };

        // Create provider based on destination type
        let provider = providers::create_provider(&destination);

        self.log(run_id, LogLevel::Info, RUN_STARTED_MESSAGE, "system").await;

        // Validate configuration
        if let Err(e) = provider.validate_config(&destination, credential.as_ref()) {
            self.log(run_id, LogLevel::Error, &format!("Configuration error: {}", e), "system").await;
            return Ok(JobResult {
                exit_code: 1,
                error_count: 1,
                ..Default::default()
            });
        }

        // --- Compression phase (runs before provider) ---
        let mut include_only: Vec<String> = Vec::new();
        let effective_source_dirs = if sync_options.compress_dirs_enabled() {
            let compress_opts = sync_options.compress_dirs.as_ref().unwrap();
            let staging_dir = PathBuf::from(&compress_opts.staging_path);

            if sync_options.dry_run {
                // Dry-run: log what would happen but skip actual compression.
                // Use original source_dirs for the provider's dry-run pass — the staging
                // directory doesn't exist yet and passing it would cause spurious errors.
                for source_dir in &source_dirs {
                    let relevant_excludes: Vec<String> = sync_options
                        .exclude_dirs
                        .iter()
                        .filter(|ex| ex.starts_with(&format!("{}/", source_dir)))
                        .cloned()
                        .collect();
                    let excl_note = if relevant_excludes.is_empty() {
                        String::new()
                    } else {
                        format!(" (excluding: {})", relevant_excludes.join(", "))
                    };
                    self.log(
                        run_id,
                        LogLevel::Info,
                        &format!(
                            "[DRY RUN] Would compress '{}' → {}/{}",
                            source_dir,
                            staging_dir.display(),
                            excl_note
                        ),
                        "compress",
                    )
                    .await;
                }
                source_dirs.clone()
            } else {
                self.log(
                    run_id,
                    LogLevel::Info,
                    &format!(
                        "Compress mode: archiving {} source director{} to {}",
                        source_dirs.len(),
                        if source_dirs.len() == 1 { "y" } else { "ies" },
                        staging_dir.display()
                    ),
                    "compress",
                )
                .await;

                for source_dir in &source_dirs {
                    // Check cancellation before starting each archive
                    if self.is_cancelled(run_id).await {
                        return Ok(JobResult {
                            exit_code: 130,
                            ..Default::default()
                        });
                    }

                    let dir_name = Path::new(source_dir)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("backup")
                        .to_string();

                    // Collect exclude_dirs that are children of this specific source_dir.
                    // Paths belonging to other source dirs are filtered out here and will
                    // be applied during their own compress_directory call.
                    let relevant_excludes: Vec<String> = sync_options
                        .exclude_dirs
                        .iter()
                        .filter(|ex| ex.starts_with(&format!("{}/", source_dir)))
                        .cloned()
                        .collect();

                    let cancelled_runs = Arc::clone(&self.cancelled_runs);
                    let is_cancelled_fn = move || {
                        cancelled_runs
                            .try_lock()
                            .map(|g| g.contains(&run_id))
                            .unwrap_or(false)
                    };

                    let log_tx = self.log_tx.clone();
                    // Persist as well, so compression output shows up in History.
                    // log_fn is synchronous, so lines go through a channel to one
                    // writer task: History is ordered by insert ID, and a task
                    // per line could insert them out of order.
                    let (line_tx, mut line_rx) =
                        tokio::sync::mpsc::unbounded_channel::<(String, chrono::DateTime<Utc>)>();
                    let logs_db = self.logs_db.clone();
                    let writer = tokio::spawn(async move {
                        while let Some((message, timestamp)) = line_rx.recv().await {
                            let _ = sqlx::query(
                                "INSERT INTO log_entries (job_run_id, level, message, source, timestamp) VALUES (?, ?, ?, ?, ?)",
                            )
                            .bind(run_id)
                            .bind(LogLevel::Info.as_str())
                            .bind(message)
                            .bind("compress")
                            .bind(timestamp)
                            .execute(&logs_db)
                            .await;
                        }
                    });
                    let log_fn = move |msg: String| {
                        let timestamp = Utc::now();
                        let _ = line_tx.send((msg.clone(), timestamp));
                        let _ = log_tx.send(LogMessage {
                            run_id,
                            level: LogLevel::Info,
                            message: msg,
                            source: "compress".to_string(),
                            timestamp,
                        });
                    };

                    let archive_path = compress_service::compress_directory(
                        Path::new(source_dir),
                        job.id,
                        run_id,
                        compress_opts,
                        &relevant_excludes,
                        log_fn,
                        is_cancelled_fn,
                    )
                    .await;

                    // log_fn (the only sender) is dropped now: let the writer
                    // store the remaining lines before the next log entries
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(10), writer).await;

                    let archive_path = match archive_path {
                        Ok(p) => p,
                        Err(e) => {
                            if self.is_cancelled(run_id).await {
                                return Ok(JobResult {
                                    exit_code: 130,
                                    ..Default::default()
                                });
                            }
                            let msg = format!("Compression failed for '{}': {:#}", source_dir, e);
                            tracing::error!("{}", msg);
                            self.log(run_id, LogLevel::Error, &msg, "compress").await;
                            return Err(anyhow::anyhow!(msg));
                        }
                    };

                    let archive_size = archive_path.metadata().map(|m| m.len()).unwrap_or(0);
                    self.log(
                        run_id,
                        LogLevel::Info,
                        &format!(
                            "Archived '{}' → {} ({})",
                            source_dir,
                            archive_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            format_file_size(archive_size)
                        ),
                        "compress",
                    )
                    .await;

                    // Clean up old archives if retention limit is set (timestamped only)
                    if compress_opts.add_timestamp {
                        if let Some(max) = compress_opts.max_archives_per_dir {
                            match compress_service::cleanup_old_archives(
                                &staging_dir,
                                &dir_name,
                                compress_opts.custom_name.as_deref(),
                                max,
                                job.id,
                            ) {
                                Ok(deleted) if deleted > 0 => {
                                    self.log(
                                        run_id,
                                        LogLevel::Info,
                                        &format!(
                                            "Removed {} old archive(s) for '{}'",
                                            deleted, dir_name
                                        ),
                                        "compress",
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    self.log(
                                        run_id,
                                        LogLevel::Warning,
                                        &format!("Failed to clean up old archives for '{}': {}", dir_name, e),
                                        "compress",
                                    )
                                    .await;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // The staging dir is now the source for the provider. It may hold
                // other jobs' archives too, so only transfer (and mirror) this
                // job's archives; archive names end in `_<job id>.<ext>`.
                include_only = compress_service::job_archive_patterns(job.id);
                vec![staging_dir.to_string_lossy().to_string()]
            }
        } else {
            source_dirs.clone()
        };

        // Pre-flight space check for local destinations (skipped when compression is enabled,
        // since the staging space is managed via retention and compression ratio is unpredictable)
        if matches!(destination, DestinationConfig::Local { .. }) && !sync_options.compress_dirs_enabled() {
            let space_mode = sync_options.space_check_mode();
            if space_mode != "none" {
                let rsync = RsyncProvider::new();
                match rsync.check_space(&source_dirs, &destination, &sync_options).await {
                    Ok(result) if !result.fits => {
                        let deficit = result.deficit.unwrap_or(0);
                        let msg = format!(
                            "Insufficient space: need {} to transfer, have {} free ({} short)",
                            format_file_size(result.transfer_size),
                            format_file_size(result.destination_free),
                            format_file_size(deficit)
                        );
                        if space_mode == "fail" {
                            self.log(run_id, LogLevel::Error, &msg, "space-check").await;
                            return Ok(JobResult {
                                exit_code: 1,
                                error_count: 1,
                                ..Default::default()
                            });
                        } else {
                            // "warn" mode - log warning but continue
                            self.log(run_id, LogLevel::Warning, &msg, "space-check").await;
                        }
                    }
                    Ok(result) => {
                        self.log(
                            run_id,
                            LogLevel::Info,
                            &format!(
                                "Space check passed: {} to transfer, {} free",
                                format_file_size(result.transfer_size),
                                format_file_size(result.destination_free)
                            ),
                            "space-check",
                        ).await;
                    }
                    Err(e) => {
                        // Space check failed - log warning but don't block
                        self.log(
                            run_id,
                            LogLevel::Warning,
                            &format!("Space check failed: {}", e),
                            "space-check",
                        ).await;
                    }
                }
            }
        }

        // Create cancellation checker closure
        let cancelled_runs = Arc::clone(&self.cancelled_runs);
        let is_cancelled: providers::CancellationChecker = Arc::new(move |rid: i64| {
            cancelled_runs.try_lock().map(|guard| guard.contains(&rid)).unwrap_or(false)
        });

        let oauth = match (job.credential_id, credential.as_ref()) {
            (Some(cred_id), Some(data)) => {
                crate::services::OAuthTokenSource::new(Arc::clone(&self.credential_service), cred_id, data)
                    .map(Arc::new)
            }
            _ => None,
        };

        if !include_only.is_empty() {
            sync_options.include_only = include_only;
            // Temporary files of archives still being written (other jobs)
            sync_options.exclude_patterns.push("*.tmp".to_string());
        }

        // Build sync context (uses effective_source_dirs: staging dir when compress is enabled)
        let ctx = Arc::new(SyncContext {
            run_id,
            source_dirs: effective_source_dirs,
            destination,
            options: sync_options,
            credential,
            log_sender: self.log_tx.clone(),
            logs_db: self.logs_db.clone(),
            is_cancelled,
            oauth,
        });

        // Execute sync via provider
        let sync_result = provider.sync(ctx).await;

        // Convert provider result to JobResult
        match sync_result {
            Ok(result) => Ok(JobResult {
                exit_code: if result.success { 0 } else { 1 },
                files_transferred: result.files_transferred,
                bytes_transferred: result.bytes_transferred,
                error_count: result.error_count,
                storage_info: result.storage_info,
            }),
            Err(ProviderError::Cancelled) => {
                self.log(run_id, LogLevel::Warning, "Job was cancelled", "system").await;
                Ok(JobResult {
                    exit_code: 130, // Standard cancelled exit code
                    error_count: 0,
                    ..Default::default()
                })
            }
            Err(e) => {
                self.log(run_id, LogLevel::Error, &format!("Provider error: {}", e), "system").await;
                Ok(JobResult {
                    exit_code: 1,
                    error_count: 1,
                    ..Default::default()
                })
            }
        }
    }

    async fn log(&self, run_id: i64, level: LogLevel, message: &str, source: &str) {
        let timestamp = Utc::now();

        let log_msg = LogMessage {
            run_id,
            level,
            message: message.to_string(),
            source: source.to_string(),
            timestamp,
        };

        // Broadcast to WebSocket clients
        let _ = self.log_tx.send(log_msg);

        // Store in logs database (separate from main database for performance)
        let _ = sqlx::query(
            "INSERT INTO log_entries (job_run_id, level, message, source, timestamp) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(run_id)
        .bind(level.as_str())
        .bind(message)
        .bind(source)
        .bind(timestamp)
        .execute(&self.logs_db)
        .await;
    }

    /// Finalise a completed job run: write status/stats to the DB, update storage info,
    /// and clean up old runs. Preserves the `cancelled` status if already set.
    ///
    /// This is the single canonical implementation shared by the HTTP handler and the
    /// scheduler — previously this block was duplicated verbatim in both.
    pub async fn finalize_run(
        &self,
        run_id: i64,
        job_id: i64,
        job: &crate::models::Job,
        result: anyhow::Result<JobResult>,
    ) {
        let (exit_code, files, bytes, errors) = match &result {
            Ok(r) => (r.exit_code, r.files_transferred, r.bytes_transferred, r.error_count),
            Err(_) => (1, 0, 0, 1),
        };

        let current_status: Option<(String,)> =
            sqlx::query_as("SELECT status FROM job_runs WHERE id = ?")
                .bind(run_id)
                .fetch_optional(&self.db)
                .await
                .ok()
                .flatten();

        let was_cancelled = current_status.as_ref().map(|s| s.0.as_str()) == Some("cancelled");
        let mut final_status = JobRunStatus::Cancelled;

        if was_cancelled {
            let _ = sqlx::query(
                "UPDATE job_runs SET exit_code = ?, files_transferred = ?, bytes_transferred = ?, error_count = ? WHERE id = ? AND status = 'cancelled'",
            )
            .bind(exit_code)
            .bind(files)
            .bind(bytes)
            .bind(errors)
            .bind(run_id)
            .execute(&self.db)
            .await;
        } else {
            let status = match &result {
                Ok(r) => {
                    if r.error_count > 0 {
                        JobRunStatus::Failed
                    } else {
                        JobRunStatus::Completed
                    }
                }
                Err(_) => JobRunStatus::Failed,
            };
            final_status = status;

            let _ = sqlx::query(
                "UPDATE job_runs SET status = ?, completed_at = ?, exit_code = ?, files_transferred = ?, bytes_transferred = ?, error_count = ? WHERE id = ?",
            )
            .bind(status.as_str())
            .bind(Utc::now())
            .bind(exit_code)
            .bind(files)
            .bind(bytes)
            .bind(errors)
            .bind(run_id)
            .execute(&self.db)
            .await;

            if status == JobRunStatus::Completed && !job.get_sync_options().dry_run {
                let _ = sqlx::query("UPDATE jobs SET last_success_at = ? WHERE id = ?")
                    .bind(Utc::now())
                    .bind(job_id)
                    .execute(&self.db)
                    .await;
            }

            if result.as_ref().map(|r| r.error_count == 0).unwrap_or(false) {
                let storage_info = result.as_ref().ok().and_then(|r| r.storage_info.clone());
                let _ = self.update_storage_info(job_id, job, storage_info).await;
            }
        }

        // After the status is stored, so clients reloading on this event see it
        self.log(
            run_id,
            LogLevel::Info,
            &format!("{}: {}", RUN_FINISHED_PREFIX, final_status.as_str()),
            "system",
        )
        .await;

        self.cleanup_old_runs(job_id).await;
    }

    /// Delete log entries for a specific run (used when deleting runs manually)
    pub async fn delete_logs_for_run(&self, run_id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM log_entries WHERE job_run_id = ?")
            .bind(run_id)
            .execute(&self.logs_db)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete log entries for all runs of a job
    pub async fn delete_logs_for_job(&self, job_id: i64) -> anyhow::Result<u64> {
        let run_ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM job_runs WHERE job_id = ?")
            .bind(job_id)
            .fetch_all(&self.db)
            .await?;

        if run_ids.is_empty() {
            return Ok(0);
        }

        let mut qb = QueryBuilder::new("DELETE FROM log_entries WHERE job_run_id IN (");
        let mut sep = qb.separated(", ");
        for (id,) in &run_ids {
            sep.push_bind(id);
        }
        qb.push(")");
        let result = qb.build().execute(&self.logs_db).await?;
        Ok(result.rows_affected())
    }

    /// Delete all log entries (used for purge all)
    pub async fn delete_all_logs(&self) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM log_entries")
            .execute(&self.logs_db)
            .await?;
        Ok(result.rows_affected())
    }

    /// Update destination storage info for a job after successful completion
    /// If `pre_captured` is Some, uses that instead of fetching from the provider
    /// (important for USB drives that get unmounted after sync)
    pub async fn update_storage_info(&self, job_id: i64, job: &Job, pre_captured: Option<StorageInfo>) -> anyhow::Result<()> {
        // Use pre-captured storage info if available (e.g., captured before unmounting USB)
        let storage_info = if let Some(info) = pre_captured {
            Ok(info)
        } else {
            // Fall back to fetching from provider (for providers that don't unmount)
            let destination = job.get_destination_config();
            let provider = providers::create_provider(&destination);

            // Load credentials if needed
            let credential = if let Some(cred_id) = job.credential_id {
                self.credential_service.get_fresh(cred_id).await.ok()
            } else {
                None
            };

            provider.get_storage_info(&destination, credential.as_ref()).await
        };

        if let Ok(info) = storage_info {
            if info.supported {
                let _ = sqlx::query(
                    "UPDATE jobs SET dest_storage_free = ?, dest_storage_total = ?, dest_storage_updated_at = ? WHERE id = ?",
                )
                .bind(info.free.map(|v| v as i64))
                .bind(info.total.map(|v| v as i64))
                .bind(Utc::now())
                .bind(job_id)
                .execute(&self.db)
                .await;

                tracing::info!(
                    "Updated storage info for job {}: free={:?}, total={:?}",
                    job_id,
                    info.free,
                    info.total
                );
            }
        }

        Ok(())
    }
}

/// Format a byte count as a human-readable string (B / KB / MB / GB / TB).
fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else if b < KB * KB * KB * KB {
        format!("{:.2} GB", b / (KB * KB * KB))
    } else {
        format!("{:.2} TB", b / (KB * KB * KB * KB))
    }
}

#[cfg(test)]
mod format_tests {
    use super::format_file_size;

    #[test]
    fn formats_sizes() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(300_300), "293.3 KB");
        assert_eq!(format_file_size(64_944_992_256), "60.48 GB");
        assert_eq!(format_file_size(3 * 1024u64.pow(4)), "3.00 TB");
    }
}

#[cfg(test)]
mod last_success_tests {
    use super::*;

    async fn setup() -> (SqlitePool, BackupService) {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(100);
        let service = BackupService::new(db.clone(), logs_db, log_tx, None, credentials);
        (db, service)
    }

    async fn job_with_run(db: &SqlitePool, sync_options: &str) -> (Job, i64) {
        let job_id = sqlx::query(
            "INSERT INTO jobs (name, mount_point, source_dirs, destination_type, destination_config, sync_options) \
             VALUES ('j', '', '[]', 'local', '{}', ?)",
        )
        .bind(sync_options)
        .execute(db)
        .await
        .unwrap()
        .last_insert_rowid();
        let run_id = sqlx::query("INSERT INTO job_runs (job_id, status) VALUES (?, 'running')")
            .bind(job_id)
            .execute(db)
            .await
            .unwrap()
            .last_insert_rowid();
        let job: Job = sqlx::query_as("SELECT * FROM jobs WHERE id = ?").bind(job_id).fetch_one(db).await.unwrap();
        (job, run_id)
    }

    async fn last_success(db: &SqlitePool, job_id: i64) -> Option<chrono::DateTime<Utc>> {
        sqlx::query_as::<_, (Option<chrono::DateTime<Utc>>,)>("SELECT last_success_at FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(db)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn only_real_successful_runs_count() {
        let (db, service) = setup().await;

        let (job, run) = job_with_run(&db, "{}").await;
        service.finalize_run(run, job.id, &job, Ok(JobResult { error_count: 1, ..Default::default() })).await;
        assert!(last_success(&db, job.id).await.is_none(), "failed run");

        let (job, run) = job_with_run(&db, r#"{"dry_run": true}"#).await;
        service.finalize_run(run, job.id, &job, Ok(JobResult::default())).await;
        assert!(last_success(&db, job.id).await.is_none(), "dry run");

        let (job, run) = job_with_run(&db, "{}").await;
        service.finalize_run(run, job.id, &job, Ok(JobResult::default())).await;
        assert!(last_success(&db, job.id).await.is_some(), "successful run");
    }

    #[tokio::test]
    async fn migration_backfills_from_kept_runs() {
        let options = sqlx::sqlite::SqliteConnectOptions::new().filename(":memory:");
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        // A database from before the column existed
        sqlx::raw_sql(include_str!("../../../migrations/001_initial.sql")).execute(&db).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO jobs (id, name, mount_point, source_dirs) VALUES (1, 'a', '', '[]'), (2, 'b', '', '[]');
             INSERT INTO job_runs (job_id, status, completed_at) VALUES
                (1, 'completed', '2026-11-01T02:00:00Z'),
                (1, 'completed', '2026-11-20T02:00:00Z'),
                (1, 'failed', '2026-11-23T02:00:00Z'),
                (2, 'failed', '2026-11-23T02:00:00Z');",
        )
        .execute(&db)
        .await
        .unwrap();

        crate::db::run_migrations(&db).await.unwrap();

        let a = last_success(&db, 1).await.unwrap();
        assert_eq!(a.to_rfc3339(), "2026-11-20T02:00:00+00:00");
        assert!(last_success(&db, 2).await.is_none());

        // Running again (every start) leaves the value alone
        sqlx::query("UPDATE jobs SET last_success_at = '2026-11-24T02:00:00Z' WHERE id = 1").execute(&db).await.unwrap();
        crate::db::run_migrations(&db).await.unwrap();
        assert_eq!(last_success(&db, 1).await.unwrap().to_rfc3339(), "2026-11-24T02:00:00+00:00");
    }
}

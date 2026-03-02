use crate::models::{DestinationConfig, Job, JobRunStatus, LogLevel, LogMessage};
use crate::services::compress_service;
use crate::services::credential_service::CredentialService;
use crate::services::providers::{self, RsyncProvider, StorageInfo, SyncContext, ProviderError};
use chrono::Utc;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

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

        // Delete log entries from the separate logs database
        for run_id in &run_ids {
            let _ = sqlx::query("DELETE FROM log_entries WHERE job_run_id = ?")
                .bind(run_id)
                .execute(&self.logs_db)
                .await;
        }

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

    /// Cancel a running job by marking it as cancelled.
    /// The provider will check this flag and stop execution.
    pub async fn cancel_job(&self, run_id: i64) -> anyhow::Result<bool> {
        tracing::info!("cancel_job called for run_id: {}", run_id);

        // Mark as cancelled in memory (providers will check this)
        self.cancelled_runs.lock().await.insert(run_id);

        // Mark as cancelled in database
        sqlx::query("UPDATE job_runs SET status = ?, completed_at = ? WHERE id = ? AND status = 'running'")
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
        // Mark job as running
        self.running_jobs.write().await.insert(job.id);

        let result = self.do_execute(job, run_id, schedule_id).await;

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
        let sync_options = job.get_sync_options();
        let source_dirs = job.source_dirs_vec();

        // Load credentials if needed
        let credential = if let Some(cred_id) = job.credential_id {
            match self.credential_service.get_decrypted(cred_id).await {
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

        self.log(run_id, LogLevel::Info, "Starting backup job", "system").await;

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
        let effective_source_dirs = if sync_options.compress_dirs_enabled() {
            let compress_opts = sync_options.compress_dirs.as_ref().unwrap();
            let staging_dir = PathBuf::from(&compress_opts.staging_path).join(job.id.to_string());

            if sync_options.dry_run {
                // Dry-run: log what would happen but skip actual compression.
                // Use original source_dirs for the provider's dry-run pass — the staging
                // directory doesn't exist yet and passing it would cause spurious errors.
                for source_dir in &source_dirs {
                    self.log(
                        run_id,
                        LogLevel::Info,
                        &format!(
                            "[DRY RUN] Would compress '{}' → {}/",
                            source_dir,
                            staging_dir.display()
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

                    let cancelled_runs = Arc::clone(&self.cancelled_runs);
                    let is_cancelled_fn = move || {
                        cancelled_runs
                            .try_lock()
                            .map(|g| g.contains(&run_id))
                            .unwrap_or(false)
                    };

                    let log_tx = self.log_tx.clone();
                    let log_fn = move |msg: String| {
                        let _ = log_tx.send(LogMessage {
                            run_id,
                            level: LogLevel::Info,
                            message: msg,
                            source: "compress".to_string(),
                            timestamp: Utc::now(),
                        });
                    };

                    let archive_path = compress_service::compress_directory(
                        Path::new(source_dir),
                        job.id,
                        run_id,
                        compress_opts,
                        log_fn,
                        is_cancelled_fn,
                    )
                    .await
                    .map_err(|e| {
                        let msg = format!("Compression failed for '{}': {}", source_dir, e);
                        tracing::error!("{}", msg);
                        anyhow::anyhow!(msg)
                    })?;

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
                            let has_password = compress_opts
                                .password
                                .as_deref()
                                .map(|p| !p.is_empty())
                                .unwrap_or(false);
                            match compress_service::cleanup_old_archives(
                                &staging_dir,
                                &dir_name,
                                compress_opts.custom_name.as_deref(),
                                &compress_opts.format,
                                has_password,
                                max,
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

                // The staging dir is now the source for the provider
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
                            "Insufficient space: need {} bytes to transfer, have {} bytes free (deficit: {} bytes)",
                            result.transfer_size, result.destination_free, deficit
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
                                "Space check passed: {} bytes to transfer, {} bytes free",
                                result.transfer_size, result.destination_free
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

    /// Delete log entries for a specific run (used when deleting runs manually)
    pub async fn delete_logs_for_run(&self, run_id: i64) -> anyhow::Result<u64> {
        let result = sqlx::query("DELETE FROM log_entries WHERE job_run_id = ?")
            .bind(run_id)
            .execute(&self.logs_db)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete log entries for all runs of a job
    pub async fn delete_logs_for_job(&self, job_id: i64, db: &SqlitePool) -> anyhow::Result<u64> {
        // Get all run IDs for this job
        let run_ids: Vec<(i64,)> = sqlx::query_as("SELECT id FROM job_runs WHERE job_id = ?")
            .bind(job_id)
            .fetch_all(db)
            .await?;

        let mut total_deleted = 0u64;
        for (run_id,) in run_ids {
            let result = sqlx::query("DELETE FROM log_entries WHERE job_run_id = ?")
                .bind(run_id)
                .execute(&self.logs_db)
                .await?;
            total_deleted += result.rows_affected();
        }
        Ok(total_deleted)
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
                self.credential_service.get_decrypted(cred_id).await.ok()
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

/// Format a byte count as a human-readable string (B / KB / MB / GB).
fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

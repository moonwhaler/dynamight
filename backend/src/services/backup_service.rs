use crate::models::{Job, JobRunStatus, LogLevel, LogMessage};
use crate::services::credential_service::CredentialService;
use crate::services::providers::{self, SyncContext, ProviderError};
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};

#[derive(Debug, Default)]
pub struct JobResult {
    pub exit_code: i32,
    pub files_transferred: i64,
    pub bytes_transferred: i64,
    pub error_count: i32,
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

        // Validate configuration
        if let Err(e) = provider.validate_config(&destination, credential.as_ref()) {
            self.log(run_id, LogLevel::Error, &format!("Configuration error: {}", e), "system").await;
            return Ok(JobResult {
                exit_code: 1,
                error_count: 1,
                ..Default::default()
            });
        }

        // Create cancellation checker closure
        let cancelled_runs = Arc::clone(&self.cancelled_runs);
        let is_cancelled: providers::CancellationChecker = Arc::new(move |rid: i64| {
            cancelled_runs.try_lock().map(|guard| guard.contains(&rid)).unwrap_or(false)
        });

        // Build sync context
        let ctx = Arc::new(SyncContext {
            run_id,
            source_dirs,
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
}

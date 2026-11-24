use crate::models::{Job, JobRunStatus, LogLevel, LogMessage};
use crate::services::compress_service;
use crate::services::credential_service::CredentialService;
use crate::services::log_writer::{LogRow, LogWriter};
use crate::services::notification_service::{self, EventKind, Notifier, RunInfo};
use crate::services::providers::{self, StorageInfo, SyncContext, ProviderError};
use chrono::Utc;
use sqlx::{QueryBuilder, SqlitePool};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, OwnedSemaphorePermit, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

/// Logged (source "system") when a run starts.
pub const RUN_STARTED_MESSAGE: &str = "Starting backup job";
/// Prefix of the message `finalize_run` logs (source "system") once the final
/// run status is stored; live log viewers reload on it. Status clients get a
/// [`RunEvent`] instead.
pub const RUN_FINISHED_PREFIX: &str = "Run finished";

/// A run started or finished. Sent on its own channel (see
/// [`BackupService::subscribe_events`]) so these few events cannot be lost
/// among thousands of log lines when a client falls behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEventKind {
    /// Waits for another backup to finish
    Queued,
    Started,
    /// Final status and the whole log are stored
    Completed,
}

#[derive(Debug, Clone)]
pub struct RunEvent {
    pub kind: RunEventKind,
    pub run_id: i64,
    pub job_id: i64,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Run events held for slow subscribers (a handful per run)
const EVENT_CAPACITY: usize = 64;

/// Backups running at the same time; further runs wait in line (FIFO).
///
/// Must stay 1 as long as a job may queue a follow-up run behind its own
/// running one: with more slots the two could run side by side.
const MAX_PARALLEL_RUNS: usize = 1;

/// A started run: its ID and whether it waits for another backup first.
#[derive(Debug, Clone, Copy)]
pub struct StartedRun {
    pub run_id: i64,
    pub queued: bool,
}

/// Runs of one job that are active (at most one running and one waiting).
#[derive(Debug, Default)]
struct JobSlot {
    running: bool,
    waiting: bool,
}

/// What is active right now. Behind a plain mutex: every use is a short,
/// synchronous update.
#[derive(Default)]
struct ActiveRuns {
    jobs: std::collections::HashMap<i64, JobSlot>,
    /// Cancellation per active run, with its job
    tokens: std::collections::HashMap<i64, (i64, CancellationToken)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Waiting,
    Running,
}

/// Holds one run's place: its waiting/running flag, its cancellation token
/// and (while running) its run slot. Dropping it releases all of them, on
/// every path, panics included.
struct RunGuard {
    active: Arc<std::sync::Mutex<ActiveRuns>>,
    job_id: i64,
    run_id: Option<i64>,
    phase: Phase,
    permit: Option<OwnedSemaphorePermit>,
}

impl RunGuard {
    /// The run got its slot and starts
    fn start(&mut self, permit: OwnedSemaphorePermit) {
        let mut active = lock(&self.active);
        let slot = active.jobs.entry(self.job_id).or_default();
        debug_assert!(!slot.running, "two runs of one job at once");
        slot.waiting = false;
        slot.running = true;
        self.phase = Phase::Running;
        self.permit = Some(permit);
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        let mut active = lock(&self.active);
        if let Some(slot) = active.jobs.get_mut(&self.job_id) {
            // Only this run's own flag: a queued follow-up keeps its place
            match self.phase {
                Phase::Waiting => slot.waiting = false,
                Phase::Running => slot.running = false,
            }
            if !slot.running && !slot.waiting {
                active.jobs.remove(&self.job_id);
            }
        }
        if let Some(run_id) = self.run_id {
            active.tokens.remove(&run_id);
        }
        // The permit (if any) is released after this, so the next run in
        // line sees the flags already cleared
    }
}

fn lock(m: &std::sync::Mutex<ActiveRuns>) -> std::sync::MutexGuard<'_, ActiveRuns> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Why [`BackupService::start_run`] did not start a run.
#[derive(Debug)]
pub enum StartRunError {
    AlreadyRunning,
    ShuttingDown,
    Database(sqlx::Error),
}

/// Summary of runs cancelled because the server stopped.
pub const SHUTDOWN_SUMMARY: &str = "Cancelled: server shutting down";
/// Longest reason kept in a failed run's summary
const MAX_SUMMARY_CHARS: usize = 500;

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
    log_writer: LogWriter,
    log_tx: broadcast::Sender<LogMessage>,
    events: broadcast::Sender<RunEvent>,
    /// Active runs per job and their cancellation tokens
    active: Arc<std::sync::Mutex<ActiveRuns>>,
    /// Run slots (see MAX_PARALLEL_RUNS)
    run_slots: Arc<Semaphore>,
    max_runs_per_job: Arc<RwLock<Option<u32>>>,
    /// Days run logs are kept (None: forever)
    log_retention_days: RwLock<Option<u32>>,
    credential_service: Arc<CredentialService>,
    /// Set by `shutdown`: no new runs start
    shutting_down: AtomicBool,
    /// Every run task, so `shutdown` can wait for them to finish
    tasks: TaskTracker,
    notifier: Notifier,
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
            notifier: Notifier::new(db.clone()),
            db,
            log_writer: LogWriter::spawn(logs_db.clone()),
            logs_db,
            log_tx,
            events: broadcast::channel(EVENT_CAPACITY).0,
            active: Arc::new(std::sync::Mutex::new(ActiveRuns::default())),
            run_slots: Arc::new(Semaphore::new(MAX_PARALLEL_RUNS)),
            max_runs_per_job: Arc::new(RwLock::new(max_runs_per_job)),
            log_retention_days: RwLock::new(Some(crate::handlers::settings::DEFAULT_LOG_RETENTION_DAYS)),
            credential_service,
            shutting_down: AtomicBool::new(false),
            tasks: TaskTracker::new(),
        }
    }

    pub fn notifier(&self) -> &Notifier {
        &self.notifier
    }

    /// Run started/finished events (for the status WebSocket)
    pub fn subscribe_events(&self) -> broadcast::Receiver<RunEvent> {
        self.events.subscribe()
    }

    fn emit(&self, kind: RunEventKind, run_id: i64, job_id: i64) {
        let _ = self.events.send(RunEvent { kind, run_id, job_id, timestamp: Utc::now() });
    }

    /// Update the job's alert state for a finished run and send the matching
    /// notification (failed, recovered or completed). Cancelled runs send none.
    async fn notify_run_result(&self, run_id: i64, job: &Job, status: JobRunStatus) {
        let dry_run = job.get_sync_options().dry_run;
        let previous: String = sqlx::query_as::<_, (String,)>("SELECT alert_state FROM jobs WHERE id = ?")
            .bind(job.id)
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten()
            .map(|(s,)| s)
            .unwrap_or_else(|| "ok".to_string());

        let kind = match status {
            JobRunStatus::Failed => {
                self.set_alert_state(job.id, "failed").await;
                EventKind::Failed
            }
            // A dry run proves nothing about the backup, so it does not end a problem
            JobRunStatus::Completed if !dry_run && previous != "ok" => {
                self.set_alert_state(job.id, "ok").await;
                EventKind::Recovered
            }
            JobRunStatus::Completed => EventKind::Success,
            _ => return,
        };

        type Time = Option<chrono::DateTime<Utc>>;
        type RunRow = (String, Time, Time, Option<i64>, Option<i64>, Option<i64>);
        let row: Option<RunRow> =
            sqlx::query_as(
                "SELECT status, started_at, completed_at, files_transferred, bytes_transferred, error_count \
                 FROM job_runs WHERE id = ?",
            )
            .bind(run_id)
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten();
        let Some((run_status, started_at, completed_at, files, bytes, errors)) = row else {
            return;
        };
        let run = RunInfo {
            id: run_id,
            status: run_status,
            started_at,
            completed_at,
            files_transferred: files.unwrap_or(0),
            bytes_transferred: bytes.unwrap_or(0),
            error_count: errors.unwrap_or(0),
        };

        // The log is complete here (finalize_run flushed it)
        let last_error = if kind == EventKind::Failed {
            self.last_error_line(run_id).await
        } else {
            None
        };

        let notification =
            notification_service::run_notification(kind, job.id, &job.name, run, dry_run, last_error);
        let notifier = self.notifier.clone();
        self.tasks.spawn(async move { notifier.notify(notification).await });
    }

    async fn set_alert_state(&self, job_id: i64, state: &str) {
        let _ = sqlx::query("UPDATE jobs SET alert_state = ? WHERE id = ?")
            .bind(state)
            .bind(job_id)
            .execute(&self.db)
            .await;
    }

    /// Mark enabled, scheduled jobs without a recent successful backup as
    /// overdue and notify once (until a successful run resets the state).
    /// Jobs already failing are left alone: that problem was reported.
    /// Returns the number of jobs that became overdue.
    pub async fn check_overdue(&self, tz: chrono_tz::Tz) -> usize {
        type JobRow = (i64, String, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>);
        let jobs: Vec<JobRow> = match sqlx::query_as(
            "SELECT id, name, created_at, last_success_at FROM jobs WHERE enabled = 1 AND alert_state = 'ok'",
        )
        .fetch_all(&self.db)
        .await
        {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::warn!("Overdue check failed: {}", e);
                return 0;
            }
        };
        let schedules: Vec<(i64, String)> =
            sqlx::query_as("SELECT job_id, cron_expression FROM schedules WHERE enabled = 1")
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();

        let now = Utc::now();
        let mut count = 0;
        for (job_id, name, created_at, last_success) in jobs {
            let crons = schedules.iter().filter(|(j, _)| *j == job_id).map(|(_, c)| c.as_str());
            let Some(limit) = crate::models::overdue_after(crons, tz) else {
                continue;
            };
            // A run in progress will settle it either way
            if now - last_success.unwrap_or(created_at) <= limit || self.is_job_running(job_id).await {
                continue;
            }
            let updated = sqlx::query("UPDATE jobs SET alert_state = 'overdue' WHERE id = ? AND alert_state = 'ok'")
                .bind(job_id)
                .execute(&self.db)
                .await
                .map(|r| r.rows_affected())
                .unwrap_or(0);
            if updated == 0 {
                continue;
            }
            tracing::warn!("Job '{}' is overdue (last success: {:?})", name, last_success);
            count += 1;
            let notification = notification_service::overdue_notification(job_id, &name, last_success, now, tz);
            let notifier = self.notifier.clone();
            self.tasks.spawn(async move { notifier.notify(notification).await });
        }
        count
    }

    /// Start a run of `job` in the background and return its run ID.
    ///
    /// The one way runs start (manual and scheduled). Runs go one at a time
    /// (see MAX_PARALLEL_RUNS); a run that has to wait is stored as `pending`
    /// and starts when the slot frees up.
    ///
    /// A job runs at most once at a time. A manual start is refused while the
    /// job has a run running or waiting; a scheduled one only while a run is
    /// already waiting, so a schedule that fires during a long run queues one
    /// follow-up run instead of being lost.
    pub async fn start_run(
        self: &Arc<Self>,
        job: Job,
        schedule_id: Option<i64>,
    ) -> Result<StartedRun, StartRunError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(StartRunError::ShuttingDown);
        }
        let Some(mut guard) = self.reserve(job.id, schedule_id.is_some()) else {
            return Err(StartRunError::AlreadyRunning);
        };

        let run_id = match sqlx::query("INSERT INTO job_runs (job_id, schedule_id, status) VALUES (?, ?, ?)")
            .bind(job.id)
            .bind(schedule_id)
            .bind(JobRunStatus::Pending.as_str())
            .execute(&self.db)
            .await
        {
            Ok(r) => r.last_insert_rowid(),
            // Dropping the guard releases the reservation
            Err(e) => return Err(StartRunError::Database(e)),
        };
        let token = CancellationToken::new();
        lock(&self.active).tokens.insert(run_id, (job.id, token.clone()));
        guard.run_id = Some(run_id);

        let permit_now = Arc::clone(&self.run_slots).try_acquire_owned().ok();
        let queued = permit_now.is_none();
        if queued {
            self.log(run_id, LogLevel::Info, "Waiting for another backup to finish", "system").await;
            self.emit(RunEventKind::Queued, run_id, job.id);
        }

        let service = Arc::clone(self);
        self.tasks.spawn(async move {
            let permit = match permit_now {
                Some(permit) => Some(permit),
                None => tokio::select! {
                    permit = Arc::clone(&service.run_slots).acquire_owned() => permit.ok(),
                    _ = token.cancelled() => None,
                },
            };
            let permit = permit.filter(|_| !token.is_cancelled());
            let Some(permit) = permit else {
                // Cancelled while waiting: never started. Stored as cancelled
                // here too (a shutdown cancels through the token alone)
                let _ = sqlx::query(
                    "UPDATE job_runs SET status = ?, completed_at = ? WHERE id = ? AND status IN ('pending', 'running')",
                )
                .bind(JobRunStatus::Cancelled.as_str())
                .bind(Utc::now())
                .bind(run_id)
                .execute(&service.db)
                .await;
                let cancelled = JobResult { exit_code: 130, ..Default::default() };
                service.finalize_run(run_id, job.id, &job, Ok(cancelled)).await;
                drop(guard);
                return;
            };
            guard.start(permit);

            let _ = sqlx::query(
                "UPDATE job_runs SET status = ?, started_at = ? WHERE id = ? AND status = 'pending'",
            )
            .bind(JobRunStatus::Running.as_str())
            .bind(Utc::now())
            .bind(run_id)
            .execute(&service.db)
            .await;
            service.emit(RunEventKind::Started, run_id, job.id);

            // Changes saved while the run waited apply to it
            let job = match sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = ?")
                .bind(job.id)
                .fetch_optional(&service.db)
                .await
            {
                Ok(Some(current)) => current,
                _ => job,
            };

            let result = service.execute_job(&job, run_id, schedule_id, &token).await;
            service.finalize_run(run_id, job.id, &job, result).await;
            drop(guard);
        });

        Ok(StartedRun { run_id, queued })
    }

    /// Reserve a place for a run of `job_id` (see [`start_run`](Self::start_run)).
    fn reserve(&self, job_id: i64, scheduled: bool) -> Option<RunGuard> {
        let mut active = lock(&self.active);
        let slot = active.jobs.entry(job_id).or_default();
        let refused = if scheduled { slot.waiting } else { slot.running || slot.waiting };
        if refused {
            return None;
        }
        slot.waiting = true;
        Some(RunGuard {
            active: Arc::clone(&self.active),
            job_id,
            run_id: None,
            phase: Phase::Waiting,
            permit: None,
        })
    }

    /// Stop for a server shutdown: start no new runs, cancel the active ones,
    /// and wait up to `grace` for them to stop (rsync is killed, drives are
    /// unmounted, the run is stored as cancelled). Then store all log lines.
    pub async fn shutdown(&self, grace: Duration) {
        self.shutting_down.store(true, Ordering::SeqCst);

        let active: Vec<(i64,)> =
            sqlx::query_as("SELECT id FROM job_runs WHERE status IN ('pending', 'running')")
                .fetch_all(&self.db)
                .await
                .unwrap_or_default();
        if !active.is_empty() {
            tracing::info!("Shutdown: cancelling {} active run(s)", active.len());
        }
        for (run_id,) in active {
            let _ = sqlx::query(
                "UPDATE job_runs SET status = ?, completed_at = ?, summary = ? \
                 WHERE id = ? AND status IN ('pending', 'running')",
            )
            .bind(JobRunStatus::Cancelled.as_str())
            .bind(Utc::now())
            .bind(SHUTDOWN_SUMMARY)
            .bind(run_id)
            .execute(&self.db)
            .await;
            self.log(run_id, LogLevel::Warning, "Server is shutting down, cancelling the run", "system").await;
        }
        // After the status is stored (the runs read it when they wake);
        // wakes runs waiting for a slot, too
        for (_, token) in lock(&self.active).tokens.values() {
            token.cancel();
        }

        self.tasks.close();
        if tokio::time::timeout(grace, self.tasks.wait()).await.is_err() {
            tracing::warn!(
                "Shutdown: {} run(s) did not stop within {:?}",
                self.tasks.len(),
                grace
            );
        }
        self.log_writer.flush().await;
    }

    /// Update the max_runs_per_job setting dynamically
    pub async fn set_max_runs_per_job(&self, value: Option<u32>) {
        *self.max_runs_per_job.write().await = value;
        tracing::info!("Updated max_runs_per_job to {:?}", value);
    }

    pub async fn set_log_retention_days(&self, value: Option<u32>) {
        *self.log_retention_days.write().await = value;
    }

    /// Log retention: remove old log lines, then compact the logs database
    /// when that frees a lot of space. Errors are only logged.
    pub async fn maintain_logs(&self) {
        match self.prune_logs().await {
            Ok(0) => {}
            Ok(n) => tracing::info!("Log retention: removed the log lines of {} old run(s)", n),
            Err(e) => tracing::warn!("Log retention failed: {}", e),
        }
        if let Err(e) = self.compact_logs().await {
            tracing::warn!("Compacting the logs database failed: {}", e);
        }
    }

    /// Remove the log lines of runs that finished more than the configured
    /// number of days ago. The run records stay (status, sizes, error
    /// reason); each job's newest finished run and newest failed run keep
    /// their log. Returns how many runs lost their log.
    pub async fn prune_logs(&self) -> anyhow::Result<usize> {
        let Some(days) = *self.log_retention_days.read().await else {
            return Ok(0);
        };
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let runs: Vec<(i64,)> = sqlx::query_as(
            "SELECT id FROM job_runs \
             WHERE log_pruned = 0 AND completed_at IS NOT NULL \
               AND julianday(completed_at) < julianday(?) \
               AND id NOT IN (SELECT MAX(id) FROM job_runs WHERE completed_at IS NOT NULL GROUP BY job_id) \
               AND id NOT IN (SELECT MAX(id) FROM job_runs WHERE status = 'failed' GROUP BY job_id)",
        )
        .bind(cutoff.to_rfc3339())
        .fetch_all(&self.db)
        .await?;

        let ids: Vec<i64> = runs.into_iter().map(|(id,)| id).collect();
        for chunk in ids.chunks(500) {
            let mut del = QueryBuilder::new("DELETE FROM log_entries WHERE job_run_id IN (");
            let mut sep = del.separated(", ");
            for id in chunk {
                sep.push_bind(*id);
            }
            del.push(")");
            del.build().execute(&self.logs_db).await?;

            let mut mark = QueryBuilder::new("UPDATE job_runs SET log_pruned = 1 WHERE id IN (");
            let mut sep = mark.separated(", ");
            for id in chunk {
                sep.push_bind(*id);
            }
            mark.push(")");
            mark.build().execute(&self.db).await?;
        }
        Ok(ids.len())
    }

    /// VACUUM the logs database when at least a quarter of it is free pages.
    ///
    /// Holds every run slot meanwhile, so no backup writes log lines into the
    /// locked database (they would be dropped); skipped while a run is active
    /// or when the disk lacks room for the copy VACUUM writes.
    async fn compact_logs(&self) -> anyhow::Result<()> {
        let pragma = |name: &'static str| async move {
            sqlx::query_scalar::<_, i64>(&format!("PRAGMA {}", name)).fetch_one(&self.logs_db).await
        };
        let (pages, free_pages, page_size) =
            (pragma("page_count").await?, pragma("freelist_count").await?, pragma("page_size").await?);
        if free_pages * 4 <= pages {
            return Ok(());
        }
        let Ok(_slots) = Arc::clone(&self.run_slots).try_acquire_many_owned(MAX_PARALLEL_RUNS as u32) else {
            return Ok(());
        };
        let file: Option<String> = sqlx::query_as::<_, (i64, String, String)>("PRAGMA database_list")
            .fetch_all(&self.logs_db)
            .await?
            .into_iter()
            .find(|(_, name, _)| name == "main")
            .map(|(_, _, file)| file)
            .filter(|f| !f.is_empty());
        if let Some(file) = file {
            let dir = Path::new(&file).parent().unwrap_or(Path::new("."));
            let needed = ((pages - free_pages) * page_size) as u64;
            if crate::services::fs_space(dir).is_some_and(|(free, _)| free < needed) {
                tracing::warn!("Not compacting the logs database: too little free disk space");
                return Ok(());
            }
        }
        let before = pages * page_size;
        sqlx::query("VACUUM").execute(&self.logs_db).await?;
        let after = pragma("page_count").await? * page_size;
        tracing::info!("Compacted the logs database: {} MB to {} MB", before >> 20, after >> 20);
        Ok(())
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

    /// Whether the job has a run running or waiting for its turn
    pub async fn is_job_running(&self, job_id: i64) -> bool {
        lock(&self.active).jobs.get(&job_id).is_some_and(|s| s.running || s.waiting)
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

    /// Mark a job as running without starting a run (tests, and callers
    /// that must keep runs away). Returns `false` if it is active already.
    /// Undo with [`release_job`](Self::release_job).
    pub async fn try_reserve_job(&self, job_id: i64) -> bool {
        let mut active = lock(&self.active);
        let slot = active.jobs.entry(job_id).or_default();
        if slot.running || slot.waiting {
            return false;
        }
        slot.running = true;
        true
    }

    /// Undo [`try_reserve_job`](Self::try_reserve_job).
    pub async fn release_job(&self, job_id: i64) {
        let mut active = lock(&self.active);
        if let Some(slot) = active.jobs.get_mut(&job_id) {
            slot.running = false;
            if !slot.waiting {
                active.jobs.remove(&job_id);
            }
        }
    }

    /// Cancel a run: running or still waiting for its turn. Providers see it
    /// at once through the run's cancellation token.
    pub async fn cancel_job(&self, run_id: i64) -> anyhow::Result<bool> {
        tracing::info!("cancel_job called for run_id: {}", run_id);

        self.log(run_id, LogLevel::Warning, "Job cancellation requested", "system").await;

        // Stored before the token fires: the woken run reads this status to
        // finish as cancelled
        sqlx::query("UPDATE job_runs SET status = ?, completed_at = ? WHERE id = ? AND status IN ('pending', 'running')")
            .bind(JobRunStatus::Cancelled.as_str())
            .bind(Utc::now())
            .bind(run_id)
            .execute(&self.db)
            .await?;

        let token = lock(&self.active).tokens.get(&run_id).map(|(_, t)| t.clone());
        if let Some(token) = &token {
            token.cancel();
        }
        Ok(token.is_some())
    }

    /// Cancel every active run of a job (the running one and a queued
    /// follow-up). Returns how many runs were cancelled.
    pub async fn cancel_job_runs(&self, job_id: i64) -> anyhow::Result<usize> {
        let runs: Vec<i64> = lock(&self.active)
            .tokens
            .iter()
            .filter(|(_, (job, _))| *job == job_id)
            .map(|(run, _)| *run)
            .collect();
        for run_id in &runs {
            self.cancel_job(*run_id).await?;
        }
        Ok(runs.len())
    }

    /// Check if a job run has been cancelled
    pub fn is_cancelled_sync(&self, run_id: i64) -> bool {
        lock(&self.active).tokens.get(&run_id).is_some_and(|(_, t)| t.is_cancelled())
    }

    /// Run a job (the run's place is held by the caller, see start_run)
    async fn execute_job(
        &self,
        job: &Job,
        run_id: i64,
        schedule_id: Option<i64>,
        cancel: &CancellationToken,
    ) -> anyhow::Result<JobResult> {
        use futures::FutureExt;

        // Catch panics so a provider bug cannot leave the job marked as running
        // forever or the run stuck in the 'running' state.
        let result = match std::panic::AssertUnwindSafe(self.do_execute(job, run_id, schedule_id, cancel))
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

        result
    }

    async fn do_execute(
        &self,
        job: &Job,
        run_id: i64,
        _schedule_id: Option<i64>,
        cancel: &CancellationToken,
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
                    if cancel.is_cancelled() {
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

                    let token = cancel.clone();
                    let is_cancelled_fn = move || token.is_cancelled();

                    let log_tx = self.log_tx.clone();
                    // Persist as well, so compression output shows up in History.
                    // log_fn is synchronous; the writer keeps the lines in order.
                    let log_writer = self.log_writer.clone();
                    let log_fn = move |msg: String| {
                        let timestamp = Utc::now();
                        log_writer.write_now(LogRow {
                            run_id,
                            level: LogLevel::Info,
                            message: msg.clone(),
                            source: "compress".to_string(),
                            timestamp,
                        });
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

                    let archive_path = match archive_path {
                        Ok(p) => p,
                        Err(e) => {
                            if cancel.is_cancelled() {
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

        // The space check for local destinations runs in RsyncProvider::sync,
        // once the drive is mounted (one mount per run)

        // Create cancellation checker closure
        let token = cancel.clone();
        let is_cancelled: providers::CancellationChecker = Arc::new(move |_| token.is_cancelled());

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

        // For the quick space check: the last completed run's transfer size
        let last_transfer_bytes: Option<u64> = sqlx::query_scalar::<_, i64>(
            "SELECT bytes_transferred FROM job_runs \
             WHERE job_id = ? AND id != ? AND status = 'completed' AND bytes_transferred > 0 \
             ORDER BY id DESC LIMIT 1",
        )
        .bind(job.id)
        .bind(run_id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
        .map(|b| b as u64);

        // Build sync context (uses effective_source_dirs: staging dir when compress is enabled)
        let ctx = Arc::new(SyncContext {
            run_id,
            source_dirs: effective_source_dirs,
            destination,
            options: sync_options,
            credential,
            log_sender: self.log_tx.clone(),
            log_writer: self.log_writer.clone(),
            is_cancelled,
            cancel: cancel.clone(),
            last_transfer_bytes,
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
        let row = LogRow {
            run_id,
            level,
            message: message.to_string(),
            source: source.to_string(),
            timestamp: Utc::now(),
        };
        // Broadcast to WebSocket clients, then store (batched)
        let _ = self.log_tx.send(LogMessage {
            run_id,
            level,
            message: row.message.clone(),
            source: row.source.clone(),
            timestamp: row.timestamp,
        });
        self.log_writer.write(row).await;
    }

    /// Store a line, wait until it and all earlier lines are in the database,
    /// then broadcast it. For messages that make clients reload the log.
    async fn log_stored(&self, run_id: i64, level: LogLevel, message: &str, source: &str) {
        let row = LogRow {
            run_id,
            level,
            message: message.to_string(),
            source: source.to_string(),
            timestamp: Utc::now(),
        };
        let msg = LogMessage {
            run_id,
            level,
            message: row.message.clone(),
            source: row.source.clone(),
            timestamp: row.timestamp,
        };
        self.log_writer.write(row).await;
        self.log_writer.flush().await;
        let _ = self.log_tx.send(msg);
    }

    /// Wait until every queued log line is stored.
    pub async fn flush_logs(&self) {
        self.log_writer.flush().await;
    }

    /// The last error line of a run's log (call after flushing the log)
    async fn last_error_line(&self, run_id: i64) -> Option<String> {
        sqlx::query_as::<_, (String,)>(
            "SELECT message FROM log_entries WHERE job_run_id = ? AND level = 'error' ORDER BY id DESC LIMIT 1",
        )
        .bind(run_id)
        .fetch_optional(&self.logs_db)
        .await
        .ok()
        .flatten()
        .map(|(m,)| m)
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

            // A failed run keeps its reason, so lists can show it without the log
            if status == JobRunStatus::Failed {
                self.flush_logs().await;
                let reason = match self.last_error_line(run_id).await {
                    Some(line) => Some(line),
                    None => result.as_ref().err().map(|e| e.to_string()),
                };
                if let Some(reason) = reason {
                    let reason: String = reason.chars().take(MAX_SUMMARY_CHARS).collect();
                    let _ = sqlx::query(
                        "UPDATE job_runs SET summary = ? WHERE id = ? AND (summary IS NULL OR summary = '')",
                    )
                    .bind(reason)
                    .bind(run_id)
                    .execute(&self.db)
                    .await;
                }
            }

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

        // After the status and the whole log are stored, so clients reloading
        // on this line or the Completed event see both
        self.log_stored(
            run_id,
            LogLevel::Info,
            &format!("{}: {}", RUN_FINISHED_PREFIX, final_status.as_str()),
            "system",
        )
        .await;
        self.emit(RunEventKind::Completed, run_id, job_id);

        self.notify_run_result(run_id, job, final_status).await;

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
pub(crate) fn format_file_size(bytes: u64) -> String {
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

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    use std::io::Write;

    async fn service() -> (SqlitePool, SqlitePool, Arc<BackupService>) {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(1000);
        let service = Arc::new(BackupService::new(db.clone(), logs_db.clone(), log_tx, None, credentials));
        (db, logs_db, service)
    }

    async fn slow_local_job(db: &SqlitePool, dir: &Path) -> Job {
        let src = dir.join("src");
        let dst = dir.join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        // 20 MB at 200 KB/s: far longer than the test waits
        let mut f = std::fs::File::create(src.join("big.bin")).unwrap();
        f.write_all(&vec![7u8; 20 * 1024 * 1024]).unwrap();

        let destination = serde_json::json!({
            "type": "local", "mount_point": dst, "backup_subdir": "b",
            "auto_mount": false, "auto_unmount": false
        });
        let id = sqlx::query(
            "INSERT INTO jobs (name, mount_point, source_dirs, destination_type, destination_config, sync_options) \
             VALUES ('slow', ?, ?, 'local', ?, ?)",
        )
        .bind(dst.to_string_lossy().to_string())
        .bind(serde_json::json!([src]).to_string())
        .bind(destination.to_string())
        .bind(r#"{"bandwidth_limit_kbps": 200, "space_check_mode": "none"}"#)
        .execute(db)
        .await
        .unwrap()
        .last_insert_rowid();
        sqlx::query_as("SELECT * FROM jobs WHERE id = ?").bind(id).fetch_one(db).await.unwrap()
    }

    #[tokio::test]
    async fn shutdown_cancels_running_runs_and_blocks_new_ones() {
        let (db, logs_db, service) = service().await;
        let dir = tempfile::tempdir().unwrap();
        let job = slow_local_job(&db, dir.path()).await;

        let run_id = service.start_run(job.clone(), None).await.unwrap().run_id;
        // Wait until rsync is running
        for _ in 0..100 {
            let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM log_entries WHERE message LIKE 'Spawned rsync%'")
                .fetch_one(&logs_db)
                .await
                .unwrap();
            if n > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(service.is_job_running(job.id).await, "run should still be active");

        let started = std::time::Instant::now();
        service.shutdown(Duration::from_secs(10)).await;
        assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());

        let (status, summary): (String, Option<String>) =
            sqlx::query_as("SELECT status, summary FROM job_runs WHERE id = ?")
                .bind(run_id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "cancelled");
        assert_eq!(summary.as_deref(), Some(SHUTDOWN_SUMMARY));
        assert!(!service.is_job_running(job.id).await);

        // The whole log is stored, ending with the final status
        let (last,): (String,) = sqlx::query_as("SELECT message FROM log_entries WHERE job_run_id = ? ORDER BY id DESC LIMIT 1")
            .bind(run_id)
            .fetch_one(&logs_db)
            .await
            .unwrap();
        assert_eq!(last, format!("{}: cancelled", RUN_FINISHED_PREFIX));

        assert!(matches!(service.start_run(job, None).await, Err(StartRunError::ShuttingDown)));
    }

    #[tokio::test]
    async fn shutdown_without_runs_returns_at_once() {
        let (_db, _logs, service) = service().await;
        tokio::time::timeout(Duration::from_secs(2), service.shutdown(Duration::from_secs(10)))
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod notification_flow_tests {
    use super::*;
    use crate::services::notification_service::{NotificationSettings, WebhookSettings};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A webhook receiver that reports the "event" field of each request.
    async fn webhook_receiver() -> (String, tokio::sync::mpsc::UnboundedReceiver<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/hook", listener.local_addr().unwrap());
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    loop {
                        let n = sock.read(&mut chunk).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        let text = String::from_utf8_lossy(&buf).to_string();
                        if let Some(end) = text.find("\r\n\r\n") {
                            let len = text
                                .lines()
                                .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.trim().parse::<usize>().unwrap()))
                                .unwrap_or(0);
                            if buf.len() >= end + 4 + len {
                                let body: serde_json::Value = serde_json::from_str(&text[end + 4..]).unwrap();
                                let _ = tx.send(body["event"].as_str().unwrap().to_string());
                                break;
                            }
                        }
                    }
                    let _ = sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await;
                });
            }
        });
        (url, rx)
    }

    async fn next_event(rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> Option<String> {
        tokio::time::timeout(Duration::from_secs(3), rx.recv()).await.ok().flatten()
    }

    async fn setup(success: bool) -> (SqlitePool, BackupService, tokio::sync::mpsc::UnboundedReceiver<String>, Job) {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(100);
        let service = BackupService::new(db.clone(), logs_db, log_tx, None, credentials);

        let (url, rx) = webhook_receiver().await;
        let mut settings = NotificationSettings {
            webhook: WebhookSettings { enabled: true, url, auth_header: String::new() },
            ..Default::default()
        };
        settings.events.success = success;
        settings.save(&db).await.unwrap();

        let id = sqlx::query(
            "INSERT INTO jobs (name, mount_point, source_dirs, destination_type, destination_config) \
             VALUES ('Docs', '', '[]', 'local', '{}')",
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();
        let job: Job = sqlx::query_as("SELECT * FROM jobs WHERE id = ?").bind(id).fetch_one(&db).await.unwrap();
        (db, service, rx, job)
    }

    async fn finish(db: &SqlitePool, service: &BackupService, job: &Job, errors: i32) {
        let run_id = sqlx::query("INSERT INTO job_runs (job_id, status, started_at) VALUES (?, 'running', ?)")
            .bind(job.id)
            .bind(Utc::now())
            .execute(db)
            .await
            .unwrap()
            .last_insert_rowid();
        let result = JobResult { error_count: errors, exit_code: if errors > 0 { 1 } else { 0 }, ..Default::default() };
        service.finalize_run(run_id, job.id, job, Ok(result)).await;
    }

    async fn alert_state(db: &SqlitePool, job_id: i64) -> String {
        sqlx::query_as::<_, (String,)>("SELECT alert_state FROM jobs WHERE id = ?")
            .bind(job_id)
            .fetch_one(db)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn failure_then_recovery_then_quiet_success() {
        let (db, service, mut rx, job) = setup(false).await;

        finish(&db, &service, &job, 1).await;
        assert_eq!(next_event(&mut rx).await.as_deref(), Some("failed"));
        assert_eq!(alert_state(&db, job.id).await, "failed");

        finish(&db, &service, &job, 0).await;
        assert_eq!(next_event(&mut rx).await.as_deref(), Some("recovered"));
        assert_eq!(alert_state(&db, job.id).await, "ok");

        // "Every successful run" is off: nothing more
        finish(&db, &service, &job, 0).await;
        assert_eq!(next_event(&mut rx).await, None);
    }

    #[tokio::test]
    async fn failed_run_keeps_its_last_error_as_summary() {
        let (db, service, _rx, job) = setup(false).await;
        let run_id = sqlx::query("INSERT INTO job_runs (job_id, status, started_at) VALUES (?, 'running', ?)")
            .bind(job.id)
            .bind(Utc::now())
            .execute(&db)
            .await
            .unwrap()
            .last_insert_rowid();
        service.log(run_id, LogLevel::Error, "first problem", "rsync").await;
        service.log(run_id, LogLevel::Error, "rsync: connection refused", "rsync").await;
        let result = JobResult { error_count: 2, exit_code: 1, ..Default::default() };
        service.finalize_run(run_id, job.id, &job, Ok(result)).await;

        let (summary,): (Option<String>,) = sqlx::query_as("SELECT summary FROM job_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(summary.as_deref(), Some("rsync: connection refused"));

        // A successful run gets none
        finish(&db, &service, &job, 0).await;
        let (summary,): (Option<String>,) = sqlx::query_as("SELECT summary FROM job_runs ORDER BY id DESC LIMIT 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(summary, None);
    }

    #[tokio::test]
    async fn success_events_when_enabled() {
        let (db, service, mut rx, job) = setup(true).await;
        finish(&db, &service, &job, 0).await;
        assert_eq!(next_event(&mut rx).await.as_deref(), Some("success"));
    }

    #[tokio::test]
    async fn overdue_is_reported_once() {
        let (db, service, mut rx, job) = setup(false).await;
        sqlx::query("INSERT INTO schedules (job_id, enabled, cron_expression) VALUES (?, 1, '0 2 * * *')")
            .bind(job.id)
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("UPDATE jobs SET last_success_at = ? WHERE id = ?")
            .bind(Utc::now() - chrono::Duration::days(5))
            .bind(job.id)
            .execute(&db)
            .await
            .unwrap();

        assert_eq!(service.check_overdue(chrono_tz::Tz::UTC).await, 1);
        assert_eq!(next_event(&mut rx).await.as_deref(), Some("overdue"));
        assert_eq!(alert_state(&db, job.id).await, "overdue");
        assert_eq!(service.check_overdue(chrono_tz::Tz::UTC).await, 0);

        // A successful run ends it
        finish(&db, &service, &job, 0).await;
        assert_eq!(next_event(&mut rx).await.as_deref(), Some("recovered"));
    }

    #[tokio::test]
    async fn recent_backup_is_not_overdue() {
        let (db, service, _rx, job) = setup(false).await;
        sqlx::query("INSERT INTO schedules (job_id, enabled, cron_expression) VALUES (?, 1, '0 2 * * *')")
            .bind(job.id)
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("UPDATE jobs SET last_success_at = ? WHERE id = ?")
            .bind(Utc::now() - chrono::Duration::hours(30))
            .bind(job.id)
            .execute(&db)
            .await
            .unwrap();
        assert_eq!(service.check_overdue(chrono_tz::Tz::UTC).await, 0);
    }
}

#[cfg(test)]
mod run_event_tests {
    use super::*;

    #[tokio::test]
    async fn a_run_sends_started_then_completed() {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(100);
        let service = Arc::new(BackupService::new(db.clone(), logs_db, log_tx, None, credentials));
        let mut events = service.subscribe_events();

        // No mount point: the run fails right away at the config check
        let id = sqlx::query(
            "INSERT INTO jobs (name, mount_point, source_dirs, destination_type, destination_config) \
             VALUES ('j', '', '[]', 'local', '{}')",
        )
        .execute(&db)
        .await
        .unwrap()
        .last_insert_rowid();
        let job: Job = sqlx::query_as("SELECT * FROM jobs WHERE id = ?").bind(id).fetch_one(&db).await.unwrap();
        let run_id = service.start_run(job, None).await.unwrap().run_id;

        let started = tokio::time::timeout(Duration::from_secs(3), events.recv()).await.unwrap().unwrap();
        assert_eq!((started.kind, started.run_id, started.job_id), (RunEventKind::Started, run_id, id));
        let completed = tokio::time::timeout(Duration::from_secs(3), events.recv()).await.unwrap().unwrap();
        assert_eq!((completed.kind, completed.run_id), (RunEventKind::Completed, run_id));

        // The status is final by the time Completed arrives
        let (status,): (String,) = sqlx::query_as("SELECT status FROM job_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(status, "failed");
    }
}

#[cfg(test)]
mod run_queue_tests {
    use super::*;

    async fn setup() -> (SqlitePool, SqlitePool, Arc<BackupService>) {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(100);
        let service = Arc::new(BackupService::new(db.clone(), logs_db.clone(), log_tx, None, credentials));
        (db, logs_db, service)
    }

    /// A job that fails right away (no mount point), with one schedule
    async fn add_job(db: &SqlitePool, name: &str) -> (Job, i64) {
        let id = sqlx::query(
            "INSERT INTO jobs (name, mount_point, source_dirs, destination_type, destination_config) \
             VALUES (?, '', '[]', 'local', '{}')",
        )
        .bind(name)
        .execute(db)
        .await
        .unwrap()
        .last_insert_rowid();
        let schedule = sqlx::query("INSERT INTO schedules (job_id, cron_expression) VALUES (?, '0 3 * * *')")
            .bind(id)
            .execute(db)
            .await
            .unwrap()
            .last_insert_rowid();
        let job: Job = sqlx::query_as("SELECT * FROM jobs WHERE id = ?").bind(id).fetch_one(db).await.unwrap();
        (job, schedule)
    }

    async fn status(db: &SqlitePool, run_id: i64) -> String {
        sqlx::query_as::<_, (String,)>("SELECT status FROM job_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(db)
            .await
            .unwrap()
            .0
    }

    async fn next_event(rx: &mut broadcast::Receiver<RunEvent>) -> (RunEventKind, i64) {
        let e = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await.unwrap().unwrap();
        (e.kind, e.run_id)
    }

    #[tokio::test]
    async fn a_run_waits_for_the_slot_then_runs() {
        let (db, _, service) = setup().await;
        let (job, _) = add_job(&db, "a").await;
        let mut events = service.subscribe_events();

        // Another backup holds the only slot
        let busy = Arc::clone(&service.run_slots).acquire_owned().await.unwrap();
        let started = service.start_run(job.clone(), None).await.unwrap();
        assert!(started.queued);
        assert_eq!(next_event(&mut events).await, (RunEventKind::Queued, started.run_id));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(status(&db, started.run_id).await, "pending");
        assert!(service.is_job_running(job.id).await);

        drop(busy);
        assert_eq!(next_event(&mut events).await, (RunEventKind::Started, started.run_id));
        assert_eq!(next_event(&mut events).await, (RunEventKind::Completed, started.run_id));
        assert_eq!(status(&db, started.run_id).await, "failed");
        service.tasks.close();
        service.tasks.wait().await;
        assert!(!service.is_job_running(job.id).await);
    }

    #[tokio::test]
    async fn a_schedule_queues_one_follow_up_behind_a_running_job() {
        let (db, _, service) = setup().await;
        let (job, schedule) = add_job(&db, "a").await;
        let _busy = Arc::clone(&service.run_slots).acquire_owned().await.unwrap();

        // The job is running (reserved here, as a long run would be)
        assert!(service.try_reserve_job(job.id).await);
        assert!(matches!(service.start_run(job.clone(), None).await, Err(StartRunError::AlreadyRunning)));
        let follow_up = service.start_run(job.clone(), Some(schedule)).await.unwrap();
        assert!(follow_up.queued);
        // Only one follow-up
        assert!(matches!(
            service.start_run(job.clone(), Some(schedule)).await,
            Err(StartRunError::AlreadyRunning)
        ));

        // The running run ending keeps the follow-up in place
        service.release_job(job.id).await;
        assert!(service.is_job_running(job.id).await);
        assert!(matches!(service.start_run(job, None).await, Err(StartRunError::AlreadyRunning)));
    }

    #[tokio::test]
    async fn cancelling_a_waiting_run_never_starts_it() {
        let (db, logs_db, service) = setup().await;
        let (job, _) = add_job(&db, "a").await;
        let mut events = service.subscribe_events();
        let _busy = Arc::clone(&service.run_slots).acquire_owned().await.unwrap();

        let started = service.start_run(job.clone(), None).await.unwrap();
        assert_eq!(next_event(&mut events).await.0, RunEventKind::Queued);
        assert_eq!(service.cancel_job_runs(job.id).await.unwrap(), 1);
        assert_eq!(next_event(&mut events).await, (RunEventKind::Completed, started.run_id));

        assert_eq!(status(&db, started.run_id).await, "cancelled");
        service.tasks.close();
        service.tasks.wait().await;
        assert!(!service.is_job_running(job.id).await);
        let (starts,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM log_entries WHERE job_run_id = ? AND message = ?")
            .bind(started.run_id)
            .bind(RUN_STARTED_MESSAGE)
            .fetch_one(&logs_db)
            .await
            .unwrap();
        assert_eq!(starts, 0);
    }

    #[tokio::test]
    async fn shutdown_wakes_waiting_runs() {
        let (db, _, service) = setup().await;
        let (job, _) = add_job(&db, "a").await;
        let _busy = Arc::clone(&service.run_slots).acquire_owned().await.unwrap();
        let started = service.start_run(job, None).await.unwrap();

        tokio::time::timeout(Duration::from_secs(5), service.shutdown(Duration::from_secs(3)))
            .await
            .unwrap();
        assert_eq!(service.tasks.len(), 0);
        assert_eq!(status(&db, started.run_id).await, "cancelled");
        let (summary,): (Option<String>,) = sqlx::query_as("SELECT summary FROM job_runs WHERE id = ?")
            .bind(started.run_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(summary.as_deref(), Some(SHUTDOWN_SUMMARY));
    }
}

#[cfg(test)]
mod log_retention_tests {
    use super::*;

    #[tokio::test]
    async fn old_logs_go_but_the_newest_and_newest_failed_runs_keep_theirs() {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(100);
        let service = BackupService::new(db.clone(), logs_db.clone(), log_tx, None, credentials);
        service.set_log_retention_days(Some(30)).await;

        let job = sqlx::query("INSERT INTO jobs (name, mount_point, source_dirs) VALUES ('j', '/m', '[]')")
            .execute(&db)
            .await
            .unwrap()
            .last_insert_rowid();
        let old = Utc::now() - chrono::Duration::days(60);
        let mut runs = Vec::new();
        // old failed, old completed, old completed (newest finished), recent... in id order
        for (status, done) in [
            ("failed", Some(old)),
            ("completed", Some(old)),
            ("failed", Some(old)),
            ("completed", Some(old)),
            ("completed", Some(Utc::now())),
            ("pending", None),
        ] {
            let id = sqlx::query("INSERT INTO job_runs (job_id, status, completed_at) VALUES (?, ?, ?)")
                .bind(job)
                .bind(status)
                .bind(done)
                .execute(&db)
                .await
                .unwrap()
                .last_insert_rowid();
            sqlx::query("INSERT INTO log_entries (job_run_id, level, message) VALUES (?, 'info', 'x')")
                .bind(id)
                .execute(&logs_db)
                .await
                .unwrap();
            runs.push(id);
        }

        // Runs 0, 1, 3 are old and neither the newest finished (4) nor the
        // newest failed (2); 5 has not finished
        assert_eq!(service.prune_logs().await.unwrap(), 3);
        assert_eq!(service.prune_logs().await.unwrap(), 0);

        for (i, id) in runs.iter().enumerate() {
            let (lines,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM log_entries WHERE job_run_id = ?")
                .bind(id)
                .fetch_one(&logs_db)
                .await
                .unwrap();
            let (pruned,): (bool,) = sqlx::query_as("SELECT log_pruned FROM job_runs WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
            let gone = [0, 1, 3].contains(&i);
            assert_eq!((lines == 0, pruned), (gone, gone), "run index {}", i);
        }

        // Off: nothing more is removed
        service.set_log_retention_days(None).await;
        assert_eq!(service.prune_logs().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn compacting_shrinks_a_mostly_empty_logs_database() {
        let dir = tempfile::tempdir().unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(dir.path().join("logs.db"))
            .create_if_missing(true);
        let logs_db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(2).connect_with(options).await.unwrap();
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let db = crate::test_helpers::test_db().await;
        let credentials = Arc::new(CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = broadcast::channel(100);
        let service = BackupService::new(db, logs_db.clone(), log_tx, None, credentials);

        let line = "x".repeat(2000);
        for _ in 0..2000 {
            sqlx::query("INSERT INTO log_entries (job_run_id, level, message) VALUES (1, 'info', ?)")
                .bind(&line)
                .execute(&logs_db)
                .await
                .unwrap();
        }
        sqlx::query("DELETE FROM log_entries").execute(&logs_db).await.unwrap();
        let pages = || async { sqlx::query_scalar::<_, i64>("PRAGMA page_count").fetch_one(&logs_db).await.unwrap() };
        let before = pages().await;

        // A running backup holds the slot: no compaction
        let busy = Arc::clone(&service.run_slots).acquire_owned().await.unwrap();
        service.compact_logs().await.unwrap();
        assert_eq!(pages().await, before);
        drop(busy);

        service.compact_logs().await.unwrap();
        assert!(pages().await < before / 10, "{} -> {}", before, pages().await);
    }
}

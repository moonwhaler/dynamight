use crate::models::{Job, JobRunStatus, LogLevel, LogMessage};
use chrono::Utc;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, RwLock, Mutex};

#[derive(Debug, Default)]
pub struct JobResult {
    pub exit_code: i32,
    pub files_transferred: i64,
    pub bytes_transferred: i64,
    pub error_count: i32,
}

/// Holds information about a running process for cancellation
struct RunningProcess {
    child: Child,
    cancelled: bool,
}

pub struct BackupService {
    db: SqlitePool,
    log_tx: broadcast::Sender<LogMessage>,
    running_jobs: Arc<RwLock<std::collections::HashSet<i64>>>,
    /// Maps run_id to the currently running child process
    running_processes: Arc<Mutex<HashMap<i64, RunningProcess>>>,
    max_runs_per_job: Option<u32>,
}

impl BackupService {
    pub fn new(db: SqlitePool, log_tx: broadcast::Sender<LogMessage>, max_runs_per_job: Option<u32>) -> Self {
        Self {
            db,
            log_tx,
            running_jobs: Arc::new(RwLock::new(std::collections::HashSet::new())),
            running_processes: Arc::new(Mutex::new(HashMap::new())),
            max_runs_per_job,
        }
    }

    /// Cleanup old job runs, keeping only the most recent `max_runs_per_job` runs.
    /// Log entries are deleted automatically via CASCADE.
    pub async fn cleanup_old_runs(&self, job_id: i64) {
        let Some(max_runs) = self.max_runs_per_job else {
            return;
        };

        // Delete old runs beyond the limit, keeping the most recent ones
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

    /// Cancel a running job. If force is false, sends SIGTERM and waits up to 5 seconds.
    /// If force is true (or after grace period), sends SIGKILL.
    pub async fn cancel_job(&self, run_id: i64, force: bool) -> anyhow::Result<bool> {
        // Mark as cancelled in database first
        sqlx::query("UPDATE job_runs SET status = ?, completed_at = ? WHERE id = ? AND status = 'running'")
            .bind(JobRunStatus::Cancelled.as_str())
            .bind(Utc::now())
            .bind(run_id)
            .execute(&self.db)
            .await?;

        // Log cancellation request
        self.log(run_id, LogLevel::Warning,
            if force { "Force kill requested" } else { "Cancellation requested, sending SIGTERM..." },
            "system"
        ).await;

        // Try to kill the running process
        let mut processes = self.running_processes.lock().await;
        if let Some(running) = processes.get_mut(&run_id) {
            running.cancelled = true;

            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                if let Some(pid) = running.child.id() {
                    let pid = Pid::from_raw(pid as i32);

                    if force {
                        // Force kill - SIGKILL immediately
                        let _ = kill(pid, Signal::SIGKILL);
                        self.log(run_id, LogLevel::Warning, "Sent SIGKILL to process", "system").await;
                        return Ok(true);
                    }

                    // Graceful - send SIGTERM first
                    if kill(pid, Signal::SIGTERM).is_ok() {
                        self.log(run_id, LogLevel::Info, "Sent SIGTERM, waiting for process to exit...", "system").await;

                        // Wait up to 5 seconds for graceful shutdown
                        drop(processes); // Release lock during wait

                        for _ in 0..10 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                            // Check if process exited
                            let mut processes = self.running_processes.lock().await;
                            if !processes.contains_key(&run_id) {
                                self.log(run_id, LogLevel::Info, "Process terminated gracefully", "system").await;
                                return Ok(true);
                            }

                            // Check if still running
                            if let Some(running) = processes.get_mut(&run_id) {
                                if running.child.try_wait().ok().flatten().is_some() {
                                    processes.remove(&run_id);
                                    self.log(run_id, LogLevel::Info, "Process terminated gracefully", "system").await;
                                    return Ok(true);
                                }
                            }
                        }

                        // Grace period expired, send SIGKILL
                        let mut processes = self.running_processes.lock().await;
                        if let Some(running) = processes.get_mut(&run_id) {
                            if let Some(pid) = running.child.id() {
                                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
                                self.log(run_id, LogLevel::Warning, "Grace period expired, sent SIGKILL", "system").await;
                            }
                        }
                        return Ok(true);
                    }
                }
            }

            #[cfg(not(unix))]
            {
                // On non-Unix, just try to kill
                let _ = running.child.kill().await;
                return Ok(true);
            }
        }

        Ok(false) // No process found to kill
    }

    /// Check if a job run has been cancelled
    pub async fn is_cancelled(&self, run_id: i64) -> bool {
        let processes = self.running_processes.lock().await;
        processes.get(&run_id).map(|p| p.cancelled).unwrap_or(false)
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

        result
    }

    async fn do_execute(
        &self,
        job: &Job,
        run_id: i64,
        _schedule_id: Option<i64>,
    ) -> anyhow::Result<JobResult> {
        let mut result = JobResult::default();

        // Log start
        self.log(run_id, LogLevel::Info, "Starting backup job", "system")
            .await;

        // Mount if needed
        if job.auto_mount {
            if let Some(uuid) = &job.usb_uuid {
                self.log(
                    run_id,
                    LogLevel::Info,
                    &format!("Mounting UUID {} to {}", uuid, job.mount_point),
                    "mount",
                )
                .await;

                // Create mount point if needed
                if let Err(e) = tokio::fs::create_dir_all(&job.mount_point).await {
                    self.log(
                        run_id,
                        LogLevel::Warning,
                        &format!("Could not create mount point: {}", e),
                        "mount",
                    )
                    .await;
                }

                // Mount
                let mount_result = Command::new("mount")
                    .args(["-U", uuid, &job.mount_point])
                    .output()
                    .await;

                match mount_result {
                    Ok(output) if output.status.success() => {
                        self.log(run_id, LogLevel::Info, "Mount successful", "mount")
                            .await;
                    }
                    Ok(output) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        // Check if already mounted
                        if !stderr.contains("already mounted") {
                            self.log(
                                run_id,
                                LogLevel::Error,
                                &format!("Mount failed: {}", stderr),
                                "mount",
                            )
                            .await;
                            result.error_count += 1;
                        }
                    }
                    Err(e) => {
                        self.log(
                            run_id,
                            LogLevel::Error,
                            &format!("Mount error: {}", e),
                            "mount",
                        )
                        .await;
                        result.error_count += 1;
                    }
                }
            }
        }

        // Detect filesystem type
        let fstype = self.detect_filesystem(&job.mount_point).await;
        self.log(
            run_id,
            LogLevel::Info,
            &format!("Filesystem type: {}", fstype),
            "system",
        )
        .await;

        // Build rsync args
        let rsync_args = self.build_rsync_args(job, &fstype);
        self.log(
            run_id,
            LogLevel::Debug,
            &format!("Rsync args: {:?}", rsync_args),
            "rsync",
        )
        .await;

        // Execute rsync for each source directory
        let source_dirs = job.source_dirs_vec();
        for source_dir in &source_dirs {
            if !Path::new(source_dir).exists() {
                self.log(
                    run_id,
                    LogLevel::Warning,
                    &format!("Source directory does not exist: {}", source_dir),
                    "rsync",
                )
                .await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest = format!("{}/{}/{}", job.mount_point, job.backup_subdir, dest_name);

            self.log(
                run_id,
                LogLevel::Info,
                &format!("Syncing {} -> {}", source_dir, dest),
                "rsync",
            )
            .await;

            // Create destination directory
            if let Err(e) = tokio::fs::create_dir_all(&dest).await {
                self.log(
                    run_id,
                    LogLevel::Warning,
                    &format!("Could not create destination: {}", e),
                    "rsync",
                )
                .await;
            }

            // Check for cancellation before starting
            if self.is_cancelled(run_id).await {
                self.log(run_id, LogLevel::Warning, "Job cancelled, skipping remaining sources", "system").await;
                break;
            }

            // Run rsync
            let mut cmd = Command::new("rsync");
            cmd.args(&rsync_args)
                .arg(format!("{}/", source_dir))
                .arg(format!("{}/", dest))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    self.log(
                        run_id,
                        LogLevel::Error,
                        &format!("Failed to spawn rsync: {}", e),
                        "rsync",
                    )
                    .await;
                    result.error_count += 1;
                    continue;
                }
            };

            // Take stdout/stderr before registering
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            // Register the child process for cancellation tracking
            {
                let mut processes = self.running_processes.lock().await;
                processes.insert(run_id, RunningProcess {
                    child,
                    cancelled: false,
                });
            }

            // Stream stdout
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    // Check for cancellation periodically
                    if self.is_cancelled(run_id).await {
                        break;
                    }

                    // Parse rsync output for stats
                    if line.contains("Number of files transferred") {
                        if let Some(num) = line.split(':').nth(1) {
                            if let Ok(n) = num.trim().replace(',', "").parse::<i64>() {
                                result.files_transferred += n;
                            }
                        }
                    } else if line.contains("Total transferred file size") {
                        if let Some(size_str) = line.split(':').nth(1) {
                            if let Some(bytes) = size_str.split_whitespace().next() {
                                if let Ok(b) = bytes.replace(',', "").parse::<i64>() {
                                    result.bytes_transferred += b;
                                }
                            }
                        }
                    }

                    self.log(run_id, LogLevel::Info, &line, "rsync").await;
                }
            }

            // Stream stderr (only if not cancelled)
            if !self.is_cancelled(run_id).await {
                if let Some(stderr) = stderr {
                    let reader = BufReader::new(stderr);
                    let mut lines = reader.lines();

                    while let Ok(Some(line)) = lines.next_line().await {
                        self.log(run_id, LogLevel::Warning, &line, "rsync").await;
                    }
                }
            }

            // Wait for process to finish and remove from tracking
            let status = {
                let mut processes = self.running_processes.lock().await;
                if let Some(mut running) = processes.remove(&run_id) {
                    running.child.wait().await.ok()
                } else {
                    None
                }
            };

            if let Some(status) = status {
                if !status.success() {
                    // Check if it was killed by signal (cancelled)
                    let was_cancelled = self.is_cancelled(run_id).await;
                    if was_cancelled {
                        self.log(run_id, LogLevel::Info, "rsync terminated by cancellation", "rsync").await;
                    } else {
                        self.log(
                            run_id,
                            LogLevel::Error,
                            &format!("rsync failed with exit code: {:?}", status.code()),
                            "rsync",
                        )
                        .await;
                        result.exit_code = status.code().unwrap_or(1);
                        result.error_count += 1;
                    }
                }
            }

            // Break out of loop if cancelled
            if self.is_cancelled(run_id).await {
                break;
            }
        }

        // Unmount if needed
        if job.auto_unmount && job.usb_uuid.is_some() {
            self.log(
                run_id,
                LogLevel::Info,
                &format!("Unmounting {}", job.mount_point),
                "mount",
            )
            .await;

            // Sync first
            let _ = Command::new("sync").output().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            let unmount_result = Command::new("umount")
                .arg(&job.mount_point)
                .output()
                .await;

            match unmount_result {
                Ok(output) if output.status.success() => {
                    self.log(run_id, LogLevel::Info, "Unmount successful", "mount")
                        .await;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.log(
                        run_id,
                        LogLevel::Warning,
                        &format!("Unmount warning: {}", stderr),
                        "mount",
                    )
                    .await;
                }
                Err(e) => {
                    self.log(
                        run_id,
                        LogLevel::Warning,
                        &format!("Unmount error: {}", e),
                        "mount",
                    )
                    .await;
                }
            }
        }

        self.log(
            run_id,
            LogLevel::Info,
            &format!(
                "Backup complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "system",
        )
        .await;

        Ok(result)
    }

    fn build_rsync_args(&self, job: &Job, fstype: &str) -> Vec<String> {
        let mut args = vec![
            "-vh".to_string(),
            "--stats".to_string(),
            "--progress".to_string(),
        ];

        // Filesystem-aware options
        match fstype {
            "exfat" | "ntfs" | "vfat" | "msdos" | "ntfs3" => {
                // Non-POSIX filesystems - avoid permission errors
                args.extend(["-r", "-l", "-t", "-D"].iter().map(|s| s.to_string()));
            }
            _ => {
                // POSIX-compliant filesystems
                args.push("-a".to_string());
            }
        }

        // Configurable options
        if job.sync_deletes {
            args.push("--delete".to_string());
        }

        if job.checksum_mode {
            args.push("--checksum".to_string());
        }

        if job.compress {
            args.push("-z".to_string());
        }

        if job.dry_run {
            args.push("--dry-run".to_string());
        }

        if let Some(limit) = job.bandwidth_limit {
            args.push(format!("--bwlimit={}", limit));
        }

        // Excludes
        for exclude in job.excludes_vec() {
            args.push(format!("--exclude={}", exclude));
        }

        args
    }

    async fn detect_filesystem(&self, mount_point: &str) -> String {
        let output = Command::new("findmnt")
            .args(["-n", "-o", "FSTYPE", "--target", mount_point])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string()
            }
            _ => "unknown".to_string(),
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

        // Store in database
        let _ = sqlx::query(
            "INSERT INTO log_entries (job_run_id, level, message, source, timestamp) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(run_id)
        .bind(level.as_str())
        .bind(message)
        .bind(source)
        .bind(timestamp)
        .execute(&self.db)
        .await;
    }
}

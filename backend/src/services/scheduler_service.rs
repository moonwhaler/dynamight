use crate::models::JobRunStatus;
use crate::services::BackupService;
use chrono::Utc;
use cron::Schedule;
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;
use tokio::time::{interval, Duration};

pub struct SchedulerService {
    db: SqlitePool,
    backup_service: Arc<BackupService>,
}

impl SchedulerService {
    pub fn new(db: SqlitePool, backup_service: Arc<BackupService>) -> Self {
        Self { db, backup_service }
    }

    pub async fn start(&self) {
        tracing::info!("Starting scheduler service");

        // Update next_run_at for all schedules on startup
        if let Err(e) = self.update_all_next_runs().await {
            tracing::error!("Failed to update schedule times: {}", e);
        }

        let mut ticker = interval(Duration::from_secs(60));

        loop {
            ticker.tick().await;
            self.check_and_run_due_jobs().await;
        }
    }

    async fn update_all_next_runs(&self) -> anyhow::Result<()> {
        let schedules: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, cron_expression FROM schedules WHERE enabled = 1")
                .fetch_all(&self.db)
                .await?;

        for (id, cron_expr) in schedules {
            if let Ok(next) = calculate_next_run(&cron_expr) {
                sqlx::query("UPDATE schedules SET next_run_at = ? WHERE id = ?")
                    .bind(next)
                    .bind(id)
                    .execute(&self.db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn check_and_run_due_jobs(&self) {
        let now = Utc::now();

        // Get schedules that are due
        let schedules: Vec<ScheduleWithJob> = match sqlx::query_as(
            r#"
            SELECT
                s.id as schedule_id,
                s.job_id,
                s.cron_expression,
                j.name as job_name,
                j.enabled as job_enabled
            FROM schedules s
            JOIN jobs j ON s.job_id = j.id
            WHERE s.enabled = 1
              AND j.enabled = 1
              AND s.next_run_at <= ?
            "#,
        )
        .bind(now)
        .fetch_all(&self.db)
        .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to fetch due schedules: {}", e);
                return;
            }
        };

        for schedule in schedules {
            tracing::info!(
                "Running scheduled job: {} (schedule {})",
                schedule.job_name,
                schedule.schedule_id
            );

            // Check if job is already running
            if self.backup_service.is_job_running(schedule.job_id).await {
                tracing::warn!(
                    "Job {} is already running, skipping scheduled run",
                    schedule.job_name
                );
                continue;
            }

            // Create job run record
            let run_id = match self
                .create_job_run(schedule.job_id, Some(schedule.schedule_id))
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!("Failed to create job run: {}", e);
                    continue;
                }
            };

            // Get full job details
            let job = match sqlx::query_as::<_, crate::models::Job>(
                "SELECT * FROM jobs WHERE id = ?",
            )
            .bind(schedule.job_id)
            .fetch_one(&self.db)
            .await
            {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!("Failed to fetch job: {}", e);
                    continue;
                }
            };

            // Update schedule timestamps
            let _ = sqlx::query(
                "UPDATE schedules SET last_run_at = ?, next_run_at = ? WHERE id = ?",
            )
            .bind(now)
            .bind(calculate_next_run(&schedule.cron_expression).ok())
            .bind(schedule.schedule_id)
            .execute(&self.db)
            .await;

            // Spawn job execution
            let backup_service = self.backup_service.clone();
            let db = self.db.clone();

            tokio::spawn(async move {
                // Update status to running
                let _ = sqlx::query(
                    "UPDATE job_runs SET status = ?, started_at = ? WHERE id = ?",
                )
                .bind(JobRunStatus::Running.as_str())
                .bind(Utc::now())
                .bind(run_id)
                .execute(&db)
                .await;

                // Execute
                let result = backup_service
                    .execute_job(&job, run_id, Some(schedule.schedule_id))
                    .await;

                // Update completion status
                let (status, exit_code, files, bytes, errors) = match result {
                    Ok(r) => (
                        if r.error_count > 0 {
                            JobRunStatus::Failed
                        } else {
                            JobRunStatus::Completed
                        },
                        r.exit_code,
                        r.files_transferred,
                        r.bytes_transferred,
                        r.error_count,
                    ),
                    Err(e) => {
                        tracing::error!("Job execution failed: {}", e);
                        (JobRunStatus::Failed, 1, 0, 0, 1)
                    }
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
            });
        }
    }

    async fn create_job_run(&self, job_id: i64, schedule_id: Option<i64>) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO job_runs (job_id, schedule_id, status) VALUES (?, ?, ?)",
        )
        .bind(job_id)
        .bind(schedule_id)
        .bind(JobRunStatus::Pending.as_str())
        .execute(&self.db)
        .await?;

        Ok(result.last_insert_rowid())
    }
}

#[derive(sqlx::FromRow)]
struct ScheduleWithJob {
    schedule_id: i64,
    job_id: i64,
    cron_expression: String,
    job_name: String,
    #[allow(dead_code)]
    job_enabled: bool,
}

fn calculate_next_run(cron_expr: &str) -> anyhow::Result<chrono::DateTime<Utc>> {
    // Cron crate expects 6-field format (with seconds)
    // If 5-field format, prepend "0 " for seconds
    let expr = if cron_expr.split_whitespace().count() == 5 {
        format!("0 {}", cron_expr)
    } else {
        cron_expr.to_string()
    };

    let schedule = Schedule::from_str(&expr)?;
    schedule
        .upcoming(Utc)
        .next()
        .ok_or_else(|| anyhow::anyhow!("No next run time"))
}

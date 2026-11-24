use crate::models::JobRunStatus;
use crate::services::BackupService;
use chrono::Utc;
use chrono_tz::Tz;
use crate::models::parse_cron;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::{interval, Duration};

pub struct SchedulerService {
    db: SqlitePool,
    backup_service: Arc<BackupService>,
    timezone: Tz,
}

impl SchedulerService {
    pub fn new(db: SqlitePool, backup_service: Arc<BackupService>, timezone: Tz) -> Self {
        Self { db, backup_service, timezone }
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
            if let Ok(next) = calculate_next_run(&cron_expr, self.timezone) {
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
            // Advance the schedule first, so a skipped or failed start does not
            // make the same schedule fire again on every tick. Schedules of
            // disabled jobs are advanced too: otherwise their overdue time
            // would fire right after the job is enabled again.
            // last_run_at only changes when the job really starts
            let _ = sqlx::query(
                "UPDATE schedules SET last_run_at = CASE WHEN ? THEN ? ELSE last_run_at END, next_run_at = ? WHERE id = ?",
            )
            .bind(schedule.job_enabled)
            .bind(now)
            .bind(calculate_next_run(&schedule.cron_expression, self.timezone).ok())
            .bind(schedule.schedule_id)
            .execute(&self.db)
            .await;

            if !schedule.job_enabled {
                continue;
            }

            tracing::info!(
                "Running scheduled job: {} (schedule {})",
                schedule.job_name,
                schedule.schedule_id
            );

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

            // Atomically reserve the job (skips if a manual or other scheduled run is active)
            if !self.backup_service.try_reserve_job(schedule.job_id).await {
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
                    self.backup_service.release_job(schedule.job_id).await;
                    continue;
                }
            };

            // Spawn job execution
            let backup_service = self.backup_service.clone();
            let db = self.db.clone();
            let job_id = schedule.job_id;
            let schedule_id = schedule.schedule_id;

            tokio::spawn(async move {
                let _ = sqlx::query(
                    "UPDATE job_runs SET status = ?, started_at = ? WHERE id = ? AND status = 'pending'",
                )
                .bind(JobRunStatus::Running.as_str())
                .bind(Utc::now())
                .bind(run_id)
                .execute(&db)
                .await;

                let result = backup_service.execute_job(&job, run_id, Some(schedule_id)).await;
                backup_service.finalize_run(run_id, job_id, &job, result).await;
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
    job_enabled: bool,
}

fn calculate_next_run(cron_expr: &str, timezone: Tz) -> anyhow::Result<chrono::DateTime<Utc>> {
    // Translates standard 5-field cron (seconds field, day-of-week numbering)
    let schedule = parse_cron(cron_expr)?;
    schedule
        .upcoming(timezone)
        .next()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| anyhow::anyhow!("No next run time"))
}

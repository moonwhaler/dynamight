use crate::services::{BackupService, StartRunError};
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
        let mut tick: u64 = 0;

        loop {
            ticker.tick().await;
            self.check_and_run_due_jobs().await;
            // Overdue backups: every 10 minutes is plenty for limits of hours or days
            if tick.is_multiple_of(10) {
                self.backup_service.check_overdue(self.timezone).await;
            }
            tick += 1;
        }
    }

    /// Recalculate `next_run_at` on startup (the timezone may have changed).
    ///
    /// A schedule that came due while the server was down keeps its past
    /// `next_run_at`, so the first check runs it once (like anacron) instead
    /// of skipping that backup; the run then advances it as usual.
    async fn update_all_next_runs(&self) -> anyhow::Result<()> {
        let schedules: Vec<(i64, String, Option<chrono::DateTime<Utc>>)> =
            sqlx::query_as("SELECT id, cron_expression, next_run_at FROM schedules WHERE enabled = 1")
                .fetch_all(&self.db)
                .await?;

        let now = Utc::now();
        for (id, cron_expr, next_run_at) in schedules {
            if next_run_at.is_some_and(|t| t <= now) {
                tracing::info!("Schedule {} was missed while the server was down; running it now", id);
                continue;
            }
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

            // Skipped if a manual or other scheduled run is active
            match self.backup_service.start_run(job, Some(schedule.schedule_id)).await {
                Ok(_) => {}
                Err(StartRunError::AlreadyRunning) => tracing::warn!(
                    "Job {} is already running, skipping scheduled run",
                    schedule.job_name
                ),
                Err(StartRunError::ShuttingDown) => return,
                Err(StartRunError::Database(e)) => {
                    tracing::error!("Failed to create job run: {}", e)
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    async fn setup() -> (SqlitePool, SchedulerService) {
        let db = crate::test_helpers::test_db().await;
        let logs_db = crate::test_helpers::test_db().await;
        crate::db::run_logs_migrations(&logs_db).await.unwrap();
        let credentials = Arc::new(crate::services::CredentialService::new("test-secret", db.clone()));
        let (log_tx, _) = tokio::sync::broadcast::channel(100);
        let backup = Arc::new(BackupService::new(db.clone(), logs_db, log_tx, None, credentials));
        let scheduler = SchedulerService::new(db.clone(), backup, chrono_tz::Tz::UTC);
        (db, scheduler)
    }

    async fn add_job(db: &SqlitePool, name: &str, enabled: bool) -> i64 {
        sqlx::query(
            "INSERT INTO jobs (name, enabled, mount_point, source_dirs, destination_type, destination_config) \
             VALUES (?, ?, '', '[]', 'local', '{}')",
        )
        .bind(name)
        .bind(enabled)
        .execute(db)
        .await
        .unwrap()
        .last_insert_rowid()
    }

    async fn add_schedule(db: &SqlitePool, job_id: i64, next: Option<chrono::DateTime<Utc>>) -> i64 {
        sqlx::query("INSERT INTO schedules (job_id, enabled, cron_expression, next_run_at) VALUES (?, 1, '0 3 * * *', ?)")
            .bind(job_id)
            .bind(next)
            .execute(db)
            .await
            .unwrap()
            .last_insert_rowid()
    }

    async fn next_run(db: &SqlitePool, id: i64) -> Option<chrono::DateTime<Utc>> {
        sqlx::query_as::<_, (Option<chrono::DateTime<Utc>>,)>("SELECT next_run_at FROM schedules WHERE id = ?")
            .bind(id)
            .fetch_one(db)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn startup_keeps_missed_schedules_and_recalculates_others() {
        let (db, scheduler) = setup().await;
        let job = add_job(&db, "a", true).await;
        let missed_at = Utc::now() - ChronoDuration::hours(5);
        let missed = add_schedule(&db, job, Some(missed_at)).await;
        let unset = add_schedule(&db, job, None).await;
        let future = add_schedule(&db, job, Some(Utc::now() + ChronoDuration::days(30))).await;

        scheduler.update_all_next_runs().await.unwrap();

        assert_eq!(next_run(&db, missed).await.map(|t| t.timestamp()), Some(missed_at.timestamp()));
        let expected = calculate_next_run("0 3 * * *", chrono_tz::Tz::UTC).unwrap();
        assert_eq!(next_run(&db, unset).await, Some(expected));
        assert_eq!(next_run(&db, future).await, Some(expected));
    }

    #[tokio::test]
    async fn disabled_job_schedule_is_advanced_without_running() {
        let (db, scheduler) = setup().await;
        let job = add_job(&db, "off", false).await;
        let schedule = add_schedule(&db, job, Some(Utc::now() - ChronoDuration::hours(1))).await;

        scheduler.check_and_run_due_jobs().await;

        assert!(next_run(&db, schedule).await.unwrap() > Utc::now());
        let (runs,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM job_runs").fetch_one(&db).await.unwrap();
        assert_eq!(runs, 0);
        let (last,): (Option<String>,) = sqlx::query_as("SELECT last_run_at FROM schedules WHERE id = ?")
            .bind(schedule)
            .fetch_one(&db)
            .await
            .unwrap();
        assert!(last.is_none());
    }
}

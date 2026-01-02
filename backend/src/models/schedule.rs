use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Schedule {
    pub id: i64,
    pub job_id: i64,
    pub enabled: bool,

    pub cron_expression: String,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,

    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub enabled: Option<bool>,
    pub cron_expression: Option<String>,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,
}

impl CreateScheduleRequest {
    /// Convert simple schedule to cron expression
    pub fn to_cron_expression(&self) -> String {
        if let Some(cron) = &self.cron_expression {
            return cron.clone();
        }

        let time = self.time_of_day.as_deref().unwrap_or("00:00");
        let parts: Vec<&str> = time.split(':').collect();
        let hour = parts.first().unwrap_or(&"0");
        let minute = parts.get(1).unwrap_or(&"0");

        match self.schedule_type.as_deref() {
            Some("daily") => format!("{} {} * * *", minute, hour),
            Some("weekly") => {
                let dow = self.day_of_week.unwrap_or(0);
                format!("{} {} * * {}", minute, hour, dow)
            }
            Some("monthly") => {
                let dom = self.day_of_month.unwrap_or(1);
                format!("{} {} {} * *", minute, hour, dom)
            }
            _ => format!("{} {} * * *", minute, hour), // Default to daily
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateScheduleRequest {
    pub enabled: Option<bool>,
    pub cron_expression: Option<String>,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warning => "warning",
            LogLevel::Error => "error",
        }
    }
}

/// Lenient conversion from a stored level; unknown values become `Info`.
impl From<&str> for LogLevel {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" => LogLevel::Debug,
            "warning" | "warn" => LogLevel::Warning,
            "error" | "err" => LogLevel::Error,
            _ => LogLevel::Info,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct LogEntryRow {
    pub id: i64,
    pub job_run_id: i64,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: i64,
    pub job_run_id: i64,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
}

impl From<LogEntryRow> for LogEntry {
    fn from(row: LogEntryRow) -> Self {
        Self {
            id: row.id,
            job_run_id: row.job_run_id,
            timestamp: row.timestamp,
            level: LogLevel::from(row.level.as_str()),
            message: row.message,
            source: row.source,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub run_id: i64,
    pub level: LogLevel,
    pub message: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobRunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobRunStatus::Pending => "pending",
            JobRunStatus::Running => "running",
            JobRunStatus::Completed => "completed",
            JobRunStatus::Failed => "failed",
            JobRunStatus::Cancelled => "cancelled",
        }
    }
}

/// Lenient conversion from a stored status; unknown values become `Pending`.
impl From<&str> for JobRunStatus {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pending" => JobRunStatus::Pending,
            "running" => JobRunStatus::Running,
            "completed" => JobRunStatus::Completed,
            "failed" => JobRunStatus::Failed,
            "cancelled" => JobRunStatus::Cancelled,
            _ => JobRunStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct JobRunRow {
    pub id: i64,
    pub job_id: i64,
    pub schedule_id: Option<i64>,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub files_transferred: Option<i64>,
    pub bytes_transferred: Option<i64>,
    pub total_size: Option<i64>,
    pub error_count: i32,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobRun {
    pub id: i64,
    pub job_id: i64,
    pub schedule_id: Option<i64>,
    pub status: JobRunStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub files_transferred: Option<i64>,
    pub bytes_transferred: Option<i64>,
    pub total_size: Option<i64>,
    pub error_count: i32,
    /// Why a run ended the way it did (failure reason, cancel note), as text
    pub summary: Option<String>,
}

impl From<JobRunRow> for JobRun {
    fn from(row: JobRunRow) -> Self {
        Self {
            id: row.id,
            job_id: row.job_id,
            schedule_id: row.schedule_id,
            status: JobRunStatus::from(row.status.as_str()),
            started_at: row.started_at,
            completed_at: row.completed_at,
            exit_code: row.exit_code,
            files_transferred: row.files_transferred,
            bytes_transferred: row.bytes_transferred,
            total_size: row.total_size,
            error_count: row.error_count,
            summary: row.summary.filter(|s| !s.trim().is_empty()),
        }
    }
}

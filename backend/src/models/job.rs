use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Job {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,

    // Mount configuration
    pub usb_uuid: Option<String>,
    pub mount_point: String,
    pub auto_mount: bool,
    pub auto_unmount: bool,

    // Source and destination
    pub source_dirs: String, // JSON array
    pub backup_subdir: String,

    // Rsync options
    pub sync_deletes: bool,
    pub rsync_excludes: Option<String>, // JSON array
    pub checksum_mode: bool,
    pub compress: bool,
    pub dry_run: bool,
    pub bandwidth_limit: Option<i32>,
    pub verbosity: String, // "quiet", "normal", "verbose"

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Job {
    pub fn source_dirs_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.source_dirs).unwrap_or_default()
    }

    pub fn excludes_vec(&self) -> Vec<String> {
        self.rsync_excludes
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,

    pub usb_uuid: Option<String>,
    pub mount_point: String,
    pub auto_mount: Option<bool>,
    pub auto_unmount: Option<bool>,

    pub source_dirs: Vec<String>,
    pub backup_subdir: Option<String>,

    pub sync_deletes: Option<bool>,
    pub rsync_excludes: Option<Vec<String>>,
    pub checksum_mode: Option<bool>,
    pub compress: Option<bool>,
    pub dry_run: Option<bool>,
    pub bandwidth_limit: Option<i32>,
    pub verbosity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,

    pub usb_uuid: Option<String>,
    pub mount_point: Option<String>,
    pub auto_mount: Option<bool>,
    pub auto_unmount: Option<bool>,

    pub source_dirs: Option<Vec<String>>,
    pub backup_subdir: Option<String>,

    pub sync_deletes: Option<bool>,
    pub rsync_excludes: Option<Vec<String>>,
    pub checksum_mode: Option<bool>,
    pub compress: Option<bool>,
    pub dry_run: Option<bool>,
    pub bandwidth_limit: Option<i32>,
    pub verbosity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,

    pub usb_uuid: Option<String>,
    pub mount_point: String,
    pub auto_mount: bool,
    pub auto_unmount: bool,

    pub source_dirs: Vec<String>,
    pub backup_subdir: String,

    pub sync_deletes: bool,
    pub rsync_excludes: Vec<String>,
    pub checksum_mode: bool,
    pub compress: bool,
    pub dry_run: bool,
    pub bandwidth_limit: Option<i32>,
    pub verbosity: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Job> for JobResponse {
    fn from(job: Job) -> Self {
        // Call methods before moving fields out of job
        let source_dirs = job.source_dirs_vec();
        let rsync_excludes = job.excludes_vec();

        Self {
            id: job.id,
            name: job.name,
            description: job.description,
            enabled: job.enabled,
            usb_uuid: job.usb_uuid,
            mount_point: job.mount_point,
            auto_mount: job.auto_mount,
            auto_unmount: job.auto_unmount,
            source_dirs,
            backup_subdir: job.backup_subdir,
            sync_deletes: job.sync_deletes,
            rsync_excludes,
            checksum_mode: job.checksum_mode,
            compress: job.compress,
            dry_run: job.dry_run,
            bandwidth_limit: job.bandwidth_limit,
            verbosity: job.verbosity,
            created_at: job.created_at,
            updated_at: job.updated_at,
        }
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::{DestinationConfig, SyncOptions};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Job {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,

    // Legacy mount configuration (for backwards compatibility)
    pub usb_uuid: Option<String>,
    pub mount_point: String,
    pub auto_mount: bool,
    pub auto_unmount: bool,

    // Source and destination
    pub source_dirs: String, // JSON array
    pub backup_subdir: String,

    // Legacy rsync options (for backwards compatibility)
    pub sync_deletes: bool,
    pub rsync_excludes: Option<String>, // JSON array
    pub checksum_mode: bool,
    pub compress: bool,
    pub dry_run: bool,
    pub bandwidth_limit: Option<i32>,
    pub verbosity: String, // "quiet", "normal", "verbose"

    // New provider-based fields
    pub destination_type: Option<String>,
    pub destination_config: Option<String>, // JSON
    pub sync_options: Option<String>,       // JSON
    pub credential_id: Option<i64>,

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

    /// Get the destination configuration (unified format)
    pub fn get_destination_config(&self) -> DestinationConfig {
        // If new format is set, use it
        if let Some(config_json) = &self.destination_config {
            if let Ok(config) = serde_json::from_str(config_json) {
                return config;
            }
        }

        // Otherwise, construct from legacy fields
        DestinationConfig::Local {
            mount_point: self.mount_point.clone(),
            backup_subdir: self.backup_subdir.clone(),
            usb_uuid: self.usb_uuid.clone(),
            auto_mount: self.auto_mount,
            auto_unmount: self.auto_unmount,
        }
    }

    /// Get sync options (unified format)
    pub fn get_sync_options(&self) -> SyncOptions {
        // If new format is set, use it
        if let Some(options_json) = &self.sync_options {
            if let Ok(options) = serde_json::from_str(options_json) {
                return options;
            }
        }

        // Otherwise, construct from legacy fields
        SyncOptions::from_legacy(
            self.sync_deletes,
            self.excludes_vec(),
            self.checksum_mode,
            self.compress,
            self.dry_run,
            self.bandwidth_limit,
            self.verbosity.clone(),
        )
    }

    /// Get the destination type
    pub fn get_destination_type(&self) -> &str {
        self.destination_type.as_deref().unwrap_or("local")
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub name: String,
    pub description: Option<String>,
    pub enabled: Option<bool>,

    // Legacy fields (for backwards compatibility)
    pub usb_uuid: Option<String>,
    pub mount_point: Option<String>,
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

    // Provider-based fields
    #[serde(default)]
    pub destination_type: Option<String>,
    #[serde(default)]
    pub destination: Option<DestinationConfig>,
    #[serde(default)]
    pub sync_options: Option<SyncOptions>,
    #[serde(default)]
    pub credential_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateJobRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<bool>,

    // Legacy fields
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

    // Provider-based fields
    #[serde(default)]
    pub destination_type: Option<String>,
    #[serde(default)]
    pub destination: Option<DestinationConfig>,
    #[serde(default)]
    pub sync_options: Option<SyncOptions>,
    #[serde(default)]
    pub credential_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,

    // Legacy fields (always populated for backwards compatibility)
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

    // New provider-based fields
    pub destination_type: String,
    pub destination: DestinationConfig,
    pub sync_options: SyncOptions,
    pub credential_id: Option<i64>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<DateTime<Utc>>,
}

impl From<Job> for JobResponse {
    fn from(job: Job) -> Self {
        // Get unified configs
        let destination = job.get_destination_config();
        let sync_options = job.get_sync_options();
        let destination_type = job.get_destination_type().to_string();

        // Extract source_dirs before moving other fields
        let source_dirs = job.source_dirs_vec();

        // Extract legacy values from destination for backwards compatibility
        let (mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount) = match &destination {
            DestinationConfig::Local {
                mount_point,
                backup_subdir,
                usb_uuid,
                auto_mount,
                auto_unmount,
            } => (
                mount_point.clone(),
                backup_subdir.clone(),
                usb_uuid.clone(),
                *auto_mount,
                *auto_unmount,
            ),
            _ => (
                job.mount_point.clone(),
                job.backup_subdir.clone(),
                job.usb_uuid.clone(),
                job.auto_mount,
                job.auto_unmount,
            ),
        };

        Self {
            id: job.id,
            name: job.name,
            description: job.description,
            enabled: job.enabled,
            usb_uuid,
            mount_point,
            auto_mount,
            auto_unmount,
            source_dirs,
            backup_subdir,
            sync_deletes: sync_options.delete_extraneous,
            rsync_excludes: sync_options.exclude_patterns.clone(),
            checksum_mode: sync_options.checksum_mode(),
            compress: sync_options.compress(),
            dry_run: sync_options.dry_run,
            bandwidth_limit: sync_options.bandwidth_limit_kbps,
            verbosity: sync_options.verbosity.clone(),
            destination_type,
            destination,
            sync_options,
            credential_id: job.credential_id,
            created_at: job.created_at,
            updated_at: job.updated_at,
            last_run_status: None,
            last_run_at: None,
        }
    }
}

impl JobResponse {
    pub fn with_run_status(mut self, status: Option<String>, run_at: Option<DateTime<Utc>>) -> Self {
        self.last_run_status = status;
        self.last_run_at = run_at;
        self
    }
}

//! Destination configuration types for different sync providers

use serde::{Deserialize, Serialize};

/// Configuration for different sync destination types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DestinationConfig {
    /// Local or USB-mounted destination (uses rsync)
    Local {
        mount_point: String,
        backup_subdir: String,
        #[serde(default)]
        usb_uuid: Option<String>,
        #[serde(default = "default_true")]
        auto_mount: bool,
        #[serde(default = "default_true")]
        auto_unmount: bool,
    },

    /// Google Drive destination
    GoogleDrive {
        folder_id: String,
        #[serde(default)]
        shared_drive_id: Option<String>,
    },

    /// Microsoft OneDrive destination
    OneDrive {
        folder_path: String,
        #[serde(default)]
        drive_id: Option<String>,
    },

    /// S3 or S3-compatible storage (AWS, MinIO, Backblaze B2)
    S3 {
        bucket: String,
        #[serde(default)]
        prefix: String,
        region: String,
        #[serde(default)]
        endpoint: Option<String>,
        #[serde(default)]
        storage_class: Option<String>,
    },

    /// SFTP/SSH destination
    Sftp {
        host: String,
        #[serde(default = "default_ssh_port")]
        port: u16,
        username: String,
        remote_path: String,
        #[serde(default)]
        key_based_auth: bool,
        /// SSH host key fingerprint for MITM protection (TOFU model)
        /// Format: "SHA256:base64encodedfingerprint"
        #[serde(default)]
        host_key_fingerprint: Option<String>,
    },

    /// WebDAV destination (Nextcloud, ownCloud, etc.)
    WebDav {
        url: String,
        remote_path: String,
    },
}

fn default_true() -> bool {
    true
}

fn default_ssh_port() -> u16 {
    22
}

fn default_space_check() -> String {
    "warn".to_string()
}

/// Archive format for directory compression
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompressFormat {
    #[default]
    TarGz,
    Zip,
}

/// Options for per-directory compression before transfer.
/// When enabled, each source directory is compressed into a single archive
/// and stored locally at `staging_path/<job_id>/` before being transferred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressDirsOptions {
    /// Master toggle
    pub enabled: bool,
    /// Archive format (default: TarGz)
    #[serde(default)]
    pub format: CompressFormat,
    /// Append timestamp (e.g. 2026-03-01T14-30-00) to archive name, enabling versioned archives.
    /// Without timestamp, archives are overwritten on each run.
    #[serde(default)]
    pub add_timestamp: bool,
    /// Optional custom name prefix, e.g. "myproject" →
    ///   "myproject_dirname_2026-03-01T14-30-00.tar.gz"
    /// Allowlist: [a-zA-Z0-9_-], max 64 chars
    #[serde(default)]
    pub custom_name: Option<String>,
    /// Max local archives to retain per source directory.
    /// Only meaningful when add_timestamp = true.
    /// None = unlimited. Must be >= 1 if set.
    #[serde(default)]
    pub max_archives_per_dir: Option<u32>,
    /// Root path where archives are staged before transfer.
    /// Actual archives go into: staging_path/<job_id>/
    pub staging_path: String,
    /// Optional password to protect the archive.
    /// Zip: uses built-in zip encryption (-P flag).
    /// TarGz: produces an AES-256-CBC encrypted `.enc` file via openssl.
    #[serde(default)]
    pub password: Option<String>,
    /// Store files without compression (archive only).
    /// TarGz → plain tar archive (`.tar`); Zip → store mode (`zip -0`).
    #[serde(default)]
    pub store_only: bool,
}

impl Default for DestinationConfig {
    fn default() -> Self {
        Self::Local {
            mount_point: "/mnt/backup".to_string(),
            backup_subdir: "backups".to_string(),
            usb_uuid: None,
            auto_mount: true,
            auto_unmount: true,
        }
    }
}

impl DestinationConfig {
    /// Get the destination type as a string
    pub fn destination_type(&self) -> &'static str {
        match self {
            Self::Local { .. } => "local",
            Self::GoogleDrive { .. } => "google_drive",
            Self::OneDrive { .. } => "onedrive",
            Self::S3 { .. } => "s3",
            Self::Sftp { .. } => "sftp",
            Self::WebDav { .. } => "webdav",
        }
    }

    /// Check if this destination requires credentials
    pub fn requires_credentials(&self) -> bool {
        !matches!(self, Self::Local { .. })
    }

    /// Create a Local destination from legacy job fields
    pub fn from_legacy(
        mount_point: String,
        backup_subdir: String,
        usb_uuid: Option<String>,
        auto_mount: bool,
        auto_unmount: bool,
    ) -> Self {
        Self::Local {
            mount_point,
            backup_subdir,
            usb_uuid,
            auto_mount,
            auto_unmount,
        }
    }
}

/// Unified sync options that work across all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOptions {
    /// Delete files at destination that don't exist in source (Mirror Mode)
    #[serde(default)]
    pub delete_extraneous: bool,

    /// Patterns to exclude from sync (e.g., "*.tmp", ".cache")
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Specific source directories to exclude from sync (absolute paths)
    /// These must be children of source_dirs
    #[serde(default)]
    pub exclude_dirs: Vec<String>,

    /// Bandwidth limit in KB/s (0 = unlimited)
    #[serde(default)]
    pub bandwidth_limit_kbps: Option<i32>,

    /// Simulate sync without making changes
    #[serde(default)]
    pub dry_run: bool,

    /// Output verbosity level
    #[serde(default)]
    pub verbosity: String,

    /// Provider-specific options as JSON
    #[serde(default)]
    pub provider_options: Option<serde_json::Value>,

    /// Space check behavior before sync: "fail", "warn", or "none"
    #[serde(default = "default_space_check")]
    pub space_check: String,

    /// Directory compression options.
    /// When set and enabled, each source directory is compressed before transfer.
    #[serde(default)]
    pub compress_dirs: Option<CompressDirsOptions>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            delete_extraneous: false,
            exclude_patterns: Vec::new(),
            exclude_dirs: Vec::new(),
            bandwidth_limit_kbps: None,
            dry_run: false,
            verbosity: "normal".to_string(),
            provider_options: None,
            space_check: "warn".to_string(),
            compress_dirs: None,
        }
    }
}

impl SyncOptions {
    /// Create SyncOptions from legacy rsync job fields
    pub fn from_legacy(
        sync_deletes: bool,
        rsync_excludes: Vec<String>,
        checksum_mode: bool,
        compress: bool,
        dry_run: bool,
        bandwidth_limit: Option<i32>,
        verbosity: String,
    ) -> Self {
        // Store rsync-specific options in provider_options
        let provider_options = serde_json::json!({
            "checksum_mode": checksum_mode,
            "compress": compress,
        });

        Self {
            delete_extraneous: sync_deletes,
            exclude_patterns: rsync_excludes,
            exclude_dirs: Vec::new(),
            bandwidth_limit_kbps: bandwidth_limit,
            dry_run,
            verbosity,
            provider_options: Some(provider_options),
            space_check: "warn".to_string(),
            compress_dirs: None,
        }
    }

    /// Get rsync-specific checksum mode option
    pub fn checksum_mode(&self) -> bool {
        self.provider_options
            .as_ref()
            .and_then(|o| o.get("checksum_mode"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get rsync-specific compress option
    pub fn compress(&self) -> bool {
        self.provider_options
            .as_ref()
            .and_then(|o| o.get("compress"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get rsync-specific ignore_times option (force sync)
    pub fn ignore_times(&self) -> bool {
        self.provider_options
            .as_ref()
            .and_then(|o| o.get("ignore_times"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Get space check mode, defaulting to "warn" if empty
    pub fn space_check_mode(&self) -> &str {
        if self.space_check.is_empty() {
            "warn"
        } else {
            &self.space_check
        }
    }

    /// Whether directory compression is enabled
    pub fn compress_dirs_enabled(&self) -> bool {
        self.compress_dirs.as_ref().map(|c| c.enabled).unwrap_or(false)
    }
}

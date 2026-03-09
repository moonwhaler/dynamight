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
/// and stored directly in `staging_path/` before being transferred.
/// The job ID is embedded in every archive filename (e.g. `photos_42.tar.gz`)
/// to prevent collisions when multiple jobs share the same staging directory.
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
    /// Archives are stored directly here; the job ID is part of the filename.
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- DestinationConfig round-trip serialization tests ---

    #[test]
    fn test_local_roundtrip() {
        let config = DestinationConfig::Local {
            mount_point: "/mnt/usb".to_string(),
            backup_subdir: "data".to_string(),
            usb_uuid: Some("abcd-1234".to_string()),
            auto_mount: true,
            auto_unmount: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DestinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.destination_type(), "local");
        if let DestinationConfig::Local { mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount } = &parsed {
            assert_eq!(mount_point, "/mnt/usb");
            assert_eq!(backup_subdir, "data");
            assert_eq!(usb_uuid.as_deref(), Some("abcd-1234"));
            assert!(*auto_mount);
            assert!(!*auto_unmount);
        } else {
            panic!("Expected Local variant");
        }
    }

    #[test]
    fn test_google_drive_roundtrip() {
        let config = DestinationConfig::GoogleDrive {
            folder_id: "abc123".to_string(),
            shared_drive_id: Some("shared-1".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DestinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.destination_type(), "google_drive");
        if let DestinationConfig::GoogleDrive { folder_id, shared_drive_id } = &parsed {
            assert_eq!(folder_id, "abc123");
            assert_eq!(shared_drive_id.as_deref(), Some("shared-1"));
        } else {
            panic!("Expected GoogleDrive variant");
        }
    }

    #[test]
    fn test_onedrive_roundtrip() {
        let config = DestinationConfig::OneDrive {
            folder_path: "/Documents/Backup".to_string(),
            drive_id: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DestinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.destination_type(), "onedrive");
        if let DestinationConfig::OneDrive { folder_path, drive_id } = &parsed {
            assert_eq!(folder_path, "/Documents/Backup");
            assert!(drive_id.is_none());
        } else {
            panic!("Expected OneDrive variant");
        }
    }

    #[test]
    fn test_s3_roundtrip() {
        let config = DestinationConfig::S3 {
            bucket: "my-bucket".to_string(),
            prefix: "backups/".to_string(),
            region: "us-east-1".to_string(),
            endpoint: Some("https://s3.custom.com".to_string()),
            storage_class: Some("GLACIER".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DestinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.destination_type(), "s3");
        if let DestinationConfig::S3 { bucket, prefix, region, endpoint, storage_class } = &parsed {
            assert_eq!(bucket, "my-bucket");
            assert_eq!(prefix, "backups/");
            assert_eq!(region, "us-east-1");
            assert_eq!(endpoint.as_deref(), Some("https://s3.custom.com"));
            assert_eq!(storage_class.as_deref(), Some("GLACIER"));
        } else {
            panic!("Expected S3 variant");
        }
    }

    #[test]
    fn test_sftp_roundtrip() {
        let config = DestinationConfig::Sftp {
            host: "backup.example.com".to_string(),
            port: 2222,
            username: "admin".to_string(),
            remote_path: "/srv/backup".to_string(),
            key_based_auth: true,
            host_key_fingerprint: Some("SHA256:abc123".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DestinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.destination_type(), "sftp");
        if let DestinationConfig::Sftp { host, port, username, remote_path, key_based_auth, host_key_fingerprint } = &parsed {
            assert_eq!(host, "backup.example.com");
            assert_eq!(*port, 2222);
            assert_eq!(username, "admin");
            assert_eq!(remote_path, "/srv/backup");
            assert!(*key_based_auth);
            assert_eq!(host_key_fingerprint.as_deref(), Some("SHA256:abc123"));
        } else {
            panic!("Expected Sftp variant");
        }
    }

    #[test]
    fn test_webdav_roundtrip() {
        let config = DestinationConfig::WebDav {
            url: "https://nextcloud.example.com/remote.php/dav".to_string(),
            remote_path: "/backups".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: DestinationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.destination_type(), "webdav");
        if let DestinationConfig::WebDav { url, remote_path } = &parsed {
            assert_eq!(url, "https://nextcloud.example.com/remote.php/dav");
            assert_eq!(remote_path, "/backups");
        } else {
            panic!("Expected WebDav variant");
        }
    }

    // --- destination_type tests ---

    #[test]
    fn test_destination_type_all_variants() {
        let cases: Vec<(DestinationConfig, &str)> = vec![
            (DestinationConfig::Local { mount_point: "".into(), backup_subdir: "".into(), usb_uuid: None, auto_mount: false, auto_unmount: false }, "local"),
            (DestinationConfig::GoogleDrive { folder_id: "".into(), shared_drive_id: None }, "google_drive"),
            (DestinationConfig::OneDrive { folder_path: "".into(), drive_id: None }, "onedrive"),
            (DestinationConfig::S3 { bucket: "".into(), prefix: "".into(), region: "".into(), endpoint: None, storage_class: None }, "s3"),
            (DestinationConfig::Sftp { host: "".into(), port: 22, username: "".into(), remote_path: "".into(), key_based_auth: false, host_key_fingerprint: None }, "sftp"),
            (DestinationConfig::WebDav { url: "".into(), remote_path: "".into() }, "webdav"),
        ];
        for (config, expected) in cases {
            assert_eq!(config.destination_type(), expected);
        }
    }

    // --- requires_credentials tests ---

    #[test]
    fn test_requires_credentials() {
        let local = DestinationConfig::default();
        assert!(!local.requires_credentials());

        let gdrive = DestinationConfig::GoogleDrive { folder_id: "x".into(), shared_drive_id: None };
        assert!(gdrive.requires_credentials());

        let onedrive = DestinationConfig::OneDrive { folder_path: "/".into(), drive_id: None };
        assert!(onedrive.requires_credentials());

        let s3 = DestinationConfig::S3 { bucket: "b".into(), prefix: "".into(), region: "r".into(), endpoint: None, storage_class: None };
        assert!(s3.requires_credentials());

        let sftp = DestinationConfig::Sftp { host: "h".into(), port: 22, username: "u".into(), remote_path: "/".into(), key_based_auth: false, host_key_fingerprint: None };
        assert!(sftp.requires_credentials());

        let webdav = DestinationConfig::WebDav { url: "http://x".into(), remote_path: "/".into() };
        assert!(webdav.requires_credentials());
    }

    // --- Default tests ---

    #[test]
    fn test_default_is_local() {
        let config = DestinationConfig::default();
        assert_eq!(config.destination_type(), "local");
        if let DestinationConfig::Local { mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount } = &config {
            assert_eq!(mount_point, "/mnt/backup");
            assert_eq!(backup_subdir, "backups");
            assert!(usb_uuid.is_none());
            assert!(*auto_mount);
            assert!(*auto_unmount);
        } else {
            panic!("Default should be Local");
        }
    }

    // --- from_legacy tests ---

    #[test]
    fn test_from_legacy() {
        let config = DestinationConfig::from_legacy(
            "/mnt/usb".to_string(),
            "mybackup".to_string(),
            Some("uuid-123".to_string()),
            false,
            true,
        );
        if let DestinationConfig::Local { mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount } = &config {
            assert_eq!(mount_point, "/mnt/usb");
            assert_eq!(backup_subdir, "mybackup");
            assert_eq!(usb_uuid.as_deref(), Some("uuid-123"));
            assert!(!*auto_mount);
            assert!(*auto_unmount);
        } else {
            panic!("from_legacy should create Local");
        }
    }

    // --- SyncOptions tests ---

    #[test]
    fn test_sync_options_from_legacy() {
        let opts = SyncOptions::from_legacy(
            true,
            vec!["*.tmp".to_string(), ".cache".to_string()],
            true,
            true,
            false,
            Some(1000),
            "verbose".to_string(),
        );
        assert!(opts.delete_extraneous);
        assert_eq!(opts.exclude_patterns, vec!["*.tmp", ".cache"]);
        assert!(opts.checksum_mode());
        assert!(opts.compress());
        assert!(!opts.dry_run);
        assert_eq!(opts.bandwidth_limit_kbps, Some(1000));
        assert_eq!(opts.verbosity, "verbose");
        assert_eq!(opts.space_check_mode(), "warn");
    }

    #[test]
    fn test_sync_options_checksum_and_compress_defaults() {
        let opts = SyncOptions::default();
        assert!(!opts.checksum_mode());
        assert!(!opts.compress());
        assert!(!opts.ignore_times());
        assert!(!opts.delete_extraneous);
        assert!(!opts.dry_run);
        assert!(!opts.compress_dirs_enabled());
    }

    #[test]
    fn test_sync_options_space_check_mode_default_when_empty() {
        let mut opts = SyncOptions::default();
        opts.space_check = "".to_string();
        assert_eq!(opts.space_check_mode(), "warn");
    }

    #[test]
    fn test_sync_options_space_check_mode_explicit() {
        let mut opts = SyncOptions::default();
        opts.space_check = "fail".to_string();
        assert_eq!(opts.space_check_mode(), "fail");
    }

    #[test]
    fn test_sync_options_compress_dirs_enabled() {
        let mut opts = SyncOptions::default();
        assert!(!opts.compress_dirs_enabled());

        opts.compress_dirs = Some(CompressDirsOptions {
            enabled: false,
            format: CompressFormat::TarGz,
            add_timestamp: false,
            custom_name: None,
            max_archives_per_dir: None,
            staging_path: "/tmp/staging".to_string(),
            password: None,
            store_only: false,
        });
        assert!(!opts.compress_dirs_enabled());

        opts.compress_dirs.as_mut().unwrap().enabled = true;
        assert!(opts.compress_dirs_enabled());
    }

    #[test]
    fn test_sync_options_roundtrip() {
        let opts = SyncOptions::from_legacy(
            true,
            vec!["*.log".to_string()],
            false,
            true,
            true,
            None,
            "quiet".to_string(),
        );
        let json = serde_json::to_string(&opts).unwrap();
        let parsed: SyncOptions = serde_json::from_str(&json).unwrap();
        assert!(parsed.delete_extraneous);
        assert_eq!(parsed.exclude_patterns, vec!["*.log"]);
        assert!(!parsed.checksum_mode());
        assert!(parsed.compress());
        assert!(parsed.dry_run);
        assert!(parsed.bandwidth_limit_kbps.is_none());
        assert_eq!(parsed.verbosity, "quiet");
    }
}

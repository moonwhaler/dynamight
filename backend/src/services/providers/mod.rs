//! Sync provider abstraction layer
//!
//! This module provides a trait-based abstraction for different sync backends
//! (rsync, S3, SFTP, WebDAV, Google Drive, OneDrive).

mod googledrive;
mod onedrive;
mod rsync;
mod s3;
mod sftp;
mod webdav;

pub use googledrive::GoogleDriveProvider;
pub use onedrive::OneDriveProvider;
pub use rsync::{RsyncProvider, SpaceCheckResult, SourceSizeInfo};
pub use s3::S3Provider;
pub use sftp::SftpProvider;
pub use webdav::WebDavProvider;

use crate::models::{CredentialData, DestinationConfig, LogLevel, LogMessage, SyncOptions};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Result of a sync operation
#[derive(Debug, Default, Clone, Serialize)]
pub struct SyncResult {
    pub success: bool,
    pub files_transferred: i64,
    pub bytes_transferred: i64,
    pub files_deleted: i64,
    pub error_count: i32,
    pub error_message: Option<String>,
}

/// Progress update during sync (for future use with progress callbacks)
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SyncProgress {
    pub current_file: Option<String>,
    pub bytes_transferred: i64,
    pub total_bytes: Option<i64>,
    pub percentage: Option<f32>,
}

/// Capabilities that a provider supports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_delete: bool,
    pub supports_compression: bool,
    pub supports_checksum: bool,
    pub supports_bandwidth_limit: bool,
    pub supports_exclude_patterns: bool,
    pub supports_incremental: bool,
    pub supports_dry_run: bool,
    pub requires_credentials: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            supports_delete: true,
            supports_compression: false,
            supports_checksum: false,
            supports_bandwidth_limit: false,
            supports_exclude_patterns: true,
            supports_incremental: true,
            supports_dry_run: true,
            requires_credentials: false,
        }
    }
}

/// Callback type for checking if a run has been cancelled
pub type CancellationChecker = Arc<dyn Fn(i64) -> bool + Send + Sync>;

/// Context passed to providers during sync execution
pub struct SyncContext {
    pub run_id: i64,
    pub source_dirs: Vec<String>,
    pub destination: DestinationConfig,
    pub options: SyncOptions,
    pub credential: Option<CredentialData>,
    pub log_sender: broadcast::Sender<LogMessage>,
    pub logs_db: SqlitePool,
    /// Callback to check if the run has been cancelled
    pub is_cancelled: CancellationChecker,
}

impl SyncContext {
    /// Log a message to the broadcast channel and persist to database
    pub async fn log(&self, level: LogLevel, message: &str, source: &str) {
        let timestamp = Utc::now();
        let log_msg = LogMessage {
            run_id: self.run_id,
            level,
            message: message.to_string(),
            source: source.to_string(),
            timestamp,
        };

        // Broadcast to WebSocket clients
        let _ = self.log_sender.send(log_msg);

        // Persist to logs database
        let _ = sqlx::query(
            "INSERT INTO log_entries (job_run_id, level, message, source, timestamp) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(self.run_id)
        .bind(level.as_str())
        .bind(message)
        .bind(source)
        .bind(timestamp)
        .execute(&self.logs_db)
        .await;
    }

    pub async fn log_info(&self, message: &str, source: &str) {
        self.log(LogLevel::Info, message, source).await;
    }

    pub async fn log_warning(&self, message: &str, source: &str) {
        self.log(LogLevel::Warning, message, source).await;
    }

    pub async fn log_error(&self, message: &str, source: &str) {
        self.log(LogLevel::Error, message, source).await;
    }

    #[allow(dead_code)]
    pub async fn log_debug(&self, message: &str, source: &str) {
        self.log(LogLevel::Debug, message, source).await;
    }

    /// Check if the current run has been cancelled
    pub fn check_cancelled(&self) -> bool {
        (self.is_cancelled)(self.run_id)
    }
}

/// Result of a connection test
#[derive(Debug, Clone, Serialize)]
pub struct TestConnectionResult {
    pub success: bool,
    pub message: String,
    /// Additional details (e.g., user info, bucket name, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// SSH host key fingerprint (for SFTP TOFU verification)
    /// Returned on first connection so the user can verify and save it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_key_fingerprint: Option<String>,
}

/// Trait that all sync providers must implement
#[async_trait]
pub trait SyncProvider: Send + Sync {
    /// Get the provider type identifier
    #[allow(dead_code)]
    fn provider_type(&self) -> &'static str;

    /// Get provider capabilities
    fn capabilities(&self) -> ProviderCapabilities;

    /// Validate the configuration before execution
    fn validate_config(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<(), ProviderError>;

    /// Execute the sync operation
    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError>;

    /// Test the connection to the destination
    /// Returns Ok with success message, or Err with error details
    async fn test_connection(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<TestConnectionResult, ProviderError> {
        // Default implementation just validates config
        self.validate_config(destination, credential)?;
        Ok(TestConnectionResult {
            success: true,
            message: "Configuration is valid".to_string(),
            details: None,
            host_key_fingerprint: None,
        })
    }

    /// Check if a run has been cancelled (providers should check this periodically)
    /// Deprecated: Use ctx.check_cancelled() instead
    #[allow(dead_code)]
    async fn is_cancelled(&self, _run_id: i64) -> bool {
        false
    }
}

/// Errors that can occur during provider operations
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Credential error: {0}")]
    CredentialError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Transfer error: {0}")]
    #[allow(dead_code)]
    TransferError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Host key mismatch: expected {expected}, got {actual}. This could indicate a Man-in-the-Middle attack!")]
    HostKeyMismatch {
        expected: String,
        actual: String,
    },

    #[error("Provider error: {0}")]
    #[allow(dead_code)]
    Other(String),
}

/// Factory function to create a provider based on destination type
pub fn create_provider(destination: &DestinationConfig) -> Box<dyn SyncProvider> {
    match destination {
        DestinationConfig::Local { .. } => Box::new(RsyncProvider::new()),
        DestinationConfig::S3 { .. } => Box::new(S3Provider::new()),
        DestinationConfig::Sftp { .. } => Box::new(SftpProvider::new()),
        DestinationConfig::WebDav { .. } => Box::new(WebDavProvider::new()),
        DestinationConfig::GoogleDrive { .. } => Box::new(GoogleDriveProvider::new()),
        DestinationConfig::OneDrive { .. } => Box::new(OneDriveProvider::new()),
    }
}

/// Get capabilities for a provider type without needing a full config
pub fn get_capabilities(provider_type: &str) -> Option<ProviderCapabilities> {
    match provider_type {
        "local" | "rsync" => Some(RsyncProvider::new().capabilities()),
        "s3" => Some(S3Provider::new().capabilities()),
        "sftp" => Some(SftpProvider::new().capabilities()),
        "webdav" => Some(WebDavProvider::new().capabilities()),
        "google_drive" => Some(GoogleDriveProvider::new().capabilities()),
        "onedrive" => Some(OneDriveProvider::new().capabilities()),
        _ => None,
    }
}

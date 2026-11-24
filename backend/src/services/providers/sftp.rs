//! SFTP sync provider for remote SSH servers

use super::common::{self, needs_upload, LocalEntry, RemoteFile};
use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use russh_keys::PublicKeyBase64;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub struct SftpProvider;

impl SftpProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SftpProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncProvider for SftpProvider {
    fn provider_type(&self) -> &'static str {
        "sftp"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_delete: true,
            supports_compression: false,
            supports_checksum: false,
            supports_bandwidth_limit: false,
            supports_exclude_patterns: true,
            supports_incremental: true,
            supports_dry_run: true,
            requires_credentials: true,
        }
    }

    fn validate_config(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<(), ProviderError> {
        match destination {
            DestinationConfig::Sftp { host, username, remote_path, .. } => {
                if host.is_empty() {
                    return Err(ProviderError::ConfigError("Host is required".to_string()));
                }
                if username.is_empty() {
                    return Err(ProviderError::ConfigError("Username is required".to_string()));
                }
                if remote_path.is_empty() {
                    return Err(ProviderError::ConfigError("Remote path is required".to_string()));
                }
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "SftpProvider only supports SFTP destination".to_string(),
                ))
            }
        }

        // Validate credentials
        match credential {
            Some(CredentialData::Sftp { password, private_key, .. }) => {
                if password.is_none() && private_key.is_none() {
                    return Err(ProviderError::CredentialError(
                        "Either password or private key is required for SFTP".to_string(),
                    ));
                }
            }
            None => {
                return Err(ProviderError::CredentialError(
                    "SFTP credentials are required".to_string(),
                ))
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "Invalid credential type for SFTP".to_string(),
                ))
            }
        }

        Ok(())
    }

    async fn test_connection(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<TestConnectionResult, ProviderError> {
        self.validate_config(destination, credential)?;

        let (host, port, username, remote_path, expected_fingerprint) = match destination {
            DestinationConfig::Sftp { host, port, username, remote_path, host_key_fingerprint, .. } => {
                (host.clone(), *port, username.clone(), remote_path.clone(), host_key_fingerprint.clone())
            }
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };

        let (password, private_key, passphrase) = match credential {
            Some(CredentialData::Sftp { password, private_key, passphrase }) => {
                (password.clone(), private_key.clone(), passphrase.clone())
            }
            _ => return Err(ProviderError::CredentialError("SFTP credentials required".to_string())),
        };

        // Connect via SSH with host key verification
        let config = Arc::new(russh::client::Config::default());
        let handler = SftpClientHandler::new(expected_fingerprint.clone());

        let mut session = russh::client::connect(config, (host.as_str(), port), handler)
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("SSH connection failed: {}", e)))?;

        // Check if the connection was rejected due to host key mismatch
        // Note: russh returns an error when check_server_key returns false
        // We need to extract the handler to check the verification result
        // Since we can't access the handler after connect(), we check if authentication fails
        // with a specific pattern that indicates host key rejection

        // Authenticate
        let auth_result = if let Some(key) = private_key {
            match russh_keys::decode_secret_key(&key, passphrase.as_deref()) {
                Ok(key_pair) => session.authenticate_publickey(&username, Arc::new(key_pair)).await,
                Err(e) => return Err(ProviderError::CredentialError(format!("Invalid private key: {}", e))),
            }
        } else if let Some(pw) = password {
            session.authenticate_password(&username, &pw).await
        } else {
            return Err(ProviderError::CredentialError("No authentication method".to_string()));
        };

        match auth_result {
            Ok(true) => {}
            Ok(false) => return Err(ProviderError::CredentialError("Authentication failed. Check your credentials.".to_string())),
            Err(e) => return Err(ProviderError::CredentialError(format!("Authentication error: {}", e))),
        }

        // Open SFTP channel to verify full connectivity
        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Channel open failed: {}", e)))?;

        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("SFTP subsystem failed: {}", e)))?;

        let sftp = russh_sftp::client::SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("SFTP session failed: {}", e)))?;

        // Try to stat the remote path to verify it exists or is writable
        let path_info = match sftp.metadata(&remote_path).await {
            Ok(attrs) => {
                if attrs.is_dir() {
                    format!("Remote path '{}' exists and is accessible", remote_path)
                } else {
                    format!("Remote path '{}' exists (not a directory)", remote_path)
                }
            }
            Err(_) => format!("Remote path '{}' will be created on first sync", remote_path),
        };

        // To get the fingerprint, we need to do a separate connection with a handler we can query
        // This is a limitation of russh's ownership model
        let fingerprint = self.get_host_fingerprint(&host, port).await.ok();

        // Determine the appropriate message based on whether this is first connection
        let (message, fingerprint_to_return) = if expected_fingerprint.is_some() {
            ("Successfully connected to SFTP server (host key verified)".to_string(), None)
        } else if let Some(ref fp) = fingerprint {
            (
                format!("Successfully connected to SFTP server. Please verify and save the host key fingerprint: {}", fp),
                Some(fp.clone())
            )
        } else {
            ("Successfully connected to SFTP server".to_string(), None)
        };

        Ok(TestConnectionResult {
            success: true,
            message,
            details: Some(format!("{}@{}:{}. {}", username, host, port, path_info)),
            host_key_fingerprint: fingerprint_to_return,
        })
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (host, port, username, remote_path, expected_fingerprint) = match &ctx.destination {
            DestinationConfig::Sftp {
                host,
                port,
                username,
                remote_path,
                host_key_fingerprint,
                ..
            } => (host.clone(), *port, username.clone(), remote_path.clone(), host_key_fingerprint.clone()),
            _ => {
                return Err(ProviderError::ConfigError(
                    "SftpProvider only supports SFTP destination".to_string(),
                ))
            }
        };

        let (password, private_key, passphrase) = match &ctx.credential {
            Some(CredentialData::Sftp { password, private_key, passphrase }) => {
                (password.clone(), private_key.clone(), passphrase.clone())
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "SFTP credentials required".to_string(),
                ))
            }
        };

        ctx.log_info("Starting SFTP sync", "sftp").await;
        ctx.log_info(&format!("Connecting to {}@{}:{}", username, host, port), "sftp").await;

        // Verify host key before proceeding with sync
        if expected_fingerprint.is_none() {
            ctx.log_warning(
                "No host key fingerprint configured. Please test connection first to verify the server identity.",
                "sftp"
            ).await;
            return Err(ProviderError::ConfigError(
                "Host key fingerprint not configured. Please test connection and save the destination to store the host key fingerprint.".to_string()
            ));
        }

        // Log that we're verifying the host key
        ctx.log_info("Verifying SSH host key...", "sftp").await;

        // Use russh for SSH connection with host key verification
        let config = Arc::new(russh::client::Config::default());

        let handler = SftpClientHandler::new(expected_fingerprint.clone());
        let mut session = match russh::client::connect(config, (host.as_str(), port), handler).await {
            Ok(session) => session,
            Err(e) => {
                // Check if this is a host key verification failure
                let error_msg = e.to_string();
                if error_msg.contains("key") || error_msg.contains("disconnect") {
                    // Likely a host key mismatch - get the current fingerprint to report
                    if let Ok(current_fp) = self.get_host_fingerprint(&host, port).await {
                        if let Some(ref expected) = expected_fingerprint {
                            if !fingerprints_match(expected, &current_fp) {
                                ctx.log_error(
                                    &format!(
                                        "HOST KEY MISMATCH! Expected: {}, Got: {}. This could indicate a Man-in-the-Middle attack!",
                                        expected, current_fp
                                    ),
                                    "sftp"
                                ).await;
                                return Err(ProviderError::HostKeyMismatch {
                                    expected: expected.clone(),
                                    actual: current_fp,
                                });
                            }
                        }
                    }
                }
                ctx.log_error(&format!("Failed to connect: {}", e), "sftp").await;
                return Err(ProviderError::ConnectionError(format!("SSH connection failed: {}", e)));
            }
        };

        ctx.log_info("SSH host key verified successfully", "sftp").await;

        // Authenticate
        let auth_result = if let Some(key) = private_key {
            // Parse private key
            match russh_keys::decode_secret_key(&key, passphrase.as_deref()) {
                Ok(key_pair) => {
                    session.authenticate_publickey(&username, Arc::new(key_pair)).await
                }
                Err(e) => {
                    ctx.log_error(&format!("Failed to parse private key: {}", e), "sftp").await;
                    return Err(ProviderError::CredentialError(format!("Invalid private key: {}", e)));
                }
            }
        } else if let Some(pw) = password {
            session.authenticate_password(&username, &pw).await
        } else {
            return Err(ProviderError::CredentialError("No authentication method available".to_string()));
        };

        match auth_result {
            Ok(true) => {
                ctx.log_info("Authentication successful", "sftp").await;
            }
            Ok(false) => {
                ctx.log_error("Authentication failed", "sftp").await;
                return Err(ProviderError::CredentialError("Authentication failed".to_string()));
            }
            Err(e) => {
                ctx.log_error(&format!("Authentication error: {}", e), "sftp").await;
                return Err(ProviderError::CredentialError(format!("Authentication error: {}", e)));
            }
        }

        // Open SFTP channel
        let channel = match session.channel_open_session().await {
            Ok(ch) => ch,
            Err(e) => {
                ctx.log_error(&format!("Failed to open channel: {}", e), "sftp").await;
                return Err(ProviderError::ConnectionError(format!("Channel open failed: {}", e)));
            }
        };

        // Request SFTP subsystem
        if let Err(e) = channel.request_subsystem(true, "sftp").await {
            ctx.log_error(&format!("Failed to start SFTP subsystem: {}", e), "sftp").await;
            return Err(ProviderError::ConnectionError(format!("SFTP subsystem failed: {}", e)));
        }

        let sftp = match russh_sftp::client::SftpSession::new(channel.into_stream()).await {
            Ok(sftp) => sftp,
            Err(e) => {
                ctx.log_error(&format!("Failed to create SFTP session: {}", e), "sftp").await;
                return Err(ProviderError::ConnectionError(format!("SFTP session failed: {}", e)));
            }
        };

        ctx.log_info("SFTP session established", "sftp").await;

        // Make sure the base remote path exists
        if !ctx.options.dry_run {
            if let Err(e) = create_dir_all(&sftp, remote_path.trim_end_matches('/')).await {
                ctx.log_error(&format!("Cannot create remote path {}: {}", remote_path, e), "sftp").await;
                return Err(e);
            }
        }

        // Sync each source directory
        for source_dir in &ctx.source_dirs {
            if ctx.check_cancelled() {
                ctx.log_warning("Sync cancelled by user", "sftp").await;
                return Err(ProviderError::Cancelled);
            }
            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "sftp").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest_path = format!("{}/{}", remote_path.trim_end_matches('/'), dest_name);

            ctx.log_info(&format!("Syncing {} -> {}", source_dir, dest_path), "sftp").await;

            match self.sync_source(&sftp, source_dir, &dest_path, &ctx, &mut result).await {
                Ok(_) => {}
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "sftp").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "SFTP sync complete. Files: {}, Bytes: {}, Deleted: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.files_deleted, result.error_count
            ),
            "sftp",
        ).await;

        Ok(result)
    }
}

impl SftpProvider {
    /// Get the host key fingerprint for a server without full authentication
    async fn get_host_fingerprint(&self, host: &str, port: u16) -> Result<String, ProviderError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
        use sha2::{Sha256, Digest};

        // We need a custom minimal handler just to capture the fingerprint
        struct FingerprintCaptureHandler {
            fingerprint: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
        }

        #[async_trait]
        impl russh::client::Handler for FingerprintCaptureHandler {
            type Error = russh::Error;

            async fn check_server_key(
                &mut self,
                server_public_key: &russh_keys::key::PublicKey,
            ) -> Result<bool, Self::Error> {
                let key_bytes = server_public_key.public_key_bytes();
                let mut hasher = Sha256::new();
                hasher.update(&key_bytes);
                let hash = hasher.finalize();
                let fingerprint = format!("SHA256:{}", STANDARD_NO_PAD.encode(hash));
                *self.fingerprint.lock().await = Some(fingerprint);
                // Accept the key for fingerprint capture
                Ok(true)
            }
        }

        let fingerprint_holder = std::sync::Arc::new(tokio::sync::Mutex::new(None));
        let handler = FingerprintCaptureHandler {
            fingerprint: fingerprint_holder.clone(),
        };

        let config = Arc::new(russh::client::Config::default());

        // Connect just to get the fingerprint (connection will fail auth, but that's ok)
        let _ = russh::client::connect(config, (host, port), handler).await;

        // Get the captured fingerprint
        let result = fingerprint_holder.lock().await.clone();
        result.ok_or_else(|| ProviderError::ConnectionError("Failed to capture host key".to_string()))
    }

    /// Sync one source directory to `dest_path` incrementally.
    async fn sync_source(
        &self,
        sftp: &russh_sftp::client::SftpSession,
        source_dir: &str,
        dest_path: &str,
        ctx: &SyncContext,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        let dry_run = ctx.options.dry_run;
        let local = common::walk_source_async(source_dir, &ctx.options).await?;
        for msg in &local.skipped {
            ctx.log_warning(msg, "sftp").await;
        }

        let (remote_files, remote_dirs) = list_remote_tree(sftp, dest_path, ctx).await?;

        if !dry_run {
            create_dir_all(sftp, dest_path).await?;
        }

        for dir in local.dirs() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            if remote_dirs.contains(&dir.rel) {
                continue;
            }
            let path = format!("{}/{}", dest_path, dir.rel);
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would create directory: {}", path), "sftp").await;
                continue;
            }
            if let Err(e) = sftp.create_dir(path.clone()).await {
                if !matches!(sftp.try_exists(path.clone()).await, Ok(true)) {
                    ctx.log_error(&format!("Failed to create directory {}: {}", path, e), "sftp").await;
                    result.error_count += 1;
                }
            }
        }

        let mut skipped = 0u64;
        let mut uploads = Vec::new();
        for file in local.files() {
            if !needs_upload(file, remote_files.get(&file.rel)) {
                skipped += 1;
                continue;
            }
            let path = format!("{}/{}", dest_path, file.rel);
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would upload: {}", path), "sftp").await;
                result.files_transferred += 1;
                result.bytes_transferred += file.size as i64;
                continue;
            }
            uploads.push((file.clone(), path));
        }
        // Several files at once over the one session (SFTP multiplexes requests)
        common::upload_parallel(&ctx.cancel, uploads, common::PARALLEL_UPLOADS, result, |(file, path)| async move {
            match upload_file(sftp, &file, &path).await {
                Ok(()) => {
                    ctx.log_info(&format!("Uploaded: {}", path), "sftp").await;
                    Ok(file.size)
                }
                Err(e) => {
                    ctx.log_error(&format!("Failed to upload {}: {}", path, e), "sftp").await;
                    Err(e)
                }
            }
        })
        .await?;
        if skipped > 0 {
            ctx.log_info(&format!("{} unchanged file(s) skipped", skipped), "sftp").await;
        }

        if ctx.options.delete_extraneous {
            let local_paths = local.rel_paths();
            let excludes = common::ExcludeMatcher::new(&ctx.options);
            let is_stale = |rel: &String| !local_paths.contains(rel) && !excludes.rel_excluded(source_dir, rel);

            let mut files: Vec<&String> = remote_files.keys().filter(|r| is_stale(r)).collect();
            files.sort();
            // Deepest directories first, so they are empty when removed
            let mut dirs: Vec<&String> = remote_dirs.iter().filter(|r| is_stale(r)).collect();
            dirs.sort_by(|a, b| b.matches('/').count().cmp(&a.matches('/').count()).then(b.cmp(a)));

            for rel in files {
                let path = format!("{}/{}", dest_path, rel);
                if dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would delete: {}", path), "sftp").await;
                    result.files_deleted += 1;
                    continue;
                }
                match sftp.remove_file(path.clone()).await {
                    Ok(()) => {
                        ctx.log_info(&format!("Deleted: {}", path), "sftp").await;
                        result.files_deleted += 1;
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to delete {}: {}", path, e), "sftp").await;
                        result.error_count += 1;
                    }
                }
            }
            for rel in dirs {
                let path = format!("{}/{}", dest_path, rel);
                if dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would delete directory: {}", path), "sftp").await;
                    continue;
                }
                if let Err(e) = sftp.remove_dir(path.clone()).await {
                    ctx.log_warning(&format!("Could not remove directory {}: {}", path, e), "sftp").await;
                }
            }
        }

        Ok(())
    }
}

/// `mkdir -p` over SFTP.
async fn create_dir_all(sftp: &russh_sftp::client::SftpSession, path: &str) -> Result<(), ProviderError> {
    let absolute = path.starts_with('/');
    let mut current = String::new();
    for segment in path.split('/').filter(|s| !s.is_empty()) {
        if current.is_empty() && !absolute {
            current = segment.to_string();
        } else {
            current = format!("{}/{}", current, segment);
        }
        if matches!(sftp.try_exists(current.clone()).await, Ok(true)) {
            continue;
        }
        sftp.create_dir(current.clone())
            .await
            .map_err(|e| ProviderError::TransferError(format!("Cannot create directory {}: {}", current, e)))?;
    }
    Ok(())
}

/// List files and directories below `root` (relative paths). A missing root
/// gives empty results.
async fn list_remote_tree(
    sftp: &russh_sftp::client::SftpSession,
    root: &str,
    ctx: &SyncContext,
) -> Result<(HashMap<String, RemoteFile>, HashSet<String>), ProviderError> {
    let mut files = HashMap::new();
    let mut dirs = HashSet::new();
    if !matches!(sftp.try_exists(root.to_string()).await, Ok(true)) {
        return Ok((files, dirs));
    }

    common::walk_remote(
        &ctx.cancel,
        String::new(),
        common::PARALLEL_LISTINGS,
        |prefix: String| async move {
            let dir_path = if prefix.is_empty() { root.to_string() } else { format!("{}/{}", root, prefix) };
            let entries = sftp
                .read_dir(dir_path.clone())
                .await
                .map_err(|e| ProviderError::ConnectionError(format!("Cannot list {}: {}", dir_path, e)))?;
            Ok((prefix, entries))
        },
        |prefix, entries| {
            let mut subdirs = Vec::new();
            for entry in entries {
                let name = entry.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                let rel = if prefix.is_empty() { name } else { format!("{}/{}", prefix, name) };
                let ft = entry.file_type();
                if ft.is_dir() {
                    dirs.insert(rel.clone());
                    subdirs.push(rel);
                } else if ft.is_file() {
                    let meta = entry.metadata();
                    files.insert(
                        rel,
                        RemoteFile {
                            size: meta.size.unwrap_or(0),
                            mtime: meta.mtime.map(i64::from).unwrap_or(0),
                            id: None,
                        },
                    );
                }
            }
            subdirs
        },
    )
    .await?;
    Ok((files, dirs))
}

/// Stream a local file to the server and copy its modification time, so the
/// next run can skip it.
async fn upload_file(
    sftp: &russh_sftp::client::SftpSession,
    file: &LocalEntry,
    remote: &str,
) -> Result<(), ProviderError> {
    use tokio::io::AsyncWriteExt;

    let mut local = tokio::fs::File::open(&file.abs).await?;
    let mut remote_file = sftp
        .create(remote.to_string())
        .await
        .map_err(|e| ProviderError::TransferError(format!("Cannot create {}: {}", remote, e)))?;
    tokio::io::copy(&mut local, &mut remote_file)
        .await
        .map_err(|e| ProviderError::TransferError(format!("Write failed: {}", e)))?;
    remote_file
        .shutdown()
        .await
        .map_err(|e| ProviderError::TransferError(format!("Close failed: {}", e)))?;
    drop(remote_file);

    let mut attrs = russh_sftp::protocol::FileAttributes::empty();
    attrs.mtime = Some(file.mtime.clamp(0, u32::MAX as i64) as u32);
    attrs.atime = attrs.mtime;
    // Not every server allows setting times; the file is uploaded either way
    let _ = sftp.set_metadata(remote.to_string(), attrs).await;
    Ok(())
}

/// Result of host key verification
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum HostKeyVerification {
    /// Key matches expected fingerprint
    Verified,
    /// First connection (TOFU) - key was accepted and stored
    FirstConnection(String),
    /// Key mismatch - potential MITM attack
    Mismatch { expected: String, actual: String },
}

/// SSH client handler for russh with host key verification
struct SftpClientHandler {
    /// Expected host key fingerprint (None for first connection / TOFU)
    expected_fingerprint: Option<String>,
    /// Captured fingerprint from server (populated during check_server_key)
    captured_fingerprint: std::sync::Arc<tokio::sync::Mutex<Option<String>>>,
    /// Verification result
    verification_result: std::sync::Arc<tokio::sync::Mutex<Option<HostKeyVerification>>>,
}

impl SftpClientHandler {
    fn new(expected_fingerprint: Option<String>) -> Self {
        Self {
            expected_fingerprint,
            captured_fingerprint: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
            verification_result: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Get the captured fingerprint after connection
    #[allow(dead_code)]
    async fn get_captured_fingerprint(&self) -> Option<String> {
        self.captured_fingerprint.lock().await.clone()
    }

    /// Get the verification result after connection
    #[allow(dead_code)]
    async fn get_verification_result(&self) -> Option<HostKeyVerification> {
        self.verification_result.lock().await.clone()
    }
}

#[async_trait]
impl russh::client::Handler for SftpClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // Compute SHA256 fingerprint of the server's public key
        use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
        use sha2::{Sha256, Digest};

        let key_bytes = server_public_key.public_key_bytes();
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes);
        let hash = hasher.finalize();
        let fingerprint = format!("SHA256:{}", STANDARD_NO_PAD.encode(hash));

        // Store the captured fingerprint
        *self.captured_fingerprint.lock().await = Some(fingerprint.clone());

        // Verify against expected fingerprint
        if let Some(ref expected) = self.expected_fingerprint {
            if fingerprints_match(expected, &fingerprint) {
                // Key matches - connection is verified
                tracing::info!("SFTP host key verified: {}", fingerprint);
                *self.verification_result.lock().await = Some(HostKeyVerification::Verified);
                Ok(true)
            } else {
                // CRITICAL: Key mismatch - potential MITM attack!
                tracing::error!(
                    "SFTP HOST KEY MISMATCH! Expected: {}, Got: {}. Possible Man-in-the-Middle attack!",
                    expected,
                    fingerprint
                );
                *self.verification_result.lock().await = Some(HostKeyVerification::Mismatch {
                    expected: expected.clone(),
                    actual: fingerprint,
                });
                // Reject the connection
                Ok(false)
            }
        } else {
            // No expected fingerprint - first connection (TOFU mode)
            // Accept the key but flag it for the user to verify
            tracing::info!("SFTP first connection (TOFU). Host key fingerprint: {}", fingerprint);
            *self.verification_result.lock().await = Some(HostKeyVerification::FirstConnection(fingerprint));
            Ok(true)
        }
    }
}

/// Compare SSH host key fingerprints, ignoring base64 padding and surrounding
/// whitespace. OpenSSH prints `SHA256:` fingerprints without `=` padding;
/// older versions of this app stored them with padding.
fn fingerprints_match(a: &str, b: &str) -> bool {
    fn normalize(s: &str) -> &str {
        s.trim().trim_end_matches('=')
    }
    normalize(a) == normalize(b)
}

#[cfg(test)]
mod fingerprint_tests {
    use super::fingerprints_match;

    #[test]
    fn padded_and_unpadded_fingerprints_match() {
        assert!(fingerprints_match("SHA256:abc+def/12=", "SHA256:abc+def/12"));
        assert!(fingerprints_match(" SHA256:abc ", "SHA256:abc"));
        assert!(!fingerprints_match("SHA256:abc", "SHA256:abd"));
    }
}

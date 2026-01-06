//! SFTP sync provider for remote SSH servers

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use russh_keys::PublicKeyBase64;
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
            supports_compression: true,
            supports_checksum: false,
            supports_bandwidth_limit: true,
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
                            if expected != &current_fp {
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

        // Sync each source directory
        for source_dir in &ctx.source_dirs {
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

            // Create remote directory
            if !ctx.options.dry_run {
                let _ = sftp.create_dir(&dest_path).await;
            }

            // Sync directory contents
            match self.sync_directory(&sftp, source_dir, &dest_path, &ctx, &mut result).await {
                Ok(_) => {}
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "sftp").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "SFTP sync complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "sftp",
        ).await;

        Ok(result)
    }
}

impl SftpProvider {
    /// Get the host key fingerprint for a server without full authentication
    async fn get_host_fingerprint(&self, host: &str, port: u16) -> Result<String, ProviderError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
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
                let fingerprint = format!("SHA256:{}", STANDARD.encode(hash));
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

    async fn sync_directory(
        &self,
        sftp: &russh_sftp::client::SftpSession,
        source_dir: &str,
        remote_dir: &str,
        ctx: &SyncContext,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        use tokio::fs;

        let mut entries = fs::read_dir(source_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Check exclude patterns
            if ctx.options.exclude_patterns.iter().any(|p| {
                glob::Pattern::new(p).map(|pat| pat.matches(&file_name)).unwrap_or(false)
            }) {
                continue;
            }

            let remote_path = format!("{}/{}", remote_dir, file_name);

            if path.is_dir() {
                // Create remote directory and recurse
                if !ctx.options.dry_run {
                    let _ = sftp.create_dir(&remote_path).await;
                }
                let path_str = path.to_str().ok_or_else(|| {
                    ProviderError::TransferError(format!("Invalid UTF-8 in path: {}", path.display()))
                })?;
                Box::pin(self.sync_directory(sftp, path_str, &remote_path, ctx, result)).await?;
            } else if path.is_file() {
                if ctx.options.dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would upload: {}", remote_path), "sftp").await;
                    result.files_transferred += 1;
                    continue;
                }

                // Read and upload file
                match fs::read(&path).await {
                    Ok(data) => {
                        let file_size = data.len() as i64;

                        match sftp.create(&remote_path).await {
                            Ok(mut file) => {
                                use tokio::io::AsyncWriteExt;
                                match file.write_all(&data).await {
                                    Ok(_) => {
                                        ctx.log_info(&format!("Uploaded: {}", remote_path), "sftp").await;
                                        result.files_transferred += 1;
                                        result.bytes_transferred += file_size;
                                    }
                                    Err(e) => {
                                        ctx.log_error(&format!("Failed to write {}: {}", remote_path, e), "sftp").await;
                                        result.error_count += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                ctx.log_error(&format!("Failed to create {}: {}", remote_path, e), "sftp").await;
                                result.error_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to read {}: {}", path.display(), e), "sftp").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }
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
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use sha2::{Sha256, Digest};

        let key_bytes = server_public_key.public_key_bytes();
        let mut hasher = Sha256::new();
        hasher.update(&key_bytes);
        let hash = hasher.finalize();
        let fingerprint = format!("SHA256:{}", STANDARD.encode(hash));

        // Store the captured fingerprint
        *self.captured_fingerprint.lock().await = Some(fingerprint.clone());

        // Verify against expected fingerprint
        if let Some(ref expected) = self.expected_fingerprint {
            if expected == &fingerprint {
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

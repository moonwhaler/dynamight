//! SFTP sync provider for remote SSH servers

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
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

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (host, port, username, remote_path) = match &ctx.destination {
            DestinationConfig::Sftp {
                host,
                port,
                username,
                remote_path,
                ..
            } => (host.clone(), *port, username.clone(), remote_path.clone()),
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

        // Use russh for SSH connection
        let config = Arc::new(russh::client::Config::default());

        let sh = SftpClientHandler;
        let mut session = match russh::client::connect(config, (host.as_str(), port), sh).await {
            Ok(session) => session,
            Err(e) => {
                ctx.log_error(&format!("Failed to connect: {}", e), "sftp").await;
                return Err(ProviderError::ConnectionError(format!("SSH connection failed: {}", e)));
            }
        };

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
                Box::pin(self.sync_directory(sftp, path.to_str().unwrap(), &remote_path, ctx, result)).await?;
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

/// SSH client handler for russh
struct SftpClientHandler;

#[async_trait]
impl russh::client::Handler for SftpClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        // In production, you'd want to verify the host key
        // For now, accept all keys (like ssh -o StrictHostKeyChecking=no)
        Ok(true)
    }
}

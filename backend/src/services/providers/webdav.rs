//! WebDAV sync provider (Nextcloud, ownCloud, etc.)

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;

pub struct WebDavProvider {
    client: Client,
}

impl WebDavProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for WebDavProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncProvider for WebDavProvider {
    fn provider_type(&self) -> &'static str {
        "webdav"
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
            DestinationConfig::WebDav { url, .. } => {
                if url.is_empty() {
                    return Err(ProviderError::ConfigError("WebDAV URL is required".to_string()));
                }
                // Validate URL format
                if url::Url::parse(url).is_err() {
                    return Err(ProviderError::ConfigError("Invalid WebDAV URL".to_string()));
                }
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "WebDavProvider only supports WebDAV destination".to_string(),
                ))
            }
        }

        // Validate credentials
        match credential {
            Some(CredentialData::WebDav { username, password }) => {
                if username.is_empty() || password.is_empty() {
                    return Err(ProviderError::CredentialError(
                        "WebDAV username and password are required".to_string(),
                    ));
                }
            }
            None => {
                return Err(ProviderError::CredentialError(
                    "WebDAV credentials are required".to_string(),
                ))
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "Invalid credential type for WebDAV".to_string(),
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

        let (base_url, remote_path) = match destination {
            DestinationConfig::WebDav { url, remote_path } => (url.clone(), remote_path.clone()),
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };

        let (username, password) = match credential {
            Some(CredentialData::WebDav { username, password }) => (username.clone(), password.clone()),
            _ => return Err(ProviderError::CredentialError("WebDAV credentials required".to_string())),
        };

        // Test URL - use base URL or append remote path
        let test_url = if remote_path.is_empty() {
            base_url.trim_end_matches('/').to_string()
        } else {
            format!("{}/{}", base_url.trim_end_matches('/'), remote_path.trim_start_matches('/'))
        };

        // Use PROPFIND to test connection (standard WebDAV method)
        let response = self
            .client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &test_url)
            .basic_auth(&username, Some(&password))
            .header("Depth", "0")
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Failed to connect: {}", e)))?;

        match response.status().as_u16() {
            200..=299 => Ok(TestConnectionResult {
                success: true,
                message: "Successfully connected to WebDAV server".to_string(),
                details: Some(format!("URL: {}", test_url)),
            }),
            401 => Err(ProviderError::CredentialError(
                "Authentication failed. Check your username and password.".to_string(),
            )),
            403 => Err(ProviderError::CredentialError(
                "Access forbidden. Check your permissions.".to_string(),
            )),
            404 => Ok(TestConnectionResult {
                success: true,
                message: "Connected to WebDAV server".to_string(),
                details: Some(format!("Remote path '{}' will be created on first sync", remote_path)),
            }),
            status => Err(ProviderError::ConnectionError(format!(
                "Server returned status {}: {}",
                status,
                response.text().await.unwrap_or_default()
            ))),
        }
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (base_url, remote_path) = match &ctx.destination {
            DestinationConfig::WebDav { url, remote_path } => (url.clone(), remote_path.clone()),
            _ => {
                return Err(ProviderError::ConfigError(
                    "WebDavProvider only supports WebDAV destination".to_string(),
                ))
            }
        };

        let (username, password) = match &ctx.credential {
            Some(CredentialData::WebDav { username, password }) => {
                (username.clone(), password.clone())
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "WebDAV credentials required".to_string(),
                ))
            }
        };

        ctx.log_info("Starting WebDAV sync", "webdav").await;
        ctx.log_info(&format!("Server: {}", base_url), "webdav").await;

        // Test connection
        let test_url = format!("{}/{}", base_url.trim_end_matches('/'), remote_path.trim_start_matches('/'));
        let response = self.client
            .request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &test_url)
            .basic_auth(&username, Some(&password))
            .header("Depth", "0")
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() || resp.status() == 207 => {
                ctx.log_info("Connection successful", "webdav").await;
            }
            Ok(resp) if resp.status() == 404 => {
                ctx.log_info("Remote path doesn't exist, will create", "webdav").await;
            }
            Ok(resp) if resp.status() == 401 => {
                ctx.log_error("Authentication failed", "webdav").await;
                return Err(ProviderError::CredentialError("Authentication failed".to_string()));
            }
            Ok(resp) => {
                ctx.log_error(&format!("Connection failed: {}", resp.status()), "webdav").await;
                return Err(ProviderError::ConnectionError(format!("HTTP {}", resp.status())));
            }
            Err(e) => {
                ctx.log_error(&format!("Connection error: {}", e), "webdav").await;
                return Err(ProviderError::ConnectionError(e.to_string()));
            }
        }

        // Sync each source directory
        for source_dir in &ctx.source_dirs {
            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "webdav").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest_path = format!("{}/{}", remote_path.trim_end_matches('/'), dest_name);
            let dest_url = format!("{}/{}", base_url.trim_end_matches('/'), dest_path.trim_start_matches('/'));

            ctx.log_info(&format!("Syncing {} -> {}", source_dir, dest_url), "webdav").await;

            // Create remote directory (MKCOL)
            if !ctx.options.dry_run {
                let _ = self.client
                    .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &dest_url)
                    .basic_auth(&username, Some(&password))
                    .send()
                    .await;
            }

            // Sync directory contents
            match self.sync_directory(&base_url, &dest_path, source_dir, &username, &password, &ctx, &mut result).await {
                Ok(_) => {}
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "webdav").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "WebDAV sync complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "webdav",
        ).await;

        Ok(result)
    }
}

impl WebDavProvider {
    async fn sync_directory(
        &self,
        base_url: &str,
        remote_dir: &str,
        source_dir: &str,
        username: &str,
        password: &str,
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
            let remote_url = format!("{}/{}", base_url.trim_end_matches('/'), remote_path.trim_start_matches('/'));

            if path.is_dir() {
                // Create remote directory
                if !ctx.options.dry_run {
                    let _ = self.client
                        .request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &remote_url)
                        .basic_auth(username, Some(password))
                        .send()
                        .await;
                }
                Box::pin(self.sync_directory(base_url, &remote_path, path.to_str().unwrap(), username, password, ctx, result)).await?;
            } else if path.is_file() {
                if ctx.options.dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would upload: {}", remote_path), "webdav").await;
                    result.files_transferred += 1;
                    continue;
                }

                // Read and upload file
                match fs::read(&path).await {
                    Ok(data) => {
                        let file_size = data.len() as i64;

                        match self.client
                            .put(&remote_url)
                            .basic_auth(username, Some(password))
                            .body(data)
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() || resp.status() == 201 || resp.status() == 204 => {
                                ctx.log_info(&format!("Uploaded: {}", remote_path), "webdav").await;
                                result.files_transferred += 1;
                                result.bytes_transferred += file_size;
                            }
                            Ok(resp) => {
                                ctx.log_error(&format!("Failed to upload {}: HTTP {}", remote_path, resp.status()), "webdav").await;
                                result.error_count += 1;
                            }
                            Err(e) => {
                                ctx.log_error(&format!("Failed to upload {}: {}", remote_path, e), "webdav").await;
                                result.error_count += 1;
                            }
                        }
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to read {}: {}", path.display(), e), "webdav").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }
}

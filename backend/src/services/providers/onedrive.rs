//! OneDrive sync provider using Microsoft Graph API

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

const GRAPH_API_BASE: &str = "https://graph.microsoft.com/v1.0";

pub struct OneDriveProvider {
    client: Client,
}

impl OneDriveProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for OneDriveProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadSession {
    upload_url: String,
}

#[async_trait]
impl SyncProvider for OneDriveProvider {
    fn provider_type(&self) -> &'static str {
        "onedrive"
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
            DestinationConfig::OneDrive { folder_path, .. } => {
                if folder_path.is_empty() {
                    return Err(ProviderError::ConfigError(
                        "OneDrive folder path is required".to_string(),
                    ));
                }
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "OneDriveProvider only supports OneDrive destination".to_string(),
                ))
            }
        }

        // Validate credentials
        match credential {
            Some(CredentialData::OAuth { access_token, .. }) => {
                if access_token.is_empty() {
                    return Err(ProviderError::CredentialError(
                        "OneDrive access token is required".to_string(),
                    ));
                }
            }
            None => {
                return Err(ProviderError::CredentialError(
                    "OneDrive credentials are required".to_string(),
                ))
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "Invalid credential type for OneDrive".to_string(),
                ))
            }
        }

        Ok(())
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (folder_path, drive_id) = match &ctx.destination {
            DestinationConfig::OneDrive { folder_path, drive_id } => {
                (folder_path.clone(), drive_id.clone())
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "OneDriveProvider only supports OneDrive destination".to_string(),
                ))
            }
        };

        let access_token = match &ctx.credential {
            Some(CredentialData::OAuth { access_token, .. }) => access_token.clone(),
            _ => {
                return Err(ProviderError::CredentialError(
                    "OneDrive OAuth credentials required".to_string(),
                ))
            }
        };

        ctx.log_info("Starting OneDrive sync", "onedrive").await;
        ctx.log_info(&format!("Target folder: {}", folder_path), "onedrive").await;

        // Build the base path for API calls
        let base_path = if let Some(ref id) = drive_id {
            format!("{}/drives/{}/root:", GRAPH_API_BASE, id)
        } else {
            format!("{}/me/drive/root:", GRAPH_API_BASE)
        };

        // Test connection by getting drive info
        let test_url = if drive_id.is_some() {
            format!("{}/drives/{}", GRAPH_API_BASE, drive_id.as_ref().unwrap())
        } else {
            format!("{}/me/drive", GRAPH_API_BASE)
        };

        let response = self
            .client
            .get(&test_url)
            .bearer_auth(&access_token)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                ctx.log_info("Connection successful", "onedrive").await;
            }
            Ok(resp) if resp.status() == 401 => {
                ctx.log_error("Authentication failed - token may be expired", "onedrive").await;
                return Err(ProviderError::CredentialError(
                    "Authentication failed - token may be expired".to_string(),
                ));
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                ctx.log_error(&format!("Connection failed: {} - {}", status, body), "onedrive").await;
                return Err(ProviderError::ConnectionError(format!("HTTP {}", status)));
            }
            Err(e) => {
                ctx.log_error(&format!("Connection error: {}", e), "onedrive").await;
                return Err(ProviderError::ConnectionError(e.to_string()));
            }
        }

        // Ensure target folder exists
        let folder_path_clean = folder_path.trim_matches('/');
        if !folder_path_clean.is_empty() && !ctx.options.dry_run {
            self.ensure_folder_exists(&base_path, folder_path_clean, &access_token, &ctx).await?;
        }

        // Sync each source directory
        for source_dir in &ctx.source_dirs {
            if ctx.check_cancelled() {
                ctx.log_warning("Sync cancelled by user", "onedrive").await;
                return Err(ProviderError::Cancelled);
            }

            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "onedrive").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest_path = if folder_path_clean.is_empty() {
                dest_name.clone()
            } else {
                format!("{}/{}", folder_path_clean, dest_name)
            };

            ctx.log_info(&format!("Syncing {} -> /{}", source_dir, dest_path), "onedrive").await;

            // Create destination folder
            if !ctx.options.dry_run {
                self.ensure_folder_exists(&base_path, &dest_path, &access_token, &ctx).await?;
            }

            // Sync directory contents
            match self
                .sync_directory(&base_path, source_dir, &dest_path, &access_token, &ctx, &mut result)
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "onedrive").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "OneDrive sync complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "onedrive",
        )
        .await;

        Ok(result)
    }
}

impl OneDriveProvider {
    /// Ensure a folder path exists, creating folders as needed
    async fn ensure_folder_exists(
        &self,
        base_path: &str,
        folder_path: &str,
        access_token: &str,
        ctx: &SyncContext,
    ) -> Result<(), ProviderError> {
        let parts: Vec<&str> = folder_path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current_path = String::new();

        for part in parts {
            if current_path.is_empty() {
                current_path = part.to_string();
            } else {
                current_path = format!("{}/{}", current_path, part);
            }

            // Check if folder exists
            let check_url = format!("{}/{}", base_path, current_path);
            let response = self
                .client
                .get(&check_url)
                .bearer_auth(access_token)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    // Folder exists
                    continue;
                }
                Ok(resp) if resp.status() == 404 => {
                    // Folder doesn't exist, create it
                    let parent_path = if current_path.contains('/') {
                        let idx = current_path.rfind('/').unwrap();
                        &current_path[..idx]
                    } else {
                        ""
                    };

                    let create_url = if parent_path.is_empty() {
                        format!("{}/children", base_path.trim_end_matches(':'))
                    } else {
                        format!("{}/{}/children", base_path, parent_path)
                    };

                    let folder_name = if current_path.contains('/') {
                        current_path.rsplit('/').next().unwrap()
                    } else {
                        &current_path
                    };

                    let body = serde_json::json!({
                        "name": folder_name,
                        "folder": {},
                        "@microsoft.graph.conflictBehavior": "replace"
                    });

                    let create_resp = self
                        .client
                        .post(&create_url)
                        .bearer_auth(access_token)
                        .json(&body)
                        .send()
                        .await;

                    match create_resp {
                        Ok(r) if r.status().is_success() || r.status() == 201 => {
                            ctx.log_info(&format!("Created folder: {}", current_path), "onedrive").await;
                        }
                        Ok(r) => {
                            let status = r.status();
                            let body = r.text().await.unwrap_or_default();
                            return Err(ProviderError::TransferError(format!(
                                "Failed to create folder {}: {} - {}",
                                current_path, status, body
                            )));
                        }
                        Err(e) => {
                            return Err(ProviderError::ConnectionError(format!(
                                "Failed to create folder {}: {}",
                                current_path, e
                            )));
                        }
                    }
                }
                Ok(resp) => {
                    let status = resp.status();
                    return Err(ProviderError::ConnectionError(format!(
                        "Failed to check folder {}: HTTP {}",
                        current_path, status
                    )));
                }
                Err(e) => {
                    return Err(ProviderError::ConnectionError(format!(
                        "Failed to check folder {}: {}",
                        current_path, e
                    )));
                }
            }
        }

        Ok(())
    }

    async fn sync_directory(
        &self,
        base_path: &str,
        source_dir: &str,
        remote_dir: &str,
        access_token: &str,
        ctx: &SyncContext,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        use tokio::fs;

        let mut entries = fs::read_dir(source_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }

            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Check exclude patterns
            if ctx
                .options
                .exclude_patterns
                .iter()
                .any(|p| glob::Pattern::new(p).map(|pat| pat.matches(&file_name)).unwrap_or(false))
            {
                continue;
            }

            let remote_path = format!("{}/{}", remote_dir, file_name);

            if path.is_dir() {
                // Create remote directory and recurse
                if !ctx.options.dry_run {
                    self.ensure_folder_exists(base_path, &remote_path, access_token, ctx).await?;
                }
                Box::pin(self.sync_directory(
                    base_path,
                    path.to_str().unwrap(),
                    &remote_path,
                    access_token,
                    ctx,
                    result,
                ))
                .await?;
            } else if path.is_file() {
                if ctx.options.dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would upload: {}", remote_path), "onedrive").await;
                    result.files_transferred += 1;
                    continue;
                }

                // Get file size
                let metadata = fs::metadata(&path).await?;
                let file_size = metadata.len() as i64;

                // Upload file
                match self
                    .upload_file(base_path, &path, &remote_path, access_token, ctx)
                    .await
                {
                    Ok(_) => {
                        ctx.log_info(&format!("Uploaded: {}", remote_path), "onedrive").await;
                        result.files_transferred += 1;
                        result.bytes_transferred += file_size;
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to upload {}: {}", remote_path, e), "onedrive").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    async fn upload_file(
        &self,
        base_path: &str,
        local_path: &std::path::Path,
        remote_path: &str,
        access_token: &str,
        ctx: &SyncContext,
    ) -> Result<(), ProviderError> {
        use tokio::fs;

        let data = fs::read(local_path).await?;
        let file_size = data.len();

        // For files <= 4MB, use simple upload
        // For larger files, use upload session
        if file_size <= 4 * 1024 * 1024 {
            // Simple upload
            let upload_url = format!("{}/{}:/content", base_path, remote_path);

            let response = self
                .client
                .put(&upload_url)
                .bearer_auth(access_token)
                .header("Content-Type", "application/octet-stream")
                .body(data)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() || resp.status() == 201 => Ok(()),
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    Err(ProviderError::TransferError(format!(
                        "Upload failed: {} - {}",
                        status, body
                    )))
                }
                Err(e) => Err(ProviderError::ConnectionError(format!("Upload failed: {}", e))),
            }
        } else {
            // Large file upload using upload session
            self.upload_large_file(base_path, remote_path, &data, access_token, ctx).await
        }
    }

    async fn upload_large_file(
        &self,
        base_path: &str,
        remote_path: &str,
        data: &[u8],
        access_token: &str,
        ctx: &SyncContext,
    ) -> Result<(), ProviderError> {
        // Create upload session
        let session_url = format!("{}/{}:/createUploadSession", base_path, remote_path);

        let session_body = serde_json::json!({
            "item": {
                "@microsoft.graph.conflictBehavior": "replace"
            }
        });

        let session_resp = self
            .client
            .post(&session_url)
            .bearer_auth(access_token)
            .json(&session_body)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Failed to create upload session: {}", e)))?;

        if !session_resp.status().is_success() {
            let status = session_resp.status();
            let body = session_resp.text().await.unwrap_or_default();
            return Err(ProviderError::TransferError(format!(
                "Failed to create upload session: {} - {}",
                status, body
            )));
        }

        let session: UploadSession = session_resp
            .json()
            .await
            .map_err(|e| ProviderError::TransferError(format!("Invalid upload session response: {}", e)))?;

        // Upload in chunks (10MB chunks)
        let chunk_size = 10 * 1024 * 1024;
        let total_size = data.len();
        let mut offset = 0;

        while offset < total_size {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }

            let end = std::cmp::min(offset + chunk_size, total_size);
            let chunk = &data[offset..end];

            let content_range = format!("bytes {}-{}/{}", offset, end - 1, total_size);

            let chunk_resp = self
                .client
                .put(&session.upload_url)
                .header("Content-Length", chunk.len())
                .header("Content-Range", content_range)
                .body(chunk.to_vec())
                .send()
                .await
                .map_err(|e| ProviderError::ConnectionError(format!("Chunk upload failed: {}", e)))?;

            if !chunk_resp.status().is_success() && chunk_resp.status() != 202 {
                let status = chunk_resp.status();
                let body = chunk_resp.text().await.unwrap_or_default();
                return Err(ProviderError::TransferError(format!(
                    "Chunk upload failed: {} - {}",
                    status, body
                )));
            }

            offset = end;
        }

        Ok(())
    }
}

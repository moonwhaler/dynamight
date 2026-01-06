//! Google Drive sync provider using Google Drive API v3

use super::{ProviderCapabilities, ProviderError, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;

const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API_BASE: &str = "https://www.googleapis.com/upload/drive/v3";

pub struct GoogleDriveProvider {
    client: Client,
}

impl GoogleDriveProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Default for GoogleDriveProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct FileList {
    files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken")]
    #[allow(dead_code)]
    next_page_token: Option<String>,
}

#[async_trait]
impl SyncProvider for GoogleDriveProvider {
    fn provider_type(&self) -> &'static str {
        "google_drive"
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
            DestinationConfig::GoogleDrive { .. } => {
                // folder_id can be empty to use root
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "GoogleDriveProvider only supports Google Drive destination".to_string(),
                ))
            }
        }

        // Validate credentials
        match credential {
            Some(CredentialData::OAuth { access_token, .. }) => {
                if access_token.is_empty() {
                    return Err(ProviderError::CredentialError(
                        "Google Drive access token is required".to_string(),
                    ));
                }
            }
            None => {
                return Err(ProviderError::CredentialError(
                    "Google Drive credentials are required".to_string(),
                ))
            }
            _ => {
                return Err(ProviderError::CredentialError(
                    "Invalid credential type for Google Drive".to_string(),
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

        let (folder_id, shared_drive_id) = match destination {
            DestinationConfig::GoogleDrive { folder_id, shared_drive_id } => {
                (folder_id.clone(), shared_drive_id.clone())
            }
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };

        let access_token = match credential {
            Some(CredentialData::OAuth { access_token, .. }) => access_token.clone(),
            _ => return Err(ProviderError::CredentialError("OAuth credentials required".to_string())),
        };

        // Test connection using the about endpoint
        let test_url = format!("{}/about?fields=user", DRIVE_API_BASE);
        let response = self
            .client
            .get(&test_url)
            .bearer_auth(&access_token)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Failed to connect: {}", e)))?;

        match response.status().as_u16() {
            200 => {
                // Parse user info from response
                #[derive(Deserialize)]
                struct AboutResponse {
                    user: UserInfo,
                }
                #[derive(Deserialize)]
                struct UserInfo {
                    #[serde(rename = "displayName")]
                    display_name: Option<String>,
                    #[serde(rename = "emailAddress")]
                    email_address: Option<String>,
                }

                let user_info = response
                    .json::<AboutResponse>()
                    .await
                    .map(|r| {
                        let name = r.user.display_name.unwrap_or_default();
                        let email = r.user.email_address.unwrap_or_default();
                        if !name.is_empty() && !email.is_empty() {
                            format!("{} ({})", name, email)
                        } else if !email.is_empty() {
                            email
                        } else {
                            "Unknown user".to_string()
                        }
                    })
                    .unwrap_or_else(|_| "Unknown user".to_string());

                let folder_info = if folder_id.is_empty() {
                    "Root folder".to_string()
                } else if let Some(ref drive_id) = shared_drive_id {
                    format!("Shared Drive: {}, Folder: {}", drive_id, folder_id)
                } else {
                    format!("Folder ID: {}", folder_id)
                };

                Ok(TestConnectionResult {
                    success: true,
                    message: "Successfully connected to Google Drive".to_string(),
                    details: Some(format!("Account: {}. {}", user_info, folder_info)),
                    host_key_fingerprint: None,
                })
            }
            401 => Err(ProviderError::CredentialError(
                "Authentication failed. Your access token may have expired.".to_string(),
            )),
            403 => Err(ProviderError::CredentialError(
                "Access forbidden. Check your OAuth scopes and permissions.".to_string(),
            )),
            status => Err(ProviderError::ConnectionError(format!(
                "Google Drive API returned status {}",
                status
            ))),
        }
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (folder_id, shared_drive_id) = match &ctx.destination {
            DestinationConfig::GoogleDrive { folder_id, shared_drive_id } => {
                (folder_id.clone(), shared_drive_id.clone())
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "GoogleDriveProvider only supports Google Drive destination".to_string(),
                ))
            }
        };

        let access_token = match &ctx.credential {
            Some(CredentialData::OAuth { access_token, .. }) => access_token.clone(),
            _ => {
                return Err(ProviderError::CredentialError(
                    "Google Drive OAuth credentials required".to_string(),
                ))
            }
        };

        ctx.log_info("Starting Google Drive sync", "googledrive").await;

        // Test connection by getting about info
        let test_url = format!("{}/about?fields=user", DRIVE_API_BASE);
        let response = self
            .client
            .get(&test_url)
            .bearer_auth(&access_token)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                ctx.log_info("Connection successful", "googledrive").await;
            }
            Ok(resp) if resp.status() == 401 => {
                ctx.log_error("Authentication failed - token may be expired", "googledrive").await;
                return Err(ProviderError::CredentialError(
                    "Authentication failed - token may be expired".to_string(),
                ));
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                ctx.log_error(&format!("Connection failed: {} - {}", status, body), "googledrive").await;
                return Err(ProviderError::ConnectionError(format!("HTTP {}", status)));
            }
            Err(e) => {
                ctx.log_error(&format!("Connection error: {}", e), "googledrive").await;
                return Err(ProviderError::ConnectionError(e.to_string()));
            }
        }

        // Determine the parent folder ID
        let parent_id = if folder_id.is_empty() {
            "root".to_string()
        } else {
            folder_id.clone()
        };

        if let Some(ref drive_id) = shared_drive_id {
            ctx.log_info(&format!("Using shared drive: {}", drive_id), "googledrive").await;
        }
        ctx.log_info(&format!("Target folder ID: {}", parent_id), "googledrive").await;

        // Sync each source directory
        for source_dir in &ctx.source_dirs {
            if ctx.check_cancelled() {
                ctx.log_warning("Sync cancelled by user", "googledrive").await;
                return Err(ProviderError::Cancelled);
            }

            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "googledrive").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            ctx.log_info(&format!("Syncing {} -> {}", source_dir, dest_name), "googledrive").await;

            // Find or create the destination folder
            let dest_folder_id = if !ctx.options.dry_run {
                match self.find_or_create_folder(&dest_name, &parent_id, &shared_drive_id, &access_token, &ctx).await {
                    Ok(id) => id,
                    Err(e) => {
                        ctx.log_error(&format!("Failed to create folder {}: {}", dest_name, e), "googledrive").await;
                        result.error_count += 1;
                        continue;
                    }
                }
            } else {
                "dry-run".to_string()
            };

            // Sync directory contents
            match self
                .sync_directory(source_dir, &dest_folder_id, &shared_drive_id, &access_token, &ctx, &mut result)
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "googledrive").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "Google Drive sync complete. Files: {}, Bytes: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.error_count
            ),
            "googledrive",
        )
        .await;

        Ok(result)
    }
}

impl GoogleDriveProvider {
    /// Find a folder by name in a parent, or create it if it doesn't exist
    async fn find_or_create_folder(
        &self,
        name: &str,
        parent_id: &str,
        shared_drive_id: &Option<String>,
        access_token: &str,
        ctx: &SyncContext,
    ) -> Result<String, ProviderError> {
        // Search for existing folder
        let query = format!(
            "name = '{}' and '{}' in parents and mimeType = 'application/vnd.google-apps.folder' and trashed = false",
            name.replace('\'', "\\'"),
            parent_id
        );

        let mut url = format!(
            "{}/files?q={}&fields=files(id,name)",
            DRIVE_API_BASE,
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
        );

        if let Some(ref drive_id) = shared_drive_id {
            url.push_str(&format!(
                "&supportsAllDrives=true&includeItemsFromAllDrives=true&corpora=drive&driveId={}",
                drive_id
            ));
        }

        let response = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Search failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::ConnectionError(format!(
                "Search failed: {} - {}",
                status, body
            )));
        }

        let file_list: FileList = response
            .json()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;

        // If folder exists, return its ID
        if let Some(folder) = file_list.files.first() {
            return Ok(folder.id.clone());
        }

        // Create the folder
        ctx.log_info(&format!("Creating folder: {}", name), "googledrive").await;

        let mut metadata = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
            "parents": [parent_id]
        });

        let mut create_url = format!("{}/files", DRIVE_API_BASE);
        if shared_drive_id.is_some() {
            create_url.push_str("?supportsAllDrives=true");
            if let Some(ref drive_id) = shared_drive_id {
                metadata["driveId"] = serde_json::json!(drive_id);
            }
        }

        let response = self
            .client
            .post(&create_url)
            .bearer_auth(access_token)
            .json(&metadata)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Create folder failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::TransferError(format!(
                "Create folder failed: {} - {}",
                status, body
            )));
        }

        let folder: DriveFile = response
            .json()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;

        Ok(folder.id)
    }

    async fn sync_directory(
        &self,
        source_dir: &str,
        parent_id: &str,
        shared_drive_id: &Option<String>,
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

            if path.is_dir() {
                // Find or create subfolder and recurse
                let subfolder_id = if !ctx.options.dry_run {
                    self.find_or_create_folder(&file_name, parent_id, shared_drive_id, access_token, ctx).await?
                } else {
                    "dry-run".to_string()
                };

                let path_str = path.to_str().ok_or_else(|| {
                    ProviderError::TransferError(format!("Invalid UTF-8 in path: {}", path.display()))
                })?;
                Box::pin(self.sync_directory(
                    path_str,
                    &subfolder_id,
                    shared_drive_id,
                    access_token,
                    ctx,
                    result,
                ))
                .await?;
            } else if path.is_file() {
                if ctx.options.dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would upload: {}", file_name), "googledrive").await;
                    result.files_transferred += 1;
                    continue;
                }

                // Get file size
                let metadata = fs::metadata(&path).await?;
                let file_size = metadata.len() as i64;

                // Upload file
                match self
                    .upload_file(&path, &file_name, parent_id, shared_drive_id, access_token, ctx)
                    .await
                {
                    Ok(_) => {
                        ctx.log_info(&format!("Uploaded: {}", file_name), "googledrive").await;
                        result.files_transferred += 1;
                        result.bytes_transferred += file_size;
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to upload {}: {}", file_name, e), "googledrive").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    async fn upload_file(
        &self,
        local_path: &std::path::Path,
        file_name: &str,
        parent_id: &str,
        shared_drive_id: &Option<String>,
        access_token: &str,
        ctx: &SyncContext,
    ) -> Result<(), ProviderError> {
        use tokio::fs;

        let data = fs::read(local_path).await?;
        let file_size = data.len();

        // Detect MIME type from extension
        let mime_type = local_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| match ext.to_lowercase().as_str() {
                "txt" => "text/plain",
                "html" | "htm" => "text/html",
                "css" => "text/css",
                "js" => "application/javascript",
                "json" => "application/json",
                "xml" => "application/xml",
                "pdf" => "application/pdf",
                "zip" => "application/zip",
                "gz" | "gzip" => "application/gzip",
                "tar" => "application/x-tar",
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "svg" => "image/svg+xml",
                "mp3" => "audio/mpeg",
                "mp4" => "video/mp4",
                "webm" => "video/webm",
                "doc" => "application/msword",
                "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "xls" => "application/vnd.ms-excel",
                "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                _ => "application/octet-stream",
            })
            .unwrap_or("application/octet-stream")
            .to_string();

        // For files <= 5MB, use simple upload
        // For larger files, use resumable upload
        if file_size <= 5 * 1024 * 1024 {
            self.simple_upload(&data, file_name, &mime_type, parent_id, shared_drive_id, access_token).await
        } else {
            self.resumable_upload(&data, file_name, &mime_type, parent_id, shared_drive_id, access_token, ctx).await
        }
    }

    async fn simple_upload(
        &self,
        data: &[u8],
        file_name: &str,
        mime_type: &str,
        parent_id: &str,
        shared_drive_id: &Option<String>,
        access_token: &str,
    ) -> Result<(), ProviderError> {
        let metadata = serde_json::json!({
            "name": file_name,
            "parents": [parent_id]
        });

        let mut url = format!(
            "{}/files?uploadType=multipart",
            UPLOAD_API_BASE
        );
        if shared_drive_id.is_some() {
            url.push_str("&supportsAllDrives=true");
        }

        // Build multipart request
        let boundary = "===dynamight_boundary===";
        let mut body = Vec::new();

        // Metadata part
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(metadata.to_string().as_bytes());
        body.extend_from_slice(b"\r\n");

        // File part
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", mime_type).as_bytes());
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");

        // End boundary
        body.extend_from_slice(format!("--{}--", boundary).as_bytes());

        let response = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .header("Content-Type", format!("multipart/related; boundary={}", boundary))
            .body(body)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Upload failed: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(ProviderError::TransferError(format!(
                "Upload failed: {} - {}",
                status, body
            )))
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn resumable_upload(
        &self,
        data: &[u8],
        file_name: &str,
        mime_type: &str,
        parent_id: &str,
        shared_drive_id: &Option<String>,
        access_token: &str,
        ctx: &SyncContext,
    ) -> Result<(), ProviderError> {
        let metadata = serde_json::json!({
            "name": file_name,
            "parents": [parent_id]
        });

        let mut url = format!(
            "{}/files?uploadType=resumable",
            UPLOAD_API_BASE
        );
        if shared_drive_id.is_some() {
            url.push_str("&supportsAllDrives=true");
        }

        // Initiate resumable upload
        let init_response = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .header("Content-Type", "application/json; charset=UTF-8")
            .header("X-Upload-Content-Type", mime_type)
            .header("X-Upload-Content-Length", data.len())
            .json(&metadata)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Init upload failed: {}", e)))?;

        if !init_response.status().is_success() {
            let status = init_response.status();
            let body = init_response.text().await.unwrap_or_default();
            return Err(ProviderError::TransferError(format!(
                "Init upload failed: {} - {}",
                status, body
            )));
        }

        let upload_url = init_response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ProviderError::TransferError("No upload URL in response".to_string()))?
            .to_string();

        // Upload in chunks (8MB chunks)
        let chunk_size = 8 * 1024 * 1024;
        let total_size = data.len();
        let mut offset = 0;

        while offset < total_size {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }

            let end = std::cmp::min(offset + chunk_size, total_size);
            let chunk = &data[offset..end];

            let content_range = format!("bytes {}-{}/{}", offset, end - 1, total_size);

            let chunk_response = self
                .client
                .put(&upload_url)
                .header("Content-Length", chunk.len())
                .header("Content-Range", content_range)
                .body(chunk.to_vec())
                .send()
                .await
                .map_err(|e| ProviderError::ConnectionError(format!("Chunk upload failed: {}", e)))?;

            let status = chunk_response.status();
            // 200/201 = complete, 308 = Resume Incomplete (continue uploading)
            if !status.is_success() && status.as_u16() != 308 {
                let body = chunk_response.text().await.unwrap_or_default();
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

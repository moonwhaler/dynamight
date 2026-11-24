//! Google Drive sync provider using Google Drive API v3
//!
//! Sync is incremental: the destination folder tree is listed once, files of
//! the same size whose `modifiedTime` is not older than the local file are
//! skipped, changed files are updated in place (Drive allows duplicate names,
//! so creating a new file every run would pile up copies), and new files are
//! created. With "delete extraneous" enabled, remote items that no longer
//! exist locally are moved to the Drive trash.
//!
//! API references:
//! - Uploads: <https://developers.google.com/workspace/drive/api/guides/manage-uploads>
//! - Shared drives: <https://developers.google.com/workspace/drive/api/guides/enable-shareddrives>

use super::common::{
    self, needs_upload, parse_rfc3339, read_chunk, rel_excluded, send_with_retry, LocalEntry, RemoteFile,
};
use super::{ProviderCapabilities, ProviderError, StorageInfo, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API_BASE: &str = "https://www.googleapis.com/upload/drive/v3";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
/// Files up to this size use a single multipart request
const SIMPLE_UPLOAD_LIMIT: u64 = 5 * 1024 * 1024;
/// Resumable upload chunk size; must be a multiple of 256 KiB
const CHUNK_SIZE: u64 = 8 * 1024 * 1024;
const MAX_CHUNK_RETRIES: u32 = 6;

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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DriveFile {
    id: String,
    name: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    size: Option<String>,
    #[serde(default)]
    modified_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileList {
    #[serde(default)]
    files: Vec<DriveFile>,
    next_page_token: Option<String>,
}

/// Escape a value for a Drive `q` string literal (backslash first, then quote).
fn escape_query_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn access_token_of(credential: Option<&CredentialData>) -> Result<String, ProviderError> {
    match credential {
        Some(CredentialData::OAuth { access_token, .. }) if !access_token.is_empty() => Ok(access_token.clone()),
        Some(CredentialData::OAuth { .. }) => Err(ProviderError::CredentialError(
            "No access token. Set the OAuth client ID and secret so a token can be fetched with the refresh token."
                .to_string(),
        )),
        _ => Err(ProviderError::CredentialError("OAuth credentials required".to_string())),
    }
}

/// The folder backups go into: the configured folder, else the shared drive's
/// root, else the root of My Drive.
fn parent_folder_id(folder_id: &str, shared_drive_id: &Option<String>) -> String {
    if !folder_id.trim().is_empty() {
        folder_id.trim().to_string()
    } else if let Some(drive) = shared_drive_id.as_deref().filter(|d| !d.trim().is_empty()) {
        drive.trim().to_string()
    } else {
        "root".to_string()
    }
}

fn rfc3339(secs: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Remote state of a destination folder tree, keyed by relative path.
#[derive(Default)]
struct RemoteTree {
    folders: HashMap<String, String>,
    files: HashMap<String, RemoteFile>,
    /// Extra copies with the same name (left over from older versions that
    /// created a new file on every run)
    duplicates: Vec<(String, String)>,
}

fn http_error(what: &str, status: reqwest::StatusCode, body: &str) -> ProviderError {
    let body: String = body.chars().take(500).collect();
    match status.as_u16() {
        401 => ProviderError::CredentialError(format!("{}: authentication failed (HTTP 401). {}", what, body)),
        403 => ProviderError::CredentialError(format!("{}: access denied (HTTP 403). {}", what, body)),
        _ => ProviderError::TransferError(format!("{}: HTTP {} - {}", what, status, body)),
    }
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
                // folder_id can be empty to use the root (of My Drive or the shared drive)
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "GoogleDriveProvider only supports Google Drive destination".to_string(),
                ))
            }
        }

        match credential {
            Some(CredentialData::OAuth { access_token, refresh_token, client_id, .. }) => {
                if access_token.is_empty() && (refresh_token.is_empty() || client_id.is_none()) {
                    return Err(ProviderError::CredentialError(
                        "Google Drive needs an access token, or a refresh token with client ID and secret".to_string(),
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
            DestinationConfig::GoogleDrive { folder_id, shared_drive_id } => (folder_id.clone(), shared_drive_id.clone()),
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };
        let access_token = access_token_of(credential)?;

        let response = send_with_retry(None, "Google Drive connection test", || {
            self.client
                .get(format!("{}/about?fields=user", DRIVE_API_BASE))
                .bearer_auth(&access_token)
        })
        .await?;

        match response.status().as_u16() {
            200 => {}
            401 => {
                return Err(ProviderError::CredentialError(
                    "Authentication failed. The access token is invalid or expired; check the refresh token and client settings."
                        .to_string(),
                ))
            }
            403 => {
                return Err(ProviderError::CredentialError(
                    "Access forbidden. Check your OAuth scopes and that the Drive API is enabled.".to_string(),
                ))
            }
            status => {
                return Err(ProviderError::ConnectionError(format!(
                    "Google Drive API returned status {}",
                    status
                )))
            }
        }

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

        // Check that the target folder is reachable. With the `drive.file`
        // scope the app only sees files it created itself, so a folder created
        // in the Drive web UI is not found.
        let parent = parent_folder_id(&folder_id, &shared_drive_id);
        let folder_info = if parent == "root" {
            "Folder: My Drive root".to_string()
        } else {
            let resp = send_with_retry(None, "Google Drive folder check", || {
                self.client
                    .get(format!("{}/files/{}", DRIVE_API_BASE, urlencoding_path(&parent)))
                    .query(&[("fields", "id,name,mimeType"), ("supportsAllDrives", "true")])
                    .bearer_auth(&access_token)
            })
            .await?;
            match resp.status().as_u16() {
                200 => {
                    let f: DriveFile = resp
                        .json()
                        .await
                        .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;
                    if f.mime_type != FOLDER_MIME && Some(&parent) != shared_drive_id.as_ref() {
                        return Err(ProviderError::ConfigError(format!("'{}' is not a folder", f.name)));
                    }
                    format!("Folder: {}", f.name)
                }
                404 => {
                    return Err(ProviderError::ConfigError(format!(
                        "Folder '{}' was not found. Check the ID. If the OAuth app only has the \
                         'drive.file' scope, it can only see folders it created; leave the folder ID \
                         empty or use a scope with access to existing files.",
                        parent
                    )))
                }
                _ => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(http_error("Folder check", status, &body));
                }
            }
        };

        Ok(TestConnectionResult {
            success: true,
            message: "Successfully connected to Google Drive".to_string(),
            details: Some(match &shared_drive_id {
                Some(d) if !d.is_empty() => format!("Account: {}. Shared drive: {}. {}", user_info, d, folder_info),
                _ => format!("Account: {}. {}", user_info, folder_info),
            }),
            host_key_fingerprint: None,
        })
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (folder_id, shared_drive_id) = match &ctx.destination {
            DestinationConfig::GoogleDrive { folder_id, shared_drive_id } => (folder_id.clone(), shared_drive_id.clone()),
            _ => {
                return Err(ProviderError::ConfigError(
                    "GoogleDriveProvider only supports Google Drive destination".to_string(),
                ))
            }
        };

        ctx.log_info("Starting Google Drive sync", "googledrive").await;

        // Verify access (and renew the token if needed)
        let token = ctx.oauth_access_token().await?;
        let response = send_with_retry(Some(&ctx), "Google Drive connection check", || {
            self.client
                .get(format!("{}/about?fields=user", DRIVE_API_BASE))
                .bearer_auth(&token)
        })
        .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            ctx.log_error(&format!("Connection failed: {} - {}", status, body), "googledrive").await;
            return Err(http_error("Connection check", status, &body));
        }
        ctx.log_info("Connection successful", "googledrive").await;

        let parent_id = parent_folder_id(&folder_id, &shared_drive_id);
        if let Some(drive) = shared_drive_id.as_deref().filter(|d| !d.is_empty()) {
            ctx.log_info(&format!("Using shared drive: {}", drive), "googledrive").await;
        }
        ctx.log_info(&format!("Target folder ID: {}", parent_id), "googledrive").await;

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

            let drive = shared_drive_id.as_deref().filter(|d| !d.trim().is_empty()).map(str::trim);
            match self.sync_source(&ctx, source_dir, &dest_name, &parent_id, drive, &mut result).await {
                Ok(()) => {}
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "googledrive").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "Google Drive sync complete. Files: {}, Bytes: {}, Deleted: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.files_deleted, result.error_count
            ),
            "googledrive",
        )
        .await;

        Ok(result)
    }

    async fn get_storage_info(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<StorageInfo, ProviderError> {
        let Ok(access_token) = access_token_of(credential) else {
            return Ok(StorageInfo::default());
        };

        // For shared drives, quota info is not meaningful (shared drive quota is different)
        let shared_drive_id = match destination {
            DestinationConfig::GoogleDrive { shared_drive_id, .. } => shared_drive_id.clone(),
            _ => return Ok(StorageInfo::default()),
        };
        if shared_drive_id.is_some_and(|d| !d.is_empty()) {
            return Ok(StorageInfo { free: None, total: None, supported: false });
        }

        let response = self
            .client
            .get(format!("{}/about?fields=storageQuota", DRIVE_API_BASE))
            .bearer_auth(&access_token)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                #[derive(Deserialize)]
                struct AboutResponse {
                    #[serde(rename = "storageQuota")]
                    storage_quota: Option<StorageQuota>,
                }
                #[derive(Deserialize)]
                struct StorageQuota {
                    limit: Option<String>,
                    usage: Option<String>,
                }

                match resp.json::<AboutResponse>().await {
                    Ok(AboutResponse { storage_quota: Some(quota) }) => {
                        let total = quota.limit.and_then(|s| s.parse::<u64>().ok());
                        let used = quota.usage.and_then(|s| s.parse::<u64>().ok());
                        let free = match (total, used) {
                            (Some(t), Some(u)) => Some(t.saturating_sub(u)),
                            _ => None,
                        };
                        Ok(StorageInfo { free, total, supported: true })
                    }
                    _ => Ok(StorageInfo::default()),
                }
            }
            _ => Ok(StorageInfo::default()),
        }
    }
}

/// Percent-encode a single URL path segment (Drive IDs are URL-safe, but be strict).
fn urlencoding_path(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes()).collect::<String>().replace('+', "%20")
}

impl GoogleDriveProvider {
    /// Sync one source directory into `<parent>/<dest_name>`.
    async fn sync_source(
        &self,
        ctx: &SyncContext,
        source_dir: &str,
        dest_name: &str,
        parent_id: &str,
        drive: Option<&str>,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        let dry_run = ctx.options.dry_run;
        let local = common::walk_source_async(source_dir, &ctx.options).await?;
        for msg in &local.skipped {
            ctx.log_warning(msg, "googledrive").await;
        }

        // Find (or create) the destination folder for this source
        let root_id = match self.find_child_folder(ctx, parent_id, dest_name, drive).await? {
            Some(id) => Some(id),
            None if dry_run => None,
            None => {
                ctx.log_info(&format!("Creating folder: {}", dest_name), "googledrive").await;
                Some(self.create_folder(ctx, parent_id, dest_name).await?)
            }
        };

        let mut remote = match &root_id {
            Some(id) => self.list_tree(ctx, id, drive).await?,
            None => RemoteTree::default(),
        };
        if let Some(id) = &root_id {
            remote.folders.insert(String::new(), id.clone());
        }
        if !remote.duplicates.is_empty() {
            ctx.log_warning(
                &format!(
                    "Found {} duplicate file(s) with the same name in the destination (created by older versions). \
                     Enable \"delete extraneous files\" to move them to the trash.",
                    remote.duplicates.len()
                ),
                "googledrive",
            )
            .await;
        }

        // Create missing folders (parents come before children)
        for dir in local.dirs() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            if remote.folders.contains_key(&dir.rel) {
                continue;
            }
            let (parent_rel, name) = split_rel(&dir.rel);
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would create folder: {}/{}", dest_name, dir.rel), "googledrive").await;
                continue;
            }
            let Some(parent) = remote.folders.get(parent_rel).cloned() else {
                continue;
            };
            let id = self.create_folder(ctx, &parent, name).await?;
            remote.folders.insert(dir.rel.clone(), id);
        }

        // Upload new and changed files
        let mut skipped = 0u64;
        for file in local.files() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let existing = remote.files.get(&file.rel);
            if !needs_upload(file, existing) {
                skipped += 1;
                continue;
            }
            let label = format!("{}/{}", dest_name, file.rel);
            if dry_run {
                let verb = if existing.is_some() { "update" } else { "upload" };
                ctx.log_info(&format!("[DRY RUN] Would {}: {}", verb, label), "googledrive").await;
                result.files_transferred += 1;
                result.bytes_transferred += file.size as i64;
                continue;
            }
            let (parent_rel, _) = split_rel(&file.rel);
            let Some(parent) = remote.folders.get(parent_rel).cloned() else {
                ctx.log_error(&format!("Missing remote folder for {}", label), "googledrive").await;
                result.error_count += 1;
                continue;
            };
            let existing_id = existing.and_then(|r| r.id.clone());
            match self.upload_file(ctx, file, &parent, existing_id.as_deref()).await {
                Ok(()) => {
                    let verb = if existing_id.is_some() { "Updated" } else { "Uploaded" };
                    ctx.log_info(&format!("{}: {}", verb, label), "googledrive").await;
                    result.files_transferred += 1;
                    result.bytes_transferred += file.size as i64;
                }
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to upload {}: {}", label, e), "googledrive").await;
                    result.error_count += 1;
                }
            }
        }
        if skipped > 0 {
            ctx.log_info(&format!("{} unchanged file(s) skipped", skipped), "googledrive").await;
        }

        // Mirror deletes: move items that no longer exist locally to the trash
        if ctx.options.delete_extraneous {
            let local_paths = local.rel_paths();
            let mut to_trash: Vec<(String, String)> = Vec::new();
            for (rel, file) in &remote.files {
                if !local_paths.contains(rel) && !rel_excluded(source_dir, rel, &ctx.options) {
                    if let Some(id) = &file.id {
                        to_trash.push((rel.clone(), id.clone()));
                    }
                }
            }
            for (rel, id) in &remote.folders {
                if !rel.is_empty() && !local_paths.contains(rel) && !rel_excluded(source_dir, rel, &ctx.options) {
                    to_trash.push((rel.clone(), id.clone()));
                }
            }
            to_trash.extend(remote.duplicates.iter().cloned());
            to_trash.sort();

            let mut trashed_dirs: HashSet<String> = HashSet::new();
            for (rel, id) in to_trash {
                if ctx.check_cancelled() {
                    return Err(ProviderError::Cancelled);
                }
                // Items inside an already trashed folder go with it
                if trashed_dirs.iter().any(|d| rel.starts_with(&format!("{}/", d))) {
                    continue;
                }
                let label = format!("{}/{}", dest_name, rel);
                if dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would move to trash: {}", label), "googledrive").await;
                    result.files_deleted += 1;
                    continue;
                }
                match self.trash(ctx, &id).await {
                    Ok(()) => {
                        ctx.log_info(&format!("Moved to trash: {}", label), "googledrive").await;
                        result.files_deleted += 1;
                        if remote.folders.get(&rel) == Some(&id) {
                            trashed_dirs.insert(rel);
                        }
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to delete {}: {}", label, e), "googledrive").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    /// List the children of a folder (all pages).
    async fn list_children(&self, ctx: &SyncContext, folder_id: &str, drive: Option<&str>) -> Result<Vec<DriveFile>, ProviderError> {
        let query = format!("'{}' in parents and trashed = false", escape_query_value(folder_id));
        let mut out = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let token = ctx.oauth_access_token().await?;
            let response = send_with_retry(Some(ctx), "List folder", || {
                let mut req = self
                    .client
                    .get(format!("{}/files", DRIVE_API_BASE))
                    .bearer_auth(&token)
                    .query(&[
                        ("q", query.as_str()),
                        ("fields", "nextPageToken,files(id,name,mimeType,size,modifiedTime)"),
                        ("pageSize", "1000"),
                        ("supportsAllDrives", "true"),
                        ("includeItemsFromAllDrives", "true"),
                    ]);
                if let Some(pt) = &page_token {
                    req = req.query(&[("pageToken", pt.as_str())]);
                }
                if let Some(d) = drive {
                    req = req.query(&[("corpora", "drive"), ("driveId", d)]);
                }
                req
            })
            .await?;
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(http_error("List folder", status, &body));
            }
            let page: FileList = response
                .json()
                .await
                .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;
            out.extend(page.files);
            match page.next_page_token {
                Some(t) if !t.is_empty() => page_token = Some(t),
                _ => break,
            }
        }
        Ok(out)
    }

    /// Recursively list the folder tree below `root_id`.
    async fn list_tree(&self, ctx: &SyncContext, root_id: &str, drive: Option<&str>) -> Result<RemoteTree, ProviderError> {
        let mut tree = RemoteTree::default();
        let mut queue: Vec<(String, String)> = vec![(String::new(), root_id.to_string())];

        while let Some((prefix, folder_id)) = queue.pop() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let mut children = self.list_children(ctx, &folder_id, drive).await?;
            // Newest first, so the copy we keep among duplicates is the latest
            children.sort_by(|a, b| b.modified_time.cmp(&a.modified_time));

            for child in children {
                let rel = if prefix.is_empty() { child.name.clone() } else { format!("{}/{}", prefix, child.name) };
                if child.mime_type == FOLDER_MIME {
                    match tree.folders.entry(rel.clone()) {
                        std::collections::hash_map::Entry::Occupied(_) => tree.duplicates.push((rel, child.id)),
                        std::collections::hash_map::Entry::Vacant(slot) => {
                            slot.insert(child.id.clone());
                            queue.push((rel, child.id));
                        }
                    }
                } else if child.mime_type.starts_with("application/vnd.google-apps.") {
                    // Native Google Docs/Sheets have no binary content; never touch them
                    continue;
                } else {
                    match tree.files.entry(rel.clone()) {
                        std::collections::hash_map::Entry::Occupied(_) => tree.duplicates.push((rel, child.id)),
                        std::collections::hash_map::Entry::Vacant(slot) => {
                            slot.insert(RemoteFile {
                                size: child.size.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0),
                                mtime: child.modified_time.as_deref().and_then(parse_rfc3339).unwrap_or(0),
                                id: Some(child.id),
                            });
                        }
                    }
                }
            }
        }
        Ok(tree)
    }

    async fn find_child_folder(
        &self,
        ctx: &SyncContext,
        parent_id: &str,
        name: &str,
        drive: Option<&str>,
    ) -> Result<Option<String>, ProviderError> {
        let query = format!(
            "name = '{}' and '{}' in parents and mimeType = '{}' and trashed = false",
            escape_query_value(name),
            escape_query_value(parent_id),
            FOLDER_MIME
        );
        let token = ctx.oauth_access_token().await?;
        let response = send_with_retry(Some(ctx), "Search folder", || {
            let req = self
                .client
                .get(format!("{}/files", DRIVE_API_BASE))
                .bearer_auth(&token)
                .query(&[
                    ("q", query.as_str()),
                    ("fields", "files(id,name,modifiedTime)"),
                    ("orderBy", "modifiedTime desc"),
                    ("supportsAllDrives", "true"),
                    ("includeItemsFromAllDrives", "true"),
                ]);
            match drive {
                Some(d) => req.query(&[("corpora", "drive"), ("driveId", d)]),
                None => req,
            }
        })
        .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 404 {
                return Err(ProviderError::ConfigError(format!(
                    "Target folder '{}' not found. With the 'drive.file' scope the app can only see folders it created.",
                    parent_id
                )));
            }
            return Err(http_error("Search folder", status, &body));
        }
        let list: FileList = response
            .json()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;
        Ok(list.files.into_iter().next().map(|f| f.id))
    }

    async fn create_folder(&self, ctx: &SyncContext, parent_id: &str, name: &str) -> Result<String, ProviderError> {
        let metadata = serde_json::json!({
            "name": name,
            "mimeType": FOLDER_MIME,
            "parents": [parent_id],
        });
        let token = ctx.oauth_access_token().await?;
        let response = send_with_retry(Some(ctx), "Create folder", || {
            self.client
                .post(format!("{}/files", DRIVE_API_BASE))
                .query(&[("supportsAllDrives", "true"), ("fields", "id")])
                .bearer_auth(&token)
                .json(&metadata)
        })
        .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(http_error(&format!("Create folder {}", name), status, &body));
        }
        #[derive(Deserialize)]
        struct Created {
            id: String,
        }
        let created: Created = response
            .json()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;
        Ok(created.id)
    }

    async fn trash(&self, ctx: &SyncContext, file_id: &str) -> Result<(), ProviderError> {
        let token = ctx.oauth_access_token().await?;
        let response = send_with_retry(Some(ctx), "Move to trash", || {
            self.client
                .patch(format!("{}/files/{}", DRIVE_API_BASE, urlencoding_path(file_id)))
                .query(&[("supportsAllDrives", "true"), ("fields", "id")])
                .bearer_auth(&token)
                .json(&serde_json::json!({ "trashed": true }))
        })
        .await?;
        if response.status().is_success() || response.status().as_u16() == 404 {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(http_error("Move to trash", status, &body))
        }
    }

    /// Upload a file: update `existing_id` in place, or create a new file in `parent_id`.
    async fn upload_file(
        &self,
        ctx: &SyncContext,
        file: &LocalEntry,
        parent_id: &str,
        existing_id: Option<&str>,
    ) -> Result<(), ProviderError> {
        let name = split_rel(&file.rel).1;
        let mime_type = mime_guess::from_path(&file.abs).first_or_octet_stream().to_string();
        // Keep the local modification time, so the next run can skip the file
        let mut metadata = serde_json::json!({ "modifiedTime": rfc3339(file.mtime) });
        if existing_id.is_none() {
            metadata["name"] = serde_json::json!(name);
            metadata["parents"] = serde_json::json!([parent_id]);
        }

        if file.size <= SIMPLE_UPLOAD_LIMIT {
            self.multipart_upload(ctx, file, &metadata, &mime_type, existing_id).await
        } else {
            self.resumable_upload(ctx, file, &metadata, &mime_type, existing_id).await
        }
    }

    fn upload_request(&self, existing_id: Option<&str>, upload_type: &str, token: &str) -> reqwest::RequestBuilder {
        let req = match existing_id {
            Some(id) => self.client.patch(format!("{}/files/{}", UPLOAD_API_BASE, urlencoding_path(id))),
            None => self.client.post(format!("{}/files", UPLOAD_API_BASE)),
        };
        req.query(&[("uploadType", upload_type), ("supportsAllDrives", "true"), ("fields", "id")])
            .bearer_auth(token)
    }

    async fn multipart_upload(
        &self,
        ctx: &SyncContext,
        file: &LocalEntry,
        metadata: &serde_json::Value,
        mime_type: &str,
        existing_id: Option<&str>,
    ) -> Result<(), ProviderError> {
        let data = tokio::fs::read(&file.abs).await?;
        let boundary = format!("dynamight_{}", uuid::Uuid::new_v4().simple());

        let mut body = Vec::with_capacity(data.len() + 512);
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(metadata.to_string().as_bytes());
        body.extend_from_slice(format!("\r\n--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", mime_type).as_bytes());
        body.extend_from_slice(&data);
        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
        let body = bytes::Bytes::from(body);

        let token = ctx.oauth_access_token().await?;
        let response = send_with_retry(Some(ctx), "Upload", || {
            self.upload_request(existing_id, "multipart", &token)
                .header("Content-Type", format!("multipart/related; boundary={}", boundary))
                .body(body.clone())
        })
        .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(http_error("Upload", status, &body))
        }
    }

    async fn resumable_upload(
        &self,
        ctx: &SyncContext,
        file: &LocalEntry,
        metadata: &serde_json::Value,
        mime_type: &str,
        existing_id: Option<&str>,
    ) -> Result<(), ProviderError> {
        let total = file.size;
        let token = ctx.oauth_access_token().await?;
        let init = send_with_retry(Some(ctx), "Start upload", || {
            self.upload_request(existing_id, "resumable", &token)
                .header("Content-Type", "application/json; charset=UTF-8")
                .header("X-Upload-Content-Type", mime_type)
                .header("X-Upload-Content-Length", total)
                .json(metadata)
        })
        .await?;
        if !init.status().is_success() {
            let status = init.status();
            let body = init.text().await.unwrap_or_default();
            return Err(http_error("Start upload", status, &body));
        }
        let session_url = init
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ProviderError::TransferError("No upload URL in response".to_string()))?
            .to_string();

        // Upload chunks read from disk, so memory use stays at one chunk.
        let mut offset: u64 = 0;
        let mut failures = 0u32;
        while offset < total {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let len = CHUNK_SIZE.min(total - offset);
            let chunk = read_chunk(&file.abs, offset, len as usize).await?;
            let range = format!("bytes {}-{}/{}", offset, offset + len - 1, total);

            let sent = self
                .client
                .put(&session_url)
                .header("Content-Length", len)
                .header("Content-Range", range)
                .body(chunk)
                .send()
                .await;

            match sent {
                Ok(resp) if resp.status().is_success() => return Ok(()),
                Ok(resp) if resp.status().as_u16() == 308 => {
                    // The server says how much it stored; continue from there.
                    // No Range header means nothing was stored yet.
                    offset = committed_offset(resp.headers()).unwrap_or(0);
                    failures = 0;
                }
                Ok(resp) if resp.status().as_u16() == 404 || resp.status().as_u16() == 410 => {
                    return Err(ProviderError::TransferError("Upload session expired".to_string()));
                }
                Ok(resp) if !common::is_retryable_status(resp.status().as_u16()) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(http_error("Chunk upload", status, &body));
                }
                other => {
                    failures += 1;
                    if failures > MAX_CHUNK_RETRIES {
                        let reason = match other {
                            Ok(r) => format!("HTTP {}", r.status()),
                            Err(e) => e.to_string(),
                        };
                        return Err(ProviderError::TransferError(format!("Chunk upload failed: {}", reason)));
                    }
                    let delay = std::time::Duration::from_secs((1u64 << failures).min(60));
                    ctx.log_warning(
                        &format!("Chunk upload failed, resuming in {}s", delay.as_secs()),
                        "googledrive",
                    )
                    .await;
                    tokio::time::sleep(delay).await;
                    offset = self.query_upload_status(&session_url, total).await?.unwrap_or(offset);
                }
            }
        }

        // All bytes were accepted but no final response was seen yet
        match self.query_upload_status(&session_url, total).await? {
            None => Ok(()),
            Some(n) => Err(ProviderError::TransferError(format!(
                "Upload incomplete: server has {} of {} bytes",
                n, total
            ))),
        }
    }

    /// Ask the server how many bytes of a resumable upload it has.
    /// Returns `None` when the upload is already complete.
    async fn query_upload_status(&self, session_url: &str, total: u64) -> Result<Option<u64>, ProviderError> {
        let resp = self
            .client
            .put(session_url)
            .header("Content-Length", 0)
            .header("Content-Range", format!("bytes */{}", total))
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionError(format!("Upload status check failed: {}", e)))?;
        if resp.status().is_success() {
            Ok(None)
        } else if resp.status().as_u16() == 308 {
            Ok(Some(committed_offset(resp.headers()).unwrap_or(0)))
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(http_error("Upload status check", status, &body))
        }
    }
}

/// Next byte offset from a resumable-upload `Range: bytes=0-N` header.
fn committed_offset(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let range = headers.get(reqwest::header::RANGE)?.to_str().ok()?;
    let end = range.trim().strip_prefix("bytes=")?.split('-').nth(1)?;
    end.trim().parse::<u64>().ok().map(|n| n + 1)
}

/// Split a relative path into (parent, name).
fn split_rel(rel: &str) -> (&str, &str) {
    match rel.rfind('/') {
        Some(i) => (&rel[..i], &rel[i + 1..]),
        None => ("", rel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_values_are_escaped() {
        assert_eq!(escape_query_value(r"it's a\b"), r"it\'s a\\b");
    }

    #[test]
    fn parent_folder_prefers_folder_then_shared_drive() {
        assert_eq!(parent_folder_id("abc", &Some("drive".into())), "abc");
        assert_eq!(parent_folder_id("", &Some("drive".into())), "drive");
        assert_eq!(parent_folder_id(" ", &None), "root");
    }

    #[test]
    fn range_header_gives_next_offset() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::RANGE, "bytes=0-8388607".parse().unwrap());
        assert_eq!(committed_offset(&h), Some(8_388_608));
        assert_eq!(committed_offset(&reqwest::header::HeaderMap::new()), None);
    }

    #[test]
    fn chunk_size_is_multiple_of_256k() {
        assert_eq!(CHUNK_SIZE % (256 * 1024), 0);
    }

    #[test]
    fn split_relative_paths() {
        assert_eq!(split_rel("a/b/c.txt"), ("a/b", "c.txt"));
        assert_eq!(split_rel("c.txt"), ("", "c.txt"));
    }
}

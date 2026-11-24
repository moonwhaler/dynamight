//! OneDrive sync provider using Microsoft Graph API
//!
//! Sync is incremental: the destination folder tree is listed once, files of
//! the same size that were modified at or after the local file are skipped,
//! and changed files are replaced. With "delete extraneous" enabled, remote
//! items that no longer exist locally are deleted (they go to the OneDrive
//! recycle bin).
//!
//! Items are addressed by ID after the target folder is resolved; names that
//! go into URLs are percent-encoded, so names with `#`, `%` or spaces work.
//!
//! API references:
//! - Upload sessions: <https://learn.microsoft.com/graph/api/driveitem-createuploadsession>
//! - Throttling: <https://learn.microsoft.com/graph/throttling>
//! - Invalid names: <https://support.microsoft.com/office/restrictions-and-limitations-in-onedrive-and-sharepoint-64883a5d-228e-48f5-b3d2-eb39e07630fa>

use super::common::{self, needs_upload, parse_rfc3339, read_chunk, rel_excluded, send_with_retry, LocalEntry, RemoteFile};
use super::{ProviderCapabilities, ProviderError, StorageInfo, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

const GRAPH_API_BASE: &str = "https://graph.microsoft.com/v1.0";
/// Files up to this size use a single PUT request
const SIMPLE_UPLOAD_LIMIT: u64 = 4 * 1024 * 1024;
/// Upload session chunk size: must be a multiple of 320 KiB and below 60 MiB
const CHUNK_SIZE: u64 = 32 * 320 * 1024;
const MAX_CHUNK_RETRIES: u32 = 6;

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
struct DriveItem {
    id: String,
    name: String,
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    folder: Option<serde_json::Value>,
    #[serde(default)]
    last_modified_date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ItemList {
    #[serde(default)]
    value: Vec<DriveItem>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadSession {
    upload_url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadStatus {
    #[serde(default)]
    next_expected_ranges: Vec<String>,
}

/// Percent-encode one path segment for Graph path-based addressing.
fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// Why OneDrive would reject this file or folder name, if it would.
fn invalid_name_reason(name: &str) -> Option<&'static str> {
    const RESERVED: &[&str] = &[
        ".lock", "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        "desktop.ini",
    ];
    if name.chars().any(|c| matches!(c, '"' | '*' | ':' | '<' | '>' | '?' | '/' | '\\' | '|')) {
        return Some("contains a character OneDrive does not allow (\" * : < > ? / \\ |)");
    }
    if name.starts_with(' ') || name.ends_with(' ') {
        return Some("starts or ends with a space");
    }
    if name.ends_with('.') {
        return Some("ends with a period");
    }
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(name)) {
        return Some("is a reserved name");
    }
    if name.starts_with("~$") || name.contains("_vti_") {
        return Some("is a reserved name pattern (~$ or _vti_)");
    }
    None
}

fn drive_base(drive_id: &Option<String>) -> String {
    match drive_id.as_deref().filter(|d| !d.trim().is_empty()) {
        Some(id) => format!("{}/drives/{}", GRAPH_API_BASE, encode_segment(id.trim())),
        None => format!("{}/me/drive", GRAPH_API_BASE),
    }
}

fn access_token_of(credential: Option<&CredentialData>) -> Result<String, ProviderError> {
    match credential {
        Some(CredentialData::OAuth { access_token, .. }) if !access_token.is_empty() => Ok(access_token.clone()),
        Some(CredentialData::OAuth { .. }) => Err(ProviderError::CredentialError(
            "No access token. Set the OAuth client ID so a token can be fetched with the refresh token.".to_string(),
        )),
        _ => Err(ProviderError::CredentialError("OAuth credentials required".to_string())),
    }
}

fn http_error(what: &str, status: reqwest::StatusCode, body: &str) -> ProviderError {
    let body: String = body.chars().take(500).collect();
    match status.as_u16() {
        401 => ProviderError::CredentialError(format!("{}: authentication failed (HTTP 401). {}", what, body)),
        403 => ProviderError::CredentialError(format!("{}: access denied (HTTP 403). {}", what, body)),
        _ => ProviderError::TransferError(format!("{}: HTTP {} - {}", what, status, body)),
    }
}

/// Remote state of a destination folder tree, keyed by relative path.
#[derive(Default)]
struct RemoteTree {
    folders: HashMap<String, String>,
    files: HashMap<String, RemoteFile>,
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
                if folder_path.trim_matches('/').trim().is_empty() {
                    return Err(ProviderError::ConfigError("OneDrive folder path is required".to_string()));
                }
                for segment in folder_path.split('/').filter(|s| !s.is_empty()) {
                    if let Some(reason) = invalid_name_reason(segment) {
                        return Err(ProviderError::ConfigError(format!(
                            "Folder name '{}' {}",
                            segment, reason
                        )));
                    }
                }
            }
            _ => {
                return Err(ProviderError::ConfigError(
                    "OneDriveProvider only supports OneDrive destination".to_string(),
                ))
            }
        }

        match credential {
            Some(CredentialData::OAuth { access_token, refresh_token, client_id, .. }) => {
                if access_token.is_empty() && (refresh_token.is_empty() || client_id.is_none()) {
                    return Err(ProviderError::CredentialError(
                        "OneDrive needs an access token, or a refresh token with a client ID".to_string(),
                    ));
                }
            }
            None => return Err(ProviderError::CredentialError("OneDrive credentials are required".to_string())),
            _ => return Err(ProviderError::CredentialError("Invalid credential type for OneDrive".to_string())),
        }

        Ok(())
    }

    async fn test_connection(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<TestConnectionResult, ProviderError> {
        self.validate_config(destination, credential)?;

        let (folder_path, drive_id) = match destination {
            DestinationConfig::OneDrive { folder_path, drive_id } => (folder_path.clone(), drive_id.clone()),
            _ => return Err(ProviderError::ConfigError("Invalid destination type".to_string())),
        };
        let access_token = access_token_of(credential)?;
        let base = drive_base(&drive_id);

        let response = send_with_retry(None, "OneDrive connection test", || {
            self.client.get(&base).bearer_auth(&access_token)
        })
        .await?;

        match response.status().as_u16() {
            200 => {
                #[derive(Deserialize)]
                struct DriveInfo {
                    name: Option<String>,
                    owner: Option<OwnerInfo>,
                }
                #[derive(Deserialize)]
                struct OwnerInfo {
                    user: Option<UserInfo>,
                }
                #[derive(Deserialize)]
                struct UserInfo {
                    #[serde(rename = "displayName")]
                    display_name: Option<String>,
                }

                let drive_info = response
                    .json::<DriveInfo>()
                    .await
                    .map(|r| {
                        let drive_name = r.name.unwrap_or_else(|| "OneDrive".to_string());
                        let owner = r.owner.and_then(|o| o.user).and_then(|u| u.display_name).unwrap_or_default();
                        if !owner.is_empty() {
                            format!("{} ({})", drive_name, owner)
                        } else {
                            drive_name
                        }
                    })
                    .unwrap_or_else(|_| "OneDrive".to_string());

                Ok(TestConnectionResult {
                    success: true,
                    message: "Successfully connected to OneDrive".to_string(),
                    details: Some(format!("Drive: {}. Folder: /{}", drive_info, folder_path.trim_matches('/'))),
                    host_key_fingerprint: None,
                })
            }
            401 => Err(ProviderError::CredentialError(
                "Authentication failed. The access token is invalid or expired; check the refresh token and client ID."
                    .to_string(),
            )),
            403 => Err(ProviderError::CredentialError(
                "Access forbidden. Check your OAuth scopes and permissions.".to_string(),
            )),
            404 => Err(ProviderError::ConfigError("Drive not found. Check the drive ID.".to_string())),
            status => Err(ProviderError::ConnectionError(format!(
                "Microsoft Graph API returned status {}",
                status
            ))),
        }
    }

    async fn sync(&self, ctx: Arc<SyncContext>) -> Result<SyncResult, ProviderError> {
        let mut result = SyncResult::default();

        let (folder_path, drive_id) = match &ctx.destination {
            DestinationConfig::OneDrive { folder_path, drive_id } => (folder_path.clone(), drive_id.clone()),
            _ => {
                return Err(ProviderError::ConfigError(
                    "OneDriveProvider only supports OneDrive destination".to_string(),
                ))
            }
        };
        let base = drive_base(&drive_id);

        ctx.log_info("Starting OneDrive sync", "onedrive").await;
        ctx.log_info(&format!("Target folder: {}", folder_path), "onedrive").await;

        // Verify access (and renew the token if needed)
        let token = ctx.oauth_access_token().await?;
        let response = send_with_retry(Some(&ctx), "OneDrive connection check", || {
            self.client.get(&base).bearer_auth(&token)
        })
        .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            ctx.log_error(&format!("Connection failed: {} - {}", status, body), "onedrive").await;
            return Err(http_error("Connection check", status, &body));
        }
        ctx.log_info("Connection successful", "onedrive").await;

        // Resolve (or create) the target folder
        let target_path = folder_path.trim_matches('/').to_string();
        let target_id = self.resolve_folder(&ctx, &base, &target_path, !ctx.options.dry_run).await?;

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

            ctx.log_info(&format!("Syncing {} -> /{}/{}", source_dir, target_path, dest_name), "onedrive").await;

            match self
                .sync_source(&ctx, &base, target_id.as_deref(), source_dir, &dest_name, &mut result)
                .await
            {
                Ok(()) => {}
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "onedrive").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "OneDrive sync complete. Files: {}, Bytes: {}, Deleted: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.files_deleted, result.error_count
            ),
            "onedrive",
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
        let drive_id = match destination {
            DestinationConfig::OneDrive { drive_id, .. } => drive_id.clone(),
            _ => return Ok(StorageInfo::default()),
        };

        let response = self.client.get(drive_base(&drive_id)).bearer_auth(&access_token).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                #[derive(Deserialize)]
                struct DriveResponse {
                    quota: Option<Quota>,
                }
                #[derive(Deserialize)]
                struct Quota {
                    total: Option<u64>,
                    remaining: Option<u64>,
                }

                match resp.json::<DriveResponse>().await {
                    Ok(DriveResponse { quota: Some(quota) }) => Ok(StorageInfo {
                        free: quota.remaining,
                        total: quota.total,
                        supported: true,
                    }),
                    _ => Ok(StorageInfo::default()),
                }
            }
            _ => Ok(StorageInfo::default()),
        }
    }
}

impl OneDriveProvider {
    /// Resolve a folder path below the drive root to its item ID, creating
    /// missing folders when `create` is set. Returns `None` if it does not
    /// exist and was not created (dry run).
    async fn resolve_folder(
        &self,
        ctx: &SyncContext,
        base: &str,
        path: &str,
        create: bool,
    ) -> Result<Option<String>, ProviderError> {
        let token = ctx.oauth_access_token().await?;
        let root = send_with_retry(Some(ctx), "Get drive root", || {
            self.client.get(format!("{}/root", base)).query(&[("$select", "id")]).bearer_auth(&token)
        })
        .await?;
        if !root.status().is_success() {
            let status = root.status();
            let body = root.text().await.unwrap_or_default();
            return Err(http_error("Get drive root", status, &body));
        }
        let mut current: String = root
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v["id"].as_str().map(String::from))
            .ok_or_else(|| ProviderError::ConnectionError("Invalid drive root response".to_string()))?;

        for segment in path.split('/').filter(|s| !s.is_empty()) {
            match self.find_child(ctx, base, &current, segment).await? {
                Some(item) if item.folder.is_some() => current = item.id,
                Some(_) => {
                    return Err(ProviderError::ConfigError(format!(
                        "'{}' exists in OneDrive but is not a folder",
                        segment
                    )))
                }
                None if create => {
                    ctx.log_info(&format!("Created folder: {}", segment), "onedrive").await;
                    current = self.create_folder(ctx, base, &current, segment).await?;
                }
                None => return Ok(None),
            }
        }
        Ok(Some(current))
    }

    async fn find_child(
        &self,
        ctx: &SyncContext,
        base: &str,
        parent_id: &str,
        name: &str,
    ) -> Result<Option<DriveItem>, ProviderError> {
        let token = ctx.oauth_access_token().await?;
        let url = format!("{}/items/{}:/{}", base, encode_segment(parent_id), encode_segment(name));
        let resp = send_with_retry(Some(ctx), "Look up item", || {
            self.client
                .get(&url)
                .query(&[("$select", "id,name,size,folder,lastModifiedDateTime")])
                .bearer_auth(&token)
        })
        .await?;
        match resp.status().as_u16() {
            404 => Ok(None),
            s if (200..300).contains(&s) => resp
                .json::<DriveItem>()
                .await
                .map(Some)
                .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e))),
            _ => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                Err(http_error("Look up item", status, &body))
            }
        }
    }

    async fn create_folder(&self, ctx: &SyncContext, base: &str, parent_id: &str, name: &str) -> Result<String, ProviderError> {
        let body = serde_json::json!({
            "name": name,
            "folder": {},
            // Never replace an existing folder (that would delete its contents)
            "@microsoft.graph.conflictBehavior": "fail",
        });
        let token = ctx.oauth_access_token().await?;
        let url = format!("{}/items/{}/children", base, encode_segment(parent_id));
        let resp = send_with_retry(Some(ctx), "Create folder", || {
            self.client.post(&url).bearer_auth(&token).json(&body)
        })
        .await?;

        match resp.status().as_u16() {
            s if (200..300).contains(&s) => resp
                .json::<DriveItem>()
                .await
                .map(|i| i.id)
                .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e))),
            // Created concurrently (or listing was stale): use the existing folder
            409 => match self.find_child(ctx, base, parent_id, name).await? {
                Some(item) if item.folder.is_some() => Ok(item.id),
                _ => Err(ProviderError::TransferError(format!(
                    "Cannot create folder '{}': a file with that name exists",
                    name
                ))),
            },
            _ => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                Err(http_error(&format!("Create folder {}", name), status, &body))
            }
        }
    }

    /// List the children of a folder (all pages).
    async fn list_children(&self, ctx: &SyncContext, base: &str, folder_id: &str) -> Result<Vec<DriveItem>, ProviderError> {
        let mut out = Vec::new();
        let mut next: Option<String> = None;
        loop {
            let token = ctx.oauth_access_token().await?;
            let first_url = format!("{}/items/{}/children", base, encode_segment(folder_id));
            let resp = send_with_retry(Some(ctx), "List folder", || match &next {
                Some(link) => self.client.get(link).bearer_auth(&token),
                None => self
                    .client
                    .get(&first_url)
                    .query(&[("$select", "id,name,size,folder,lastModifiedDateTime"), ("$top", "999")])
                    .bearer_auth(&token),
            })
            .await?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(http_error("List folder", status, &body));
            }
            let page: ItemList = resp
                .json()
                .await
                .map_err(|e| ProviderError::ConnectionError(format!("Invalid response: {}", e)))?;
            out.extend(page.value);
            match page.next_link {
                Some(link) if !link.is_empty() => next = Some(link),
                _ => break,
            }
        }
        Ok(out)
    }

    async fn list_tree(&self, ctx: &SyncContext, base: &str, root_id: &str) -> Result<RemoteTree, ProviderError> {
        let mut tree = RemoteTree::default();
        tree.folders.insert(String::new(), root_id.to_string());
        let mut queue = vec![(String::new(), root_id.to_string())];
        while let Some((prefix, id)) = queue.pop() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            for child in self.list_children(ctx, base, &id).await? {
                let rel = if prefix.is_empty() { child.name.clone() } else { format!("{}/{}", prefix, child.name) };
                if child.folder.is_some() {
                    tree.folders.insert(rel.clone(), child.id.clone());
                    queue.push((rel, child.id));
                } else {
                    tree.files.insert(
                        rel,
                        RemoteFile {
                            size: child.size.unwrap_or(0),
                            mtime: child.last_modified_date_time.as_deref().and_then(parse_rfc3339).unwrap_or(0),
                            id: Some(child.id),
                        },
                    );
                }
            }
        }
        Ok(tree)
    }

    async fn sync_source(
        &self,
        ctx: &SyncContext,
        base: &str,
        target_id: Option<&str>,
        source_dir: &str,
        dest_name: &str,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        let dry_run = ctx.options.dry_run;
        let local = common::walk_source_async(source_dir, &ctx.options).await?;
        for msg in &local.skipped {
            ctx.log_warning(msg, "onedrive").await;
        }

        // Destination folder for this source
        let root_id = match target_id {
            Some(target) => match self.find_child(ctx, base, target, dest_name).await? {
                Some(item) if item.folder.is_some() => Some(item.id),
                Some(_) => {
                    return Err(ProviderError::ConfigError(format!(
                        "'{}' exists in the target folder but is not a folder",
                        dest_name
                    )))
                }
                None if dry_run => None,
                None => {
                    ctx.log_info(&format!("Created folder: {}", dest_name), "onedrive").await;
                    Some(self.create_folder(ctx, base, target, dest_name).await?)
                }
            },
            None => None,
        };

        let mut remote = match &root_id {
            Some(id) => self.list_tree(ctx, base, id).await?,
            None => RemoteTree::default(),
        };

        // Names OneDrive rejects are skipped (with everything below them)
        let mut invalid_dirs: Vec<String> = Vec::new();
        let is_under_invalid =
            |rel: &str, invalid: &[String]| invalid.iter().any(|d| rel.starts_with(&format!("{}/", d)));

        for dir in local.dirs() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            if is_under_invalid(&dir.rel, &invalid_dirs) {
                continue;
            }
            let (parent_rel, name) = split_rel(&dir.rel);
            if let Some(reason) = invalid_name_reason(name) {
                ctx.log_warning(&format!("Skipping folder '{}': name {}", dir.rel, reason), "onedrive").await;
                invalid_dirs.push(dir.rel.clone());
                continue;
            }
            if remote.folders.contains_key(&dir.rel) {
                continue;
            }
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would create folder: {}/{}", dest_name, dir.rel), "onedrive").await;
                continue;
            }
            let Some(parent) = remote.folders.get(parent_rel).cloned() else {
                continue;
            };
            let id = self.create_folder(ctx, base, &parent, name).await?;
            remote.folders.insert(dir.rel.clone(), id);
        }

        let mut skipped = 0u64;
        for file in local.files() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            if is_under_invalid(&file.rel, &invalid_dirs) {
                continue;
            }
            let (parent_rel, name) = split_rel(&file.rel);
            if let Some(reason) = invalid_name_reason(name) {
                ctx.log_warning(&format!("Skipping file '{}': name {}", file.rel, reason), "onedrive").await;
                continue;
            }
            let existing = remote.files.get(&file.rel);
            if !needs_upload(file, existing) {
                skipped += 1;
                continue;
            }
            let label = format!("{}/{}", dest_name, file.rel);
            if dry_run {
                let verb = if existing.is_some() { "update" } else { "upload" };
                ctx.log_info(&format!("[DRY RUN] Would {}: {}", verb, label), "onedrive").await;
                result.files_transferred += 1;
                result.bytes_transferred += file.size as i64;
                continue;
            }
            let Some(parent) = remote.folders.get(parent_rel).cloned() else {
                ctx.log_error(&format!("Missing remote folder for {}", label), "onedrive").await;
                result.error_count += 1;
                continue;
            };
            match self.upload_file(ctx, base, &parent, file).await {
                Ok(()) => {
                    ctx.log_info(&format!("Uploaded: {}", label), "onedrive").await;
                    result.files_transferred += 1;
                    result.bytes_transferred += file.size as i64;
                }
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to upload {}: {}", label, e), "onedrive").await;
                    result.error_count += 1;
                }
            }
        }
        if skipped > 0 {
            ctx.log_info(&format!("{} unchanged file(s) skipped", skipped), "onedrive").await;
        }

        if ctx.options.delete_extraneous {
            let local_paths = local.rel_paths();
            let mut to_delete: Vec<(String, String, bool)> = remote
                .files
                .iter()
                .filter_map(|(rel, f)| f.id.clone().map(|id| (rel.clone(), id, false)))
                .chain(
                    remote
                        .folders
                        .iter()
                        .filter(|(rel, _)| !rel.is_empty())
                        .map(|(rel, id)| (rel.clone(), id.clone(), true)),
                )
                .filter(|(rel, _, _)| !local_paths.contains(rel) && !rel_excluded(source_dir, rel, &ctx.options))
                .collect();
            to_delete.sort();

            let mut deleted_dirs: HashSet<String> = HashSet::new();
            for (rel, id, is_dir) in to_delete {
                if ctx.check_cancelled() {
                    return Err(ProviderError::Cancelled);
                }
                if deleted_dirs.iter().any(|d| rel.starts_with(&format!("{}/", d))) {
                    continue;
                }
                let label = format!("{}/{}", dest_name, rel);
                if dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would delete: {}", label), "onedrive").await;
                    result.files_deleted += 1;
                    continue;
                }
                match self.delete_item(ctx, base, &id).await {
                    Ok(()) => {
                        ctx.log_info(&format!("Deleted (moved to recycle bin): {}", label), "onedrive").await;
                        result.files_deleted += 1;
                        if is_dir {
                            deleted_dirs.insert(rel);
                        }
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to delete {}: {}", label, e), "onedrive").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    async fn delete_item(&self, ctx: &SyncContext, base: &str, item_id: &str) -> Result<(), ProviderError> {
        let token = ctx.oauth_access_token().await?;
        let url = format!("{}/items/{}", base, encode_segment(item_id));
        let resp = send_with_retry(Some(ctx), "Delete item", || self.client.delete(&url).bearer_auth(&token)).await?;
        if resp.status().is_success() || resp.status().as_u16() == 404 {
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            Err(http_error("Delete item", status, &body))
        }
    }

    async fn upload_file(&self, ctx: &SyncContext, base: &str, parent_id: &str, file: &LocalEntry) -> Result<(), ProviderError> {
        let name = split_rel(&file.rel).1;
        let item_path = format!("{}/items/{}:/{}:", base, encode_segment(parent_id), encode_segment(name));

        if file.size <= SIMPLE_UPLOAD_LIMIT {
            let data = bytes::Bytes::from(tokio::fs::read(&file.abs).await?);
            let token = ctx.oauth_access_token().await?;
            let url = format!("{}/content", item_path);
            let resp = send_with_retry(Some(ctx), "Upload", || {
                self.client
                    .put(&url)
                    .query(&[("@microsoft.graph.conflictBehavior", "replace")])
                    .bearer_auth(&token)
                    .header("Content-Type", "application/octet-stream")
                    .body(data.clone())
            })
            .await?;
            return if resp.status().is_success() {
                Ok(())
            } else {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                Err(http_error("Upload", status, &body))
            };
        }

        self.upload_large_file(ctx, &item_path, file).await
    }

    async fn upload_large_file(&self, ctx: &SyncContext, item_path: &str, file: &LocalEntry) -> Result<(), ProviderError> {
        let session_body = serde_json::json!({
            "item": { "@microsoft.graph.conflictBehavior": "replace" }
        });
        let token = ctx.oauth_access_token().await?;
        let url = format!("{}/createUploadSession", item_path);
        let resp = send_with_retry(Some(ctx), "Create upload session", || {
            self.client.post(&url).bearer_auth(&token).json(&session_body)
        })
        .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(http_error("Create upload session", status, &body));
        }
        let session: UploadSession = resp
            .json()
            .await
            .map_err(|e| ProviderError::TransferError(format!("Invalid upload session response: {}", e)))?;

        let result = self.upload_chunks(ctx, &session.upload_url, file).await;
        if result.is_err() {
            // Cancel the session so the partial upload does not linger
            let _ = self.client.delete(&session.upload_url).send().await;
        }
        result
    }

    /// Send the file in chunks read from disk. The upload URL is pre-authenticated:
    /// it must not get an Authorization header.
    async fn upload_chunks(&self, ctx: &SyncContext, upload_url: &str, file: &LocalEntry) -> Result<(), ProviderError> {
        let total = file.size;
        let mut offset: u64 = 0;
        let mut failures = 0u32;

        while offset < total {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let len = CHUNK_SIZE.min(total - offset);
            let chunk = read_chunk(&file.abs, offset, len as usize).await?;
            let sent = self
                .client
                .put(upload_url)
                .header("Content-Length", len)
                .header("Content-Range", format!("bytes {}-{}/{}", offset, offset + len - 1, total))
                .body(chunk)
                .send()
                .await;

            match sent {
                // 200/201: upload complete
                Ok(resp) if resp.status().as_u16() == 200 || resp.status().as_u16() == 201 => return Ok(()),
                // 202: chunk accepted; the server says which range comes next
                Ok(resp) if resp.status().as_u16() == 202 => {
                    let status: Option<UploadStatus> = resp.json().await.ok();
                    offset = status
                        .and_then(|s| next_expected_offset(&s.next_expected_ranges))
                        .unwrap_or(offset + len);
                    failures = 0;
                }
                Ok(resp) if resp.status().as_u16() == 404 => {
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
                    let delay = match &other {
                        Ok(r) => r
                            .headers()
                            .get(reqwest::header::RETRY_AFTER)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(1u64 << failures),
                        Err(_) => 1u64 << failures,
                    }
                    .min(120);
                    ctx.log_warning(&format!("Chunk upload failed, resuming in {}s", delay), "onedrive").await;
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;

                    // Ask the session which bytes it still needs
                    if let Ok(resp) = self.client.get(upload_url).send().await {
                        if resp.status().is_success() {
                            if let Ok(s) = resp.json::<UploadStatus>().await {
                                if let Some(next) = next_expected_offset(&s.next_expected_ranges) {
                                    offset = next;
                                }
                            }
                        } else if resp.status().as_u16() == 404 {
                            return Err(ProviderError::TransferError("Upload session expired".to_string()));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Start of the first range in `nextExpectedRanges` (e.g. `["12345-"]`).
fn next_expected_offset(ranges: &[String]) -> Option<u64> {
    ranges.first()?.split('-').next()?.trim().parse().ok()
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
    fn special_characters_are_percent_encoded() {
        assert_eq!(encode_segment("my file#1 100%.txt"), "my%20file%231%20100%25.txt");
        assert_eq!(encode_segment("Bilder ä"), "Bilder%20%C3%A4");
    }

    #[test]
    fn invalid_names_are_detected() {
        assert!(invalid_name_reason("ok name.txt").is_none());
        assert!(invalid_name_reason("what?.txt").is_some());
        assert!(invalid_name_reason("trailing.").is_some());
        assert!(invalid_name_reason(" lead").is_some());
        assert!(invalid_name_reason("CON").is_some());
        assert!(invalid_name_reason("con.txt").is_none());
        assert!(invalid_name_reason("desktop.ini").is_some());
        assert!(invalid_name_reason("~$draft.docx").is_some());
        assert!(invalid_name_reason("console.log").is_none());
    }

    // Graph limits: chunks must be a multiple of 320 KiB and below 60 MiB
    #[allow(clippy::manual_is_multiple_of)]
    const _: () = assert!(CHUNK_SIZE % (320 * 1024) == 0 && CHUNK_SIZE < 60 * 1024 * 1024);

    #[test]
    fn next_expected_range_parsed() {
        assert_eq!(next_expected_offset(&["10485760-".to_string()]), Some(10_485_760));
        assert_eq!(next_expected_offset(&["0-99".to_string(), "200-".to_string()]), Some(0));
        assert_eq!(next_expected_offset(&[]), None);
    }
}

//! WebDAV sync provider (Nextcloud, ownCloud, etc.)

use super::common::{self, needs_upload, parse_http_date, rel_excluded, send_with_retry, LocalEntry, RemoteFile};
use super::{ProviderCapabilities, ProviderError, StorageInfo, SyncContext, SyncProvider, SyncResult, TestConnectionResult};
use crate::models::{CredentialData, DestinationConfig};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub struct WebDavProvider {
    client: Client,
    /// Client for sync requests: no total timeout, since a streamed upload of
    /// a large file can take longer than any fixed limit
    upload_client: Client,
}

impl WebDavProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            upload_client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(30))
                .read_timeout(std::time::Duration::from_secs(300))
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
        let test_url = collection_url(&base_url, &remote_path);

        // Use PROPFIND to test connection (standard WebDAV method)
        let response = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND")
                    .expect("PROPFIND is a valid HTTP method"),
                &test_url,
            )
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
                host_key_fingerprint: None,
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
                host_key_fingerprint: None,
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
            Some(CredentialData::WebDav { username, password }) => (username.clone(), password.clone()),
            _ => return Err(ProviderError::CredentialError("WebDAV credentials required".to_string())),
        };

        ctx.log_info("Starting WebDAV sync", "webdav").await;
        ctx.log_info(&format!("Server: {}", base_url), "webdav").await;

        let dav = Dav { client: &self.upload_client, base_url: &base_url, username: &username, password: &password };

        // Test connection
        let response = send_with_retry(Some(&ctx), "WebDAV connection check", || {
            dav.request("PROPFIND", remote_path.trim_matches('/')).header("Depth", "0")
        })
        .await?;
        match response.status().as_u16() {
            200 | 207 => ctx.log_info("Connection successful", "webdav").await,
            404 => ctx.log_info("Remote path doesn't exist, will create", "webdav").await,
            401 => {
                ctx.log_error("Authentication failed", "webdav").await;
                return Err(ProviderError::CredentialError("Authentication failed".to_string()));
            }
            status => {
                ctx.log_error(&format!("Connection failed: HTTP {}", status), "webdav").await;
                return Err(ProviderError::ConnectionError(format!("HTTP {}", status)));
            }
        }

        if !ctx.options.dry_run {
            dav.mkdir_all(&ctx, remote_path.trim_matches('/')).await?;
        }

        for source_dir in &ctx.source_dirs {
            if ctx.check_cancelled() {
                ctx.log_warning("Sync cancelled by user", "webdav").await;
                return Err(ProviderError::Cancelled);
            }
            if !Path::new(source_dir).exists() {
                ctx.log_warning(&format!("Source directory does not exist: {}", source_dir), "webdav").await;
                result.error_count += 1;
                continue;
            }

            let dest_name = Path::new(source_dir)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "backup".to_string());

            let dest_path = join_path(remote_path.trim_matches('/'), &dest_name);
            ctx.log_info(&format!("Syncing {} -> {}", source_dir, dav.url(&dest_path)), "webdav").await;

            match self.sync_source(&ctx, &dav, source_dir, &dest_path, &mut result).await {
                Ok(()) => {}
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to sync {}: {}", source_dir, e), "webdav").await;
                    result.error_count += 1;
                }
            }
        }

        result.success = result.error_count == 0;
        ctx.log_info(
            &format!(
                "WebDAV sync complete. Files: {}, Bytes: {}, Deleted: {}, Errors: {}",
                result.files_transferred, result.bytes_transferred, result.files_deleted, result.error_count
            ),
            "webdav",
        )
        .await;

        Ok(result)
    }

    async fn get_storage_info(
        &self,
        destination: &DestinationConfig,
        credential: Option<&CredentialData>,
    ) -> Result<StorageInfo, ProviderError> {
        let (base_url, remote_path) = match destination {
            DestinationConfig::WebDav { url, remote_path } => (url.clone(), remote_path.clone()),
            _ => return Ok(StorageInfo::default()),
        };

        let (username, password) = match credential {
            Some(CredentialData::WebDav { username, password }) => (username.clone(), password.clone()),
            _ => return Ok(StorageInfo::default()),
        };

        let check_url = collection_url(&base_url, &remote_path);

        // RFC 4331 quota properties
        let propfind_body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:quota-available-bytes/>
    <D:quota-used-bytes/>
  </D:prop>
</D:propfind>"#;

        let response = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").expect("PROPFIND is a valid HTTP method"),
                &check_url,
            )
            .basic_auth(&username, Some(&password))
            .header("Depth", "0")
            .header("Content-Type", "application/xml")
            .body(propfind_body)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 207 => {
                let body = resp.text().await.unwrap_or_default();

                // Simple XML parsing for quota values
                let available = extract_xml_value(&body, "quota-available-bytes");
                let used = extract_xml_value(&body, "quota-used-bytes");

                match (available, used) {
                    (Some(avail), Some(u)) => {
                        let total = avail.saturating_add(u);
                        Ok(StorageInfo {
                            free: Some(avail),
                            total: Some(total),
                            supported: true,
                        })
                    }
                    (Some(avail), None) => Ok(StorageInfo {
                        free: Some(avail),
                        total: None,
                        supported: true,
                    }),
                    _ => Ok(StorageInfo::default()),
                }
            }
            _ => Ok(StorageInfo::default()),
        }
    }
}

/// Simple XML value extraction helper for WebDAV quota properties
fn extract_xml_value(xml: &str, tag: &str) -> Option<u64> {
    // Look for <D:tag>value</D:tag> or <tag>value</tag> or <d:tag>value</d:tag>
    let patterns = [
        format!(r"<D:{}[^>]*>(\d+)</D:{}>", tag, tag),
        format!(r"<d:{}[^>]*>(\d+)</d:{}>", tag, tag),
        format!(r"<{}[^>]*>(\d+)</{}>", tag, tag),
    ];

    for pattern in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(xml) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().parse().ok();
                }
            }
        }
    }
    None
}

/// URL of the remote path below the base URL, percent-encoded per segment
/// (the same encoding sync uses, so names with `#`, `%` or spaces work).
fn collection_url(base_url: &str, remote_path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let encoded = encode_path(remote_path);
    if encoded.is_empty() {
        base.to_string()
    } else {
        format!("{}/{}", base, encoded)
    }
}

/// Join two `/`-separated relative paths.
fn join_path(a: &str, b: &str) -> String {
    match (a.trim_matches('/'), b.trim_matches('/')) {
        ("", b) => b.to_string(),
        (a, "") => a.to_string(),
        (a, b) => format!("{}/{}", a, b),
    }
}

/// Percent-encode a relative path segment by segment for use in a URL.
fn encode_path(path: &str) -> String {
    path.split('/')
        .filter(|s| !s.is_empty())
        .map(|seg| {
            let mut out = String::new();
            for b in seg.bytes() {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                    out.push(b as char);
                } else {
                    out.push_str(&format!("%{:02X}", b));
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Decode percent-encoding in an href.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// One entry of a PROPFIND multistatus response.
#[derive(Debug, Default, Clone, PartialEq)]
struct DavEntry {
    href: String,
    is_collection: bool,
    size: u64,
    mtime: i64,
}

/// Parse a PROPFIND multistatus body (namespace prefixes vary by server).
fn parse_multistatus(xml: &str) -> Vec<DavEntry> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut entries = Vec::new();
    let mut current: Option<DavEntry> = None;
    let mut field: Option<&'static str> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"response" => current = Some(DavEntry::default()),
                b"href" => field = Some("href"),
                b"getcontentlength" => field = Some("size"),
                b"getlastmodified" => field = Some("mtime"),
                b"collection" => {
                    if let Some(c) = current.as_mut() {
                        c.is_collection = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(e)) => {
                if e.local_name().as_ref() == b"collection" {
                    if let Some(c) = current.as_mut() {
                        c.is_collection = true;
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(f), Some(c)) = (field, current.as_mut()) {
                    let text = t.unescape().map(|s| s.trim().to_string()).unwrap_or_default();
                    match f {
                        "href" => c.href = text,
                        "size" => c.size = text.parse().unwrap_or(0),
                        "mtime" => c.mtime = parse_http_date(&text).unwrap_or(0),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"response" => {
                    if let Some(c) = current.take() {
                        entries.push(c);
                    }
                }
                b"href" | b"getcontentlength" | b"getlastmodified" => field = None,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    entries
}

/// Last path segment of an href (decoded), ignoring a trailing slash.
fn href_name(href: &str) -> String {
    let path = href.split(['?', '#']).next().unwrap_or("");
    percent_decode(path.trim_end_matches('/').rsplit('/').next().unwrap_or(""))
}

/// WebDAV request helper bound to one server and account.
struct Dav<'a> {
    client: &'a Client,
    base_url: &'a str,
    username: &'a str,
    password: &'a str,
}

impl Dav<'_> {
    fn url(&self, rel: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let encoded = encode_path(rel);
        if encoded.is_empty() {
            format!("{}/", base)
        } else {
            format!("{}/{}", base, encoded)
        }
    }

    fn request(&self, method: &str, rel: &str) -> reqwest::RequestBuilder {
        let method = reqwest::Method::from_bytes(method.as_bytes()).expect("valid WebDAV method");
        self.client
            .request(method, self.url(rel))
            .basic_auth(self.username, Some(self.password))
    }

    /// `mkdir -p` with MKCOL. 405 means the collection already exists.
    async fn mkdir_all(&self, ctx: &SyncContext, rel: &str) -> Result<(), ProviderError> {
        let mut current = String::new();
        for seg in rel.split('/').filter(|s| !s.is_empty()) {
            current = join_path(&current, seg);
            self.mkcol(ctx, &current).await?;
        }
        Ok(())
    }

    async fn mkcol(&self, ctx: &SyncContext, rel: &str) -> Result<(), ProviderError> {
        let resp = send_with_retry(Some(ctx), "Create collection", || self.request("MKCOL", &format!("{}/", rel))).await?;
        match resp.status().as_u16() {
            200..=299 | 405 => Ok(()),
            status => Err(ProviderError::TransferError(format!("MKCOL {} failed: HTTP {}", rel, status))),
        }
    }

    /// List the children of a collection (Depth: 1).
    async fn list(&self, ctx: &SyncContext, rel: &str) -> Result<Option<Vec<DavEntry>>, ProviderError> {
        let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:"><D:prop><D:resourcetype/><D:getcontentlength/><D:getlastmodified/></D:prop></D:propfind>"#;
        let resp = send_with_retry(Some(ctx), "List collection", || {
            self.request("PROPFIND", &format!("{}/", rel))
                .header("Depth", "1")
                .header("Content-Type", "application/xml")
                .body(body)
        })
        .await?;
        match resp.status().as_u16() {
            404 => Ok(None),
            200 | 207 => {
                let text = resp.text().await.unwrap_or_default();
                let mut entries = parse_multistatus(&text);
                // The first response describes the collection itself
                let own = href_name(&format!("/{}", rel));
                if let Some(pos) = entries.iter().position(|e| e.is_collection && href_name(&e.href) == own) {
                    entries.remove(pos);
                } else if !entries.is_empty() {
                    entries.remove(0);
                }
                Ok(Some(entries))
            }
            status => Err(ProviderError::ConnectionError(format!("PROPFIND {} failed: HTTP {}", rel, status))),
        }
    }
}

impl WebDavProvider {
    async fn sync_source(
        &self,
        ctx: &SyncContext,
        dav: &Dav<'_>,
        source_dir: &str,
        dest_path: &str,
        result: &mut SyncResult,
    ) -> Result<(), ProviderError> {
        let dry_run = ctx.options.dry_run;
        let local = common::walk_source_async(source_dir, &ctx.options).await?;
        for msg in &local.skipped {
            ctx.log_warning(msg, "webdav").await;
        }

        // Remote tree (relative to dest_path)
        let mut remote_files: HashMap<String, RemoteFile> = HashMap::new();
        let mut remote_dirs: HashSet<String> = HashSet::new();
        let mut queue = vec![String::new()];
        while let Some(prefix) = queue.pop() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let Some(children) = dav.list(ctx, &join_path(dest_path, &prefix)).await? else {
                continue;
            };
            for child in children {
                let name = href_name(&child.href);
                if name.is_empty() {
                    continue;
                }
                let rel = join_path(&prefix, &name);
                if child.is_collection {
                    remote_dirs.insert(rel.clone());
                    queue.push(rel);
                } else {
                    remote_files.insert(rel, RemoteFile { size: child.size, mtime: child.mtime, id: None });
                }
            }
        }

        if !dry_run {
            dav.mkcol(ctx, dest_path).await?;
        }
        for dir in local.dirs() {
            if remote_dirs.contains(&dir.rel) {
                continue;
            }
            let rel = join_path(dest_path, &dir.rel);
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would create directory: {}", rel), "webdav").await;
                continue;
            }
            if let Err(e) = dav.mkcol(ctx, &rel).await {
                ctx.log_error(&e.to_string(), "webdav").await;
                result.error_count += 1;
            }
        }

        let mut skipped = 0u64;
        for file in local.files() {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            if !needs_upload(file, remote_files.get(&file.rel)) {
                skipped += 1;
                continue;
            }
            let rel = join_path(dest_path, &file.rel);
            if dry_run {
                ctx.log_info(&format!("[DRY RUN] Would upload: {}", rel), "webdav").await;
                result.files_transferred += 1;
                result.bytes_transferred += file.size as i64;
                continue;
            }
            match self.put_file(ctx, dav, &rel, file).await {
                Ok(()) => {
                    ctx.log_info(&format!("Uploaded: {}", rel), "webdav").await;
                    result.files_transferred += 1;
                    result.bytes_transferred += file.size as i64;
                }
                Err(ProviderError::Cancelled) => return Err(ProviderError::Cancelled),
                Err(e) => {
                    ctx.log_error(&format!("Failed to upload {}: {}", rel, e), "webdav").await;
                    result.error_count += 1;
                }
            }
        }
        if skipped > 0 {
            ctx.log_info(&format!("{} unchanged file(s) skipped", skipped), "webdav").await;
        }

        if ctx.options.delete_extraneous {
            let local_paths = local.rel_paths();
            let mut stale: Vec<(String, bool)> = remote_files
                .keys()
                .map(|r| (r.clone(), false))
                .chain(remote_dirs.iter().map(|r| (r.clone(), true)))
                .filter(|(rel, _)| !local_paths.contains(rel) && !rel_excluded(source_dir, rel, &ctx.options))
                .collect();
            stale.sort();

            let mut deleted_dirs: Vec<String> = Vec::new();
            for (rel, is_dir) in stale {
                if ctx.check_cancelled() {
                    return Err(ProviderError::Cancelled);
                }
                if deleted_dirs.iter().any(|d| rel.starts_with(&format!("{}/", d))) {
                    continue;
                }
                let full = join_path(dest_path, &rel);
                if dry_run {
                    ctx.log_info(&format!("[DRY RUN] Would delete: {}", full), "webdav").await;
                    result.files_deleted += 1;
                    continue;
                }
                let target = if is_dir { format!("{}/", full) } else { full.clone() };
                let resp = send_with_retry(Some(ctx), "Delete", || dav.request("DELETE", &target)).await;
                match resp {
                    Ok(r) if r.status().is_success() || r.status().as_u16() == 404 => {
                        ctx.log_info(&format!("Deleted: {}", full), "webdav").await;
                        result.files_deleted += 1;
                        if is_dir {
                            deleted_dirs.push(rel);
                        }
                    }
                    Ok(r) => {
                        ctx.log_error(&format!("Failed to delete {}: HTTP {}", full, r.status()), "webdav").await;
                        result.error_count += 1;
                    }
                    Err(e) => {
                        ctx.log_error(&format!("Failed to delete {}: {}", full, e), "webdav").await;
                        result.error_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    /// Stream a file with PUT. Retries a few times on throttling or server errors.
    async fn put_file(&self, ctx: &SyncContext, dav: &Dav<'_>, rel: &str, file: &LocalEntry) -> Result<(), ProviderError> {
        const ATTEMPTS: u32 = 4;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let handle = tokio::fs::File::open(&file.abs).await?;
            let body = reqwest::Body::wrap_stream(tokio_util::io::ReaderStream::new(handle));
            let sent = dav
                .request("PUT", rel)
                .header(reqwest::header::CONTENT_LENGTH, file.size)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                // Nextcloud/ownCloud keep the original modification time with this header
                .header("X-OC-Mtime", file.mtime.to_string())
                .body(body)
                .send()
                .await;

            let retry = match &sent {
                Ok(r) if r.status().is_success() => return Ok(()),
                Ok(r) => common::is_retryable_status(r.status().as_u16()),
                Err(e) => e.is_connect() || e.is_timeout(),
            };
            if !retry || attempt >= ATTEMPTS {
                return match sent {
                    Ok(r) => Err(ProviderError::TransferError(format!("HTTP {}", r.status()))),
                    Err(e) => Err(ProviderError::ConnectionError(e.to_string())),
                };
            }
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            let delay = 1u64 << attempt;
            ctx.log_warning(&format!("Upload of {} failed, retrying in {}s", rel, delay), "webdav").await;
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_encoded_per_segment() {
        assert_eq!(encode_path("/backup/my photos/#1?.jpg"), "backup/my%20photos/%231%3F.jpg");
        assert_eq!(href_name("/dav/files/u/my%20photos/"), "my photos");
        assert_eq!(href_name("https://h/dav/a%23b.txt"), "a#b.txt");
        assert_eq!(join_path("", "a"), "a");
        assert_eq!(join_path("/x/", "a"), "x/a");
        assert_eq!(collection_url("https://h/dav/", "/my #1/"), "https://h/dav/my%20%231");
        assert_eq!(collection_url("https://h/dav", ""), "https://h/dav");
    }

    #[test]
    fn parses_multistatus_with_any_prefix() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response><d:href>/dav/backup/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response>
  <d:response><d:href>/dav/backup/a%20b.txt</d:href><d:propstat><d:prop><d:resourcetype/><d:getcontentlength>12</d:getcontentlength><d:getlastmodified>Thu, 01 Jan 2026 00:00:00 GMT</d:getlastmodified></d:prop></d:propstat></d:response>
  <d:response><d:href>/dav/backup/sub/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response>
</d:multistatus>"#;
        let entries = parse_multistatus(xml);
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_collection);
        assert_eq!(href_name(&entries[1].href), "a b.txt");
        assert_eq!(entries[1].size, 12);
        assert_eq!(entries[1].mtime, 1767225600);
        assert!(entries[2].is_collection);
    }
}

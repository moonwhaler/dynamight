//! Helpers shared by the remote sync providers (S3, SFTP, WebDAV, Google Drive,
//! OneDrive): walking the local source tree, deciding which files changed, and
//! retrying throttled HTTP requests.

use super::{ProviderError, SyncContext};
use crate::models::SyncOptions;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

/// A file or directory in the local source tree.
#[derive(Debug, Clone)]
pub struct LocalEntry {
    /// Path relative to the source directory, `/`-separated
    pub rel: String,
    pub abs: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    /// Modification time, seconds since the Unix epoch
    pub mtime: i64,
}

/// What a remote listing knows about an existing file.
#[derive(Debug, Clone, Default)]
pub struct RemoteFile {
    pub size: u64,
    /// Modification (or upload) time, seconds since the Unix epoch
    pub mtime: i64,
    /// Provider-specific ID (Drive file ID, OneDrive item ID)
    pub id: Option<String>,
}

/// Result of walking a source directory.
#[derive(Debug, Default)]
pub struct LocalTree {
    /// Directories first (parents before children), then files
    pub entries: Vec<LocalEntry>,
    /// Symlinks and unreadable entries that were skipped
    pub skipped: Vec<String>,
}

impl LocalTree {
    pub fn dirs(&self) -> impl Iterator<Item = &LocalEntry> {
        self.entries.iter().filter(|e| e.is_dir)
    }

    pub fn files(&self) -> impl Iterator<Item = &LocalEntry> {
        self.entries.iter().filter(|e| !e.is_dir)
    }

    /// Relative paths of all entries (files and directories)
    pub fn rel_paths(&self) -> HashSet<String> {
        self.entries.iter().map(|e| e.rel.clone()).collect()
    }
}

/// The exclude rules of a job, compiled once per run instead of once per
/// file and pattern.
pub struct ExcludeMatcher {
    excludes: Vec<glob::Pattern>,
    /// `None`: no include-only list (everything is included)
    include_only: Option<Vec<glob::Pattern>>,
    /// Excluded directories (absolute), without trailing slash
    exclude_dirs: Vec<String>,
}

fn compile(patterns: &[String]) -> Vec<glob::Pattern> {
    patterns.iter().filter_map(|p| glob::Pattern::new(p).ok()).collect()
}

impl ExcludeMatcher {
    pub fn new(options: &SyncOptions) -> Self {
        Self {
            excludes: compile(&options.exclude_patterns),
            include_only: (!options.include_only.is_empty()).then(|| compile(&options.include_only)),
            exclude_dirs: options
                .exclude_dirs
                .iter()
                .map(|d| d.trim_end_matches('/').to_string())
                .collect(),
        }
    }

    /// True if a file name matches one of the exclude patterns.
    pub fn name_excluded(&self, name: &str) -> bool {
        self.excludes.iter().any(|p| p.matches(name))
    }

    /// True if `name` matches one of the include-only globs (or there are none).
    pub fn include_only_matches(&self, name: &str) -> bool {
        self.include_only.as_ref().is_none_or(|list| list.iter().any(|p| p.matches(name)))
    }

    /// True if the absolute path is an excluded directory or lies in one.
    pub fn dir_excluded(&self, abs: &str) -> bool {
        self.exclude_dirs
            .iter()
            .any(|ex| abs == ex || (abs.len() > ex.len() && abs.starts_with(ex.as_str()) && abs.as_bytes()[ex.len()] == b'/'))
    }

    /// True if a path relative to `source_dir` is excluded: one of its
    /// components matches an exclude pattern, or it lies in an excluded
    /// directory.
    ///
    /// Used to keep excluded files on the destination when mirroring deletes,
    /// like rsync `--delete` (which does not delete excluded files).
    pub fn rel_excluded(&self, source_dir: &str, rel: &str) -> bool {
        if rel.split('/').any(|component| self.name_excluded(component)) {
            return true;
        }
        // In compress mode only this job's archives belong to it; never delete others
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if !self.include_only_matches(name) {
            return true;
        }
        self.dir_excluded(&format!("{}/{}", source_dir.trim_end_matches('/'), rel))
    }
}

fn mtime_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Walk a source directory without following symlinks, applying the job's
/// exclude patterns and excluded directories.
pub fn walk_source(source_dir: &str, options: &SyncOptions) -> Result<LocalTree, ProviderError> {
    let root = Path::new(source_dir);
    if !root.is_dir() {
        return Err(ProviderError::ConfigError(format!(
            "Source directory does not exist: {}",
            source_dir
        )));
    }

    let mut tree = LocalTree::default();
    let mut files = Vec::new();
    let excludes = ExcludeMatcher::new(options);

    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            if excludes.name_excluded(&name) {
                return false;
            }
            !(e.file_type().is_dir() && excludes.dir_excluded(&e.path().to_string_lossy()))
        });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tree.skipped.push(format!("Cannot read {}", e));
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }

        let rel_path = match entry.path().strip_prefix(root) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let Some(rel) = rel_path.to_str() else {
            tree.skipped.push(format!("Skipping path with invalid UTF-8: {}", entry.path().display()));
            continue;
        };
        let rel = rel.replace('\\', "/");

        let file_type = entry.file_type();
        if file_type.is_symlink() {
            tree.skipped.push(format!("Skipping symlink: {}", entry.path().display()));
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                tree.skipped.push(format!("Cannot read {}: {}", entry.path().display(), e));
                continue;
            }
        };

        let local = LocalEntry {
            rel,
            abs: entry.path().to_path_buf(),
            is_dir: file_type.is_dir(),
            size: if file_type.is_dir() { 0 } else { meta.len() },
            mtime: mtime_secs(&meta),
        };
        if file_type.is_dir() {
            tree.entries.push(local);
        } else if file_type.is_file() {
            if !excludes.include_only_matches(&entry.file_name().to_string_lossy()) {
                continue;
            }
            files.push(local);
        } else {
            tree.skipped.push(format!("Skipping special file: {}", entry.path().display()));
        }
    }

    tree.entries.extend(files);
    Ok(tree)
}

/// Walk a source directory on a blocking thread.
pub async fn walk_source_async(source_dir: &str, options: &SyncOptions) -> Result<LocalTree, ProviderError> {
    let dir = source_dir.to_string();
    let opts = options.clone();
    tokio::task::spawn_blocking(move || walk_source(&dir, &opts))
        .await
        .map_err(|e| ProviderError::Other(format!("Directory walk failed: {}", e)))?
}

/// Files uploaded at the same time by the remote providers. Small files are
/// bound by round trips, so a few transfers in flight speed them up a lot;
/// Google Drive throttles writes harder than the others.
pub const PARALLEL_UPLOADS: usize = 4;
pub const PARALLEL_UPLOADS_GOOGLE_DRIVE: usize = 3;
/// Folders listed at the same time when reading a remote tree
pub const PARALLEL_LISTINGS: usize = 4;

/// Run `upload` for every item, up to `limit` at a time, and count the
/// results into `result`: `Ok(bytes)` is a transferred file, any other error
/// a failed one (`upload` logs it). Stops at once when the run is cancelled.
pub async fn upload_parallel<T, F, Fut>(
    cancel: &tokio_util::sync::CancellationToken,
    items: Vec<T>,
    limit: usize,
    result: &mut super::SyncResult,
    upload: F,
) -> Result<(), ProviderError>
where
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = Result<u64, ProviderError>>,
{
    use futures::StreamExt;
    let mut uploads = futures::stream::iter(items).map(upload).buffer_unordered(limit.max(1));
    loop {
        let next = tokio::select! {
            next = uploads.next() => next,
            _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
        };
        match next {
            None => return Ok(()),
            Some(Ok(bytes)) => {
                result.files_transferred += 1;
                result.bytes_transferred += bytes as i64;
            }
            Some(Err(ProviderError::Cancelled)) => return Err(ProviderError::Cancelled),
            Some(Err(_)) => result.error_count += 1,
        }
    }
}

/// Walk a remote folder tree with up to `limit` folder listings in flight.
///
/// `list(folder)` lists one folder and returns it with its listing; `visit`
/// takes each listing in turn (one at a time, so it can fill plain maps) and
/// returns the subfolders to list next.
pub async fn walk_remote<K, O, F, Fut>(
    cancel: &tokio_util::sync::CancellationToken,
    root: K,
    limit: usize,
    list: F,
    mut visit: impl FnMut(&K, O) -> Vec<K>,
) -> Result<(), ProviderError>
where
    F: Fn(K) -> Fut,
    Fut: std::future::Future<Output = Result<(K, O), ProviderError>>,
{
    use futures::StreamExt;
    let mut pending = vec![root];
    let mut in_flight = futures::stream::FuturesUnordered::new();
    loop {
        while in_flight.len() < limit.max(1) {
            match pending.pop() {
                Some(folder) => in_flight.push(list(folder)),
                None => break,
            }
        }
        let next = tokio::select! {
            next = in_flight.next() => next,
            _ = cancel.cancelled() => return Err(ProviderError::Cancelled),
        };
        match next {
            None => return Ok(()),
            Some(listed) => {
                let (folder, listing) = listed?;
                pending.extend(visit(&folder, listing));
            }
        }
    }
}

/// Decide whether a local file has to be uploaded.
///
/// A file is skipped when the destination has a file of the same size that
/// was modified (or uploaded) at or after the local modification time. This is
/// the same rule `aws s3 sync` uses, and it works for providers that only
/// report the upload time.
pub fn needs_upload(local: &LocalEntry, remote: Option<&RemoteFile>) -> bool {
    match remote {
        None => true,
        Some(r) => r.size != local.size || r.mtime < local.mtime,
    }
}

/// Unix time from an RFC 3339 timestamp (as used by Drive and Graph).
pub fn parse_rfc3339(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value).ok().map(|d| d.timestamp())
}

/// Unix time from an HTTP date (as used by WebDAV `getlastmodified`).
pub fn parse_http_date(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc2822(value).ok().map(|d| d.timestamp())
}

const MAX_ATTEMPTS: u32 = 6;
const MAX_BACKOFF: Duration = Duration::from_secs(120);

/// True for HTTP statuses worth retrying: throttling and transient server errors.
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

/// Delay requested by a `Retry-After` header (seconds form), capped.
fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|secs| Duration::from_secs(secs).min(MAX_BACKOFF))
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.min(6)).min(MAX_BACKOFF)
}

/// Send an HTTP request, retrying throttled (429), transient (5xx) and
/// network failures with the server's `Retry-After` or exponential backoff.
///
/// `build` is called for every attempt, so request bodies must be cheap to
/// recreate (e.g. `bytes::Bytes` chunks). Non-retryable responses (including
/// 4xx errors) are returned to the caller unchanged.
pub async fn send_with_retry<F>(ctx: Option<&SyncContext>, what: &str, build: F) -> Result<reqwest::Response, ProviderError>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let no_check = || async { Ok::<Option<String>, ProviderError>(None) };
    match send_create_with_retry(ctx, what, build, no_check).await? {
        Created::Response(resp) => Ok(resp),
        Created::Existing(_) => unreachable!("no existence check was given"),
    }
}

/// Outcome of [`send_create_with_retry`].
pub enum Created {
    /// The (last) response of the request
    Response(reqwest::Response),
    /// An earlier attempt had created the item after all; its ID
    Existing(String),
}

/// Like [`send_with_retry`], for requests that create an item and are not
/// idempotent (a Drive file or folder: Drive allows duplicate names).
///
/// A timed-out or failed attempt may still have created the item, so before
/// each retry `already_created` is asked whether it exists now; if it returns
/// an ID, that item is used instead of creating a duplicate.
pub async fn send_create_with_retry<F, C, Fut>(
    ctx: Option<&SyncContext>,
    what: &str,
    mut build: F,
    mut already_created: C,
) -> Result<Created, ProviderError>
where
    F: FnMut() -> reqwest::RequestBuilder,
    C: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Option<String>, ProviderError>>,
{
    let mut attempt = 0;
    loop {
        attempt += 1;
        let result = build().send().await;
        let delay = match &result {
            Ok(resp) if is_retryable_status(resp.status().as_u16()) && attempt < MAX_ATTEMPTS => {
                retry_after(resp).unwrap_or_else(|| backoff(attempt))
            }
            Err(e) if (e.is_connect() || e.is_timeout() || e.is_request()) && attempt < MAX_ATTEMPTS => backoff(attempt),
            _ => {
                return result
                    .map(Created::Response)
                    .map_err(|e| ProviderError::ConnectionError(format!("{} failed: {}", what, e)));
            }
        };

        let reason = match &result {
            Ok(resp) => format!("HTTP {}", resp.status()),
            Err(e) => e.to_string(),
        };
        if let Some(ctx) = ctx {
            if ctx.check_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            ctx.log_warning(
                &format!("{} failed ({}), retrying in {}s (attempt {}/{})", what, reason, delay.as_secs(), attempt, MAX_ATTEMPTS),
                "http",
            )
            .await;
        }
        tokio::time::sleep(delay).await;

        // A connection that could not be opened sent nothing; anything else
        // may have reached the server.
        let maybe_sent = !matches!(&result, Err(e) if e.is_connect());
        if maybe_sent {
            if let Some(id) = already_created().await? {
                return Ok(Created::Existing(id));
            }
        }
    }
}

/// Read `len` bytes of a file starting at `offset`.
pub async fn read_chunk(path: &Path, offset: u64, len: usize) -> Result<bytes::Bytes, ProviderError> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(std::io::SeekFrom::Start(offset)).await?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).await?;
    Ok(bytes::Bytes::from(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn opts(patterns: &[&str], dirs: &[&str]) -> SyncOptions {
        SyncOptions {
            exclude_patterns: patterns.iter().map(|s| s.to_string()).collect(),
            exclude_dirs: dirs.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn walk_applies_excludes_and_skips_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("a/cache")).unwrap();
        std::fs::create_dir_all(root.join("skipme")).unwrap();
        std::fs::write(root.join("a/file.txt"), b"hello").unwrap();
        std::fs::write(root.join("a/file.tmp"), b"x").unwrap();
        std::fs::write(root.join("a/cache/c.txt"), b"x").unwrap();
        std::fs::write(root.join("skipme/s.txt"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("a"), root.join("link")).unwrap();

        let src = root.to_str().unwrap();
        let skip_dir = format!("{}/skipme", src);
        let tree = walk_source(src, &opts(&["*.tmp", "cache"], &[&skip_dir])).unwrap();

        let rels: Vec<&str> = tree.entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(rels, vec!["a", "a/file.txt"]);
        assert_eq!(tree.files().next().unwrap().size, 5);
        #[cfg(unix)]
        assert!(tree.skipped.iter().any(|s| s.contains("symlink")));
    }

    #[test]
    fn include_only_limits_files_and_protects_others() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("photos_42.tar.gz"), b"a").unwrap();
        std::fs::write(tmp.path().join("photos_142.tar.gz"), b"b").unwrap();
        std::fs::write(tmp.path().join("docs_7.zip"), b"c").unwrap();
        let mut o = opts(&[], &[]);
        o.include_only = crate::services::compress_service::job_archive_patterns(42);
        std::fs::write(tmp.path().join("x_42.notes_7.zip"), b"d").unwrap();
        let tree = walk_source(tmp.path().to_str().unwrap(), &o).unwrap();
        let rels: Vec<&str> = tree.entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(rels, vec!["photos_42.tar.gz"]);
        assert!(ExcludeMatcher::new(&o).rel_excluded("/s", "docs_7.zip"));
        assert!(ExcludeMatcher::new(&o).rel_excluded("/s", "photos_142.tar.gz"));
        // Another job's archive whose custom name contains "_42."
        assert!(ExcludeMatcher::new(&o).rel_excluded("/s", "x_42.notes_7.zip"));
        assert!(!ExcludeMatcher::new(&o).rel_excluded("/s", "2026-01-01_photos_42.tar.gz"));
    }

    #[test]
    fn rel_excluded_matches_components_and_dirs() {
        let o = opts(&["*.tmp", "node_modules"], &["/src/private"]);
        assert!(ExcludeMatcher::new(&o).rel_excluded("/src", "x/node_modules/y.js"));
        assert!(ExcludeMatcher::new(&o).rel_excluded("/src", "a.tmp"));
        assert!(ExcludeMatcher::new(&o).rel_excluded("/src", "private/secret.txt"));
        assert!(!ExcludeMatcher::new(&o).rel_excluded("/src", "public/a.txt"));
    }

    #[tokio::test]
    async fn uploads_run_in_parallel_and_are_counted() {
        let cancel = tokio_util::sync::CancellationToken::new();
        let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peak = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut result = super::super::SyncResult::default();
        upload_parallel(&cancel, (1..=10u64).collect(), 3, &mut result, |n| {
            let (in_flight, peak) = (Arc::clone(&in_flight), Arc::clone(&peak));
            async move {
                use std::sync::atomic::Ordering::SeqCst;
                peak.fetch_max(in_flight.fetch_add(1, SeqCst) + 1, SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                in_flight.fetch_sub(1, SeqCst);
                if n == 7 { Err(ProviderError::TransferError("x".into())) } else { Ok(n) }
            }
        })
        .await
        .unwrap();
        assert_eq!((result.files_transferred, result.bytes_transferred, result.error_count), (9, 55 - 7, 1));
        assert_eq!(peak.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn cancelling_stops_uploads_at_once() {
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut result = super::super::SyncResult::default();
        let token = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            token.cancel();
        });
        let outcome = tokio::time::timeout(
            Duration::from_secs(2),
            upload_parallel(&cancel, vec![1u64, 2], 2, &mut result, |_| async {
                tokio::time::sleep(Duration::from_secs(60)).await;
                Ok(0)
            }),
        )
        .await
        .unwrap();
        assert!(matches!(outcome, Err(ProviderError::Cancelled)));
    }

    #[tokio::test]
    async fn walk_remote_lists_the_whole_tree() {
        // a -> b, c; b -> d
        let tree: std::collections::HashMap<&str, Vec<&str>> =
            [("", vec!["a"]), ("a", vec!["a/b", "a/c"]), ("a/b", vec!["a/b/d"]), ("a/c", vec![]), ("a/b/d", vec![])]
                .into_iter()
                .collect();
        let mut seen = Vec::new();
        walk_remote(
            &tokio_util::sync::CancellationToken::new(),
            String::new(),
            4,
            |folder: String| {
                let children: Vec<String> = tree[folder.as_str()].iter().map(|s| s.to_string()).collect();
                async move { Ok((folder, children)) }
            },
            |folder, children| {
                seen.push(folder.clone());
                children
            },
        )
        .await
        .unwrap();
        seen.sort();
        assert_eq!(seen, vec!["", "a", "a/b", "a/b/d", "a/c"]);
    }

    #[test]
    fn upload_decision() {
        let local = LocalEntry { rel: "f".into(), abs: "/f".into(), is_dir: false, size: 10, mtime: 100 };
        assert!(needs_upload(&local, None));
        assert!(!needs_upload(&local, Some(&RemoteFile { size: 10, mtime: 100, id: None })));
        assert!(!needs_upload(&local, Some(&RemoteFile { size: 10, mtime: 200, id: None })));
        assert!(needs_upload(&local, Some(&RemoteFile { size: 11, mtime: 200, id: None })));
        assert!(needs_upload(&local, Some(&RemoteFile { size: 10, mtime: 99, id: None })));
    }

    #[test]
    fn parses_dates() {
        assert_eq!(parse_rfc3339("2026-01-01T00:00:00Z"), Some(1767225600));
        assert_eq!(parse_rfc3339("2026-01-01T00:00:00.123Z"), Some(1767225600));
        assert_eq!(parse_http_date("Thu, 01 Jan 2026 00:00:00 GMT"), Some(1767225600));
    }

    /// A server that answers every request with HTTP 503 and counts requests.
    async fn failing_server() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let _ = sock
                    .write_all(b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
            }
        });
        (format!("http://{}", addr), hits)
    }

    #[tokio::test]
    async fn create_retry_uses_item_created_by_failed_attempt() {
        let (url, hits) = failing_server().await;
        let client = reqwest::Client::new();
        let created = send_create_with_retry(None, "Create", || client.post(&url), || async {
            Ok(Some("existing-id".to_string()))
        })
        .await
        .unwrap();
        assert!(matches!(created, Created::Existing(ref id) if id == "existing-id"));
        // No second create request was sent
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn create_retry_retries_when_nothing_was_created() {
        let (url, hits) = failing_server().await;
        let client = reqwest::Client::new();
        let checks = std::sync::atomic::AtomicUsize::new(0);
        let created = send_create_with_retry(None, "Create", || client.post(&url), || async {
            // Found on the second check
            let n = checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok((n == 1).then(|| "late-id".to_string()))
        })
        .await
        .unwrap();
        assert!(matches!(created, Created::Existing(ref id) if id == "late-id"));
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn retryable_statuses() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(401));
    }
}

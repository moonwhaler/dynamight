//! File manager endpoints (`/files/*`): places, raw file access with Range,
//! the trash and streamed folder sizes.

use axum::{
    body::{Body, Bytes},
    extract::{Query, Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::errors::ApiError;
use crate::extractors::AuthClaims;
use crate::services::fs_space;
use crate::services::path_guard::{
    contains_mount_point, content_disposition_attachment, is_allowed_root, is_path_allowed, mount_points,
    resolve_entry, resolve_existing, PathDenied, TRASH_DIR,
};
use crate::services::trash::{self, RestoreError, TrashItem};
use crate::services::{MountService, UsbDrive};
use crate::AppState;

pub(crate) fn denied(e: PathDenied) -> Response {
    match e {
        PathDenied::NotAllowed => ApiError::path_not_allowed().into_response(),
        PathDenied::NotFound => ApiError::file_not_found().into_response(),
    }
}

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

// ── Places ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct Place {
    path: String,
    total: Option<u64>,
    free: Option<u64>,
}

#[derive(Serialize)]
struct DriveInfo {
    #[serde(flatten)]
    drive: UsbDrive,
    total: Option<u64>,
    free: Option<u64>,
}

/// GET /files/places - the allowed paths and USB drives with free space,
/// and how many items are in the trash
pub async fn places(State(state): State<Arc<AppState>>) -> Response {
    let allowed = state.config.allowed_browse_paths.clone();
    let roots = known_roots(&state.db, &allowed).await;
    let result = blocking(move || {
        let places: Vec<Place> = allowed
            .iter()
            .filter(|p| Path::new(p).is_dir())
            .map(|p| {
                let space = fs_space(Path::new(p));
                Place { path: p.clone(), free: space.map(|s| s.0), total: space.map(|s| s.1) }
            })
            .collect();
        let drives: Vec<DriveInfo> = MountService
            .list_usb_drives()
            .unwrap_or_default()
            .into_iter()
            .map(|drive| {
                let space = drive.mountpoint.as_deref().and_then(|m| fs_space(Path::new(m)));
                DriveInfo { drive, free: space.map(|s| s.0), total: space.map(|s| s.1) }
            })
            .collect();
        let trash_items: usize = roots.iter().map(|r| trash::list(r).len()).sum();
        json!({ "places": places, "drives": drives, "trash_items": trash_items })
    })
    .await;
    match result {
        Some(body) => Json(body).into_response(),
        None => ApiError::new(crate::errors::ErrorCode::BrowseFailed).into_response(),
    }
}

// ── Raw file access ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RawQuery {
    pub path: String,
    /// `inline` to show the file in the browser; anything else downloads it
    pub disposition: Option<String>,
}

/// How a file may be shown inline
#[derive(Debug, PartialEq, Eq)]
enum Inline {
    /// Served with its own type (images, audio, video, plain text)
    Media(String),
    /// PDF: the browser's viewer does not work in a sandbox
    Pdf,
    /// Any other text (html, svg source, scripts) shown as plain text
    Text,
    /// Binary data: downloaded instead
    No,
}

fn inline_kind(mime: &str, looks_like_text: bool) -> Inline {
    let (top, sub) = mime.split_once('/').unwrap_or((mime, ""));
    match (top, sub) {
        ("image" | "audio" | "video", _) => Inline::Media(mime.to_string()),
        ("text", "plain") => Inline::Text,
        ("application", "pdf") => Inline::Pdf,
        _ if looks_like_text => Inline::Text,
        _ => Inline::No,
    }
}

/// True if the start of the file has no NUL bytes and is valid UTF-8
fn looks_like_text(path: &Path) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 8192];
    let Ok(n) = std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)) else {
        return false;
    };
    let head = &buf[..n];
    if head.contains(&0) {
        return false;
    }
    match std::str::from_utf8(head) {
        Ok(_) => true,
        // A multi-byte character cut off at the end of the buffer
        Err(e) => e.error_len().is_none(),
    }
}

/// Content-Disposition `inline` with the file name
fn content_disposition_inline(filename: &str) -> String {
    content_disposition_attachment(filename).replacen("attachment", "inline", 1)
}

/// Scripts in a served file must never run with the app's origin
const SANDBOX_CSP: &str =
    "sandbox; default-src 'none'; img-src 'self' data:; media-src 'self'; style-src 'unsafe-inline'";

/// GET /files/raw?path=&disposition= - a file, with Range support, as a
/// download or (for safe types) shown in the browser
pub async fn raw(State(state): State<Arc<AppState>>, Query(q): Query<RawQuery>, request: Request) -> Response {
    let canonical = match resolve_existing(&q.path, &state.config.allowed_browse_paths) {
        Ok(p) => p,
        Err(e) => return denied(e),
    };
    let Ok(meta) = std::fs::metadata(&canonical) else {
        return ApiError::file_not_found().into_response();
    };
    if !meta.is_file() {
        return ApiError::not_a_file().into_response();
    }
    let filename = canonical.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "download".into());
    let guessed = mime_guess::from_path(&canonical).first_or_octet_stream();

    let want_inline = q.disposition.as_deref() == Some("inline");
    let kind = if want_inline {
        let path = canonical.clone();
        let text = blocking(move || looks_like_text(&path)).await.unwrap_or(false);
        inline_kind(guessed.essence_str(), text)
    } else {
        Inline::No
    };
    if kind == Inline::No && meta.len() > state.config.max_download_size {
        return ApiError::file_too_large(state.config.max_download_size).into_response();
    }

    let mime = match &kind {
        Inline::Media(m) => m.parse().unwrap_or(mime_guess::mime::APPLICATION_OCTET_STREAM),
        Inline::Pdf => mime_guess::mime::APPLICATION_PDF,
        Inline::Text => mime_guess::mime::TEXT_PLAIN_UTF_8,
        Inline::No => guessed,
    };
    let response = match ServeFile::new_with_mime(&canonical, &mime).oneshot(request).await {
        Ok(r) => r.map(Body::new),
        Err(_) => return ApiError::download_failed().into_response(),
    };

    let (mut parts, body) = response.into_parts();
    let h = &mut parts.headers;
    let disposition = if kind == Inline::No {
        content_disposition_attachment(&filename)
    } else {
        content_disposition_inline(&filename)
    };
    if let Ok(v) = HeaderValue::from_str(&disposition) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"));
    match kind {
        Inline::Pdf => {
            // The preview panel shows PDFs in a frame of the app itself
            h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("frame-ancestors 'self'"));
            h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("SAMEORIGIN"));
        }
        _ => {
            h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(SANDBOX_CSP));
        }
    }
    Response::from_parts(parts, body)
}

// ── Trash ───────────────────────────────────────────────────────────────

/// Trash folders that may hold items: the ones used before (recorded in the
/// database) and those at the allowed roots and mount points.
async fn known_roots(db: &sqlx::SqlitePool, allowed: &[String]) -> Vec<PathBuf> {
    let recorded: Vec<(String,)> = sqlx::query_as("SELECT path FROM trash_roots")
        .fetch_all(db)
        .await
        .unwrap_or_default();
    let allowed = allowed.to_vec();
    blocking(move || {
        let mut candidates: Vec<PathBuf> = recorded.into_iter().map(|(p,)| PathBuf::from(p)).collect();
        candidates.extend(allowed.iter().map(|p| Path::new(p).join(TRASH_DIR)));
        candidates.extend(
            mount_points()
                .into_iter()
                .filter(|m| is_path_allowed(m, &allowed))
                .map(|m| m.join(TRASH_DIR)),
        );
        let mut roots: Vec<PathBuf> = candidates
            .into_iter()
            .filter(|root| {
                // A real folder (not a link) whose drive is attached and allowed
                std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir())
                    && root.parent().is_some_and(|p| is_path_allowed(p, &allowed))
            })
            .filter_map(|root| std::fs::canonicalize(root).ok())
            .collect();
        roots.sort();
        roots.dedup();
        roots
    })
    .await
    .unwrap_or_default()
}

async fn record_roots(db: &sqlx::SqlitePool, roots: &[PathBuf]) {
    for root in roots {
        let _ = sqlx::query(
            "INSERT INTO trash_roots (path, last_seen) VALUES (?, CURRENT_TIMESTAMP) \
             ON CONFLICT(path) DO UPDATE SET last_seen = CURRENT_TIMESTAMP",
        )
        .bind(root.to_string_lossy().to_string())
        .execute(db)
        .await;
    }
}

#[derive(Deserialize)]
pub struct TrashRequest {
    pub paths: Vec<String>,
}

#[derive(Serialize)]
struct TrashFailure {
    path: String,
    /// not_allowed, not_found, protected, no_anchor, cross_device or io
    reason: &'static str,
}

/// POST /files/trash - move files and folders to the trash of their drive
pub async fn move_to_trash(State(state): State<Arc<AppState>>, Json(req): Json<TrashRequest>) -> Response {
    let allowed = state.config.allowed_browse_paths.clone();
    let result = blocking(move || {
        let mounts = mount_points();
        let mut moved: Vec<TrashItem> = Vec::new();
        let mut failed: Vec<TrashFailure> = Vec::new();
        let mut roots: Vec<PathBuf> = Vec::new();
        for path in req.paths {
            let entry = match resolve_entry(&path, &allowed) {
                Ok(e) => e,
                Err(PathDenied::NotAllowed) => {
                    failed.push(TrashFailure { path, reason: "not_allowed" });
                    continue;
                }
                Err(PathDenied::NotFound) => {
                    failed.push(TrashFailure { path, reason: "not_found" });
                    continue;
                }
            };
            // Never an allowed root, a mount point or a folder holding one
            let is_link = std::fs::symlink_metadata(&entry).is_ok_and(|m| m.file_type().is_symlink());
            let protected = !is_link
                && std::fs::canonicalize(&entry)
                    .map(|c| is_allowed_root(&c, &allowed) || contains_mount_point(&c) || !is_path_allowed(&c, &allowed))
                    .unwrap_or(true);
            if protected {
                failed.push(TrashFailure { path, reason: "protected" });
                continue;
            }
            match trash::move_to_trash(&entry, &allowed, &mounts) {
                Ok((root, item)) => {
                    tracing::info!("Moved '{}' to the trash", entry.display());
                    if !roots.contains(&root) {
                        roots.push(root);
                    }
                    moved.push(item);
                }
                Err(e) => {
                    if let trash::TrashError::Io(err) = &e {
                        tracing::warn!("Could not move '{}' to the trash: {}", entry.display(), err);
                    }
                    failed.push(TrashFailure { path, reason: e.reason() });
                }
            }
        }
        (moved, failed, roots)
    })
    .await;
    let Some((moved, failed, roots)) = result else {
        return ApiError::delete_failed().into_response();
    };
    record_roots(&state.db, &roots).await;
    Json(json!({ "moved": moved, "failed": failed })).into_response()
}

#[derive(Serialize)]
struct TrashRoot {
    root: String,
    /// The folder the trash belongs to (drive or allowed path)
    anchor: String,
    items: usize,
}

/// GET /files/trash - everything in the trash folders that are attached
pub async fn list_trash(State(state): State<Arc<AppState>>) -> Response {
    let roots = known_roots(&state.db, &state.config.allowed_browse_paths).await;
    let result = blocking(move || {
        let mut items: Vec<TrashItem> = Vec::new();
        let mut summary: Vec<TrashRoot> = Vec::new();
        for root in &roots {
            let list = trash::list(root);
            summary.push(TrashRoot {
                root: root.to_string_lossy().to_string(),
                anchor: root.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
                items: list.len(),
            });
            items.extend(list);
        }
        items.sort_by_key(|i| std::cmp::Reverse(i.deleted_at));
        json!({ "items": items, "roots": summary })
    })
    .await;
    match result {
        Some(body) => Json(body).into_response(),
        None => ApiError::new(crate::errors::ErrorCode::BrowseFailed).into_response(),
    }
}

#[derive(Deserialize, Clone)]
pub struct TrashRef {
    pub root: String,
    pub id: String,
}

/// Look up client references: only known trash folders and valid ids
/// (`None` if any reference is not one)
async fn checked_refs(state: &AppState, refs: Vec<TrashRef>) -> Option<Vec<(PathBuf, String)>> {
    let roots = known_roots(&state.db, &state.config.allowed_browse_paths).await;
    refs.into_iter()
        .map(|r| {
            let root = PathBuf::from(&r.root);
            (roots.contains(&root) && trash::is_valid_id(&r.id)).then_some((root, r.id))
        })
        .collect()
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub items: Vec<TrashRef>,
    /// Restore under a new name when the original one is taken
    #[serde(default)]
    pub keep_both: bool,
}

/// POST /files/trash/restore - put items back where they were
pub async fn restore_trash(State(state): State<Arc<AppState>>, Json(req): Json<RestoreRequest>) -> Response {
    let Some(refs) = checked_refs(&state, req.items).await else {
        return ApiError::path_not_allowed().into_response();
    };
    let keep_both = req.keep_both;
    let result = blocking(move || {
        let mut restored = Vec::new();
        let mut conflicts = Vec::new();
        let mut failed = Vec::new();
        for (root, id) in refs {
            let reference = json!({ "root": root.to_string_lossy(), "id": id });
            match trash::restore(&root, &id, keep_both) {
                Ok(path) => restored.push(json!({ "id": id, "path": path.to_string_lossy() })),
                Err(RestoreError::Conflict(path)) => {
                    conflicts.push(json!({ "root": root.to_string_lossy(), "id": id, "path": path.to_string_lossy() }))
                }
                Err(RestoreError::NotFound) => failed.push(reference),
                Err(RestoreError::Io(e)) => {
                    tracing::warn!("Could not restore trash item {}: {}", id, e);
                    failed.push(reference)
                }
            }
        }
        json!({ "restored": restored, "conflicts": conflicts, "failed": failed })
    })
    .await;
    match result {
        Some(body) => Json(body).into_response(),
        None => ApiError::delete_failed().into_response(),
    }
}

#[derive(Deserialize)]
pub struct PurgeRequest {
    #[serde(default)]
    pub items: Vec<TrashRef>,
    /// Empty whole trash folders (all of them when `roots` is empty)
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub roots: Vec<String>,
}

/// DELETE /files/trash - delete items for good (needs the password check)
pub async fn purge_trash(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
    Json(req): Json<PurgeRequest>,
) -> Response {
    if !super::system::is_delete_verified(&state, claims.sub).await {
        return ApiError::delete_verification_required().into_response();
    }
    let Some(mut targets) = checked_refs(&state, req.items).await else {
        return ApiError::path_not_allowed().into_response();
    };
    if req.all {
        let roots = known_roots(&state.db, &state.config.allowed_browse_paths).await;
        let chosen: Vec<PathBuf> = if req.roots.is_empty() {
            roots
        } else {
            let wanted: Vec<PathBuf> = req.roots.iter().map(PathBuf::from).collect();
            if wanted.iter().any(|w| !roots.contains(w)) {
                return ApiError::path_not_allowed().into_response();
            }
            wanted
        };
        let listed = blocking(move || {
            chosen
                .iter()
                .flat_map(|root| trash::list(root).into_iter().map(move |i| (root.clone(), i.id)))
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        targets.extend(listed);
    }

    let result = blocking(move || {
        let mut purged = 0usize;
        let mut failed = 0usize;
        for (root, id) in targets {
            match trash::purge(&root, &id) {
                Ok(()) => purged += 1,
                Err(e) => {
                    tracing::warn!("Could not delete trash item {}: {}", id, e);
                    failed += 1;
                }
            }
        }
        (purged, failed)
    })
    .await;
    let Some((purged, failed)) = result else {
        return ApiError::delete_failed().into_response();
    };
    // Like other deletes, a successful one extends the password window
    state.delete_verifications.write().await.insert(claims.sub, Instant::now());
    tracing::info!("User {} deleted {} trash item(s) for good", claims.sub, purged);
    Json(json!({ "purged": purged, "failed": failed })).into_response()
}

/// Delete trash items older than the retention setting
pub async fn purge_expired_trash(db: &sqlx::SqlitePool, allowed: &[String]) {
    let stored = super::settings::stored_value(db, "trash_retention_days").await;
    let Some(days) = super::settings::trash_days_from_setting(stored.as_deref()) else {
        return;
    };
    let roots = known_roots(db, allowed).await;
    let removed = blocking(move || roots.iter().map(|r| trash::purge_expired(r, days)).sum::<usize>())
        .await
        .unwrap_or(0);
    if removed > 0 {
        tracing::info!("Trash: removed {} item(s) older than {} days", removed, days);
    }
}

// ── Folder sizes (streamed) ─────────────────────────────────────────────

/// Folder walks running at once, across all requests
static SIZE_WALKS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

const MAX_SIZE_PATHS: usize = 5000;
const SIZE_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
pub struct DirSizesRequest {
    pub paths: Vec<String>,
}

/// Sets the flag when dropped: the client went away, so the walks stop
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// POST /files/dir-sizes - folder sizes as NDJSON, one `{"path","size"}`
/// line per folder as soon as it is known
pub async fn dir_sizes(State(state): State<Arc<AppState>>, Json(req): Json<DirSizesRequest>) -> Response {
    let ndjson = |body: Body| {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/x-ndjson")
            .header(header::CACHE_CONTROL, "no-store")
            .body(body)
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    };
    if !*state.show_directory_sizes.read().await {
        return ndjson(Body::empty());
    }
    if req.paths.len() > MAX_SIZE_PATHS {
        return ApiError::new(crate::errors::ErrorCode::BrowseFailed).into_response();
    }

    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let allowed = state.config.allowed_browse_paths.clone();
    let cache = state.dir_size_cache.clone();

    for path in req.paths {
        let (tx, stop, allowed, cache) = (tx.clone(), Arc::clone(&stop), allowed.clone(), Arc::clone(&cache));
        tokio::spawn(async move {
            let Ok(_permit) = SIZE_WALKS.clone().acquire_owned().await else { return };
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let size = blocking(move || {
                let canonical = resolve_existing(&path, &allowed).ok().filter(|p| p.is_dir())?;
                let cached = cache
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(&canonical)
                    .filter(|(_, at)| at.elapsed() < SIZE_CACHE_TTL)
                    .map(|(size, _)| *size);
                let size = match cached {
                    Some(size) => size,
                    None => {
                        let size = MountService::dir_size_until(&canonical, &stop)?;
                        let mut cache = cache.write().unwrap_or_else(|e| e.into_inner());
                        if cache.len() >= 10_000 {
                            cache.retain(|_, (_, at)| at.elapsed() < SIZE_CACHE_TTL);
                        }
                        cache.insert(canonical, (size, Instant::now()));
                        size
                    }
                };
                Some((path, size))
            })
            .await
            .flatten();
            if let Some((path, size)) = size {
                let mut line = serde_json::to_vec(&json!({ "path": path, "size": size })).unwrap_or_default();
                line.push(b'\n');
                let _ = tx.send(Bytes::from(line)).await;
            }
        });
    }
    drop(tx);

    let guard = StopOnDrop(stop);
    let stream = futures::stream::unfold((rx, guard), |(mut rx, guard)| async move {
        rx.recv().await.map(|line| (Ok::<_, Infallible>(line), (rx, guard)))
    });
    ndjson(Body::from_stream(stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_types() {
        assert_eq!(inline_kind("image/jpeg", false), Inline::Media("image/jpeg".into()));
        assert_eq!(inline_kind("image/svg+xml", true), Inline::Media("image/svg+xml".into()));
        assert_eq!(inline_kind("video/mp4", false), Inline::Media("video/mp4".into()));
        assert_eq!(inline_kind("application/pdf", false), Inline::Pdf);
        assert_eq!(inline_kind("text/html", true), Inline::Text);
        assert_eq!(inline_kind("text/plain", false), Inline::Text);
        assert_eq!(inline_kind("application/octet-stream", true), Inline::Text);
        assert_eq!(inline_kind("application/zip", false), Inline::No);
    }

    #[test]
    fn text_detection() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("a.log");
        std::fs::write(&text, "Grüße\nline 2\n").unwrap();
        assert!(looks_like_text(&text));
        let bin = dir.path().join("b.bin");
        std::fs::write(&bin, [0x89, b'P', b'N', b'G', 0, 1, 2]).unwrap();
        assert!(!looks_like_text(&bin));
    }

    #[test]
    fn inline_disposition_keeps_name() {
        assert_eq!(content_disposition_inline("a b.txt"), "inline; filename=\"a_b.txt\"; filename*=UTF-8''a%20b.txt");
    }
}

//! File manager actions: copy/move tasks, rename and folder downloads.

use axum::{
    body::Body,
    extract::{Path as UrlPath, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

use super::files::{blocking, denied};
use crate::errors::{ApiError, ErrorCode};
use crate::services::file_ops::{Choice, TaskKind};
use crate::services::path_guard::{
    contains_mount_point, content_disposition_attachment, is_allowed_root, is_path_allowed, rename_noreplace,
    resolve_entry, resolve_existing, valid_name, PathDenied,
};
use crate::AppState;

/// The resolved entry and, unless it is a link, its canonical path
fn entry_and_canonical(path: &str, allowed: &[String]) -> Result<(PathBuf, Option<PathBuf>), PathDenied> {
    let entry = resolve_entry(path, allowed)?;
    let is_link = std::fs::symlink_metadata(&entry).is_ok_and(|m| m.file_type().is_symlink());
    let canonical = if is_link { None } else { Some(std::fs::canonicalize(&entry).map_err(|_| PathDenied::NotFound)?) };
    Ok((entry, canonical))
}

// ── Copy and move ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StartTaskRequest {
    pub kind: TaskKind,
    pub sources: Vec<String>,
    /// Existing folder the sources go into
    pub target: String,
}

/// POST /files/tasks - copy or move files and folders into a folder
pub async fn start_task(State(state): State<Arc<AppState>>, Json(req): Json<StartTaskRequest>) -> Response {
    let allowed = &state.config.allowed_browse_paths;
    if req.sources.is_empty() {
        return ApiError::field_required("sources").into_response();
    }
    let target = match resolve_existing(&req.target, allowed) {
        Ok(t) if t.is_dir() => t,
        Ok(_) => return ApiError::not_a_file().into_response(),
        Err(e) => return denied(e),
    };
    let mut sources = Vec::with_capacity(req.sources.len());
    for source in &req.sources {
        let (entry, canonical) = match entry_and_canonical(source, allowed) {
            Ok(v) => v,
            Err(e) => return denied(e),
        };
        if let Some(canonical) = &canonical {
            // Never a mounted drive, nor (when moving) an allowed path itself
            let root = req.kind == TaskKind::Move && is_allowed_root(canonical, allowed);
            if root || contains_mount_point(canonical) || !is_path_allowed(canonical, allowed) {
                return ApiError::path_not_allowed().into_response();
            }
            if target.starts_with(canonical) {
                return ApiError::new(ErrorCode::TargetInsideSource).into_response();
            }
        }
        if req.kind == TaskKind::Move && entry.parent() == Some(target.as_path()) {
            return ApiError::new(ErrorCode::TargetInsideSource).into_response();
        }
        sources.push(entry);
    }
    let info = state.file_ops.start(req.kind, sources, target);
    (StatusCode::ACCEPTED, Json(info)).into_response()
}

/// GET /files/tasks - running and recently finished tasks
pub async fn list_tasks(State(state): State<Arc<AppState>>) -> Response {
    Json(state.file_ops.list()).into_response()
}

/// POST /files/tasks/:id/cancel
pub async fn cancel_task(State(state): State<Arc<AppState>>, UrlPath(id): UrlPath<u64>) -> Response {
    if state.file_ops.cancel(id) {
        Json(json!({ "success": true })).into_response()
    } else {
        ApiError::new(ErrorCode::FileTaskNotFound).into_response()
    }
}

#[derive(Deserialize)]
pub struct ResolveRequest {
    pub choice: Choice,
    #[serde(default)]
    pub apply_to_all: bool,
}

/// POST /files/tasks/:id/resolve - answer a name conflict
pub async fn resolve_task(
    State(state): State<Arc<AppState>>,
    UrlPath(id): UrlPath<u64>,
    Json(req): Json<ResolveRequest>,
) -> Response {
    if state.file_ops.resolve(id, req.choice, req.apply_to_all) {
        Json(json!({ "success": true })).into_response()
    } else {
        ApiError::new(ErrorCode::FileTaskNotFound).into_response()
    }
}

// ── Rename ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RenameRequest {
    pub path: String,
    pub new_name: String,
}

/// POST /files/rename - rename in place; never replaces anything
pub async fn rename(State(state): State<Arc<AppState>>, Json(req): Json<RenameRequest>) -> Response {
    let allowed = &state.config.allowed_browse_paths;
    let name = req.new_name.trim().to_string();
    if !valid_name(&name) {
        return ApiError::new(ErrorCode::NameInvalid).into_response();
    }
    let (entry, canonical) = match entry_and_canonical(&req.path, allowed) {
        Ok(v) => v,
        Err(e) => return denied(e),
    };
    if canonical.as_ref().is_some_and(|c| is_allowed_root(c, allowed) || contains_mount_point(c)) {
        return ApiError::path_not_allowed().into_response();
    }
    let Some(parent) = entry.parent() else {
        return ApiError::path_not_allowed().into_response();
    };
    let dest = parent.join(&name);
    let from = entry.clone();
    let to = dest.clone();
    match blocking(move || rename_noreplace(&from, &to)).await {
        Some(Ok(())) => {
            tracing::info!("Renamed '{}' to '{}'", entry.display(), name);
            Json(json!({ "path": dest.to_string_lossy() })).into_response()
        }
        Some(Err(e)) if e.kind() == std::io::ErrorKind::AlreadyExists => ApiError::new(ErrorCode::NameTaken).into_response(),
        _ => ApiError::new(ErrorCode::FileOperationFailed).into_response(),
    }
}

// ── Download as archive ─────────────────────────────────────────────────

struct Prepared {
    /// Folder the archive's names are relative to
    base: PathBuf,
    names: Vec<String>,
    filename: String,
    created: Instant,
}

/// Archives prepared by POST and fetched by a plain GET link (a link
/// downloads natively; a form could not show errors)
static PREPARED: LazyLock<Mutex<HashMap<String, Prepared>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
const PREPARED_TTL: Duration = Duration::from_secs(300);

/// The deepest folder holding all `paths`
fn common_base(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut base = paths.first()?.parent()?.to_path_buf();
    for path in &paths[1..] {
        while !path.starts_with(&base) || path == &base {
            if !base.pop() {
                return None;
            }
        }
    }
    Some(base)
}

#[derive(Deserialize)]
pub struct ArchiveRequest {
    pub paths: Vec<String>,
}

/// POST /files/archive - check paths for a download as one archive;
/// returns the URL to fetch it from
pub async fn prepare_archive(State(state): State<Arc<AppState>>, Json(req): Json<ArchiveRequest>) -> Response {
    let allowed = &state.config.allowed_browse_paths;
    if req.paths.is_empty() {
        return ApiError::field_required("paths").into_response();
    }
    let mut entries = Vec::with_capacity(req.paths.len());
    for path in &req.paths {
        let (entry, canonical) = match entry_and_canonical(path, allowed) {
            Ok(v) => v,
            Err(e) => return denied(e),
        };
        if canonical.as_ref().is_some_and(|c| contains_mount_point(c)) {
            return ApiError::path_not_allowed().into_response();
        }
        entries.push(entry);
    }
    let Some(base) = common_base(&entries).filter(|b| is_path_allowed(b, allowed)) else {
        return ApiError::path_not_allowed().into_response();
    };
    let names: Vec<String> = entries
        .iter()
        .filter_map(|e| e.strip_prefix(&base).ok())
        .map(|p| p.to_string_lossy().to_string())
        .filter(|n| !n.contains('\0'))
        .collect();
    let stem = if entries.len() == 1 {
        entries[0].file_name().map(|n| n.to_string_lossy().to_string())
    } else {
        base.file_name().map(|n| n.to_string_lossy().to_string())
    }
    .unwrap_or_else(|| "files".to_string());
    // tar, not zip: zip cannot stream stored entries into a pipe, and
    // photos and videos are stored; tar streams any size and keeps links
    let filename = format!("{stem}-{}.tar", chrono::Local::now().format("%Y-%m-%d"));

    let token = uuid::Uuid::new_v4().simple().to_string();
    let mut prepared = PREPARED.lock().unwrap();
    prepared.retain(|_, p| p.created.elapsed() < PREPARED_TTL);
    prepared.insert(token.clone(), Prepared { base, names, filename: filename.clone(), created: Instant::now() });
    Json(json!({ "url": format!("/api/files/archive/{token}"), "filename": filename })).into_response()
}

/// GET /files/archive/:token - the archive, streamed while it is written
pub async fn download_archive(UrlPath(token): UrlPath<String>) -> Response {
    let found = {
        let prepared = PREPARED.lock().unwrap();
        prepared
            .get(&token)
            .filter(|p| p.created.elapsed() < PREPARED_TTL)
            .map(|p| (p.base.clone(), p.names.clone(), p.filename.clone()))
    };
    let Some((base, names, filename)) = found else {
        return ApiError::file_not_found().into_response();
    };

    let mut cmd = tokio::process::Command::new("tar");
    // Names come NUL-separated on stdin; links are stored as links
    cmd.args(["-cf", "-", "--null", "-T", "-"]);
    for pattern in crate::services::path_guard::INTERNAL_EXCLUDES {
        cmd.arg(format!("--exclude={pattern}"));
    }
    cmd.current_dir(&base)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Could not start the archiver: {}", e);
            return ApiError::download_failed().into_response();
        }
    };
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return ApiError::download_failed().into_response();
    };
    let list = names.iter().map(|n| format!("./{n}\0")).collect::<String>();
    tokio::spawn(async move {
        let _ = stdin.write_all(list.as_bytes()).await;
        // Dropping stdin ends the list
    });

    // The child stays with the stream: when the browser goes away the
    // stream is dropped and the archiver killed
    let stream = futures::stream::unfold(
        (tokio_util::io::ReaderStream::new(stdout), child),
        |(mut reader, child)| async move {
            use futures::StreamExt;
            reader.next().await.map(|chunk| (chunk, (reader, child)))
        },
    );
    let mut response = Response::new(Body::from_stream(stream));
    let h = response.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-tar"));
    if let Ok(v) = HeaderValue::from_str(&content_disposition_attachment(&filename)) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_base_is_the_shared_folder() {
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(common_base(&[p("/m/a/x"), p("/m/a/y")]), Some(p("/m/a")));
        assert_eq!(common_base(&[p("/m/a/x"), p("/m/b/y/z")]), Some(p("/m")));
        assert_eq!(common_base(&[p("/m/a")]), Some(p("/m")));
        // A selected folder and something inside it
        assert_eq!(common_base(&[p("/m/a"), p("/m/a/b")]), Some(p("/m")));
    }
}

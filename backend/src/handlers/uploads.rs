//! Resumable uploads for the file manager.
//!
//! The client starts an upload (`POST /files/uploads`), then sends the file
//! in chunks (`PUT /files/uploads/:id` with `Content-Range`). The bytes go to
//! a hidden part file next to the target, whose length is the upload's
//! offset: after a dropped connection or a server restart the client asks
//! for the offset (`GET`) and goes on from there. The last chunk renames the
//! part file to its name; nothing is ever replaced without the user's choice.

use axum::{
    body::Body,
    extract::{Path as UrlPath, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use tokio::io::AsyncWriteExt;

use super::files::{blocking, denied};
use crate::errors::{ApiError, ErrorCode};
use crate::extractors::AuthClaims;
use crate::services::fs_space;
use crate::services::path_guard::{
    free_name, is_path_allowed, mount_points, rename_noreplace, resolve_existing, valid_name, UPLOAD_MARKER,
};
use crate::services::trash;
use crate::AppState;

/// Largest chunk the client should send (the request body limit applies)
const MAX_CHUNK: u64 = 8 * 1024 * 1024;
/// Uploads untouched this long are removed by the hourly cleanup
pub const STALE_HOURS: i64 = 24;

fn chunk_size(body_limit: usize) -> u64 {
    MAX_CHUNK.min((body_limit as u64).saturating_sub(64 * 1024)).max(64 * 1024)
}

fn part_path(dir: &Path, name: &str, id: &str) -> PathBuf {
    dir.join(format!(".{name}{UPLOAD_MARKER}{id}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnConflict {
    Replace,
    KeepBoth,
}

#[derive(Deserialize)]
pub struct StartUpload {
    /// Folder the file goes into
    pub dir: String,
    /// Subfolder below `dir` for folder uploads ("photos/2024"), created as needed
    #[serde(default)]
    pub rel_dir: Option<String>,
    pub name: String,
    pub size: u64,
    /// Modification time of the original (Unix ms), kept on the uploaded file
    pub mtime: Option<i64>,
    /// Identifies the client's file (name, size, date): only the same file resumes
    pub fingerprint: String,
    pub on_conflict: Option<OnConflict>,
}

#[derive(Serialize)]
struct UploadState {
    id: String,
    offset: u64,
    size: u64,
    chunk_size: u64,
    /// An empty file is complete right away
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    dir: String,
    name: String,
    size: i64,
    mtime: Option<i64>,
    on_conflict: Option<String>,
}

fn facts(path: &Path) -> serde_json::Value {
    let meta = std::fs::metadata(path).ok();
    json!({
        "is_dir": meta.as_ref().is_some_and(|m| m.is_dir()),
        "size": meta.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
        "modified": meta.and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
    })
}

/// POST /files/uploads - start (or resume) an upload
pub async fn start(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
    Json(req): Json<StartUpload>,
) -> Response {
    let allowed = state.config.allowed_browse_paths.clone();
    if !valid_name(&req.name) {
        return ApiError::new(ErrorCode::NameInvalid).into_response();
    }
    let base = match resolve_existing(&req.dir, &allowed) {
        Ok(p) if p.is_dir() => p,
        Ok(_) => return ApiError::not_a_file().into_response(),
        Err(e) => return denied(e),
    };
    // Folder uploads: every part of the subfolder must be a plain name
    let mut dir = base.clone();
    if let Some(rel) = req.rel_dir.as_deref().filter(|r| !r.is_empty()) {
        for part in rel.split('/').filter(|p| !p.is_empty()) {
            if !valid_name(part) {
                return ApiError::new(ErrorCode::NameInvalid).into_response();
            }
            dir.push(part);
        }
        let make = dir.clone();
        if blocking(move || std::fs::create_dir_all(&make)).await.and_then(|r| r.ok()).is_none() {
            return ApiError::new(ErrorCode::DirectoryCreateFailed).into_response();
        }
        // A link among the new folders must not lead outside
        match std::fs::canonicalize(&dir) {
            Ok(c) if is_path_allowed(&c, &allowed) => dir = c,
            _ => return ApiError::path_not_allowed().into_response(),
        }
    }
    let target = dir.join(&req.name);
    let dir_str = dir.to_string_lossy().to_string();

    // The same file again (page reloaded, other device): resume it
    let existing: Option<Row> = sqlx::query_as(
        "SELECT id, dir, name, size, mtime, on_conflict FROM uploads \
         WHERE user_id = ? AND dir = ? AND name = ? AND size = ? AND fingerprint = ?",
    )
    .bind(claims.sub)
    .bind(&dir_str)
    .bind(&req.name)
    .bind(req.size as i64)
    .bind(&req.fingerprint)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    if let Some(row) = existing {
        let part = part_path(&dir, &row.name, &row.id);
        if let Ok(meta) = std::fs::metadata(&part) {
            if req.on_conflict.is_some() {
                let choice = req.on_conflict.map(|c| if c == OnConflict::Replace { "replace" } else { "keep_both" });
                let _ = sqlx::query("UPDATE uploads SET on_conflict = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(choice)
                    .bind(&row.id)
                    .execute(&state.db)
                    .await;
            }
            return Json(UploadState {
                id: row.id,
                offset: meta.len(),
                size: req.size,
                chunk_size: chunk_size(state.config.max_request_body_size),
                path: None,
            })
            .into_response();
        }
        // The part file is gone: start over
        let _ = sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&row.id).execute(&state.db).await;
    }

    if req.on_conflict.is_none() && std::fs::symlink_metadata(&target).is_ok() {
        return ApiError::with_params(ErrorCode::NameTaken, facts(&target)).into_response();
    }
    if fs_space(&dir).is_some_and(|(free, _)| free < req.size) {
        return ApiError::new(ErrorCode::NotEnoughSpace).into_response();
    }

    let id = uuid::Uuid::new_v4().simple().to_string();
    let part = part_path(&dir, &req.name, &id);
    if let Err(e) = std::fs::OpenOptions::new().write(true).create_new(true).open(&part) {
        tracing::warn!("Could not start upload to '{}': {}", target.display(), e);
        return ApiError::new(ErrorCode::FileOperationFailed).into_response();
    }
    let on_conflict = req.on_conflict.map(|c| if c == OnConflict::Replace { "replace" } else { "keep_both" });
    let inserted = sqlx::query(
        "INSERT INTO uploads (id, user_id, dir, name, size, fingerprint, mtime, on_conflict) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(claims.sub)
    .bind(&dir_str)
    .bind(&req.name)
    .bind(req.size as i64)
    .bind(&req.fingerprint)
    .bind(req.mtime)
    .bind(on_conflict)
    .execute(&state.db)
    .await;
    if inserted.is_err() {
        let _ = std::fs::remove_file(&part);
        return ApiError::new(ErrorCode::FileOperationFailed).into_response();
    }
    let mut path = None;
    if req.size == 0 {
        // Nothing to send: done now
        let choice = on_conflict.map(str::to_string);
        match finish(&state, &dir, &req.name, &part, req.mtime, choice).await {
            Ok(p) => path = Some(p.to_string_lossy().to_string()),
            Err(code) => {
                let _ = std::fs::remove_file(&part);
                let _ = sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&id).execute(&state.db).await;
                return ApiError::new(code).into_response();
            }
        }
        let _ = sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&id).execute(&state.db).await;
    }
    Json(UploadState { id, offset: 0, size: req.size, chunk_size: chunk_size(state.config.max_request_body_size), path })
        .into_response()
}

async fn find(state: &AppState, user: i64, id: &str) -> Option<Row> {
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    sqlx::query_as("SELECT id, dir, name, size, mtime, on_conflict FROM uploads WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
}

/// GET /files/uploads/:id - where the upload stands
pub async fn status(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let Some(row) = find(&state, claims.sub, &id).await else {
        return ApiError::new(ErrorCode::UploadNotFound).into_response();
    };
    let part = part_path(Path::new(&row.dir), &row.name, &row.id);
    match std::fs::metadata(&part) {
        Ok(meta) => Json(json!({ "id": row.id, "offset": meta.len(), "size": row.size })).into_response(),
        Err(_) => ApiError::new(ErrorCode::UploadNotFound).into_response(),
    }
}

/// Uploads being written right now: one request at a time per upload
static WRITING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

struct Writing(String);

impl Writing {
    fn claim(id: &str) -> Option<Self> {
        WRITING.lock().unwrap().insert(id.to_string()).then(|| Writing(id.to_string()))
    }
}

impl Drop for Writing {
    fn drop(&mut self) {
        WRITING.lock().unwrap().remove(&self.0);
    }
}

/// "bytes 0-99/1000" -> (0, 99, 1000)
fn parse_range(value: &str) -> Option<(u64, u64, u64)> {
    let rest = value.strip_prefix("bytes ")?;
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let (start, end, total) = (start.parse().ok()?, end.parse().ok()?, total.parse().ok()?);
    (start <= end && end < total).then_some((start, end, total))
}

/// PUT /files/uploads/:id - one chunk, `Content-Range: bytes a-b/size`
pub async fn chunk(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let Some(row) = find(&state, claims.sub, &id).await else {
        return ApiError::new(ErrorCode::UploadNotFound).into_response();
    };
    let Some((start, end, total)) = headers
        .get(header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_range)
    else {
        return ApiError::field_required("Content-Range").into_response();
    };
    if total != row.size as u64 {
        return ApiError::field_required("Content-Range").into_response();
    }
    let Some(_writing) = Writing::claim(&row.id) else {
        return ApiError::new(ErrorCode::UploadBusy).into_response();
    };
    let dir = PathBuf::from(&row.dir);
    let part = part_path(&dir, &row.name, &row.id);
    let offset = match std::fs::metadata(&part) {
        Ok(m) => m.len(),
        Err(_) => return ApiError::new(ErrorCode::UploadNotFound).into_response(),
    };
    if start != offset {
        return ApiError::with_params(ErrorCode::UploadOffsetMismatch, json!({ "offset": offset })).into_response();
    }

    let mut file = match tokio::fs::OpenOptions::new().append(true).open(&part).await {
        Ok(f) => f,
        Err(_) => return ApiError::new(ErrorCode::FileOperationFailed).into_response(),
    };
    let expected = end - start + 1;
    let mut written: u64 = 0;
    let mut stream = body.into_data_stream();
    let mut failed = false;
    while let Some(piece) = stream.next().await {
        let Ok(bytes) = piece else {
            // The client went away mid-chunk
            failed = true;
            break;
        };
        written += bytes.len() as u64;
        if written > expected || file.write_all(&bytes).await.is_err() {
            failed = true;
            break;
        }
    }
    if failed || written != expected || file.flush().await.is_err() {
        // Back to where the chunk started, so the client can send it again
        let _ = file.set_len(offset).await;
        return ApiError::with_params(ErrorCode::UploadOffsetMismatch, json!({ "offset": offset })).into_response();
    }
    let offset = offset + written;
    let _ = sqlx::query("UPDATE uploads SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&row.id)
        .execute(&state.db)
        .await;
    if offset < total {
        return Json(json!({ "offset": offset, "done": false })).into_response();
    }

    // Complete: give it its name
    let _ = file.sync_all().await;
    drop(file);
    let finished = finish(&state, &dir, &row.name, &part, row.mtime, row.on_conflict.clone()).await;
    match finished {
        Ok(path) => {
            let _ = sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&row.id).execute(&state.db).await;
            tracing::info!("Upload finished: '{}'", path.display());
            Json(json!({ "offset": offset, "done": true, "path": path.to_string_lossy() })).into_response()
        }
        Err(code) => ApiError::new(code).into_response(),
    }
}

/// Give the complete part file its name (on a blocking thread)
async fn finish(
    state: &AppState,
    dir: &Path,
    name: &str,
    part: &Path,
    mtime: Option<i64>,
    on_conflict: Option<String>,
) -> Result<PathBuf, ErrorCode> {
    let allowed = state.config.allowed_browse_paths.clone();
    let (dir, name, part) = (dir.to_path_buf(), name.to_string(), part.to_path_buf());
    blocking(move || -> Result<PathBuf, ErrorCode> {
        if let Some(ms) = mtime {
            if let Ok(f) = std::fs::File::options().write(true).open(&part) {
                let _ = f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms.max(0) as u64));
            }
        }
        let mut target = dir.join(&name);
        if std::fs::symlink_metadata(&target).is_ok() {
            match on_conflict.as_deref() {
                Some("replace") => {
                    trash::move_to_trash(&target, &allowed, &mount_points()).map_err(|_| ErrorCode::FileOperationFailed)?;
                }
                Some("keep_both") => target = free_name(&dir, &name, false),
                _ => return Err(ErrorCode::NameTaken),
            }
        }
        match rename_noreplace(&part, &target) {
            Ok(()) => Ok(target),
            // Taken in the meantime: keep both rather than lose either
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let other = free_name(&dir, &name, false);
                rename_noreplace(&part, &other).map(|_| other).map_err(|_| ErrorCode::FileOperationFailed)
            }
            Err(_) => Err(ErrorCode::FileOperationFailed),
        }
    })
    .await
    .unwrap_or(Err(ErrorCode::FileOperationFailed))
}

/// DELETE /files/uploads/:id - give up an upload and remove its bytes
pub async fn cancel(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let Some(row) = find(&state, claims.sub, &id).await else {
        return ApiError::new(ErrorCode::UploadNotFound).into_response();
    };
    let _ = std::fs::remove_file(part_path(Path::new(&row.dir), &row.name, &row.id));
    let _ = sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&row.id).execute(&state.db).await;
    Json(json!({ "success": true })).into_response()
}

/// Remove uploads nobody touched for a day, with their part files
pub async fn remove_stale(db: &sqlx::SqlitePool) {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, dir, name, size, mtime, on_conflict FROM uploads WHERE updated_at < datetime('now', ?)",
    )
    .bind(format!("-{STALE_HOURS} hours"))
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for row in rows {
        let _ = std::fs::remove_file(part_path(Path::new(&row.dir), &row.name, &row.id));
        let _ = sqlx::query("DELETE FROM uploads WHERE id = ?").bind(&row.id).execute(db).await;
        tracing::info!("Removed unfinished upload of '{}'", row.name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_range() {
        assert_eq!(parse_range("bytes 0-99/1000"), Some((0, 99, 1000)));
        assert_eq!(parse_range("bytes 900-999/1000"), Some((900, 999, 1000)));
        assert_eq!(parse_range("bytes 5-4/1000"), None);
        assert_eq!(parse_range("bytes 0-1000/1000"), None);
        assert_eq!(parse_range("items 0-1/2"), None);
    }

    #[test]
    fn chunks_fit_the_body_limit() {
        assert_eq!(chunk_size(10 * 1024 * 1024), 8 * 1024 * 1024);
        assert_eq!(chunk_size(2 * 1024 * 1024), 2 * 1024 * 1024 - 64 * 1024);
    }
}

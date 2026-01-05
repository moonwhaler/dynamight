use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio_util::io::ReaderStream;

use crate::errors::{ApiError, ErrorCode};
use crate::AppState;

/// Check if a path is within one of the allowed base paths.
/// Uses canonicalization to prevent symlink bypass attacks.
fn is_path_allowed(path: &Path, allowed_paths: &[String]) -> bool {
    // Handle the case where the path doesn't exist yet (for mkdir)
    // We need to check if the path or any of its parents are under allowed paths
    let path_to_check = if path.exists() {
        match path.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        }
    } else {
        // For non-existent paths, check the string representation
        // This is safe because we'll canonicalize during actual operations
        path.to_path_buf()
    };

    for allowed in allowed_paths {
        let allowed_path = Path::new(allowed);

        // Try to canonicalize the allowed path
        if let Ok(allowed_canonical) = allowed_path.canonicalize() {
            if path_to_check.starts_with(&allowed_canonical) {
                return true;
            }
        }

        // Also check non-canonicalized for paths that don't exist yet
        if path_to_check.starts_with(allowed) {
            return true;
        }
    }

    false
}

pub async fn list_drives(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.mount_service.list_usb_drives() {
        Ok(drives) => Json(drives).into_response(),
        Err(_) => ApiError::new(ErrorCode::DrivesListFailed).into_response(),
    }
}

pub async fn list_mounts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.mount_service.list_mounts() {
        Ok(mounts) => Json(mounts).into_response(),
        Err(_) => ApiError::new(ErrorCode::MountsListFailed).into_response(),
    }
}

#[derive(Deserialize)]
pub struct MountRequest {
    pub uuid: String,
    pub mount_point: String,
}

pub async fn mount_drive(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MountRequest>,
) -> impl IntoResponse {
    match state.mount_service.mount_by_uuid(&req.uuid, &req.mount_point) {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(_) => ApiError::new(ErrorCode::MountFailed).into_response(),
    }
}

#[derive(Deserialize)]
pub struct UnmountRequest {
    pub mount_point: String,
}

pub async fn unmount_drive(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UnmountRequest>,
) -> impl IntoResponse {
    match state.mount_service.unmount(&req.mount_point) {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(_) => ApiError::new(ErrorCode::UnmountFailed).into_response(),
    }
}

#[derive(Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
}

pub async fn browse_path(
    State(state): State<Arc<AppState>>,
    Query(query): Query<BrowseQuery>,
) -> impl IntoResponse {
    let path = query.path.as_deref().unwrap_or("/");

    // Handle empty path
    if path.is_empty() {
        return ApiError::field_required("path").into_response();
    }

    let path_obj = Path::new(path);

    // Security: check if path is in allowed directories
    if !is_path_allowed(path_obj, &state.config.allowed_browse_paths) {
        tracing::debug!("Browse denied for path '{}' - not in allowed paths", path);
        return ApiError::path_not_allowed().into_response();
    }

    // Security: canonicalize to prevent path traversal
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("Failed to canonicalize path '{}': {}", path, e);
            return ApiError::new(ErrorCode::BrowseFailed).into_response();
        }
    };

    // Double-check the canonical path is still allowed (prevents symlink attacks)
    if !is_path_allowed(&canonical, &state.config.allowed_browse_paths) {
        tracing::debug!("Browse denied for canonical path '{}' - not in allowed paths", canonical.display());
        return ApiError::path_not_allowed().into_response();
    }

    match state
        .mount_service
        .browse_path(&canonical.to_string_lossy())
    {
        Ok(entries) => Json(json!({
            "path": canonical.to_string_lossy(),
            "entries": entries
        }))
        .into_response(),
        Err(e) => {
            tracing::error!("Failed to browse path '{}': {}", canonical.display(), e);
            ApiError::new(ErrorCode::BrowseFailed).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct MkdirRequest {
    pub path: String,
}

pub async fn create_directory(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MkdirRequest>,
) -> impl IntoResponse {
    let path = Path::new(&req.path);

    // Security: don't allow path traversal
    let path_str = req.path.as_str();
    if path_str.contains("..") {
        return ApiError::new(ErrorCode::PathTraversalNotAllowed).into_response();
    }

    // Security: check if path is in allowed directories
    if !is_path_allowed(path, &state.config.allowed_browse_paths) {
        return ApiError::path_not_allowed().into_response();
    }

    match std::fs::create_dir_all(path) {
        Ok(_) => Json(json!({
            "success": true,
            "path": req.path
        })).into_response(),
        Err(_) => ApiError::new(ErrorCode::DirectoryCreateFailed).into_response(),
    }
}

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /system/allowed-paths - Return the list of allowed browse paths
/// Only returns paths that actually exist on the filesystem
pub async fn allowed_paths(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let existing_paths: Vec<&String> = state
        .config
        .allowed_browse_paths
        .iter()
        .filter(|p| Path::new(p).exists())
        .collect();

    Json(json!({
        "paths": existing_paths
    }))
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub path: String,
}

/// GET /system/download - Download a file securely
/// Security measures:
/// - Two-stage path validation (before and after canonicalization)
/// - Symlink attack prevention via canonicalization
/// - File size limit enforcement
/// - Path traversal prevention
/// - Content-Disposition: attachment to prevent inline execution
pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DownloadQuery>,
) -> Response {
    let path_str = &query.path;

    // 1. Basic validation
    if path_str.is_empty() {
        return ApiError::field_required("path").into_response();
    }

    // 2. Reject obvious traversal attempts
    if path_str.contains("..") {
        return ApiError::new(ErrorCode::PathTraversalNotAllowed).into_response();
    }

    let path = Path::new(path_str);

    // 3. Pre-canonicalization check
    if !is_path_allowed(path, &state.config.allowed_browse_paths) {
        tracing::debug!("Download denied for path '{}' - not in allowed paths", path_str);
        return ApiError::path_not_allowed().into_response();
    }

    // 4. Canonicalize to resolve symlinks
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("Failed to canonicalize download path '{}': {}", path_str, e);
            return ApiError::file_not_found().into_response();
        }
    };

    // 5. Post-canonicalization check (prevents symlink attacks)
    if !is_path_allowed(&canonical, &state.config.allowed_browse_paths) {
        tracing::debug!(
            "Download denied for canonical path '{}' - not in allowed paths",
            canonical.display()
        );
        return ApiError::path_not_allowed().into_response();
    }

    // 6. Get metadata and verify it's a file
    let metadata = match std::fs::metadata(&canonical) {
        Ok(m) => m,
        Err(_) => return ApiError::file_not_found().into_response(),
    };

    if !metadata.is_file() {
        return ApiError::not_a_file().into_response();
    }

    // 7. Check file size limit
    if metadata.len() > state.config.max_download_size {
        return ApiError::file_too_large(state.config.max_download_size).into_response();
    }

    // 8. Get filename and sanitize for Content-Disposition header
    let filename = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    // Sanitize filename: keep only safe characters
    let safe_filename: String = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // 9. Detect MIME type
    let content_type = mime_guess::from_path(&canonical)
        .first_or_octet_stream()
        .to_string();

    // 10. Open file and stream
    let file = match tokio::fs::File::open(&canonical).await {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Failed to open file for download '{}': {}", canonical.display(), e);
            return ApiError::download_failed().into_response();
        }
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // 11. Build response with security headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", safe_filename),
        )
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "private, no-cache")
        .body(body)
        .unwrap_or_else(|_| {
            ApiError::download_failed().into_response()
        })
}

/// POST /system/generate-mount-point - Generate a mount point for a USB drive
#[derive(Deserialize)]
pub struct GenerateMountPointRequest {
    pub label: Option<String>,
    pub uuid: String,
}

pub async fn generate_mount_point(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GenerateMountPointRequest>,
) -> impl IntoResponse {
    let mount_point = state
        .mount_service
        .generate_mount_point(req.label.as_deref(), &req.uuid);

    Json(json!({
        "mount_point": mount_point
    }))
}

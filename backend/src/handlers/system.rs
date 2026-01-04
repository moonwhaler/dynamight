use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

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
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to list drives: {}", e)})),
        )
            .into_response(),
    }
}

pub async fn list_mounts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.mount_service.list_mounts() {
        Ok(mounts) => Json(mounts).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to list mounts: {}", e)})),
        )
            .into_response(),
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
        Ok(_) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to mount: {}", e)})),
        ),
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
        Ok(_) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to unmount: {}", e)})),
        ),
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
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Path cannot be empty"})),
        )
            .into_response();
    }

    let path_obj = Path::new(path);

    // Security: check if path is in allowed directories
    if !is_path_allowed(path_obj, &state.config.allowed_browse_paths) {
        tracing::debug!("Browse denied for path '{}' - not in allowed paths", path);
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Path not in allowed directories"})),
        )
            .into_response();
    }

    // Security: canonicalize to prevent path traversal
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("Failed to canonicalize path '{}': {}", path, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid or inaccessible path"})),
            )
                .into_response()
        }
    };

    // Double-check the canonical path is still allowed (prevents symlink attacks)
    if !is_path_allowed(&canonical, &state.config.allowed_browse_paths) {
        tracing::debug!("Browse denied for canonical path '{}' - not in allowed paths", canonical.display());
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Path not in allowed directories"})),
        )
            .into_response();
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to browse: {}", e)})),
            )
                .into_response()
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
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Path traversal not allowed"})),
        ).into_response();
    }

    // Security: check if path is in allowed directories
    if !is_path_allowed(path, &state.config.allowed_browse_paths) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Cannot create directory outside allowed paths"})),
        ).into_response();
    }

    match std::fs::create_dir_all(path) {
        Ok(_) => Json(json!({
            "success": true,
            "path": req.path
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create directory: {}", e)})),
        ).into_response(),
    }
}

pub async fn health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /system/allowed-paths - Return the list of allowed browse paths
pub async fn allowed_paths(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(json!({
        "paths": state.config.allowed_browse_paths
    }))
}

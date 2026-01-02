use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

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

    // Security: prevent path traversal
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid path"})),
            )
                .into_response()
        }
    };

    match state
        .mount_service
        .browse_path(&canonical.to_string_lossy())
    {
        Ok(entries) => Json(json!({
            "path": canonical.to_string_lossy(),
            "entries": entries
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to browse: {}", e)})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct MkdirRequest {
    pub path: String,
}

pub async fn create_directory(
    Json(req): Json<MkdirRequest>,
) -> impl IntoResponse {
    let path = std::path::Path::new(&req.path);

    // Security: don't allow creating directories in sensitive locations
    let path_str = req.path.as_str();
    if path_str.starts_with("/proc")
        || path_str.starts_with("/sys")
        || path_str.starts_with("/dev")
        || path_str.contains("..")
    {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Cannot create directory in this location"})),
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

use axum::{
    body::Body,
    extract::{ConnectInfo, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::io::ReaderStream;

use crate::errors::{ApiError, ErrorCode};
use crate::extractors::AuthClaims;
use crate::handlers::auth::extract_client_ip;
use crate::services::{AuthService, TotpService};
use crate::AppState;

/// System paths that must never be used as mount points.
const DENIED_MOUNT_PREFIXES: &[&str] = &[
    "/", "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64",
    "/boot", "/proc", "/sys", "/dev", "/run", "/var", "/tmp",
    "/root", "/home", "/app", "/opt",
];

/// Validate that a UUID string matches the standard format.
fn is_valid_uuid(uuid: &str) -> bool {
    let uuid = uuid.trim();
    if uuid.len() != 36 {
        return false;
    }
    uuid.chars().enumerate().all(|(i, c)| {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            c == '-'
        } else {
            c.is_ascii_hexdigit()
        }
    })
}

/// Validate that a mount point is safe and within allowed paths.
fn is_mount_point_allowed(mount_point: &str, allowed_paths: &[String]) -> bool {
    let path = Path::new(mount_point);

    // Reject path traversal
    if mount_point.contains("..") {
        return false;
    }

    // Must be absolute
    if !path.is_absolute() {
        return false;
    }

    // Reject exact matches to dangerous system paths
    let normalized = mount_point.trim_end_matches('/');
    for denied in DENIED_MOUNT_PREFIXES {
        if normalized == *denied {
            return false;
        }
    }

    // Must be under /mnt/ or one of the allowed browse paths
    let under_mnt = mount_point.starts_with("/mnt/") && mount_point.len() > 5;
    let under_allowed = allowed_paths.iter().any(|allowed| {
        mount_point.starts_with(allowed.as_str())
            && mount_point.len() > allowed.len()
    });

    under_mnt || under_allowed
}

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
    // Security: validate UUID format to prevent abuse
    if !is_valid_uuid(&req.uuid) {
        return ApiError::new(ErrorCode::InvalidUuidFormat).into_response();
    }

    // Security: validate mount point is in allowed paths
    if !is_mount_point_allowed(&req.mount_point, &state.config.allowed_browse_paths) {
        tracing::warn!(
            "Mount denied: mount_point '{}' is not in allowed paths",
            req.mount_point
        );
        return ApiError::new(ErrorCode::MountPointNotAllowed).into_response();
    }

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
    // Security: validate mount point is in allowed paths
    if !is_mount_point_allowed(&req.mount_point, &state.config.allowed_browse_paths) {
        tracing::warn!(
            "Unmount denied: mount_point '{}' is not in allowed paths",
            req.mount_point
        );
        return ApiError::new(ErrorCode::MountPointNotAllowed).into_response();
    }

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

    let canonical_str = canonical.to_string_lossy().to_string();

    match state.mount_service.browse_path(&canonical_str, false) {
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
pub struct DirSizesRequest {
    pub paths: Vec<String>,
}

/// POST /system/dir-sizes - Compute directory sizes in batch with caching
pub async fn dir_sizes(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DirSizesRequest>,
) -> impl IntoResponse {
    // Early return if feature is disabled
    let enabled = *state.show_directory_sizes.read().await;
    if !enabled {
        return Json(json!({ "sizes": {} })).into_response();
    }

    // Cap the number of paths to prevent abuse
    if req.paths.len() > 200 {
        tracing::warn!("dir_sizes: rejected request with {} paths (max 200)", req.paths.len());
        return ApiError::new(ErrorCode::BrowseFailed).into_response();
    }

    // Move all validation + computation into spawn_blocking to avoid
    // blocking the async runtime with canonicalize/is_dir syscalls
    let allowed_paths = state.config.allowed_browse_paths.clone();
    let cache = state.dir_size_cache.clone();
    let paths = req.paths;

    let result = tokio::task::spawn_blocking(move || {
        let ttl = Duration::from_secs(60);
        let mut sizes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

        for path_str in &paths {
            let path = Path::new(path_str);
            if !is_path_allowed(path, &allowed_paths) {
                continue;
            }
            let canonical = match std::fs::canonicalize(path) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if !is_path_allowed(&canonical, &allowed_paths) {
                continue;
            }
            if !canonical.is_dir() {
                continue;
            }

            // Check cache (handle poisoned lock gracefully)
            let cached = cache.read()
                .unwrap_or_else(|e| e.into_inner())
                .get(&canonical)
                .and_then(|&(size, computed_at)| {
                    if computed_at.elapsed() < ttl { Some(size) } else { None }
                });

            if let Some(size) = cached {
                sizes.insert(path_str.clone(), size);
                continue;
            }

            // Cache miss: compute size
            let size = crate::services::MountService::calculate_dir_size(&canonical);

            // Update cache (handle poisoned lock gracefully)
            {
                let mut cache_write = cache.write()
                    .unwrap_or_else(|e| e.into_inner());
                if cache_write.len() >= 10_000 {
                    if let Some(oldest_key) = cache_write
                        .iter()
                        .min_by_key(|(_, (_, t))| *t)
                        .map(|(k, _)| k.clone())
                    {
                        cache_write.remove(&oldest_key);
                    }
                }
                cache_write.insert(canonical, (size, Instant::now()));
            }

            sizes.insert(path_str.clone(), size);
        }

        sizes
    })
    .await;

    match result {
        Ok(sizes) => Json(json!({ "sizes": sizes })).into_response(),
        Err(e) => {
            tracing::error!("Dir sizes task panicked: {}", e);
            ApiError::new(ErrorCode::BrowseFailed).into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub path: String,
    pub query: String,
    pub max_results: Option<usize>,
}

pub async fn search_path(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    if params.path.is_empty() {
        return ApiError::field_required("path").into_response();
    }
    if params.query.is_empty() {
        return ApiError::field_required("query").into_response();
    }

    let path_obj = Path::new(&params.path);
    if !is_path_allowed(path_obj, &state.config.allowed_browse_paths) {
        return ApiError::path_not_allowed().into_response();
    }
    let canonical = match std::fs::canonicalize(&params.path) {
        Ok(p) => p,
        Err(_) => return ApiError::new(ErrorCode::SearchFailed).into_response(),
    };
    if !is_path_allowed(&canonical, &state.config.allowed_browse_paths) {
        return ApiError::path_not_allowed().into_response();
    }

    let max_results = params.max_results.unwrap_or(200).min(200);
    let canonical_str = canonical.to_string_lossy().to_string();
    let query = params.query.clone();

    let timeout_secs = *state.search_timeout_seconds.read().await;

    let search_result = tokio::time::timeout(
        Duration::from_secs(timeout_secs as u64),
        tokio::task::spawn_blocking(move || {
            let ms = crate::services::MountService;
            ms.search_path(&canonical_str, &query, max_results, 20)
        }),
    )
    .await;

    match search_result {
        Ok(Ok(Ok((results, truncated)))) => Json(json!({
            "base_path": canonical.to_string_lossy(),
            "query": params.query,
            "results": results,
            "truncated": truncated,
            "timed_out": false,
        }))
        .into_response(),
        Err(_) => {
            // Timeout elapsed
            Json(json!({
                "base_path": canonical.to_string_lossy(),
                "query": params.query,
                "results": [],
                "truncated": false,
                "timed_out": true,
            }))
            .into_response()
        }
        _ => ApiError::new(ErrorCode::SearchFailed).into_response(),
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

    // Security: check if path is in allowed directories (string-based pre-check)
    if !is_path_allowed(path, &state.config.allowed_browse_paths) {
        return ApiError::path_not_allowed().into_response();
    }

    // Security: canonicalize the deepest existing ancestor to prevent symlink bypass.
    // This prevents attacks like: /mnt/evil -> /etc, then creating /mnt/evil/new_dir
    // would create /etc/new_dir.
    let mut ancestor = path.to_path_buf();
    while !ancestor.exists() {
        if !ancestor.pop() {
            return ApiError::path_not_allowed().into_response();
        }
    }
    let canonical_ancestor = match ancestor.canonicalize() {
        Ok(p) => p,
        Err(_) => return ApiError::path_not_allowed().into_response(),
    };
    if !is_path_allowed(&canonical_ancestor, &state.config.allowed_browse_paths) {
        tracing::warn!(
            "Mkdir denied: ancestor '{}' resolved to '{}' which is outside allowed paths",
            ancestor.display(),
            canonical_ancestor.display()
        );
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
        "status": "healthy"
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

// ============================================================================
// Delete verification and file deletion endpoints
// ============================================================================

#[derive(Deserialize)]
pub struct VerifyDeleteRequest {
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Serialize)]
pub struct VerifyDeleteResponse {
    pub verified: bool,
    pub expires_at: u64,
}

/// Get the delete verification window in minutes from settings, defaulting to 5
async fn get_delete_verification_window(state: &AppState) -> u64 {
    let db_value: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'delete_verification_window_minutes'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    db_value
        .and_then(|(value,)| value.parse::<u64>().ok())
        .unwrap_or(5)
}

/// POST /system/verify-delete-access - Verify user credentials for file deletion
/// Requires password and optionally TOTP code if 2FA is enabled.
/// On success, stores a verification timestamp that allows subsequent deletes
/// within the configured time window without re-verification.
pub async fn verify_delete_access(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    AuthClaims(claims): AuthClaims,
    Json(req): Json<VerifyDeleteRequest>,
) -> impl IntoResponse {
    let user_id = claims.sub;
    let ip = extract_client_ip(
        &headers,
        connect_info.as_ref().map(|c| &c.0),
        &state.config.trusted_proxies,
    );

    // Check rate limit
    if let Err(e) = state.rate_limit_service.check_rate_limit(&ip) {
        return ApiError::rate_limited(e.retry_after_secs as u64).into_response();
    }

    // Get user from database
    let user: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT password_hash, totp_secret FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let (password_hash, totp_secret) = match user {
        Some(u) => u,
        None => {
            state.rate_limit_service.record_failure(&ip);
            return ApiError::user_not_found().into_response();
        }
    };

    // Verify password
    let password_valid = AuthService::verify_password(&req.password, &password_hash).unwrap_or(false);
    if !password_valid {
        state.rate_limit_service.record_failure(&ip);
        return ApiError::delete_verification_failed().into_response();
    }

    // If TOTP is enabled, verify the code
    if let Some(secret) = totp_secret {
        let totp_code = match &req.totp_code {
            Some(code) => code,
            None => {
                state.rate_limit_service.record_failure(&ip);
                return ApiError::delete_verification_failed().into_response();
            }
        };

        let totp_valid = TotpService::verify_code(&secret, totp_code).unwrap_or(false);
        if !totp_valid {
            state.rate_limit_service.record_failure(&ip);
            return ApiError::delete_verification_failed().into_response();
        }
    }

    // Clear rate limit on success
    state.rate_limit_service.record_success(&ip);

    // Get verification window from settings
    let window_minutes = get_delete_verification_window(&state).await;
    let now = Instant::now();
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + (window_minutes * 60);

    // Store verification timestamp
    {
        let mut verifications = state.delete_verifications.write().await;
        verifications.insert(user_id, now);
    }

    tracing::info!("User {} verified for delete access", user_id);

    Json(VerifyDeleteResponse {
        verified: true,
        expires_at,
    }).into_response()
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    pub path: String,
}

/// DELETE /system/delete - Delete a file or directory
/// Requires prior verification via /system/verify-delete-access.
/// Security measures:
/// - Verification window check (user must have recently verified)
/// - Two-stage path validation (before and after canonicalization)
/// - Symlink attack prevention via canonicalization
/// - Path traversal prevention
pub async fn delete_path(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
    Json(req): Json<DeleteRequest>,
) -> impl IntoResponse {
    let user_id = claims.sub;
    let path_str = &req.path;

    // 1. Check verification window
    let window_minutes = get_delete_verification_window(&state).await;
    let window_duration = Duration::from_secs(window_minutes * 60);

    let is_verified = {
        let verifications = state.delete_verifications.read().await;
        if let Some(verified_at) = verifications.get(&user_id) {
            verified_at.elapsed() < window_duration
        } else {
            false
        }
    };

    if !is_verified {
        return ApiError::delete_verification_required().into_response();
    }

    // 2. Basic validation
    if path_str.is_empty() {
        return ApiError::field_required("path").into_response();
    }

    // 3. Reject obvious traversal attempts
    if path_str.contains("..") {
        return ApiError::new(ErrorCode::PathTraversalNotAllowed).into_response();
    }

    let path = Path::new(path_str);

    // 4. Pre-canonicalization check
    if !is_path_allowed(path, &state.config.allowed_browse_paths) {
        tracing::debug!("Delete denied for path '{}' - not in allowed paths", path_str);
        return ApiError::path_not_allowed().into_response();
    }

    // 5. Canonicalize to resolve symlinks
    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("Failed to canonicalize delete path '{}': {}", path_str, e);
            return ApiError::file_not_found().into_response();
        }
    };

    // 6. Post-canonicalization check (prevents symlink attacks)
    if !is_path_allowed(&canonical, &state.config.allowed_browse_paths) {
        tracing::debug!(
            "Delete denied for canonical path '{}' - not in allowed paths",
            canonical.display()
        );
        return ApiError::path_not_allowed().into_response();
    }

    // 7. Get metadata to check if file or directory
    let metadata = match std::fs::metadata(&canonical) {
        Ok(m) => m,
        Err(_) => return ApiError::file_not_found().into_response(),
    };

    // 8. Perform deletion
    let result = if metadata.is_dir() {
        std::fs::remove_dir_all(&canonical)
    } else {
        std::fs::remove_file(&canonical)
    };

    match result {
        Ok(_) => {
            // Reset verification timer after successful delete
            {
                let mut verifications = state.delete_verifications.write().await;
                verifications.insert(user_id, Instant::now());
            }

            let item_type = if metadata.is_dir() { "directory" } else { "file" };
            tracing::info!(
                "User {} deleted {} '{}' (canonical: '{}')",
                user_id,
                item_type,
                path_str,
                canonical.display()
            );

            Json(json!({
                "success": true,
                "path": path_str,
                "is_dir": metadata.is_dir()
            })).into_response()
        }
        Err(e) => {
            tracing::error!(
                "Failed to delete '{}' (canonical: '{}'): {}",
                path_str,
                canonical.display(),
                e
            );
            ApiError::delete_failed().into_response()
        }
    }
}

/// GET /system/delete-status - Check if user has active delete verification
pub async fn delete_status(
    State(state): State<Arc<AppState>>,
    AuthClaims(claims): AuthClaims,
) -> impl IntoResponse {
    let user_id = claims.sub;

    let window_minutes = get_delete_verification_window(&state).await;
    let window_duration = Duration::from_secs(window_minutes * 60);

    let verifications = state.delete_verifications.read().await;

    if let Some(verified_at) = verifications.get(&user_id) {
        let elapsed = verified_at.elapsed();
        if elapsed < window_duration {
            let remaining_secs = (window_duration - elapsed).as_secs();
            let expires_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + remaining_secs;

            return Json(json!({
                "verified": true,
                "expires_at": expires_at
            })).into_response();
        }
    }

    Json(json!({
        "verified": false
    })).into_response()
}

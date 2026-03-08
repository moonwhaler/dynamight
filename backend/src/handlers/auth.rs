use axum::{
    extract::{ConnectInfo, Multipart, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{AppendHeaders, IntoResponse},
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::errors::{ApiError, ErrorCode};
use crate::extractors::{extract_token_from_headers, AuthUser};
use crate::models::{ChangePasswordRequest, LoginRequest, User, UserResponse};
use crate::services::config_backup_service;
use crate::services::AuthService;
use crate::AppState;

/// Build a cookie string with appropriate security flags.
/// When `secure_cookies` is true, adds the `Secure` flag to prevent transmission over HTTP.
pub fn build_auth_cookie(token: &str, secure: bool) -> String {
    if secure {
        format!(
            "token={}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=86400",
            token
        )
    } else {
        format!(
            "token={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400",
            token
        )
    }
}

/// Build a cookie string to clear the auth token.
pub fn build_logout_cookie(secure: bool) -> String {
    if secure {
        "token=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0".to_string()
    } else {
        "token=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0".to_string()
    }
}

/// Check if an IP address matches any of the trusted proxy patterns.
/// Supports exact IP matches and CIDR notation (e.g., "10.0.0.0/8", "192.168.1.0/24").
fn is_trusted_proxy(ip: &str, trusted_proxies: &[String]) -> bool {
    use std::net::IpAddr;

    let client_ip: IpAddr = match ip.parse() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    for proxy in trusted_proxies {
        // Check for CIDR notation
        if let Some((network, prefix_str)) = proxy.split_once('/') {
            let network_ip: IpAddr = match network.parse() {
                Ok(ip) => ip,
                Err(_) => continue,
            };
            let prefix: u8 = match prefix_str.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            if ip_in_cidr(&client_ip, &network_ip, prefix) {
                return true;
            }
        } else {
            // Exact IP match
            if let Ok(proxy_ip) = proxy.parse::<IpAddr>() {
                if client_ip == proxy_ip {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if an IP address is within a CIDR range.
fn ip_in_cidr(ip: &std::net::IpAddr, network: &std::net::IpAddr, prefix: u8) -> bool {
    use std::net::IpAddr;

    match (ip, network) {
        (IpAddr::V4(ip), IpAddr::V4(net)) => {
            if prefix > 32 {
                return false;
            }
            let ip_bits = u32::from(*ip);
            let net_bits = u32::from(*net);
            let mask = if prefix == 0 { 0 } else { !0u32 << (32 - prefix) };
            (ip_bits & mask) == (net_bits & mask)
        }
        (IpAddr::V6(ip), IpAddr::V6(net)) => {
            if prefix > 128 {
                return false;
            }
            let ip_bits = u128::from(*ip);
            let net_bits = u128::from(*net);
            let mask = if prefix == 0 { 0 } else { !0u128 << (128 - prefix) };
            (ip_bits & mask) == (net_bits & mask)
        }
        _ => false, // IPv4/IPv6 mismatch
    }
}

/// Extract client IP address from request headers or connection info.
///
/// Security: Only trusts X-Forwarded-For and X-Real-IP headers when the direct
/// connection comes from a trusted proxy. This prevents IP spoofing attacks
/// where malicious clients forge these headers to bypass rate limiting.
///
/// When trusted_proxies is empty, only the direct connection IP is used (safest default).
/// When the connection is from a trusted proxy, uses the rightmost untrusted IP
/// from X-Forwarded-For to get the actual client IP.
pub fn extract_client_ip(
    headers: &HeaderMap,
    connect_info: Option<&SocketAddr>,
    trusted_proxies: &[String],
) -> String {
    // Get the direct connection IP
    let direct_ip = connect_info
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // If no trusted proxies configured, always use direct connection IP (safest)
    if trusted_proxies.is_empty() {
        return direct_ip;
    }

    // Only trust forwarded headers if the direct connection is from a trusted proxy
    if !is_trusted_proxy(&direct_ip, trusted_proxies) {
        return direct_ip;
    }

    // Connection is from a trusted proxy, so we can trust the forwarded headers

    // Try X-Forwarded-For header (contains chain of IPs: client, proxy1, proxy2, ...)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Find the rightmost IP that is NOT a trusted proxy
            // This is the actual client IP (proxies add themselves to the right)
            let ips: Vec<&str> = forwarded_str.split(',').map(|s| s.trim()).collect();

            // Walk from right to left, find the first non-trusted IP
            for ip in ips.iter().rev() {
                if !is_trusted_proxy(ip, trusted_proxies) {
                    return ip.to_string();
                }
            }

            // All IPs in the chain are trusted proxies, use the leftmost (original)
            if let Some(first) = ips.first() {
                return first.to_string();
            }
        }
    }

    // Try X-Real-IP header (simpler, single IP set by the proxy)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return ip.trim().to_string();
        }
    }

    // Fall back to direct connection IP
    direct_ip
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(
        &headers,
        connect_info.as_ref().map(|c| &c.0),
        &state.config.trusted_proxies,
    );

    // Check rate limit before processing
    if let Err(e) = state.rate_limit_service.check_rate_limit(&client_ip) {
        return ApiError::rate_limited(e.retry_after_secs as u64).into_response();
    }

    // Find user
    let user: Option<User> =
        sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(&req.username)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    let user = match user {
        Some(u) => u,
        None => {
            state.rate_limit_service.record_failure(&client_ip);
            return ApiError::invalid_credentials().into_response();
        }
    };

    // Verify password
    let valid = AuthService::verify_password(&req.password, &user.password_hash).unwrap_or(false);

    if !valid {
        state.rate_limit_service.record_failure(&client_ip);
        return ApiError::invalid_credentials().into_response();
    }

    // Check if 2FA is enabled
    if user.totp_enabled {
        // Create pending TOTP session
        let session_id = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::minutes(5);

        let result = sqlx::query(
            "INSERT INTO pending_totp_sessions (id, user_id, expires_at) VALUES (?, ?, ?)",
        )
        .bind(&session_id)
        .bind(user.id)
        .bind(expires_at)
        .execute(&state.db)
        .await;

        if result.is_err() {
            return ApiError::totp_setup_failed().into_response();
        }

        return Json(json!({
            "requires_totp": true,
            "pending_session_id": session_id
        }))
        .into_response();
    }

    // Generate token (no 2FA required)
    let token = match state.auth_service.generate_token(user.id) {
        Ok(t) => t,
        Err(_) => {
            return ApiError::internal_error().into_response();
        }
    };

    // Login successful (no 2FA) - clear rate limit
    state.rate_limit_service.record_success(&client_ip);

    // Set httpOnly cookie with Secure flag based on configuration
    let cookie = build_auth_cookie(&token, state.config.secure_cookies);

    (
        AppendHeaders([(SET_COOKIE, cookie)]),
        Json(json!({
            "success": true,
            "user": UserResponse::from(user)
        })),
    )
        .into_response()
}

pub async fn logout(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Clear cookie with Secure flag based on configuration
    let cookie = build_logout_cookie(state.config.secure_cookies);

    (
        AppendHeaders([(SET_COOKIE, cookie)]),
        Json(json!({"success": true})),
    )
}

pub async fn me(AuthUser(user): AuthUser) -> impl IntoResponse {
    Json(UserResponse::from(user))
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    AuthUser(user): AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    // Verify current password
    let valid =
        AuthService::verify_password(&req.current_password, &user.password_hash).unwrap_or(false);

    if !valid {
        return ApiError::password_incorrect().into_response();
    }

    // Hash new password
    let new_hash = match AuthService::hash_password(&req.new_password) {
        Ok(h) => h,
        Err(_) => {
            return ApiError::new(crate::errors::ErrorCode::PasswordHashFailed).into_response();
        }
    };

    // Update password
    let result = sqlx::query("UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&new_hash)
        .bind(user.id)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => Json(json!({"success": true})).into_response(),
        Err(_) => ApiError::internal_error().into_response(),
    }
}

pub async fn setup_required(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    Json(json!({
        "setup_required": user_count.0 == 0
    }))
}

/// GET /auth/token - Return the current JWT token for WebSocket authentication
pub async fn get_token(headers: axum::http::HeaderMap) -> impl IntoResponse {
    match extract_token_from_headers(&headers) {
        Some(t) => Json(json!({"token": t})).into_response(),
        None => ApiError::not_authenticated().into_response(),
    }
}

pub async fn setup(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetupRequest>,
) -> impl IntoResponse {
    // Check if any users already exist
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    if user_count.0 > 0 {
        return ApiError::setup_already_done().into_response();
    }

    // Validate input
    let username = req.username.trim();
    if username.is_empty() || username.len() < 3 {
        return ApiError::username_too_short().into_response();
    }

    if req.password.len() < 8 {
        return ApiError::password_too_short().into_response();
    }

    // Hash password and create user
    let password_hash = match AuthService::hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => {
            return ApiError::new(crate::errors::ErrorCode::PasswordHashFailed).into_response();
        }
    };

    let result = sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
        .bind(username)
        .bind(&password_hash)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => {
            tracing::info!("Initial admin user '{}' created via setup", username);
            Json(json!({"success": true})).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create admin user: {}", e);
            ApiError::internal_error().into_response()
        }
    }
}

/// POST /auth/setup-from-backup - Restore from a .dmbackup file during initial setup (no user exists)
pub async fn setup_from_backup(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Guard: only allowed when no users exist
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    if user_count.0 > 0 {
        return ApiError::setup_already_done().into_response();
    }

    // Parse multipart fields: file + password
    let mut file_data: Option<Vec<u8>> = None;
    let mut password: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| ApiError::new(ErrorCode::BackupInvalidFormat))
                        .unwrap_or_default()
                        .to_vec(),
                );
            }
            "password" => {
                password = Some(
                    field
                        .text()
                        .await
                        .map_err(|_| ApiError::new(ErrorCode::BackupInvalidFormat))
                        .unwrap_or_default(),
                );
            }
            _ => {}
        }
    }

    let file_data = match file_data {
        Some(d) if !d.is_empty() => d,
        _ => return ApiError::new(ErrorCode::BackupInvalidFormat).into_response(),
    };
    let password = match password {
        Some(p) if p.len() >= 8 => p,
        _ => return ApiError::new(ErrorCode::BackupPasswordTooShort).into_response(),
    };

    // Decrypt backup
    let yaml = match config_backup_service::decrypt_backup(&file_data, &password) {
        Ok(y) => y,
        Err(e) => return map_backup_error(e).into_response(),
    };

    // Parse
    let backup = match config_backup_service::from_yaml(&yaml) {
        Ok(b) => b,
        Err(e) => return map_backup_error(e).into_response(),
    };

    let username = backup.user.username.clone();

    // First, create the user from the backup so apply_import's Replace strategy can find it
    let create_user_result = sqlx::query(
        "INSERT INTO users (username, password_hash, totp_enabled, totp_secret) VALUES (?, ?, ?, ?)",
    )
    .bind(&backup.user.username)
    .bind(&backup.user.password_hash)
    .bind(backup.user.totp_enabled)
    .bind(&backup.user.totp_secret)
    .execute(&state.db)
    .await;

    let user_id = match create_user_result {
        Ok(r) => r.last_insert_rowid(),
        Err(e) => {
            tracing::error!("Failed to create user from backup: {}", e);
            return ApiError::internal_error().into_response();
        }
    };

    // Import recovery codes
    for code_hash in &backup.user.recovery_codes {
        if let Err(e) =
            sqlx::query("INSERT INTO recovery_codes (user_id, code_hash) VALUES (?, ?)")
                .bind(user_id)
                .bind(code_hash)
                .execute(&state.db)
                .await
        {
            tracing::warn!("Failed to import recovery code: {}", e);
        }
    }

    // Apply the rest of the backup (settings, credentials, jobs) using Replace strategy
    let result = config_backup_service::apply_import(
        &state.db,
        &state.credential_service,
        backup,
        config_backup_service::ImportStrategy::Replace,
    )
    .await;

    match result {
        Ok(_) => {
            tracing::info!(
                "Setup from backup completed: user '{}' restored",
                username
            );
            Json(json!({
                "success": true,
                "username": username
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Setup from backup failed: {}", e);
            // Clean up the user we just created
            sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
                .bind(user_id)
                .execute(&state.db)
                .await
                .ok();
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(user_id)
                .execute(&state.db)
                .await
                .ok();
            map_backup_error(e).into_response()
        }
    }
}

/// Map config_backup_service errors to API errors.
fn map_backup_error(e: anyhow::Error) -> ApiError {
    let msg = e.to_string();
    if msg.starts_with("BACKUP_INVALID_PASSWORD") {
        ApiError::new(ErrorCode::BackupInvalidPassword)
    } else if msg.starts_with("BACKUP_INVALID_FORMAT") {
        ApiError::new(ErrorCode::BackupInvalidFormat)
    } else if msg.starts_with("BACKUP_UNSUPPORTED_VERSION") {
        ApiError::new(ErrorCode::BackupUnsupportedVersion)
    } else {
        tracing::error!("Backup operation failed: {}", e);
        ApiError::internal_error()
    }
}

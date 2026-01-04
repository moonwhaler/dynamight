use axum::{
    extract::{ConnectInfo, State},
    http::{header::SET_COOKIE, HeaderMap, StatusCode},
    response::{AppendHeaders, IntoResponse},
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::models::{ChangePasswordRequest, LoginRequest, User, UserResponse};
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

/// Extract client IP address from request headers or connection info.
/// Prioritizes X-Forwarded-For and X-Real-IP headers for reverse proxy setups.
pub fn extract_client_ip(headers: &HeaderMap, connect_info: Option<&SocketAddr>) -> String {
    // Try X-Forwarded-For header first (common with reverse proxies)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Take the first IP in the chain (original client)
            if let Some(ip) = forwarded_str.split(',').next() {
                return ip.trim().to_string();
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip) = real_ip.to_str() {
            return ip.trim().to_string();
        }
    }

    // Fall back to connection info
    if let Some(addr) = connect_info {
        return addr.ip().to_string();
    }

    // Last resort
    "unknown".to_string()
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
    let client_ip = extract_client_ip(&headers, connect_info.as_ref().map(|c| &c.0));

    // Check rate limit before processing
    if let Err(e) = state.rate_limit_service.check_rate_limit(&client_ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": format!("Too many failed attempts. Please try again in {} seconds.", e.retry_after_secs),
                "retry_after": e.retry_after_secs
            })),
        )
            .into_response();
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
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"})),
            )
                .into_response()
        }
    };

    // Verify password
    let valid = AuthService::verify_password(&req.password, &user.password_hash).unwrap_or(false);

    if !valid {
        state.rate_limit_service.record_failure(&client_ip);
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"})),
        )
            .into_response();
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create 2FA session"})),
            )
                .into_response();
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
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to generate token"})),
            )
                .into_response()
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

pub async fn me(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    // Extract token from cookie
    let token = headers
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let mut parts = c.trim().splitn(2, '=');
                if parts.next() == Some("token") {
                    parts.next().map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Not authenticated"})),
            )
                .into_response()
        }
    };

    // Validate token
    let claims = match state.auth_service.validate_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            )
                .into_response()
        }
    };

    // Get user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    match user {
        Some(u) => Json(UserResponse::from(u)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        )
            .into_response(),
    }
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    // Extract and validate token
    let token = headers
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let mut parts = c.trim().splitn(2, '=');
                if parts.next() == Some("token") {
                    parts.next().map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let token = match token {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Not authenticated"}))),
    };

    let claims = match state.auth_service.validate_token(&token) {
        Ok(c) => c,
        Err(_) => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid token"}))),
    };

    // Get user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let user = match user {
        Some(u) => u,
        None => return (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))),
    };

    // Verify current password
    let valid =
        AuthService::verify_password(&req.current_password, &user.password_hash).unwrap_or(false);

    if !valid {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Current password is incorrect"})),
        );
    }

    // Hash new password
    let new_hash = match AuthService::hash_password(&req.new_password) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to hash password"})),
            )
        }
    };

    // Update password
    let result = sqlx::query("UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&new_hash)
        .bind(user.id)
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => (StatusCode::OK, Json(json!({"success": true}))),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to update password"})),
        ),
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
    // Extract token from cookie
    let token = headers
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let mut parts = c.trim().splitn(2, '=');
                if parts.next() == Some("token") {
                    parts.next().map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    match token {
        Some(t) => Json(json!({"token": t})).into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Not authenticated"})),
        )
            .into_response(),
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
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Setup has already been completed"})),
        )
            .into_response();
    }

    // Validate input
    let username = req.username.trim();
    if username.is_empty() || username.len() < 3 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Username must be at least 3 characters"})),
        )
            .into_response();
    }

    if req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Password must be at least 8 characters"})),
        )
            .into_response();
    }

    // Hash password and create user
    let password_hash = match AuthService::hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to hash password"})),
            )
                .into_response()
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create user"})),
            )
                .into_response()
        }
    }
}

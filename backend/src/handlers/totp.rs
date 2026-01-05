use axum::{
    extract::{ConnectInfo, State},
    http::{header::SET_COOKIE, HeaderMap},
    response::{AppendHeaders, IntoResponse},
    Json,
};
use chrono::Utc;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::errors::{ApiError, ErrorCode};
use crate::handlers::auth::{build_auth_cookie, extract_client_ip};
use crate::models::{
    PendingTotpSession, RecoveryCode, TotpDisableRequest, TotpEnableRequest, TotpEnableResponse,
    TotpRecoveryRequest, TotpRecoveryResponse, TotpSetupResponse, TotpStatusResponse,
    TotpValidateRequest, User, UserResponse,
};
use crate::services::{AuthService, TotpService};
use crate::AppState;

/// Helper to extract and validate JWT token from cookies
fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
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
        })
}

/// Helper to get current user from token
async fn get_current_user(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<User, ApiError> {
    let token = extract_token(headers).ok_or_else(ApiError::not_authenticated)?;

    let claims = state
        .auth_service
        .validate_token(&token)
        .map_err(|_| ApiError::token_invalid())?;

    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(claims.sub)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    user.ok_or_else(ApiError::user_not_found)
}

/// POST /auth/totp/setup - Generate a new TOTP secret and QR code
pub async fn setup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let user = match get_current_user(&state, &headers).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    let secret = TotpService::generate_secret();

    let qr_code = match TotpService::generate_qr_code(&user.username, &secret) {
        Ok(qr) => qr,
        Err(_) => {
            return ApiError::totp_setup_failed().into_response();
        }
    };

    let otpauth_url = match TotpService::get_otpauth_url(&user.username, &secret) {
        Ok(url) => url,
        Err(_) => {
            return ApiError::totp_setup_failed().into_response();
        }
    };

    Json(TotpSetupResponse {
        secret,
        qr_code,
        otpauth_url,
    })
    .into_response()
}

/// POST /auth/totp/enable - Verify code and enable 2FA
pub async fn enable(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TotpEnableRequest>,
) -> impl IntoResponse {
    let user = match get_current_user(&state, &headers).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    // Verify the TOTP code
    let valid = match TotpService::verify_code(&req.secret, &req.code) {
        Ok(v) => v,
        Err(_) => {
            return ApiError::totp_invalid_code().into_response();
        }
    };

    if !valid {
        return ApiError::totp_invalid_code().into_response();
    }

    // Generate recovery codes
    let recovery_codes = TotpService::generate_recovery_codes();

    // Start transaction
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return ApiError::internal_error().into_response();
        }
    };

    // Update user with TOTP secret
    let result = sqlx::query(
        "UPDATE users SET totp_secret = ?, totp_enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&req.secret)
    .bind(user.id)
    .execute(&mut *tx)
    .await;

    if result.is_err() {
        return ApiError::totp_setup_failed().into_response();
    }

    // Delete any existing recovery codes
    let _ = sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
        .bind(user.id)
        .execute(&mut *tx)
        .await;

    // Store hashed recovery codes
    for code in &recovery_codes {
        let hash = match TotpService::hash_recovery_code(code) {
            Ok(h) => h,
            Err(_) => {
                return ApiError::internal_error().into_response();
            }
        };

        let result = sqlx::query("INSERT INTO recovery_codes (user_id, code_hash) VALUES (?, ?)")
            .bind(user.id)
            .bind(&hash)
            .execute(&mut *tx)
            .await;

        if result.is_err() {
            return ApiError::internal_error().into_response();
        }
    }

    // Commit transaction
    if tx.commit().await.is_err() {
        return ApiError::internal_error().into_response();
    }

    tracing::info!("2FA enabled for user '{}'", user.username);

    Json(TotpEnableResponse {
        success: true,
        recovery_codes,
    })
    .into_response()
}

/// POST /auth/totp/disable - Disable 2FA (requires password and TOTP code)
pub async fn disable(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TotpDisableRequest>,
) -> impl IntoResponse {
    let user = match get_current_user(&state, &headers).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    // Verify password
    let valid = AuthService::verify_password(&req.password, &user.password_hash).unwrap_or(false);

    if !valid {
        return ApiError::password_incorrect().into_response();
    }

    // Verify TOTP code
    let secret = match &user.totp_secret {
        Some(s) => s,
        None => {
            return ApiError::totp_not_enabled().into_response();
        }
    };

    let code_valid = match TotpService::verify_code(secret, &req.code) {
        Ok(v) => v,
        Err(_) => {
            return ApiError::totp_invalid_code().into_response();
        }
    };

    if !code_valid {
        return ApiError::totp_invalid_code().into_response();
    }

    // Disable 2FA
    let result = sqlx::query(
        "UPDATE users SET totp_secret = NULL, totp_enabled = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(user.id)
    .execute(&state.db)
    .await;

    if result.is_err() {
        return ApiError::internal_error().into_response();
    }

    // Delete recovery codes
    let _ = sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
        .bind(user.id)
        .execute(&state.db)
        .await;

    tracing::info!("2FA disabled for user '{}'", user.username);

    Json(json!({"success": true})).into_response()
}

/// GET /auth/totp/status - Get 2FA status
pub async fn status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let user = match get_current_user(&state, &headers).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };

    let codes_remaining: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM recovery_codes WHERE user_id = ? AND used_at IS NULL")
            .bind(user.id)
            .fetch_one(&state.db)
            .await
            .unwrap_or((0,));

    Json(TotpStatusResponse {
        enabled: user.totp_enabled,
        recovery_codes_remaining: codes_remaining.0,
    })
    .into_response()
}

/// POST /auth/totp/validate - Complete login with TOTP code
pub async fn validate(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(req): Json<TotpValidateRequest>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, connect_info.as_ref().map(|c| &c.0));

    // Check rate limit before processing
    if let Err(e) = state.rate_limit_service.check_rate_limit(&client_ip) {
        return ApiError::rate_limited(e.retry_after_secs as u64).into_response();
    }

    // Get pending session
    let session: Option<PendingTotpSession> =
        sqlx::query_as("SELECT user_id, expires_at FROM pending_totp_sessions WHERE id = ?")
            .bind(&req.pending_session_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    let session = match session {
        Some(s) => s,
        None => {
            state.rate_limit_service.record_failure(&client_ip);
            return ApiError::new(ErrorCode::SessionExpired).into_response();
        }
    };

    // Check if session expired
    if session.expires_at < Utc::now() {
        // Clean up expired session
        let _ = sqlx::query("DELETE FROM pending_totp_sessions WHERE id = ?")
            .bind(&req.pending_session_id)
            .execute(&state.db)
            .await;

        state.rate_limit_service.record_failure(&client_ip);
        return ApiError::new(ErrorCode::SessionExpired).into_response();
    }

    // Get user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(session.user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let user = match user {
        Some(u) => u,
        None => {
            return ApiError::user_not_found().into_response();
        }
    };

    // Verify TOTP code
    let secret = match &user.totp_secret {
        Some(s) => s,
        None => {
            return ApiError::totp_not_enabled().into_response();
        }
    };

    let valid = match TotpService::verify_code(secret, &req.code) {
        Ok(v) => v,
        Err(_) => {
            return ApiError::totp_invalid_code().into_response();
        }
    };

    if !valid {
        state.rate_limit_service.record_failure(&client_ip);
        return ApiError::totp_invalid_code().into_response();
    }

    // Delete pending session
    let _ = sqlx::query("DELETE FROM pending_totp_sessions WHERE id = ?")
        .bind(&req.pending_session_id)
        .execute(&state.db)
        .await;

    // Generate token
    let token = match state.auth_service.generate_token(user.id) {
        Ok(t) => t,
        Err(_) => {
            return ApiError::internal_error().into_response();
        }
    };

    // TOTP validation successful - clear rate limit
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

/// POST /auth/totp/recovery - Complete login with recovery code
pub async fn recovery(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(req): Json<TotpRecoveryRequest>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, connect_info.as_ref().map(|c| &c.0));

    // Check rate limit before processing
    if let Err(e) = state.rate_limit_service.check_rate_limit(&client_ip) {
        return ApiError::rate_limited(e.retry_after_secs as u64).into_response();
    }

    // Get pending session
    let session: Option<PendingTotpSession> =
        sqlx::query_as("SELECT user_id, expires_at FROM pending_totp_sessions WHERE id = ?")
            .bind(&req.pending_session_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    let session = match session {
        Some(s) => s,
        None => {
            state.rate_limit_service.record_failure(&client_ip);
            return ApiError::new(ErrorCode::SessionExpired).into_response();
        }
    };

    // Check if session expired
    if session.expires_at < Utc::now() {
        let _ = sqlx::query("DELETE FROM pending_totp_sessions WHERE id = ?")
            .bind(&req.pending_session_id)
            .execute(&state.db)
            .await;

        state.rate_limit_service.record_failure(&client_ip);
        return ApiError::new(ErrorCode::SessionExpired).into_response();
    }

    // Get user
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(session.user_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);

    let user = match user {
        Some(u) => u,
        None => {
            return ApiError::user_not_found().into_response();
        }
    };

    // Get unused recovery codes
    let recovery_codes: Vec<RecoveryCode> =
        sqlx::query_as("SELECT id, code_hash FROM recovery_codes WHERE user_id = ? AND used_at IS NULL")
            .bind(user.id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

    // Check each recovery code
    let mut matched_code_id: Option<i64> = None;
    for code in &recovery_codes {
        if TotpService::verify_recovery_code(&req.recovery_code, &code.code_hash).unwrap_or(false) {
            matched_code_id = Some(code.id);
            break;
        }
    }

    let code_id = match matched_code_id {
        Some(id) => id,
        None => {
            state.rate_limit_service.record_failure(&client_ip);
            return ApiError::totp_invalid_code().into_response();
        }
    };

    // Mark recovery code as used
    let _ = sqlx::query("UPDATE recovery_codes SET used_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(code_id)
        .execute(&state.db)
        .await;

    // Delete pending session
    let _ = sqlx::query("DELETE FROM pending_totp_sessions WHERE id = ?")
        .bind(&req.pending_session_id)
        .execute(&state.db)
        .await;

    // Count remaining codes
    let codes_remaining: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM recovery_codes WHERE user_id = ? AND used_at IS NULL")
            .bind(user.id)
            .fetch_one(&state.db)
            .await
            .unwrap_or((0,));

    // Generate token
    let token = match state.auth_service.generate_token(user.id) {
        Ok(t) => t,
        Err(_) => {
            return ApiError::internal_error().into_response();
        }
    };

    // Recovery code validation successful - clear rate limit
    state.rate_limit_service.record_success(&client_ip);

    // Set httpOnly cookie with Secure flag based on configuration
    let cookie = build_auth_cookie(&token, state.config.secure_cookies);

    tracing::info!(
        "User '{}' logged in with recovery code ({} remaining)",
        user.username,
        codes_remaining.0
    );

    (
        AppendHeaders([(SET_COOKIE, cookie)]),
        Json(TotpRecoveryResponse {
            success: true,
            user: UserResponse::from(user),
            codes_remaining: codes_remaining.0,
        }),
    )
        .into_response()
}

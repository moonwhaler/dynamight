use axum::{
    extract::State,
    http::{header::SET_COOKIE, StatusCode},
    response::{AppendHeaders, IntoResponse},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::models::{ChangePasswordRequest, LoginRequest, User, UserResponse};
use crate::services::AuthService;
use crate::AppState;

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
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
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"})),
        )
            .into_response();
    }

    // Generate token
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

    // Set httpOnly cookie
    let cookie = format!(
        "token={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400",
        token
    );

    (
        AppendHeaders([(SET_COOKIE, cookie)]),
        Json(json!({
            "success": true,
            "user": UserResponse::from(user)
        })),
    )
        .into_response()
}

pub async fn logout() -> impl IntoResponse {
    // Clear cookie
    let cookie = "token=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0";

    (
        AppendHeaders([(SET_COOKIE, cookie.to_string())]),
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

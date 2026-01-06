//! Authentication extractors for Axum handlers
//!
//! These extractors are available for handlers that need access to the authenticated user.
//! The main authentication is handled by the middleware in `middleware.rs`.

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, StatusCode},
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::models::User;
use crate::AppState;

/// Extract JWT token from Cookie header.
///
/// This is the canonical function for extracting auth tokens from HTTP headers.
/// Used by extractors, middleware, and handlers that need just the token string.
pub fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
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

/// Authenticated user extractor.
///
/// Use this in handler signatures to require authentication and get the user:
/// ```rust
/// async fn my_handler(AuthUser(user): AuthUser) -> impl IntoResponse {
///     // user is guaranteed to be authenticated
/// }
/// ```
pub struct AuthUser(pub User);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_token_from_headers(&parts.headers).ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Not authenticated"})),
            )
        })?;

        let claims = state.auth_service.validate_token(&token).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid or expired token"})),
            )
        })?;

        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

        user.map(AuthUser).ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "User not found"})),
            )
        })
    }
}

/// Extract just the claims without fetching the user.
/// Useful for lightweight auth checks where you don't need the full user object.
pub struct AuthClaims(pub crate::services::Claims);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthClaims {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = extract_token_from_headers(&parts.headers).ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Not authenticated"})),
            )
        })?;

        let claims = state.auth_service.validate_token(&token).map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid or expired token"})),
            )
        })?;

        Ok(AuthClaims(claims))
    }
}

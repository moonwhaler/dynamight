//! Authentication middleware for route-level protection

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::extractors::extract_token_from_headers;
use crate::AppState;

/// Middleware that requires authentication for all requests.
///
/// Use with `axum::middleware::from_fn_with_state`:
/// ```rust
/// Router::new()
///     .route("/protected", get(handler))
///     .layer(middleware::from_fn_with_state(state.clone(), require_auth))
/// ```
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let token = extract_token_from_headers(request.headers()).ok_or_else(|| {
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

    if let Err(code) = check_session(&state, &claims).await {
        return Err((StatusCode::UNAUTHORIZED, Json(json!({ "code": code }))));
    }

    Ok(next.run(request).await)
}

/// Reject tokens that were revoked by logout, or issued before the user's
/// last password change (which invalidates all sessions).
pub async fn check_session(state: &AppState, claims: &crate::services::Claims) -> Result<(), &'static str> {
    let revoked: Option<(String,)> = sqlx::query_as("SELECT jti FROM revoked_tokens WHERE jti = ?")
        .bind(&claims.jti)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);
    if revoked.is_some() {
        return Err("SESSION_EXPIRED");
    }

    let password_changed_at: Option<(Option<chrono::DateTime<chrono::Utc>>,)> =
        sqlx::query_as("SELECT password_changed_at FROM users WHERE id = ?")
            .bind(claims.sub)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

    if let Some((Some(changed_at),)) = password_changed_at {
        if (claims.iat as i64) < changed_at.timestamp() {
            return Err("SESSION_EXPIRED");
        }
    }
    Ok(())
}

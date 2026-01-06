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

    state.auth_service.validate_token(&token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or expired token"})),
        )
    })?;

    Ok(next.run(request).await)
}

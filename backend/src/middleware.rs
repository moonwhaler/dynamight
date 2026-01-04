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
    // Extract token from cookie header
    let token = request
        .headers()
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
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Not authenticated"})),
            ))
        }
    };

    // Validate token
    match state.auth_service.validate_token(&token) {
        Ok(_claims) => {
            // Token is valid, proceed to handler
            Ok(next.run(request).await)
        }
        Err(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid or expired token"})),
        )),
    }
}

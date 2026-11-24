//! Notification settings: GET/PUT /settings/notifications and
//! POST /settings/notifications/test.
//!
//! Secrets (ntfy token, webhook Authorization header) are write-only: GET
//! reports whether they are set, and PUT/test keep the stored value when the
//! field is omitted (null), clear it with "" and replace it otherwise.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::errors::{ApiError, ErrorCode};
use crate::services::notification_service::{
    test_notification, EventSettings, NotificationSettings, NtfySettings, WebhookSettings,
};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct NotificationSettingsResponse {
    ntfy: NtfyResponse,
    webhook: WebhookResponse,
    events: EventSettings,
    base_url: String,
}

#[derive(Debug, Serialize)]
struct NtfyResponse {
    enabled: bool,
    server: String,
    topic: String,
    token_set: bool,
}

#[derive(Debug, Serialize)]
struct WebhookResponse {
    enabled: bool,
    url: String,
    auth_header_set: bool,
}

impl From<NotificationSettings> for NotificationSettingsResponse {
    fn from(s: NotificationSettings) -> Self {
        Self {
            ntfy: NtfyResponse {
                enabled: s.ntfy.enabled,
                server: s.ntfy.server,
                topic: s.ntfy.topic,
                token_set: !s.ntfy.token.is_empty(),
            },
            webhook: WebhookResponse {
                enabled: s.webhook.enabled,
                url: s.webhook.url,
                auth_header_set: !s.webhook.auth_header.is_empty(),
            },
            events: s.events,
            base_url: s.base_url,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct NtfyRequest {
    enabled: bool,
    server: String,
    topic: String,
    /// None: keep the stored token
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookRequest {
    enabled: bool,
    url: String,
    /// None: keep the stored header
    auth_header: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NotificationSettingsRequest {
    ntfy: NtfyRequest,
    webhook: WebhookRequest,
    events: EventSettings,
    #[serde(default)]
    base_url: String,
}

impl NotificationSettingsRequest {
    /// The settings this request describes, with omitted secrets taken from `stored`.
    fn merge(self, stored: &NotificationSettings) -> NotificationSettings {
        NotificationSettings {
            ntfy: NtfySettings {
                enabled: self.ntfy.enabled,
                server: self.ntfy.server.trim().to_string(),
                topic: self.ntfy.topic.trim().to_string(),
                token: self.ntfy.token.map(|t| t.trim().to_string()).unwrap_or_else(|| stored.ntfy.token.clone()),
            },
            webhook: WebhookSettings {
                enabled: self.webhook.enabled,
                url: self.webhook.url.trim().to_string(),
                auth_header: self
                    .webhook
                    .auth_header
                    .map(|h| h.trim().to_string())
                    .unwrap_or_else(|| stored.webhook.auth_header.clone()),
            },
            events: self.events,
            base_url: self.base_url.trim().to_string(),
        }
    }
}

fn invalid(field: &str) -> axum::response::Response {
    ApiError::with_params(ErrorCode::NotificationSettingsInvalid, json!({ "field": field })).into_response()
}

pub async fn get_notification_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(NotificationSettingsResponse::from(NotificationSettings::load(&state.db).await))
}

pub async fn update_notification_settings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<NotificationSettingsRequest>,
) -> impl IntoResponse {
    let stored = NotificationSettings::load(&state.db).await;
    let settings = req.merge(&stored);
    if let Err(field) = settings.validate() {
        return invalid(field);
    }
    if let Err(e) = settings.save(&state.db).await {
        tracing::error!("Failed to save notification settings: {}", e);
        return ApiError::internal_error().into_response();
    }
    Json(NotificationSettingsResponse::from(settings)).into_response()
}

/// Send a test notification with the settings in the form (not yet saved).
pub async fn test_notification_settings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<NotificationSettingsRequest>,
) -> impl IntoResponse {
    let stored = NotificationSettings::load(&state.db).await;
    let settings = req.merge(&stored);
    if let Err(field) = settings.validate() {
        return invalid(field);
    }
    let results = state.backup_service.notifier().send(&settings, &test_notification()).await;
    (StatusCode::OK, Json(json!({ "results": results }))).into_response()
}

//! Handlers for provider information and capabilities

use crate::services::providers;
use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub provider_type: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub requires_credentials: bool,
}

/// List all available providers
pub async fn list_providers() -> impl IntoResponse {
    let providers = vec![
        ProviderInfo {
            provider_type: "local",
            name: "Local / USB",
            description: "Rsync to local or mounted drives",
            requires_credentials: false,
        },
        ProviderInfo {
            provider_type: "s3",
            name: "S3 / Compatible",
            description: "AWS S3, MinIO, Backblaze B2",
            requires_credentials: true,
        },
        ProviderInfo {
            provider_type: "sftp",
            name: "SFTP",
            description: "Sync via SSH/SFTP",
            requires_credentials: true,
        },
        ProviderInfo {
            provider_type: "webdav",
            name: "WebDAV",
            description: "Nextcloud, ownCloud, etc.",
            requires_credentials: true,
        },
        ProviderInfo {
            provider_type: "google_drive",
            name: "Google Drive",
            description: "Sync to Google Drive folder",
            requires_credentials: true,
        },
        ProviderInfo {
            provider_type: "onedrive",
            name: "OneDrive",
            description: "Sync to Microsoft OneDrive",
            requires_credentials: true,
        },
    ];

    Json(providers)
}

/// Get capabilities for a specific provider
pub async fn get_provider_capabilities(Path(provider_type): Path<String>) -> impl IntoResponse {
    match providers::get_capabilities(&provider_type) {
        Some(capabilities) => Json(capabilities).into_response(),
        None => (StatusCode::NOT_FOUND, "Unknown provider type").into_response(),
    }
}

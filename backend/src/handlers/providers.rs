//! Handlers for provider information and capabilities

use crate::models::{CredentialData, CredentialDataRequest, DestinationConfig};
use crate::services::providers::{self, create_provider, TestConnectionResult};
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Request body for testing a provider connection
#[derive(Debug, Deserialize)]
pub struct TestConnectionRequest {
    pub destination: DestinationConfig,
    /// Use existing saved credential by ID
    pub credential_id: Option<i64>,
    /// Or provide credential data directly (for testing before saving)
    pub credential_data: Option<CredentialDataRequest>,
}

/// Response for test connection
#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// SSH host key fingerprint (for SFTP TOFU verification)
    /// Returned on first connection so the user can verify and save it
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_key_fingerprint: Option<String>,
}

impl From<TestConnectionResult> for TestConnectionResponse {
    fn from(result: TestConnectionResult) -> Self {
        Self {
            success: result.success,
            message: result.message,
            details: result.details,
            host_key_fingerprint: result.host_key_fingerprint,
        }
    }
}

/// Test connection to a provider
pub async fn test_connection(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TestConnectionRequest>,
) -> impl IntoResponse {
    // Get credential - either from ID or from provided data
    let credential: Option<CredentialData> = if let Some(credential_id) = request.credential_id {
        // Use existing saved credential
        match state
            .credential_service
            .get_decrypted_with_db(&state.db, credential_id)
            .await
        {
            Ok(Some(cred)) => Some(cred),
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(TestConnectionResponse {
                        success: false,
                        message: "Credential not found".to_string(),
                        details: None,
                        host_key_fingerprint: None,
                    }),
                )
                    .into_response()
            }
            Err(e) => {
                tracing::error!("Failed to get credential: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(TestConnectionResponse {
                        success: false,
                        message: "Failed to retrieve credential".to_string(),
                        details: None,
                        host_key_fingerprint: None,
                    }),
                )
                    .into_response()
            }
        }
    } else {
        // Use provided credential data directly (for testing before saving)
        request.credential_data.map(|data| data.into())
    };

    // Create provider and test connection
    let provider = create_provider(&request.destination);

    match provider
        .test_connection(&request.destination, credential.as_ref())
        .await
    {
        Ok(result) => Json(TestConnectionResponse::from(result)).into_response(),
        Err(e) => {
            let (status, message, details) = match &e {
                providers::ProviderError::ConfigError(msg) => (StatusCode::BAD_REQUEST, msg.clone(), None),
                providers::ProviderError::CredentialError(msg) => {
                    (StatusCode::UNAUTHORIZED, msg.clone(), None)
                }
                providers::ProviderError::ConnectionError(msg) => {
                    (StatusCode::BAD_GATEWAY, msg.clone(), None)
                }
                providers::ProviderError::HostKeyMismatch { expected, actual } => {
                    (
                        StatusCode::CONFLICT,
                        "SSH host key mismatch! This could indicate a Man-in-the-Middle attack.".to_string(),
                        Some(format!("Expected: {}\nReceived: {}\n\nIf you're certain this is legitimate (e.g., the server was reinstalled), update the destination with the new fingerprint.", expected, actual))
                    )
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string(), None),
            };

            (
                status,
                Json(TestConnectionResponse {
                    success: false,
                    message,
                    details,
                    host_key_fingerprint: None,
                }),
            )
                .into_response()
        }
    }
}

//! Handlers for credential management

use crate::models::{CreateCredentialRequest, UpdateCredentialRequest};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CredentialQuery {
    pub provider: Option<String>,
}

/// List all credentials or filter by provider type
pub async fn list_credentials(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CredentialQuery>,
) -> impl IntoResponse {
    let result = if let Some(provider) = query.provider {
        state.credential_service.list_by_type(&state.db, &provider).await
    } else {
        state.credential_service.list(&state.db).await
    };

    match result {
        Ok(credentials) => Json(credentials).into_response(),
        Err(e) => {
            tracing::error!("Failed to list credentials: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get a credential by ID
pub async fn get_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.credential_service.get(&state.db, id).await {
        Ok(Some(credential)) => Json(credential).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("Failed to get credential: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Create a new credential
pub async fn create_credential(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateCredentialRequest>,
) -> impl IntoResponse {
    match state.credential_service.create(&state.db, request).await {
        Ok(credential) => (StatusCode::CREATED, Json(credential)).into_response(),
        Err(e) => {
            tracing::error!("Failed to create credential: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Update a credential
pub async fn update_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateCredentialRequest>,
) -> impl IntoResponse {
    match state.credential_service.update(&state.db, id, request).await {
        Ok(Some(credential)) => Json(credential).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("Failed to update credential: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Delete a credential
pub async fn delete_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Check if credential is in use
    match state.credential_service.is_in_use(&state.db, id).await {
        Ok(true) => {
            return (
                StatusCode::CONFLICT,
                "Credential is in use by one or more jobs",
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to check credential usage: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
        _ => {}
    }

    match state.credential_service.delete(&state.db, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("Failed to delete credential: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

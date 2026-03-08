use axum::{
    extract::{Multipart, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::errors::{ApiError, ErrorCode};
use crate::extractors::AuthClaims;
use crate::services::config_backup_service::{
    self, ImportStrategy,
};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub password: String,
}

/// Parsed multipart fields for import/preview requests.
struct ImportFields {
    file_data: Vec<u8>,
    password: String,
    strategy: ImportStrategy,
}

/// Extract file, password, and strategy from a multipart request.
async fn parse_import_multipart(mut multipart: Multipart) -> Result<ImportFields, ApiError> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut password: Option<String> = None;
    let mut strategy = ImportStrategy::Merge;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                file_data = Some(field.bytes().await.map_err(|_| {
                    ApiError::new(ErrorCode::BackupInvalidFormat)
                })?.to_vec());
            }
            "password" => {
                password = Some(field.text().await.map_err(|_| {
                    ApiError::new(ErrorCode::BackupInvalidFormat)
                })?);
            }
            "strategy" => {
                let text = field.text().await.map_err(|_| {
                    ApiError::new(ErrorCode::BackupInvalidFormat)
                })?;
                strategy = match text.as_str() {
                    "replace" => ImportStrategy::Replace,
                    _ => ImportStrategy::Merge,
                };
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| ApiError::new(ErrorCode::BackupInvalidFormat))?;
    let password = password.ok_or_else(|| ApiError::new(ErrorCode::BackupPasswordTooShort))?;

    if password.len() < 8 {
        return Err(ApiError::new(ErrorCode::BackupPasswordTooShort));
    }

    Ok(ImportFields { file_data, password, strategy })
}

/// Map config_backup_service errors to API errors.
fn map_backup_error(e: anyhow::Error) -> ApiError {
    let msg = e.to_string();
    if msg.starts_with("BACKUP_INVALID_PASSWORD") {
        ApiError::new(ErrorCode::BackupInvalidPassword)
    } else if msg.starts_with("BACKUP_INVALID_FORMAT") {
        ApiError::new(ErrorCode::BackupInvalidFormat)
    } else if msg.starts_with("BACKUP_UNSUPPORTED_VERSION") {
        ApiError::new(ErrorCode::BackupUnsupportedVersion)
    } else if msg.starts_with("BACKUP_JOBS_RUNNING") {
        ApiError::new(ErrorCode::BackupJobsRunning)
    } else {
        tracing::error!("Backup operation failed: {}", e);
        ApiError::internal_error()
    }
}

/// POST /api/config/export - Export encrypted configuration backup
pub async fn export_config(
    State(state): State<Arc<AppState>>,
    AuthClaims(_claims): AuthClaims,
    Json(req): Json<ExportRequest>,
) -> impl IntoResponse {
    if req.password.len() < 8 {
        return Err(ApiError::new(ErrorCode::BackupPasswordTooShort));
    }

    let backup = config_backup_service::build_export(&state.db, &state.credential_service)
        .await
        .map_err(|e| {
            tracing::error!("Export failed: {}", e);
            ApiError::internal_error()
        })?;

    let yaml = config_backup_service::to_yaml(&backup).map_err(|e| {
        tracing::error!("YAML serialization failed: {}", e);
        ApiError::internal_error()
    })?;

    let encrypted = config_backup_service::encrypt_backup(&yaml, &req.password).map_err(|e| {
        tracing::error!("Encryption failed: {}", e);
        ApiError::internal_error()
    })?;

    let date = chrono::Utc::now().format("%Y-%m-%d");
    let filename = format!("dynamight-backup-{}.dmbackup", date);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        encrypted,
    ))
}

/// POST /api/config/import/preview - Preview what an import would do
pub async fn preview_import(
    State(state): State<Arc<AppState>>,
    AuthClaims(_claims): AuthClaims,
    multipart: Multipart,
) -> Result<Json<config_backup_service::ImportPreview>, ApiError> {
    let fields = parse_import_multipart(multipart).await?;

    let yaml = config_backup_service::decrypt_backup(&fields.file_data, &fields.password)
        .map_err(map_backup_error)?;

    let backup = config_backup_service::from_yaml(&yaml)
        .map_err(map_backup_error)?;

    let preview = config_backup_service::preview_import(&state.db, &backup, fields.strategy)
        .await
        .map_err(map_backup_error)?;

    Ok(Json(preview))
}

/// POST /api/config/import - Apply an import
pub async fn import_config(
    State(state): State<Arc<AppState>>,
    AuthClaims(_claims): AuthClaims,
    multipart: Multipart,
) -> Result<Json<config_backup_service::ImportResult>, ApiError> {
    let fields = parse_import_multipart(multipart).await?;

    let yaml = config_backup_service::decrypt_backup(&fields.file_data, &fields.password)
        .map_err(map_backup_error)?;

    let backup = config_backup_service::from_yaml(&yaml)
        .map_err(map_backup_error)?;

    let result = config_backup_service::apply_import(
        &state.db,
        &state.credential_service,
        backup,
        fields.strategy,
    )
    .await
    .map_err(map_backup_error)?;

    Ok(Json(result))
}

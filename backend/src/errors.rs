use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;

/// Structured error codes sent to the frontend for translation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Authentication errors
    InvalidCredentials,
    NotAuthenticated,
    SessionExpired,
    TokenInvalid,
    UserNotFound,
    RateLimited,

    // TOTP errors
    TotpInvalidCode,
    TotpNotEnabled,
    TotpSetupFailed,
    #[allow(dead_code)]
    TotpAlreadyEnabled,

    // Password errors
    PasswordTooShort,
    PasswordIncorrect,
    PasswordHashFailed,
    UsernameTooShort,

    // Setup errors
    SetupAlreadyDone,

    // Job errors
    JobNotFound,
    JobAlreadyRunning,
    JobNameExists,
    JobCreateFailed,
    JobUpdateFailed,
    JobDeleteFailed,
    JobCloneFailed,
    JobCancelFailed,
    NoRunningJob,

    // Schedule errors
    ScheduleNotFound,
    InvalidCron,
    ScheduleCreateFailed,
    ScheduleUpdateFailed,
    ScheduleDeleteFailed,

    // Credential errors
    CredentialNotFound,
    CredentialInUse,
    CredentialCreateFailed,
    CredentialUpdateFailed,
    #[allow(dead_code)]
    CredentialDeleteFailed,

    // Validation errors
    ValidationFieldRequired,
    ValidationFieldTooLong,
    ValidationInvalidPattern,
    SourceDirsRequired,
    #[allow(dead_code)]
    CredentialsRequired,
    SourceDirsDuplicateBasenames,
    ExcludeDirNotInSource,

    // Compress dirs validation errors
    CompressStagingPathRequired,
    CompressStagingPathNotAllowed,
    CompressStagingOverlapsSource,
    CompressInvalidCustomName,
    CompressMaxArchivesInvalid,

    // System errors
    PathNotAllowed,
    PathTraversalNotAllowed,
    DirectoryCreateFailed,
    BrowseFailed,
    SearchFailed,
    MountFailed,
    UnmountFailed,
    DrivesListFailed,
    MountsListFailed,

    // File Browser errors
    FileNotFound,
    NotAFile,
    FileTooLarge,
    DownloadFailed,
    DeleteFailed,
    DeleteVerificationRequired,
    DeleteVerificationFailed,

    // Run errors
    RunNotFound,
    RunCreateFailed,
    #[allow(dead_code)]
    RunDeleteFailed,
    #[allow(dead_code)]
    PurgeFailed,

    // Settings errors
    #[allow(dead_code)]
    SettingsSaveFailed,

    // Generic
    InternalError,
}

/// API error response with error code and optional parameters for frontend interpolation.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: ErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: ErrorCode) -> Self {
        Self { code, params: None }
    }

    pub fn with_params(code: ErrorCode, params: serde_json::Value) -> Self {
        Self {
            code,
            params: Some(params),
        }
    }

    // Convenience constructors for common error types

    pub fn invalid_credentials() -> Self {
        Self::new(ErrorCode::InvalidCredentials)
    }

    pub fn not_authenticated() -> Self {
        Self::new(ErrorCode::NotAuthenticated)
    }

    pub fn token_invalid() -> Self {
        Self::new(ErrorCode::TokenInvalid)
    }

    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self::with_params(ErrorCode::RateLimited, json!({ "seconds": retry_after_secs }))
    }

    pub fn job_not_found() -> Self {
        Self::new(ErrorCode::JobNotFound)
    }

    pub fn job_already_running() -> Self {
        Self::new(ErrorCode::JobAlreadyRunning)
    }

    pub fn job_name_exists() -> Self {
        Self::new(ErrorCode::JobNameExists)
    }

    pub fn schedule_not_found() -> Self {
        Self::new(ErrorCode::ScheduleNotFound)
    }

    pub fn invalid_cron() -> Self {
        Self::new(ErrorCode::InvalidCron)
    }

    pub fn credential_in_use() -> Self {
        Self::new(ErrorCode::CredentialInUse)
    }

    pub fn credential_not_found() -> Self {
        Self::new(ErrorCode::CredentialNotFound)
    }

    pub fn run_not_found() -> Self {
        Self::new(ErrorCode::RunNotFound)
    }

    pub fn field_required(field: &str) -> Self {
        Self::with_params(ErrorCode::ValidationFieldRequired, json!({ "field": field }))
    }

    pub fn field_too_long(field: &str, max: usize) -> Self {
        Self::with_params(
            ErrorCode::ValidationFieldTooLong,
            json!({ "field": field, "max": max }),
        )
    }

    pub fn invalid_pattern(index: usize, pattern: &str, reason: &str) -> Self {
        Self::with_params(
            ErrorCode::ValidationInvalidPattern,
            json!({ "index": index, "pattern": pattern, "reason": reason }),
        )
    }

    pub fn source_dirs_required() -> Self {
        Self::new(ErrorCode::SourceDirsRequired)
    }

    pub fn source_dirs_duplicate_basenames(duplicates: Vec<String>) -> Self {
        Self::with_params(
            ErrorCode::SourceDirsDuplicateBasenames,
            json!({ "duplicates": duplicates }),
        )
    }

    pub fn exclude_dir_not_in_source(exclude_dir: &str) -> Self {
        Self::with_params(
            ErrorCode::ExcludeDirNotInSource,
            json!({ "exclude_dir": exclude_dir }),
        )
    }

    pub fn compress_staging_path_required() -> Self {
        Self::new(ErrorCode::CompressStagingPathRequired)
    }

    pub fn compress_staging_path_not_allowed(path: &str) -> Self {
        Self::with_params(ErrorCode::CompressStagingPathNotAllowed, json!({ "path": path }))
    }

    pub fn compress_staging_overlaps_source(path: &str) -> Self {
        Self::with_params(ErrorCode::CompressStagingOverlapsSource, json!({ "path": path }))
    }

    pub fn compress_invalid_custom_name() -> Self {
        Self::new(ErrorCode::CompressInvalidCustomName)
    }

    pub fn compress_max_archives_invalid() -> Self {
        Self::new(ErrorCode::CompressMaxArchivesInvalid)
    }

    #[allow(dead_code)]
    pub fn credentials_required() -> Self {
        Self::new(ErrorCode::CredentialsRequired)
    }

    pub fn password_too_short() -> Self {
        Self::new(ErrorCode::PasswordTooShort)
    }

    pub fn password_incorrect() -> Self {
        Self::new(ErrorCode::PasswordIncorrect)
    }

    pub fn username_too_short() -> Self {
        Self::new(ErrorCode::UsernameTooShort)
    }

    pub fn setup_already_done() -> Self {
        Self::new(ErrorCode::SetupAlreadyDone)
    }

    pub fn totp_invalid_code() -> Self {
        Self::new(ErrorCode::TotpInvalidCode)
    }

    pub fn totp_not_enabled() -> Self {
        Self::new(ErrorCode::TotpNotEnabled)
    }

    #[allow(dead_code)]
    pub fn totp_already_enabled() -> Self {
        Self::new(ErrorCode::TotpAlreadyEnabled)
    }

    pub fn path_not_allowed() -> Self {
        Self::new(ErrorCode::PathNotAllowed)
    }

    pub fn file_not_found() -> Self {
        Self::new(ErrorCode::FileNotFound)
    }

    pub fn not_a_file() -> Self {
        Self::new(ErrorCode::NotAFile)
    }

    pub fn file_too_large(max_bytes: u64) -> Self {
        Self::with_params(ErrorCode::FileTooLarge, json!({ "max_bytes": max_bytes }))
    }

    pub fn download_failed() -> Self {
        Self::new(ErrorCode::DownloadFailed)
    }

    pub fn delete_failed() -> Self {
        Self::new(ErrorCode::DeleteFailed)
    }

    pub fn delete_verification_required() -> Self {
        Self::new(ErrorCode::DeleteVerificationRequired)
    }

    pub fn delete_verification_failed() -> Self {
        Self::new(ErrorCode::DeleteVerificationFailed)
    }

    pub fn internal_error() -> Self {
        Self::new(ErrorCode::InternalError)
    }

    pub fn user_not_found() -> Self {
        Self::new(ErrorCode::UserNotFound)
    }

    pub fn totp_setup_failed() -> Self {
        Self::new(ErrorCode::TotpSetupFailed)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.code {
            // 401 Unauthorized
            ErrorCode::InvalidCredentials
            | ErrorCode::NotAuthenticated
            | ErrorCode::TokenInvalid
            | ErrorCode::SessionExpired => StatusCode::UNAUTHORIZED,

            // 404 Not Found
            ErrorCode::UserNotFound
            | ErrorCode::JobNotFound
            | ErrorCode::ScheduleNotFound
            | ErrorCode::CredentialNotFound
            | ErrorCode::RunNotFound
            | ErrorCode::NoRunningJob
            | ErrorCode::FileNotFound => StatusCode::NOT_FOUND,

            // 429 Too Many Requests
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,

            // 409 Conflict
            ErrorCode::JobAlreadyRunning
            | ErrorCode::JobNameExists
            | ErrorCode::CredentialInUse
            | ErrorCode::TotpAlreadyEnabled => StatusCode::CONFLICT,

            // 403 Forbidden
            ErrorCode::SetupAlreadyDone
            | ErrorCode::PathNotAllowed
            | ErrorCode::PathTraversalNotAllowed => StatusCode::FORBIDDEN,

            // 400 Bad Request - validation errors
            ErrorCode::PasswordTooShort
            | ErrorCode::PasswordIncorrect
            | ErrorCode::UsernameTooShort
            | ErrorCode::ValidationFieldRequired
            | ErrorCode::ValidationFieldTooLong
            | ErrorCode::ValidationInvalidPattern
            | ErrorCode::SourceDirsRequired
            | ErrorCode::CredentialsRequired
            | ErrorCode::SourceDirsDuplicateBasenames
            | ErrorCode::ExcludeDirNotInSource
            | ErrorCode::InvalidCron
            | ErrorCode::TotpInvalidCode
            | ErrorCode::TotpNotEnabled
            | ErrorCode::NotAFile
            | ErrorCode::FileTooLarge
            | ErrorCode::DeleteVerificationFailed
            | ErrorCode::CompressStagingPathRequired
            | ErrorCode::CompressStagingPathNotAllowed
            | ErrorCode::CompressStagingOverlapsSource
            | ErrorCode::CompressInvalidCustomName
            | ErrorCode::CompressMaxArchivesInvalid => StatusCode::BAD_REQUEST,

            // 428 Precondition Required - verification needed
            ErrorCode::DeleteVerificationRequired => StatusCode::PRECONDITION_REQUIRED,

            // 500 Internal Server Error - everything else
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialization() {
        let error = ApiError::new(ErrorCode::InvalidCredentials);
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("INVALID_CREDENTIALS"));
    }

    #[test]
    fn test_error_with_params() {
        let error = ApiError::rate_limited(60);
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("RATE_LIMITED"));
        assert!(json.contains("60"));
    }

    #[test]
    fn test_field_error() {
        let error = ApiError::field_too_long("name", 255);
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("VALIDATION_FIELD_TOO_LONG"));
        assert!(json.contains("name"));
        assert!(json.contains("255"));
    }
}

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
    MountPointNotAllowed,
    InvalidUuidFormat,
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

    // Config backup errors
    BackupInvalidPassword,
    BackupInvalidFormat,
    BackupUnsupportedVersion,
    BackupJobsRunning,
    BackupPasswordTooShort,

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
            | ErrorCode::PathTraversalNotAllowed
            | ErrorCode::MountPointNotAllowed => StatusCode::FORBIDDEN,

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
            | ErrorCode::InvalidUuidFormat
            | ErrorCode::NotAFile
            | ErrorCode::FileTooLarge
            | ErrorCode::DeleteVerificationFailed
            | ErrorCode::CompressStagingPathRequired
            | ErrorCode::CompressStagingPathNotAllowed
            | ErrorCode::CompressStagingOverlapsSource
            | ErrorCode::CompressInvalidCustomName
            | ErrorCode::CompressMaxArchivesInvalid
            | ErrorCode::BackupInvalidPassword
            | ErrorCode::BackupInvalidFormat
            | ErrorCode::BackupUnsupportedVersion
            | ErrorCode::BackupPasswordTooShort => StatusCode::BAD_REQUEST,

            // 409 Conflict - backup refused while jobs running
            ErrorCode::BackupJobsRunning => StatusCode::CONFLICT,

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

    // --- HTTP status code tests via IntoResponse ---

    use axum::response::IntoResponse;

    #[test]
    fn test_invalid_credentials_status() {
        let resp = ApiError::invalid_credentials().into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_not_authenticated_status() {
        let resp = ApiError::not_authenticated().into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_token_invalid_status() {
        let resp = ApiError::token_invalid().into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_session_expired_status() {
        let resp = ApiError::new(ErrorCode::SessionExpired).into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_job_not_found_status() {
        let resp = ApiError::job_not_found().into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_schedule_not_found_status() {
        let resp = ApiError::schedule_not_found().into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_credential_not_found_status() {
        let resp = ApiError::credential_not_found().into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_run_not_found_status() {
        let resp = ApiError::run_not_found().into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_user_not_found_status() {
        let resp = ApiError::user_not_found().into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_file_not_found_status() {
        let resp = ApiError::file_not_found().into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_rate_limited_status() {
        let resp = ApiError::rate_limited(30).into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_setup_already_done_status() {
        let resp = ApiError::setup_already_done().into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_path_not_allowed_status() {
        let resp = ApiError::path_not_allowed().into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_password_too_short_status() {
        let resp = ApiError::password_too_short().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_password_incorrect_status() {
        let resp = ApiError::password_incorrect().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_username_too_short_status() {
        let resp = ApiError::username_too_short().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_source_dirs_required_status() {
        let resp = ApiError::source_dirs_required().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_invalid_cron_status() {
        let resp = ApiError::invalid_cron().into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_job_already_running_status() {
        let resp = ApiError::job_already_running().into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_job_name_exists_status() {
        let resp = ApiError::job_name_exists().into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_credential_in_use_status() {
        let resp = ApiError::credential_in_use().into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_delete_verification_required_status() {
        let resp = ApiError::delete_verification_required().into_response();
        assert_eq!(resp.status(), StatusCode::PRECONDITION_REQUIRED);
    }

    #[test]
    fn test_internal_error_status() {
        let resp = ApiError::internal_error().into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    // --- Convenience constructor correctness ---

    #[test]
    fn test_rate_limited_has_params() {
        let err = ApiError::rate_limited(120);
        assert!(err.params.is_some());
        let params = err.params.unwrap();
        assert_eq!(params["seconds"], 120);
    }

    #[test]
    fn test_field_required_has_params() {
        let err = ApiError::field_required("email");
        let params = err.params.unwrap();
        assert_eq!(params["field"], "email");
    }

    #[test]
    fn test_invalid_pattern_has_params() {
        let err = ApiError::invalid_pattern(2, "***", "too many wildcards");
        let params = err.params.unwrap();
        assert_eq!(params["index"], 2);
        assert_eq!(params["pattern"], "***");
        assert_eq!(params["reason"], "too many wildcards");
    }

    #[test]
    fn test_source_dirs_duplicate_basenames_has_params() {
        let err = ApiError::source_dirs_duplicate_basenames(vec!["docs".to_string(), "photos".to_string()]);
        let params = err.params.unwrap();
        let dupes = params["duplicates"].as_array().unwrap();
        assert_eq!(dupes.len(), 2);
        assert_eq!(dupes[0], "docs");
        assert_eq!(dupes[1], "photos");
    }

    #[test]
    fn test_file_too_large_has_params() {
        let err = ApiError::file_too_large(1_073_741_824);
        let params = err.params.unwrap();
        assert_eq!(params["max_bytes"], 1_073_741_824u64);
    }
}

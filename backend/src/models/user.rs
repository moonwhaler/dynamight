use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Timestamp of last password change, used for session invalidation.
    /// Tokens issued before this time are rejected by the auth middleware.
    #[serde(skip_serializing)]
    #[allow(dead_code)]
    pub password_changed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing)]
    pub pending_totp_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub totp_enabled: bool,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            totp_enabled: user.totp_enabled,
        }
    }
}

// TOTP-related request/response types

#[derive(Debug, Serialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_code: String,
    pub otpauth_url: String,
}

#[derive(Debug, Deserialize)]
pub struct TotpEnableRequest {
    pub code: String,
    /// Kept for backward compatibility but ignored — server uses its stored pending secret.
    #[allow(dead_code)]
    pub secret: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TotpEnableResponse {
    pub success: bool,
    pub recovery_codes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TotpDisableRequest {
    pub password: String,
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub recovery_codes_remaining: i64,
}

#[derive(Debug, Deserialize)]
pub struct TotpValidateRequest {
    pub pending_session_id: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct TotpRecoveryRequest {
    pub pending_session_id: String,
    pub recovery_code: String,
}

#[derive(Debug, Serialize)]
pub struct TotpRecoveryResponse {
    pub success: bool,
    pub user: UserResponse,
    pub codes_remaining: i64,
}

#[derive(Debug, FromRow)]
pub struct PendingTotpSession {
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct RecoveryCode {
    pub id: i64,
    pub code_hash: String,
}

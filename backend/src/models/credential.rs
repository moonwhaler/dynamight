//! Credential model for storing encrypted authentication data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Provider type for credentials
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    GoogleDrive,
    OneDrive,
    S3,
    Sftp,
    WebDav,
}

impl ProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GoogleDrive => "google_drive",
            Self::OneDrive => "onedrive",
            Self::S3 => "s3",
            Self::Sftp => "sftp",
            Self::WebDav => "webdav",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "google_drive" => Some(Self::GoogleDrive),
            "onedrive" => Some(Self::OneDrive),
            "s3" => Some(Self::S3),
            "sftp" => Some(Self::Sftp),
            "webdav" => Some(Self::WebDav),
            _ => None,
        }
    }
}

/// Credential record stored in database (encrypted_data is stored separately)
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Credential {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Response when listing credentials (excludes sensitive data)
#[derive(Debug, Clone, Serialize)]
pub struct CredentialResponse {
    pub id: i64,
    pub name: String,
    pub provider_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Credential> for CredentialResponse {
    fn from(c: Credential) -> Self {
        Self {
            id: c.id,
            name: c.name,
            provider_type: c.provider_type,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

/// Decrypted credential data for different provider types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CredentialData {
    /// OAuth2 credentials (Google Drive, OneDrive)
    #[serde(rename = "oauth")]
    OAuth {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },

    /// S3/AWS credentials
    S3 {
        access_key_id: String,
        secret_access_key: String,
    },

    /// SFTP/SSH credentials
    Sftp {
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        private_key: Option<String>,
        #[serde(default)]
        passphrase: Option<String>,
    },

    /// WebDAV credentials
    #[serde(rename = "webdav")]
    WebDav {
        username: String,
        password: String,
    },
}

impl CredentialData {
    /// Get the provider type for this credential data
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Self::OAuth { .. } => ProviderType::GoogleDrive, // Could also be OneDrive
            Self::S3 { .. } => ProviderType::S3,
            Self::Sftp { .. } => ProviderType::Sftp,
            Self::WebDav { .. } => ProviderType::WebDav,
        }
    }
}

/// Request to create a new credential
#[derive(Debug, Deserialize)]
pub struct CreateCredentialRequest {
    pub name: String,
    pub provider_type: String,
    pub data: CredentialDataRequest,
}

/// Credential data in request format (before encryption)
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CredentialDataRequest {
    /// S3/AWS credentials
    S3 {
        access_key_id: String,
        secret_access_key: String,
    },

    /// SFTP/SSH credentials
    Sftp {
        #[serde(default)]
        password: Option<String>,
        #[serde(default)]
        private_key: Option<String>,
        #[serde(default)]
        passphrase: Option<String>,
    },

    /// WebDAV credentials
    #[serde(rename = "webdav")]
    WebDav {
        username: String,
        password: String,
    },

    /// OAuth credentials (OneDrive, Google Drive)
    #[serde(rename = "oauth")]
    OAuth {
        access_token: String,
        refresh_token: String,
        expires_at: i64,
    },
}

impl From<CredentialDataRequest> for CredentialData {
    fn from(req: CredentialDataRequest) -> Self {
        match req {
            CredentialDataRequest::S3 { access_key_id, secret_access_key } => {
                Self::S3 { access_key_id, secret_access_key }
            }
            CredentialDataRequest::Sftp { password, private_key, passphrase } => {
                Self::Sftp { password, private_key, passphrase }
            }
            CredentialDataRequest::WebDav { username, password } => {
                Self::WebDav { username, password }
            }
            CredentialDataRequest::OAuth { access_token, refresh_token, expires_at } => {
                Self::OAuth { access_token, refresh_token, expires_at }
            }
        }
    }
}

/// Request to update a credential
#[derive(Debug, Deserialize)]
pub struct UpdateCredentialRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub data: Option<CredentialDataRequest>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_deserialization() {
        let json = r#"{
            "name": "Test OAuth",
            "provider_type": "google_drive",
            "data": {
                "type": "oauth",
                "access_token": "test_access",
                "refresh_token": "test_refresh",
                "expires_at": 1234567890
            }
        }"#;

        let result: Result<CreateCredentialRequest, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Failed to deserialize: {:?}", result.err());
    }

    #[test]
    fn test_webdav_deserialization() {
        let json = r#"{
            "name": "Test WebDAV",
            "provider_type": "webdav",
            "data": {
                "type": "webdav",
                "username": "user",
                "password": "pass"
            }
        }"#;

        let result: Result<CreateCredentialRequest, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Failed to deserialize: {:?}", result.err());
    }
}

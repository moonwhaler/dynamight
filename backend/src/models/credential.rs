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
}

impl std::str::FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "google_drive" => Ok(Self::GoogleDrive),
            "onedrive" => Ok(Self::OneDrive),
            "s3" => Ok(Self::S3),
            "sftp" => Ok(Self::Sftp),
            "webdav" => Ok(Self::WebDav),
            other => Err(format!("Unknown provider type: {}", other)),
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
    ///
    /// Access tokens expire after about an hour. With `client_id` (and, for
    /// Google, `client_secret`) set, the backend renews `access_token` from
    /// `refresh_token` before it expires and stores the new tokens.
    #[serde(rename = "oauth")]
    OAuth {
        /// May be empty: a token is then fetched with the refresh token on first use
        #[serde(default)]
        access_token: String,
        refresh_token: String,
        /// Unix timestamp (seconds) when `access_token` expires; 0 = unknown/expired
        #[serde(default)]
        expires_at: i64,
        /// OAuth client ID of the user's own Google Cloud / Azure app registration
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
        /// OAuth client secret (required by Google, optional for Microsoft public clients)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
        /// Microsoft tenant: "common" (default), "consumers", "organizations" or a tenant ID
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tenant: Option<String>,
        /// Scopes for the Microsoft refresh request (default "offline_access Files.ReadWrite.All")
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope: Option<String>,
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
    ///
    /// On update, an empty `refresh_token` keeps the stored tokens and only
    /// changes the client settings.
    #[serde(rename = "oauth")]
    OAuth {
        #[serde(default)]
        access_token: String,
        #[serde(default)]
        refresh_token: String,
        #[serde(default)]
        expires_at: i64,
        #[serde(default)]
        client_id: Option<String>,
        #[serde(default)]
        client_secret: Option<String>,
        #[serde(default)]
        tenant: Option<String>,
        #[serde(default)]
        scope: Option<String>,
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
            CredentialDataRequest::OAuth {
                access_token,
                refresh_token,
                expires_at,
                client_id,
                client_secret,
                tenant,
                scope,
            } => Self::OAuth {
                access_token,
                refresh_token,
                expires_at,
                client_id: non_empty(client_id),
                client_secret: non_empty(client_secret),
                tenant: non_empty(tenant),
                scope: non_empty(scope),
            },
        }
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
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

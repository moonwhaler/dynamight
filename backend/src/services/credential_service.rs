//! Service for managing encrypted credentials
//!
//! # Encryption Format Versions
//!
//! - **Version 1 (current)**: Uses Argon2id for key derivation with per-credential salt.
//!   Format: `MAGIC (4 bytes) + salt (16 bytes) + nonce (12 bytes) + ciphertext`
//!   Magic bytes: `[0x44, 0x4D, 0x31, 0x00]` ("DM1\0")
//!
//! - **Legacy (v0)**: Used SHA-256 with static salt for key derivation.
//!   Format: `nonce (12 bytes) + ciphertext`
//!   Detected by absence of magic prefix.
//!
//! Credentials encrypted with the legacy format are automatically upgraded to v1
//! when they are updated.

use crate::models::{
    Credential, CredentialData, CredentialResponse,
    CreateCredentialRequest, UpdateCredentialRequest,
};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use argon2::{Argon2, Algorithm, Version, Params};
use chrono::Utc;
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::sync::Arc;

use super::oauth_service::{self, OAuthProvider};

const NONCE_SIZE: usize = 12;
const SALT_SIZE: usize = 16;
/// Magic bytes to identify v1 encrypted format: "DM1\0"
const ENCRYPTION_MAGIC: [u8; 4] = [0x44, 0x4D, 0x31, 0x00];

/// Key material for credential encryption. Cheap to clone, so encryption can
/// run on a blocking thread.
#[derive(Clone)]
struct CredentialCrypto {
    /// JWT secret used for key derivation
    jwt_secret: String,
    /// Legacy encryption key (SHA-256 derived) for backward compatibility
    legacy_encryption_key: [u8; 32],
}

impl CredentialCrypto {
    /// Derive encryption key using Argon2id with per-credential salt
    fn derive_key_argon2(&self, salt: &[u8]) -> Result<[u8; 32]> {
        // Use Argon2id with secure parameters:
        // - Memory: 64 MiB (65536 KiB)
        // - Iterations: 3
        // - Parallelism: 4
        // These parameters provide good security while keeping decryption reasonably fast
        let params = Params::new(
            65536,  // m_cost: 64 MiB
            3,      // t_cost: 3 iterations
            4,      // p_cost: 4 parallel lanes
            Some(32) // output length: 32 bytes (256 bits)
        ).map_err(|e| anyhow!("Invalid Argon2 params: {}", e))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key = [0u8; 32];
        argon2.hash_password_into(self.jwt_secret.as_bytes(), salt, &mut key)
            .map_err(|e| anyhow!("Argon2 key derivation failed: {}", e))?;

        Ok(key)
    }

    /// Encrypt credential data using v1 format (Argon2id key derivation)
    fn encrypt(&self, data: &CredentialData) -> Result<Vec<u8>> {
        // Generate random salt for this credential
        let mut salt = [0u8; SALT_SIZE];
        rand::thread_rng().fill(&mut salt);

        // Derive key using Argon2id
        let key = self.derive_key_argon2(&salt)?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Serialize and encrypt
        let plaintext = serde_json::to_vec(data)?;
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Build v1 format: magic + salt + nonce + ciphertext
        let mut result = Vec::with_capacity(
            ENCRYPTION_MAGIC.len() + SALT_SIZE + NONCE_SIZE + ciphertext.len()
        );
        result.extend_from_slice(&ENCRYPTION_MAGIC);
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);

        Ok(result)
    }

    /// Decrypt credential data (supports both v1 and legacy formats)
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<CredentialData> {
        if CredentialService::is_v1_format(encrypted) {
            self.decrypt_v1(encrypted)
        } else {
            self.decrypt_legacy(encrypted)
        }
    }

    /// Decrypt v1 format: magic + salt + nonce + ciphertext
    fn decrypt_v1(&self, encrypted: &[u8]) -> Result<CredentialData> {
        let min_len = ENCRYPTION_MAGIC.len() + SALT_SIZE + NONCE_SIZE + 16; // 16 = min GCM tag
        if encrypted.len() < min_len {
            return Err(anyhow!("Invalid v1 encrypted data: too short"));
        }

        // Parse v1 format
        let mut offset = ENCRYPTION_MAGIC.len();
        let salt = &encrypted[offset..offset + SALT_SIZE];
        offset += SALT_SIZE;
        let nonce_bytes = &encrypted[offset..offset + NONCE_SIZE];
        offset += NONCE_SIZE;
        let ciphertext = &encrypted[offset..];

        // Derive key using Argon2id with the stored salt
        let key = self.derive_key_argon2(salt)?;

        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed (v1): {}", e))?;

        // Deserialize
        let data: CredentialData = serde_json::from_slice(&plaintext)?;
        Ok(data)
    }

    /// Decrypt legacy format: nonce + ciphertext (SHA-256 derived key)
    pub(crate) fn decrypt_legacy(&self, encrypted: &[u8]) -> Result<CredentialData> {
        if encrypted.len() < NONCE_SIZE {
            return Err(anyhow!("Invalid legacy encrypted data: too short"));
        }

        let cipher = Aes256Gcm::new_from_slice(&self.legacy_encryption_key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed (legacy): {}", e))?;

        // Deserialize
        let data: CredentialData = serde_json::from_slice(&plaintext)?;
        Ok(data)
    }

}

pub struct CredentialService {
    crypto: CredentialCrypto,
    db: SqlitePool,
    /// HTTP client for OAuth token refresh requests
    http: reqwest::Client,
    /// Serializes OAuth token refreshes, so concurrent runs using the same
    /// credential do not race (Microsoft rotates refresh tokens).
    refresh_lock: tokio::sync::Mutex<()>,
}

impl CredentialService {
    /// Create a new CredentialService
    pub fn new(jwt_secret: &str, db: SqlitePool) -> Self {
        // Pre-compute legacy key for backward compatibility with existing credentials
        let legacy_key = Self::derive_legacy_key(jwt_secret);

        Self {
            crypto: CredentialCrypto {
                jwt_secret: jwt_secret.to_string(),
                legacy_encryption_key: legacy_key,
            },
            db,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Derive encryption key using legacy SHA-256 method (for backward compatibility)
    fn derive_legacy_key(jwt_secret: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(jwt_secret.as_bytes());
        hasher.update(b"credential_encryption_salt");
        let result = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }

    /// Get the database pool reference
    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    /// Check if encrypted data uses the v1 format (has magic prefix)
    fn is_v1_format(encrypted: &[u8]) -> bool {
        encrypted.len() >= ENCRYPTION_MAGIC.len()
            && encrypted[..ENCRYPTION_MAGIC.len()] == ENCRYPTION_MAGIC
    }

    fn encrypt(&self, data: &CredentialData) -> Result<Vec<u8>> {
        self.crypto.encrypt(data)
    }

    /// Decrypt credential data (supports both v1 and legacy formats)
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<CredentialData> {
        self.crypto.decrypt(encrypted)
    }

    fn decrypt_legacy(&self, encrypted: &[u8]) -> Result<CredentialData> {
        self.crypto.decrypt_legacy(encrypted)
    }

    /// Encrypt on a blocking thread: the Argon2id key derivation (64 MiB)
    /// must not stall the async runtime.
    async fn encrypt_async(&self, data: &CredentialData) -> Result<Vec<u8>> {
        let crypto = self.crypto.clone();
        let data = data.clone();
        tokio::task::spawn_blocking(move || crypto.encrypt(&data))
            .await
            .map_err(|e| anyhow!("Encryption task failed: {}", e))?
    }

    /// Decrypt on a blocking thread (see `encrypt_async`).
    async fn decrypt_async(&self, encrypted: Vec<u8>) -> Result<CredentialData> {
        let crypto = self.crypto.clone();
        tokio::task::spawn_blocking(move || crypto.decrypt(&encrypted))
            .await
            .map_err(|e| anyhow!("Decryption task failed: {}", e))?
    }

    /// Create a new credential
    pub async fn create(
        &self,
        db: &SqlitePool,
        request: CreateCredentialRequest,
    ) -> Result<CredentialResponse> {
        let data: CredentialData = request.data.into();
        let encrypted = self.encrypt_async(&data).await?;

        let now = Utc::now();

        let id = sqlx::query(
            r#"
            INSERT INTO credentials (name, provider_type, encrypted_data, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&request.name)
        .bind(&request.provider_type)
        .bind(&encrypted)
        .bind(now)
        .bind(now)
        .execute(db)
        .await?
        .last_insert_rowid();

        Ok(CredentialResponse {
            id,
            name: request.name,
            provider_type: request.provider_type,
            created_at: now,
            updated_at: now,
        })
    }

    /// List all credentials (without decrypted data)
    pub async fn list(&self, db: &SqlitePool) -> Result<Vec<CredentialResponse>> {
        let credentials: Vec<Credential> = sqlx::query_as(
            "SELECT id, name, provider_type, created_at, updated_at FROM credentials ORDER BY name",
        )
        .fetch_all(db)
        .await?;

        Ok(credentials.into_iter().map(Into::into).collect())
    }

    /// List credentials by provider type
    pub async fn list_by_type(
        &self,
        db: &SqlitePool,
        provider_type: &str,
    ) -> Result<Vec<CredentialResponse>> {
        let credentials: Vec<Credential> = sqlx::query_as(
            "SELECT id, name, provider_type, created_at, updated_at FROM credentials WHERE provider_type = ? ORDER BY name",
        )
        .bind(provider_type)
        .fetch_all(db)
        .await?;

        Ok(credentials.into_iter().map(Into::into).collect())
    }

    /// Get a credential by ID (without decrypted data)
    pub async fn get(&self, db: &SqlitePool, id: i64) -> Result<Option<CredentialResponse>> {
        let credential: Option<Credential> = sqlx::query_as(
            "SELECT id, name, provider_type, created_at, updated_at FROM credentials WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;

        Ok(credential.map(Into::into))
    }

    /// Get decrypted credential data by ID (using provided db)
    pub async fn get_decrypted_with_db(&self, db: &SqlitePool, id: i64) -> Result<Option<CredentialData>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT encrypted_data FROM credentials WHERE id = ?")
                .bind(id)
                .fetch_optional(db)
                .await?;

        match row {
            Some((encrypted,)) => Ok(Some(self.decrypt_async(encrypted).await?)),
            None => Ok(None),
        }
    }

    /// Get decrypted credential data by ID (using internal db)
    pub async fn get_decrypted(&self, id: i64) -> Result<CredentialData> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT encrypted_data FROM credentials WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;

        match row {
            Some((encrypted,)) => self.decrypt_async(encrypted).await,
            None => Err(anyhow!("Credential not found: {}", id)),
        }
    }

    /// Get decrypted credential data, renewing an OAuth access token that is
    /// missing or about to expire (and storing the renewed tokens).
    ///
    /// Credentials without refresh settings (no client ID) are returned as they
    /// are; the provider then reports the expired token.
    pub async fn get_fresh(&self, id: i64) -> Result<CredentialData> {
        let (data, provider_type) = self.load_with_type(id).await?;
        let Some(provider) = OAuthProvider::from_provider_type(&provider_type) else {
            return Ok(data);
        };
        let now = Utc::now().timestamp();
        if !oauth_service::needs_refresh(&data, now) {
            return Ok(data);
        }
        if !oauth_service::can_refresh(provider, &data) {
            tracing::warn!(
                "OAuth credential {} has an expired access token but no client ID/secret to refresh it",
                id
            );
            return Ok(data);
        }

        let _guard = self.refresh_lock.lock().await;

        // Another task may have refreshed it while we waited for the lock
        let (mut data, _) = self.load_with_type(id).await?;
        if !oauth_service::needs_refresh(&data, Utc::now().timestamp()) {
            return Ok(data);
        }

        oauth_service::refresh_access_token(&self.http, provider, &mut data).await?;

        let encrypted = self.encrypt_async(&data).await?;
        sqlx::query("UPDATE credentials SET encrypted_data = ?, updated_at = ? WHERE id = ?")
            .bind(&encrypted)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.db)
            .await?;
        tracing::info!("Refreshed OAuth access token for credential {}", id);

        Ok(data)
    }

    /// Renew the access token of credential data that is not saved yet
    /// (used by "Test connection" in the credential editor).
    pub async fn refresh_unsaved(&self, provider: OAuthProvider, data: &mut CredentialData) -> Result<()> {
        oauth_service::refresh_access_token(&self.http, provider, data).await
    }

    async fn load_with_type(&self, id: i64) -> Result<(CredentialData, String)> {
        let row: Option<(Vec<u8>, String)> =
            sqlx::query_as("SELECT encrypted_data, provider_type FROM credentials WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        let (encrypted, provider_type) = row.ok_or_else(|| anyhow!("Credential not found: {}", id))?;
        Ok((self.decrypt_async(encrypted).await?, provider_type))
    }

    /// Update a credential
    pub async fn update(
        &self,
        db: &SqlitePool,
        id: i64,
        request: UpdateCredentialRequest,
    ) -> Result<Option<CredentialResponse>> {
        let existing: Option<Credential> = sqlx::query_as(
            "SELECT id, name, provider_type, created_at, updated_at FROM credentials WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;

        let Some(existing) = existing else {
            return Ok(None);
        };

        let name = request.name.unwrap_or(existing.name);
        let now = Utc::now();

        // Update encrypted data if provided
        if let Some(data_request) = request.data {
            let mut data: CredentialData = data_request.into();

            // Hold the refresh lock while merging and writing OAuth data: a token
            // refresh running at the same time would otherwise overwrite this
            // edit, or this edit would restore a refresh token Microsoft just
            // rotated out.
            let _refresh_guard = if matches!(data, CredentialData::OAuth { .. }) {
                Some(self.refresh_lock.lock().await)
            } else {
                None
            };

            // OAuth update: fields left empty keep their stored values, so the
            // client settings can be changed without re-entering tokens (and
            // the other way round).
            if let CredentialData::OAuth {
                refresh_token,
                access_token,
                expires_at,
                client_id,
                client_secret,
                tenant,
                scope,
            } = &mut data
            {
                match self.get_decrypted_with_db(db, id).await? {
                    Some(CredentialData::OAuth {
                        refresh_token: old_refresh,
                        access_token: old_access,
                        expires_at: old_expires,
                        client_id: old_client_id,
                        client_secret: old_secret,
                        tenant: old_tenant,
                        scope: old_scope,
                    }) => {
                        if refresh_token.is_empty() {
                            *refresh_token = old_refresh;
                            if access_token.is_empty() {
                                *access_token = old_access;
                                *expires_at = old_expires;
                            }
                        }
                        if client_id.is_none() {
                            *client_id = old_client_id;
                        }
                        if client_secret.is_none() {
                            *client_secret = old_secret;
                        }
                        if tenant.is_none() {
                            *tenant = old_tenant;
                        }
                        if scope.is_none() {
                            *scope = old_scope;
                        }
                    }
                    _ if refresh_token.is_empty() => return Err(anyhow!("A refresh token is required")),
                    _ => {}
                }
            }

            let encrypted = self.encrypt_async(&data).await?;

            sqlx::query("UPDATE credentials SET name = ?, encrypted_data = ?, updated_at = ? WHERE id = ?")
                .bind(&name)
                .bind(&encrypted)
                .bind(now)
                .bind(id)
                .execute(db)
                .await?;
        } else {
            sqlx::query("UPDATE credentials SET name = ?, updated_at = ? WHERE id = ?")
                .bind(&name)
                .bind(now)
                .bind(id)
                .execute(db)
                .await?;
        }

        Ok(Some(CredentialResponse {
            id,
            name,
            provider_type: existing.provider_type,
            created_at: existing.created_at,
            updated_at: now,
        }))
    }

    /// Delete a credential
    pub async fn delete(&self, db: &SqlitePool, id: i64) -> Result<bool> {
        let result = sqlx::query("DELETE FROM credentials WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Check if a credential is in use by any jobs
    pub async fn is_in_use(&self, db: &SqlitePool, id: i64) -> Result<bool> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM jobs WHERE credential_id = ?")
            .bind(id)
            .fetch_one(db)
            .await?;

        Ok(count.0 > 0)
    }

    /// Get list of jobs using this credential
    pub async fn get_usage(&self, db: &SqlitePool, id: i64) -> Result<CredentialUsage> {
        let jobs: Vec<JobSummary> = sqlx::query_as(
            "SELECT id, name FROM jobs WHERE credential_id = ? ORDER BY name"
        )
        .bind(id)
        .fetch_all(db)
        .await?;

        let count = jobs.len();
        Ok(CredentialUsage { jobs, count })
    }

    /// Migrate all legacy-encrypted credentials to v1 format (Argon2id).
    ///
    /// Returns the number of credentials that were migrated.
    /// Credentials already using v1 format are skipped.
    ///
    /// This operation is idempotent and safe to run multiple times.
    pub async fn migrate_legacy_credentials(&self, db: &SqlitePool) -> Result<MigrationResult> {
        // Fetch all credentials with their encrypted data
        let credentials: Vec<(i64, Vec<u8>)> = sqlx::query_as(
            "SELECT id, encrypted_data FROM credentials"
        )
        .fetch_all(db)
        .await?;

        let total = credentials.len();
        let mut migrated = 0;
        let mut already_v1 = 0;
        let mut errors = Vec::new();

        for (id, encrypted_data) in credentials {
            // Check if already v1 format
            if Self::is_v1_format(&encrypted_data) {
                already_v1 += 1;
                continue;
            }

            // Decrypt with legacy method, re-encrypt with v1
            match self.decrypt_legacy(&encrypted_data) {
                Ok(data) => {
                    match self.encrypt(&data) {
                        Ok(new_encrypted) => {
                            // Update in database
                            let result = sqlx::query(
                                "UPDATE credentials SET encrypted_data = ? WHERE id = ?"
                            )
                            .bind(&new_encrypted)
                            .bind(id)
                            .execute(db)
                            .await;

                            match result {
                                Ok(_) => {
                                    migrated += 1;
                                    tracing::info!("Migrated credential {} to v1 encryption format", id);
                                }
                                Err(e) => {
                                    errors.push(format!("Credential {}: database update failed: {}", id, e));
                                }
                            }
                        }
                        Err(e) => {
                            errors.push(format!("Credential {}: re-encryption failed: {}", id, e));
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Credential {}: decryption failed: {}", id, e));
                }
            }
        }

        if migrated > 0 {
            tracing::info!(
                "Credential migration complete: {} migrated, {} already v1, {} errors out of {} total",
                migrated, already_v1, errors.len(), total
            );
        }

        Ok(MigrationResult {
            total,
            migrated,
            already_v1,
            errors,
        })
    }

    /// Encrypt credential data for import (public wrapper around encrypt).
    /// Used by the config backup service when importing credentials.
    pub fn encrypt_for_import(&self, data: &CredentialData) -> Result<Vec<u8>> {
        self.encrypt(data)
    }

    /// Check if a specific credential uses legacy encryption format.
    pub async fn is_legacy_format(&self, db: &SqlitePool, id: i64) -> Result<Option<bool>> {
        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT encrypted_data FROM credentials WHERE id = ?")
                .bind(id)
                .fetch_optional(db)
                .await?;

        Ok(row.map(|(encrypted,)| !Self::is_v1_format(&encrypted)))
    }
}

/// Result of credential migration operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationResult {
    /// Total number of credentials in database
    pub total: usize,
    /// Number of credentials migrated from legacy to v1
    pub migrated: usize,
    /// Number of credentials already using v1 format
    pub already_v1: usize,
    /// Error messages for any failed migrations
    pub errors: Vec<String>,
}

/// Summary of a job for credential usage info
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct JobSummary {
    pub id: i64,
    pub name: String,
}

/// Credential usage information
#[derive(Debug, Clone, serde::Serialize)]
pub struct CredentialUsage {
    pub jobs: Vec<JobSummary>,
    pub count: usize,
}

/// Supplies a valid OAuth access token during a sync run.
///
/// Runs can take longer than an access token lives (about an hour), so the
/// providers ask for the token before each request. The token is cached in
/// memory and only renewed (via [`CredentialService::get_fresh`]) when it is
/// about to expire.
pub struct OAuthTokenSource {
    service: Arc<CredentialService>,
    credential_id: i64,
    cached: tokio::sync::Mutex<(String, i64)>,
}

impl OAuthTokenSource {
    /// Create a token source for an OAuth credential, seeded with already
    /// loaded credential data. Returns `None` for non-OAuth credentials.
    pub fn new(service: Arc<CredentialService>, credential_id: i64, data: &CredentialData) -> Option<Self> {
        match data {
            CredentialData::OAuth { access_token, expires_at, .. } => Some(Self {
                service,
                credential_id,
                cached: tokio::sync::Mutex::new((access_token.clone(), *expires_at)),
            }),
            _ => None,
        }
    }

    pub async fn access_token(&self) -> Result<String> {
        let mut cached = self.cached.lock().await;
        let now = Utc::now().timestamp();
        if !cached.0.is_empty() && cached.1 - now >= oauth_service::REFRESH_MARGIN_SECS {
            return Ok(cached.0.clone());
        }

        let data = self.service.get_fresh(self.credential_id).await?;
        let CredentialData::OAuth { access_token, expires_at, .. } = &data else {
            return Err(anyhow!("Credential {} is not an OAuth credential", self.credential_id));
        };
        // If the token could not be renewed (no refresh settings), do not retry
        // the (expensive) decrypt on every request; the provider reports the 401.
        let expires = if oauth_service::needs_refresh(&data, Utc::now().timestamp()) {
            i64::MAX
        } else {
            *expires_at
        };
        *cached = (access_token.clone(), expires);
        Ok(access_token.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_service(secret: &str) -> CredentialService {
        let db = crate::test_helpers::test_db().await;
        CredentialService::new(secret, db)
    }

    fn s3_creds() -> CredentialData {
        CredentialData::S3 {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        }
    }

    fn webdav_creds() -> CredentialData {
        CredentialData::WebDav {
            username: "admin".to_string(),
            password: "hunter2".to_string(),
        }
    }

    fn oauth_creds() -> CredentialData {
        CredentialData::OAuth {
            access_token: "ya29.a0AfH6SMA".to_string(),
            refresh_token: "1//0eXyz789".to_string(),
            expires_at: 1700000000,
            client_id: Some("client.apps.googleusercontent.com".to_string()),
            client_secret: Some("secret".to_string()),
            tenant: None,
            scope: None,
        }
    }

    #[tokio::test]
    async fn encrypt_decrypt_roundtrip_s3() {
        let service = make_service("test_secret").await;
        let data = s3_creds();
        let encrypted = service.encrypt_for_import(&data).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        match decrypted {
            CredentialData::S3 { access_key_id, secret_access_key } => {
                assert_eq!(access_key_id, "AKIAIOSFODNN7EXAMPLE");
                assert_eq!(secret_access_key, "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
            }
            other => panic!("Expected S3, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn encrypt_decrypt_roundtrip_webdav() {
        let service = make_service("test_secret").await;
        let data = webdav_creds();
        let encrypted = service.encrypt_for_import(&data).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        match decrypted {
            CredentialData::WebDav { username, password } => {
                assert_eq!(username, "admin");
                assert_eq!(password, "hunter2");
            }
            other => panic!("Expected WebDav, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn encrypt_decrypt_roundtrip_oauth() {
        let service = make_service("test_secret").await;
        let data = oauth_creds();
        let encrypted = service.encrypt_for_import(&data).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        match decrypted {
            CredentialData::OAuth { access_token, refresh_token, expires_at, client_id, .. } => {
                assert_eq!(client_id.as_deref(), Some("client.apps.googleusercontent.com"));
                assert_eq!(access_token, "ya29.a0AfH6SMA");
                assert_eq!(refresh_token, "1//0eXyz789");
                assert_eq!(expires_at, 1700000000);
            }
            other => panic!("Expected OAuth, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn v1_format_detected_correctly() {
        let service = make_service("test_secret").await;
        let encrypted = service.encrypt_for_import(&s3_creds()).unwrap();

        // v1 format starts with magic bytes
        assert!(CredentialService::is_v1_format(&encrypted));
        assert_eq!(&encrypted[..4], &ENCRYPTION_MAGIC);
    }

    #[test]
    fn random_data_not_v1_format() {
        assert!(!CredentialService::is_v1_format(&[0x00, 0x01, 0x02, 0x03, 0x04]));
        assert!(!CredentialService::is_v1_format(&[0xFF, 0xFE, 0xFD]));
    }

    #[test]
    fn empty_data_not_v1_format() {
        assert!(!CredentialService::is_v1_format(&[]));
    }

    #[tokio::test]
    async fn wrong_key_fails_decrypt() {
        let service1 = make_service("secret_one").await;
        let service2 = make_service("secret_two").await;

        let encrypted = service1.encrypt_for_import(&s3_creds()).unwrap();
        let result = service2.decrypt(&encrypted);
        assert!(result.is_err(), "Decrypting with wrong key should fail");
    }

    #[tokio::test]
    async fn too_short_data_fails_decrypt() {
        let service = make_service("test_secret").await;

        // Too short for v1 (needs magic + salt + nonce + tag minimum)
        let short_v1 = [ENCRYPTION_MAGIC.as_slice(), &[0u8; 10]].concat();
        assert!(service.decrypt(&short_v1).is_err());

        // Too short for legacy (needs at least nonce)
        let short_legacy = vec![0u8; 5];
        assert!(service.decrypt(&short_legacy).is_err());
    }
}

//! Service for managing encrypted credentials

use crate::models::{
    Credential, CredentialData, CredentialResponse,
    CreateCredentialRequest, UpdateCredentialRequest,
};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use chrono::Utc;
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

const NONCE_SIZE: usize = 12;

pub struct CredentialService {
    encryption_key: [u8; 32],
    db: SqlitePool,
}

impl CredentialService {
    /// Create a new CredentialService with encryption key derived from JWT secret
    pub fn new(jwt_secret: &str, db: SqlitePool) -> Self {
        // Derive a 256-bit key from the JWT secret using SHA-256
        let mut hasher = Sha256::new();
        hasher.update(jwt_secret.as_bytes());
        hasher.update(b"credential_encryption_salt");
        let result = hasher.finalize();

        let mut key = [0u8; 32];
        key.copy_from_slice(&result);

        Self { encryption_key: key, db }
    }

    /// Get the database pool reference
    pub fn db(&self) -> &SqlitePool {
        &self.db
    }

    /// Encrypt credential data
    fn encrypt(&self, data: &CredentialData) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
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

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend(ciphertext);

        Ok(result)
    }

    /// Decrypt credential data
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<CredentialData> {
        if encrypted.len() < NONCE_SIZE {
            return Err(anyhow!("Invalid encrypted data: too short"));
        }

        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| anyhow!("Failed to create cipher: {}", e))?;

        // Split nonce and ciphertext
        let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        // Deserialize
        let data: CredentialData = serde_json::from_slice(&plaintext)?;
        Ok(data)
    }

    /// Create a new credential
    pub async fn create(
        &self,
        db: &SqlitePool,
        request: CreateCredentialRequest,
    ) -> Result<CredentialResponse> {
        let data: CredentialData = request.data.into();
        let encrypted = self.encrypt(&data)?;

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
            Some((encrypted,)) => Ok(Some(self.decrypt(&encrypted)?)),
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
            Some((encrypted,)) => self.decrypt(&encrypted),
            None => Err(anyhow!("Credential not found: {}", id)),
        }
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
            let data: CredentialData = data_request.into();
            let encrypted = self.encrypt(&data)?;

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

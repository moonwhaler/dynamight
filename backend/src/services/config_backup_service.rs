//! Service for exporting and importing encrypted configuration backups.
//!
//! The backup file uses YAML internally (for transparency when decrypted)
//! and is encrypted with AES-256-CBC using PBKDF2 key derivation,
//! compatible with OpenSSL CLI:
//! ```
//! openssl enc -d -aes-256-cbc -salt -pbkdf2 -iter 600000 -md sha256 \
//!   -in backup.dmbackup -out backup.yaml
//! ```

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use anyhow::{anyhow, Result};
use chrono::Utc;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::{CredentialData, DestinationConfig, SyncOptions};
use crate::services::CredentialService;

/// Magic prefix line for decrypted YAML to verify correct password
const MAGIC_PREFIX_LINE: &str = "#dynamight-backup-v1";
/// Full magic prefix with trailing newline (used when writing)
const MAGIC_PREFIX: &str = "#dynamight-backup-v1\n";

/// PBKDF2 iteration count (matches openssl default for -iter)
const PBKDF2_ITERATIONS: u32 = 600_000;

/// OpenSSL salt magic bytes
const OPENSSL_SALT_MAGIC: &[u8; 8] = b"Salted__";

/// AES-256-CBC key size
const KEY_SIZE: usize = 32;
/// AES-256-CBC IV size
const IV_SIZE: usize = 16;
/// OpenSSL salt size
const SALT_SIZE: usize = 8;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

// ──────────────────────────── YAML backup structs ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBackup {
    pub version: u32,
    pub exported_at: String,
    pub app_name: String,
    pub settings: HashMap<String, serde_json::Value>,
    pub credentials: Vec<BackupCredential>,
    pub jobs: Vec<BackupJob>,
    pub user: BackupUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupCredential {
    pub name: String,
    pub provider_type: String,
    pub data: CredentialData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJob {
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub source_dirs: Vec<String>,
    pub destination_type: String,
    pub destination: DestinationConfig,
    pub sync_options: SyncOptions,
    pub credential_name: Option<String>,
    pub schedules: Vec<BackupSchedule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSchedule {
    pub enabled: bool,
    pub cron_expression: String,
    pub schedule_type: Option<String>,
    pub time_of_day: Option<String>,
    pub day_of_week: Option<i32>,
    pub day_of_month: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupUser {
    pub username: String,
    pub password_hash: String,
    pub totp_enabled: bool,
    pub totp_secret: Option<String>,
    pub recovery_codes: Vec<String>,
}

// ──────────────────────────── Import types ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportStrategy {
    Merge,
    Replace,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub valid: bool,
    pub version: u32,
    pub settings_count: usize,
    pub credentials_count: usize,
    pub jobs_count: usize,
    pub schedules_count: usize,
    pub conflicts: Vec<String>,
    pub has_oauth_credentials: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub success: bool,
    pub settings_imported: usize,
    pub credentials_imported: usize,
    pub jobs_imported: usize,
    pub schedules_imported: usize,
    pub warnings: Vec<String>,
}

// ──────────────────────────── Encryption ──────────────────────────────────────

/// Derive key + IV from password + salt using PBKDF2-HMAC-SHA256
/// (OpenSSL-compatible: key is first 32 bytes, IV is next 16 bytes)
fn derive_key_iv(password: &[u8], salt: &[u8]) -> ([u8; KEY_SIZE], [u8; IV_SIZE]) {
    let mut derived = [0u8; KEY_SIZE + IV_SIZE];
    pbkdf2::<Hmac<Sha256>>(password, salt, PBKDF2_ITERATIONS, &mut derived)
        .expect("PBKDF2 output size is valid");
    let mut key = [0u8; KEY_SIZE];
    let mut iv = [0u8; IV_SIZE];
    key.copy_from_slice(&derived[..KEY_SIZE]);
    iv.copy_from_slice(&derived[KEY_SIZE..]);
    (key, iv)
}

/// Encrypt plaintext YAML into OpenSSL-compatible AES-256-CBC format.
pub fn encrypt_backup(yaml: &str, password: &str) -> Result<Vec<u8>> {
    // Generate random salt
    let mut salt = [0u8; SALT_SIZE];
    rand::Rng::fill(&mut rand::thread_rng(), &mut salt);

    let (key, iv) = derive_key_iv(password.as_bytes(), &salt);

    let plaintext = yaml.as_bytes();
    // PKCS7 padding needs at most one extra block
    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);

    let ciphertext = Aes256CbcEnc::new_from_slices(&key, &iv)
        .map_err(|e| anyhow!("cipher init: {}", e))?
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .map_err(|e| anyhow!("encrypt: {}", e))?;

    // OpenSSL format: "Salted__" + salt(8) + ciphertext
    let mut out = Vec::with_capacity(OPENSSL_SALT_MAGIC.len() + SALT_SIZE + ciphertext.len());
    out.extend_from_slice(OPENSSL_SALT_MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(ciphertext);
    Ok(out)
}

/// Decrypt an OpenSSL-compatible AES-256-CBC blob and verify the magic prefix.
pub fn decrypt_backup(data: &[u8], password: &str) -> Result<String> {
    let min_len = OPENSSL_SALT_MAGIC.len() + SALT_SIZE + 16; // at least one AES block
    if data.len() < min_len {
        return Err(anyhow!("BACKUP_INVALID_FORMAT"));
    }

    // Check OpenSSL magic
    if &data[..8] != OPENSSL_SALT_MAGIC {
        return Err(anyhow!("BACKUP_INVALID_FORMAT"));
    }

    let salt = &data[8..16];
    let ciphertext = &data[16..];

    let (key, iv) = derive_key_iv(password.as_bytes(), salt);

    let mut buf = ciphertext.to_vec();
    let plaintext = Aes256CbcDec::new_from_slices(&key, &iv)
        .map_err(|_| anyhow!("BACKUP_INVALID_PASSWORD"))?
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| anyhow!("BACKUP_INVALID_PASSWORD"))?;

    let text = String::from_utf8(plaintext.to_vec())
        .map_err(|_| anyhow!("BACKUP_INVALID_PASSWORD"))?;

    // Verify magic prefix
    if !text.starts_with(MAGIC_PREFIX_LINE) {
        return Err(anyhow!("BACKUP_INVALID_PASSWORD"));
    }

    Ok(text)
}

// ──────────────────────────── Export ──────────────────────────────────────────

/// Build the full backup struct by reading everything from the database.
pub async fn build_export(
    db: &SqlitePool,
    credential_service: &CredentialService,
) -> Result<ConfigBackup> {
    // 1. Settings
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM app_settings")
            .fetch_all(db)
            .await?;
    let settings: HashMap<String, serde_json::Value> = rows
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();

    // 2. Credentials (with decrypted data)
    let cred_rows: Vec<(i64, String, String, Vec<u8>)> =
        sqlx::query_as("SELECT id, name, provider_type, encrypted_data FROM credentials")
            .fetch_all(db)
            .await?;

    // Track duplicate credential names
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for (_, name, _, _) in &cred_rows {
        *name_counts.entry(name.clone()).or_default() += 1;
    }
    let mut name_seen: HashMap<String, usize> = HashMap::new();

    let mut credentials = Vec::new();
    let mut cred_id_to_name: HashMap<i64, String> = HashMap::new();

    for (id, name, provider_type, encrypted_data) in cred_rows {
        let data = credential_service.decrypt(&encrypted_data)?;

        // Handle duplicate names by appending suffix
        let export_name = if name_counts.get(&name).copied().unwrap_or(0) > 1 {
            let count = name_seen.entry(name.clone()).or_default();
            *count += 1;
            if *count == 1 {
                name.clone()
            } else {
                format!("{} ({})", name, count)
            }
        } else {
            name.clone()
        };

        cred_id_to_name.insert(id, export_name.clone());
        credentials.push(BackupCredential {
            name: export_name,
            provider_type,
            data,
        });
    }

    // 3. Jobs with schedules
    let job_rows: Vec<(i64, String, Option<String>, bool, String, Option<String>, Option<String>, Option<String>, Option<i64>)> =
        sqlx::query_as(
            "SELECT id, name, description, enabled, source_dirs, destination_type, destination_config, sync_options, credential_id FROM jobs"
        )
        .fetch_all(db)
        .await?;

    let mut jobs = Vec::new();
    for (job_id, name, description, enabled, source_dirs_json, dest_type, dest_config, sync_opts, cred_id) in job_rows {
        let source_dirs: Vec<String> = serde_json::from_str(&source_dirs_json)
            .map_err(|e| anyhow!("Job '{}': invalid source_dirs JSON: {}", name, e))?;
        let destination_type = dest_type.unwrap_or_else(|| "local".to_string());
        let destination: DestinationConfig = match dest_config.as_deref() {
            Some(s) => serde_json::from_str(s)
                .map_err(|e| anyhow!("Job '{}': invalid destination_config JSON: {}", name, e))?,
            None => DestinationConfig::default(),
        };
        let sync_options: SyncOptions = match sync_opts.as_deref() {
            Some(s) => serde_json::from_str(s)
                .map_err(|e| anyhow!("Job '{}': invalid sync_options JSON: {}", name, e))?,
            None => SyncOptions::default(),
        };

        let credential_name = cred_id.and_then(|id| cred_id_to_name.get(&id).cloned());

        // Fetch schedules for this job
        let schedule_rows: Vec<(bool, String, Option<String>, Option<String>, Option<i32>, Option<i32>)> =
            sqlx::query_as(
                "SELECT enabled, cron_expression, schedule_type, time_of_day, day_of_week, day_of_month FROM schedules WHERE job_id = ?"
            )
            .bind(job_id)
            .fetch_all(db)
            .await?;

        let schedules: Vec<BackupSchedule> = schedule_rows
            .into_iter()
            .map(|(enabled, cron_expression, schedule_type, time_of_day, day_of_week, day_of_month)| {
                BackupSchedule {
                    enabled,
                    cron_expression,
                    schedule_type,
                    time_of_day,
                    day_of_week,
                    day_of_month,
                }
            })
            .collect();

        jobs.push(BackupJob {
            name,
            description,
            enabled,
            source_dirs,
            destination_type,
            destination,
            sync_options,
            credential_name,
            schedules,
        });
    }

    // 4. User (including sensitive auth data)
    let user_row: Option<(String, String, bool, Option<String>)> =
        sqlx::query_as("SELECT username, password_hash, totp_enabled, totp_secret FROM users LIMIT 1")
            .fetch_optional(db)
            .await?;

    let (username, password_hash, totp_enabled, totp_secret) =
        user_row.ok_or_else(|| anyhow!("No user found in database"))?;

    // Fetch recovery codes (stored hashed, but we export the hashes)
    let recovery_code_hashes: Vec<(String,)> =
        sqlx::query_as("SELECT code_hash FROM recovery_codes WHERE user_id = (SELECT id FROM users LIMIT 1)")
            .fetch_all(db)
            .await?;
    let recovery_codes: Vec<String> = recovery_code_hashes.into_iter().map(|(h,)| h).collect();

    let user = BackupUser {
        username,
        password_hash,
        totp_enabled,
        totp_secret,
        recovery_codes,
    };

    Ok(ConfigBackup {
        version: 1,
        exported_at: Utc::now().to_rfc3339(),
        app_name: "dynamight".to_string(),
        settings,
        credentials,
        jobs,
        user,
    })
}

// ──────────────────────────── Validate ────────────────────────────────────────

/// Validate backup structure and return warnings.
pub fn validate_backup(backup: &ConfigBackup) -> Result<Vec<String>> {
    if backup.version != 1 {
        return Err(anyhow!("BACKUP_UNSUPPORTED_VERSION"));
    }

    let mut warnings = Vec::new();

    // Check credential name references
    let cred_names: std::collections::HashSet<&str> =
        backup.credentials.iter().map(|c| c.name.as_str()).collect();

    for job in &backup.jobs {
        if let Some(ref cred_name) = job.credential_name {
            if !cred_names.contains(cred_name.as_str()) {
                warnings.push(format!(
                    "Job '{}' references credential '{}' which is not in the backup",
                    job.name, cred_name
                ));
            }
        }
    }

    // Check for OAuth credentials
    for cred in &backup.credentials {
        if matches!(cred.data, CredentialData::OAuth { .. }) {
            warnings.push(format!(
                "Credential '{}' uses OAuth tokens which may be expired on restore",
                cred.name
            ));
        }
    }

    Ok(warnings)
}

/// Generate a preview of what an import would do.
pub async fn preview_import(
    db: &SqlitePool,
    backup: &ConfigBackup,
    strategy: ImportStrategy,
) -> Result<ImportPreview> {
    let warnings = validate_backup(backup)?;

    let schedules_count: usize = backup.jobs.iter().map(|j| j.schedules.len()).sum();

    let has_oauth = backup.credentials.iter().any(|c| matches!(c.data, CredentialData::OAuth { .. }));

    let mut conflicts = Vec::new();

    if strategy == ImportStrategy::Merge {
        // Check for existing jobs/credentials by name
        for job in &backup.jobs {
            let existing: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM jobs WHERE name = ?")
                    .bind(&job.name)
                    .fetch_optional(db)
                    .await?;
            if existing.is_some() {
                conflicts.push(format!("Job '{}' already exists (will be skipped)", job.name));
            }
        }
        for cred in &backup.credentials {
            let existing: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM credentials WHERE name = ?")
                    .bind(&cred.name)
                    .fetch_optional(db)
                    .await?;
            if existing.is_some() {
                conflicts.push(format!("Credential '{}' already exists (will be skipped)", cred.name));
            }
        }
    }

    // Add validation warnings to conflicts list for display
    conflicts.extend(warnings);

    Ok(ImportPreview {
        valid: true,
        version: backup.version,
        settings_count: backup.settings.len(),
        credentials_count: backup.credentials.len(),
        jobs_count: backup.jobs.len(),
        schedules_count,
        conflicts,
        has_oauth_credentials: has_oauth,
    })
}

// ──────────────────────────── Import ─────────────────────────────────────────

/// Apply an import inside a database transaction.
pub async fn apply_import(
    db: &SqlitePool,
    credential_service: &Arc<CredentialService>,
    backup: ConfigBackup,
    strategy: ImportStrategy,
) -> Result<ImportResult> {
    let mut warnings = validate_backup(&backup)?;
    let mut settings_imported = 0usize;
    let mut credentials_imported = 0usize;
    let mut jobs_imported = 0usize;
    let mut schedules_imported = 0usize;

    let mut tx = db.begin().await?;

    // Check for running jobs inside the transaction to avoid TOCTOU race
    let running: Option<(i64,)> = sqlx::query_as(
        "SELECT jr.id FROM job_runs jr WHERE jr.status = 'running' LIMIT 1"
    )
    .fetch_optional(&mut *tx)
    .await?;

    if running.is_some() {
        return Err(anyhow!("BACKUP_JOBS_RUNNING"));
    }

    if strategy == ImportStrategy::Replace {
        // Delete all existing data (order matters for FK constraints)
        sqlx::query("DELETE FROM schedules").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM job_runs").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM jobs").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM credentials").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM app_settings").execute(&mut *tx).await?;
        sqlx::query("DELETE FROM recovery_codes").execute(&mut *tx).await?;
    }

    // 1. Import settings
    for (key, value) in &backup.settings {
        let value_str = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        if strategy == ImportStrategy::Merge {
            // Only insert if not exists
            let existing: Option<(String,)> =
                sqlx::query_as("SELECT value FROM app_settings WHERE key = ?")
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await?;
            if existing.is_some() {
                continue;
            }
        }

        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP"
        )
        .bind(key)
        .bind(&value_str)
        .execute(&mut *tx)
        .await?;
        settings_imported += 1;
    }

    // 2. Import credentials
    let mut cred_name_to_id: HashMap<String, i64> = HashMap::new();

    for cred in &backup.credentials {
        if strategy == ImportStrategy::Merge {
            let existing: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM credentials WHERE name = ?")
                    .bind(&cred.name)
                    .fetch_optional(&mut *tx)
                    .await?;
            if let Some((id,)) = existing {
                cred_name_to_id.insert(cred.name.clone(), id);
                continue;
            }
        }

        // Encrypt the credential data using the credential service
        let encrypted = credential_service.encrypt_for_import(&cred.data)?;
        let now = Utc::now();

        let id = sqlx::query(
            "INSERT INTO credentials (name, provider_type, encrypted_data, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&cred.name)
        .bind(&cred.provider_type)
        .bind(&encrypted)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();

        cred_name_to_id.insert(cred.name.clone(), id);
        credentials_imported += 1;
    }

    // Also load existing credentials for name resolution in merge mode
    if strategy == ImportStrategy::Merge {
        let existing_creds: Vec<(i64, String)> =
            sqlx::query_as("SELECT id, name FROM credentials")
                .fetch_all(&mut *tx)
                .await?;
        for (id, name) in existing_creds {
            cred_name_to_id.entry(name).or_insert(id);
        }
    }

    // 3. Import jobs with schedules
    for job in &backup.jobs {
        if strategy == ImportStrategy::Merge {
            let existing: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM jobs WHERE name = ?")
                    .bind(&job.name)
                    .fetch_optional(&mut *tx)
                    .await?;
            if existing.is_some() {
                continue;
            }
        }

        // Resolve credential name to ID
        let credential_id = job.credential_name.as_ref().and_then(|name| {
            cred_name_to_id.get(name).copied().or_else(|| {
                warnings.push(format!(
                    "Job '{}': credential '{}' not found, setting to null",
                    job.name, name
                ));
                None
            })
        });

        let source_dirs_json = serde_json::to_string(&job.source_dirs)?;
        let dest_config_json = serde_json::to_string(&job.destination)?;
        let sync_opts_json = serde_json::to_string(&job.sync_options)?;
        let now = Utc::now();

        // Extract legacy fields from destination for backwards compatibility
        let (mount_point, backup_subdir, usb_uuid, auto_mount, auto_unmount) = match &job.destination {
            DestinationConfig::Local {
                mount_point,
                backup_subdir,
                usb_uuid,
                auto_mount,
                auto_unmount,
            } => (
                mount_point.clone(),
                backup_subdir.clone(),
                usb_uuid.clone(),
                *auto_mount,
                *auto_unmount,
            ),
            _ => (
                "/mnt/backup".to_string(),
                "backups".to_string(),
                None,
                false,
                false,
            ),
        };

        let job_id = sqlx::query(
            r#"INSERT INTO jobs (
                name, description, enabled, usb_uuid, mount_point, auto_mount, auto_unmount,
                source_dirs, backup_subdir, sync_deletes, rsync_excludes,
                checksum_mode, compress, dry_run, bandwidth_limit, verbosity,
                destination_type, destination_config, sync_options, credential_id,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(&job.name)
        .bind(&job.description)
        .bind(job.enabled)
        .bind(&usb_uuid)
        .bind(&mount_point)
        .bind(auto_mount)
        .bind(auto_unmount)
        .bind(&source_dirs_json)
        .bind(&backup_subdir)
        .bind(job.sync_options.delete_extraneous)
        .bind(serde_json::to_string(&job.sync_options.exclude_patterns).ok())
        .bind(false) // checksum_mode
        .bind(false) // compress
        .bind(job.sync_options.dry_run)
        .bind(job.sync_options.bandwidth_limit_kbps)
        .bind(&job.sync_options.verbosity)
        .bind(&job.destination_type)
        .bind(&dest_config_json)
        .bind(&sync_opts_json)
        .bind(credential_id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();

        jobs_imported += 1;

        // Import schedules for this job
        for schedule in &job.schedules {
            sqlx::query(
                r#"INSERT INTO schedules (
                    job_id, enabled, cron_expression, schedule_type, time_of_day,
                    day_of_week, day_of_month, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#
            )
            .bind(job_id)
            .bind(schedule.enabled)
            .bind(&schedule.cron_expression)
            .bind(&schedule.schedule_type)
            .bind(&schedule.time_of_day)
            .bind(schedule.day_of_week)
            .bind(schedule.day_of_month)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;

            schedules_imported += 1;
        }
    }

    // 4. Import user data (always overwrite - there's only one user)
    if strategy == ImportStrategy::Replace {
        let user_exists: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users LIMIT 1")
                .fetch_optional(&mut *tx)
                .await?;

        if let Some((user_id,)) = user_exists {
            sqlx::query(
                "UPDATE users SET username = ?, password_hash = ?, totp_enabled = ?, totp_secret = ?, updated_at = ? WHERE id = ?"
            )
            .bind(&backup.user.username)
            .bind(&backup.user.password_hash)
            .bind(backup.user.totp_enabled)
            .bind(&backup.user.totp_secret)
            .bind(Utc::now())
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            // Re-import recovery codes
            sqlx::query("DELETE FROM recovery_codes WHERE user_id = ?")
                .bind(user_id)
                .execute(&mut *tx)
                .await?;

            for code_hash in &backup.user.recovery_codes {
                sqlx::query("INSERT INTO recovery_codes (user_id, code_hash) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(code_hash)
                    .execute(&mut *tx)
                    .await?;
            }

            // Clear all sessions so old auth tokens are invalidated
            sqlx::query("DELETE FROM sessions")
                .execute(&mut *tx)
                .await
                .ok(); // Ignore if sessions table doesn't exist
            sqlx::query("DELETE FROM pending_totp_sessions")
                .execute(&mut *tx)
                .await
                .ok(); // Ignore if table doesn't exist
        }
    }

    tx.commit().await?;

    Ok(ImportResult {
        success: true,
        settings_imported,
        credentials_imported,
        jobs_imported,
        schedules_imported,
        warnings,
    })
}

/// Serialize a ConfigBackup to YAML with magic prefix.
pub fn to_yaml(backup: &ConfigBackup) -> Result<String> {
    let yaml = serde_yaml::to_string(backup)?;
    Ok(format!("{}{}", MAGIC_PREFIX, yaml))
}

/// Parse YAML (with magic prefix already verified) into a ConfigBackup.
pub fn from_yaml(yaml: &str) -> Result<ConfigBackup> {
    // Strip the magic prefix line if present
    let content = if yaml.starts_with(MAGIC_PREFIX_LINE) {
        yaml.trim_start_matches(MAGIC_PREFIX_LINE).trim_start()
    } else {
        yaml
    };

    serde_yaml::from_str(content).map_err(|e| anyhow!("BACKUP_INVALID_FORMAT: {}", e))
}

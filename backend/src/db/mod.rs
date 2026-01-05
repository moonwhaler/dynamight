use sqlx::SqlitePool;

/// Run migrations for the main application database
pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    let migration_sql = include_str!("../../../migrations/001_initial.sql");

    sqlx::raw_sql(migration_sql).execute(pool).await?;

    // Add new columns to existing tables (ignore errors if already exists)
    let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN verbosity TEXT DEFAULT 'normal'")
        .execute(pool)
        .await;

    // Migrate old show_progress column to verbosity (if it exists)
    let _ = sqlx::query("UPDATE jobs SET verbosity = CASE WHEN show_progress = 1 THEN 'verbose' ELSE 'normal' END WHERE verbosity IS NULL OR verbosity = ''")
        .execute(pool)
        .await;

    // Drop old log_entries table if it exists (migrated to separate database)
    let _ = sqlx::query("DROP TABLE IF EXISTS log_entries")
        .execute(pool)
        .await;

    // Create app_settings table for application-wide settings
    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await;

    // TOTP / 2FA migrations
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN totp_secret TEXT DEFAULT NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE users ADD COLUMN totp_enabled INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    // Pending TOTP sessions for 2FA login flow
    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pending_totp_sessions (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            expires_at DATETIME NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await;

    // Recovery codes for 2FA backup authentication
    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS recovery_codes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            code_hash TEXT NOT NULL,
            used_at DATETIME DEFAULT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
    )
    .execute(pool)
    .await;

    // Provider migrations (003_providers.sql)
    // Credentials table for multi-provider support
    let _ = sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS credentials (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            provider_type TEXT NOT NULL,
            encrypted_data BLOB NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await;

    // Add provider columns to jobs table
    let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN destination_type TEXT DEFAULT 'local'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN destination_config TEXT DEFAULT NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN sync_options TEXT DEFAULT NULL")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN credential_id INTEGER REFERENCES credentials(id) ON DELETE SET NULL")
        .execute(pool)
        .await;

    // Indexes for provider tables
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_credentials_provider_type ON credentials(provider_type)")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_credential_id ON jobs(credential_id)")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_destination_type ON jobs(destination_type)")
        .execute(pool)
        .await;

    tracing::info!("Main database migrations completed");
    Ok(())
}

/// Run migrations for the separate logs database
pub async fn run_logs_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    let migration_sql = include_str!("../../../migrations/002_logs.sql");

    sqlx::raw_sql(migration_sql).execute(pool).await?;

    tracing::info!("Logs database migrations completed");
    Ok(())
}

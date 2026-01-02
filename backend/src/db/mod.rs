use sqlx::SqlitePool;

use crate::services::AuthService;

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    let migration_sql = include_str!("../../../migrations/001_initial.sql");

    sqlx::raw_sql(migration_sql).execute(pool).await?;

    tracing::info!("Database migrations completed");
    Ok(())
}

pub async fn ensure_admin_user(pool: &SqlitePool, password: &str) -> anyhow::Result<()> {
    // Check if admin user exists
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM users WHERE username = 'admin'")
        .fetch_optional(pool)
        .await?;

    if exists.is_none() {
        let password_hash = AuthService::hash_password(password)?;

        sqlx::query("INSERT INTO users (username, password_hash) VALUES ('admin', ?)")
            .bind(&password_hash)
            .execute(pool)
            .await?;

        tracing::info!("Created admin user");
    }

    Ok(())
}

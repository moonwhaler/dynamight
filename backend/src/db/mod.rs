use sqlx::SqlitePool;

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

    tracing::info!("Database migrations completed");
    Ok(())
}

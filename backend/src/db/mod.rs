use sqlx::SqlitePool;

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    let migration_sql = include_str!("../../../migrations/001_initial.sql");

    sqlx::raw_sql(migration_sql).execute(pool).await?;

    tracing::info!("Database migrations completed");
    Ok(())
}

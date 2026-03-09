use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Create an in-memory SQLite pool with migrations applied.
pub async fn test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to create test database");

    crate::db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Create a test Config with sensible defaults.
pub fn test_config() -> crate::config::Config {
    crate::config::Config {
        database_url: "sqlite::memory:".to_string(),
        logs_database_url: "sqlite::memory:".to_string(),
        jwt_secret: "test-secret-key-for-testing-only-32chars!".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        static_files_dir: "/tmp".to_string(),
        allowed_browse_paths: vec!["/mnt".to_string(), "/home".to_string(), "/tmp".to_string()],
        cors_origins: None,
        rate_limit_max_attempts: 5,
        rate_limit_window_secs: 60,
        rate_limit_lockout_secs: 60,
        rate_limit_max_lockout_secs: 3600,
        secure_cookies: false,
        max_download_size: 2_147_483_648,
        trusted_proxies: vec![],
        max_request_body_size: 10 * 1024 * 1024,
        timezone: chrono_tz::Tz::UTC,
    }
}

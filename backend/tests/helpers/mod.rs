//! Shared test helpers for integration tests.
//!
//! Provides a `TestApp` that wraps an `axum_test::TestServer` backed by
//! in-memory SQLite databases, ready for HTTP-level testing.

use axum::Router;
use axum_test::TestServer;
use dynamight::{
    build_api_router, AppState, AuthService, BackupService, Config, CredentialService,
    MountService, RateLimitConfig, RateLimitService,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

/// A self-contained test application with an in-memory database and HTTP server.
pub struct TestApp {
    pub server: TestServer,
    pub db: SqlitePool,
}

/// Create an in-memory SQLite pool with all migrations applied.
async fn test_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to create test database");

    dynamight::db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Create a separate in-memory SQLite pool for the logs database.
async fn test_logs_db() -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to create test logs database");

    dynamight::db::run_logs_migrations(&pool)
        .await
        .expect("Failed to run logs migrations");

    pool
}

/// Build a test `Config` with sensible defaults.
fn test_config() -> Config {
    Config {
        database_url: "sqlite::memory:".to_string(),
        logs_database_url: "sqlite::memory:".to_string(),
        jwt_secret: "test-secret-key-for-testing-only-32chars!".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        static_files_dir: "/tmp".to_string(),
        allowed_browse_paths: vec!["/tmp".to_string()],
        cors_origins: None,
        rate_limit_max_attempts: 100,
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

impl TestApp {
    /// Spin up a fresh test application with empty databases.
    pub async fn new() -> Self {
        let db = test_db().await;
        let logs_db = test_logs_db().await;
        let config = test_config();

        let auth_service = AuthService::new(config.jwt_secret.clone());
        let mount_service = MountService::new();
        let credential_service = Arc::new(CredentialService::new(&config.jwt_secret, db.clone()));
        let (log_tx, _) = broadcast::channel::<dynamight::models::LogMessage>(100);

        let backup_service = Arc::new(BackupService::new(
            db.clone(),
            logs_db.clone(),
            log_tx.clone(),
            Some(5),
            Arc::clone(&credential_service),
        ));

        let rate_limit_config = RateLimitConfig {
            max_attempts: config.rate_limit_max_attempts,
            window_secs: config.rate_limit_window_secs,
            lockout_secs: config.rate_limit_lockout_secs,
            max_lockout_secs: config.rate_limit_max_lockout_secs,
        };
        let rate_limit_service = RateLimitService::new(rate_limit_config);

        let state = Arc::new(AppState {
            db: db.clone(),
            logs_db,
            config,
            auth_service,
            backup_service,
            mount_service,
            rate_limit_service,
            credential_service,
            log_tx,
            delete_verifications: RwLock::new(HashMap::new()),
            search_timeout_seconds: RwLock::new(10),
            show_directory_sizes: RwLock::new(false),
            dir_size_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        });

        let router = build_api_router(state);

        let server = TestServer::new(router).expect("Failed to create test server");

        Self { server, db }
    }

    // ── Auth convenience helpers ────────────────────────────────────────

    /// Run the initial setup (create admin user) and return the username.
    pub async fn setup_admin(&self, username: &str, password: &str) {
        let resp = self
            .server
            .post("/auth/setup")
            .json(&serde_json::json!({
                "username": username,
                "password": password,
            }))
            .await;
        resp.assert_status_ok();
    }

    /// Login and return the auth cookie string (the full `Set-Cookie` value).
    pub async fn login(&self, username: &str, password: &str) -> String {
        let resp = self
            .server
            .post("/auth/login")
            .json(&serde_json::json!({
                "username": username,
                "password": password,
            }))
            .await;
        resp.assert_status_ok();

        // Extract the Set-Cookie header
        let cookie = resp
            .headers()
            .get("set-cookie")
            .expect("Login should set a cookie")
            .to_str()
            .unwrap()
            .to_string();

        cookie
    }

    /// Convenience: setup + login, returning the cookie header value.
    pub async fn setup_and_login(&self) -> String {
        self.setup_admin("admin", "password123").await;
        self.login("admin", "password123").await
    }

    /// Extract just the `token=<value>` portion suitable for a Cookie header.
    pub fn cookie_value(set_cookie: &str) -> String {
        // set_cookie looks like: "token=eyJ...; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400"
        set_cookie
            .split(';')
            .next()
            .unwrap_or("")
            .to_string()
    }
}

mod config;
mod db;
mod errors;
mod extractors;
mod handlers;
mod middleware;
mod models;
mod services;

use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use axum::http::{header, HeaderValue, Method};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Config;
use crate::services::{AuthService, BackupService, CredentialService, MountService, RateLimitConfig, RateLimitService, SchedulerService};

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub logs_db: sqlx::SqlitePool,
    pub config: Config,
    pub auth_service: AuthService,
    pub backup_service: Arc<BackupService>,
    pub mount_service: MountService,
    pub rate_limit_service: Arc<RateLimitService>,
    pub credential_service: Arc<CredentialService>,
    pub log_tx: broadcast::Sender<models::LogMessage>,
    /// Tracks when each user last verified their credentials for file deletion.
    /// Maps user_id -> last verification timestamp.
    pub delete_verifications: RwLock<HashMap<i64, Instant>>,
    /// Configurable timeout for recursive file search (in seconds).
    pub search_timeout_seconds: RwLock<u32>,
    /// Whether to calculate and show directory sizes in the file browser.
    pub show_directory_sizes: RwLock<bool>,
    /// In-memory cache for directory sizes (path -> (size, computed_at)).
    /// Uses std::sync::RwLock because it's accessed inside spawn_blocking.
    pub dir_size_cache: Arc<std::sync::RwLock<HashMap<PathBuf, (u64, Instant)>>>,
}

use tower::ServiceBuilder;

/// Build security headers layers.
/// Includes HSTS (when secure_cookies enabled), CSP, Permissions-Policy, and other security headers.
#[allow(clippy::type_complexity)] // Tower's ServiceBuilder produces complex nested types by design
fn build_security_headers(config: &Config) -> tower::ServiceBuilder<
    tower::layer::util::Stack<
        SetResponseHeaderLayer<HeaderValue>,
        tower::layer::util::Stack<
            SetResponseHeaderLayer<HeaderValue>,
            tower::layer::util::Stack<
                SetResponseHeaderLayer<HeaderValue>,
                tower::layer::util::Stack<
                    SetResponseHeaderLayer<HeaderValue>,
                    tower::layer::util::Stack<
                        SetResponseHeaderLayer<HeaderValue>,
                        tower::layer::util::Stack<
                            SetResponseHeaderLayer<HeaderValue>,
                            tower::layer::util::Identity,
                        >,
                    >,
                >,
            >,
        >,
    >,
> {
    let builder = ServiceBuilder::new();

    // Content-Security-Policy: Restrict resource loading
    // - default-src 'self': Only load resources from same origin by default
    // - script-src 'self': Scripts only from same origin
    // - style-src 'self' 'unsafe-inline': Styles from same origin + inline (needed for Svelte)
    // - img-src 'self' data: blob:: Images from same origin + data URIs (QR codes) + blobs
    // - connect-src 'self' ws: wss:: Allow same-origin + WebSocket connections
    // - font-src 'self': Fonts from same origin
    // - frame-ancestors 'none': Prevent embedding in frames (clickjacking protection)
    let builder = builder.layer(SetResponseHeaderLayer::if_not_present(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; \
             script-src 'self'; \
             style-src 'self' 'unsafe-inline'; \
             img-src 'self' data: blob:; \
             connect-src 'self' ws: wss:; \
             font-src 'self'; \
             frame-ancestors 'none'"
        ),
    ));

    // X-Content-Type-Options: Prevent MIME type sniffing
    let builder = builder.layer(SetResponseHeaderLayer::if_not_present(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    ));

    // X-Frame-Options: Prevent clickjacking (backup for older browsers)
    let builder = builder.layer(SetResponseHeaderLayer::if_not_present(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    ));

    // Referrer-Policy: Control referrer information
    let builder = builder.layer(SetResponseHeaderLayer::if_not_present(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    ));

    // Permissions-Policy: Restrict browser features not needed by this application
    let builder = builder.layer(SetResponseHeaderLayer::if_not_present(
        axum::http::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    ));

    // Strict-Transport-Security (HSTS): Only when secure cookies are enabled (HTTPS mode)
    // max-age=31536000 (1 year), includeSubDomains
    if config.secure_cookies {
        builder.layer(SetResponseHeaderLayer::if_not_present(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
    } else {
        // When not using HTTPS, add a placeholder layer to maintain consistent types
        builder.layer(SetResponseHeaderLayer::if_not_present(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=0"),
        ))
    }
}

/// Build a CORS layer based on configuration.
/// If CORS_ORIGINS is not set, only same-origin requests are allowed (most secure).
/// If CORS_ORIGINS is set, those specific origins are allowed.
fn build_cors_layer(config: &Config) -> CorsLayer {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::COOKIE,
        ])
        .allow_credentials(true);

    match &config.cors_origins {
        Some(origins) if !origins.is_empty() => {
            // Explicit origins configured
            let origins: Vec<HeaderValue> = origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            cors.allow_origin(origins)
        }
        _ => {
            // No origins configured = same-origin only (default, most secure)
            // This works because we serve frontend from the same origin
            cors
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,dynamight=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::load();

    // Ensure database directory exists
    if let Some(db_dir) = config.database_dir() {
        std::fs::create_dir_all(db_dir).ok();
    }

    // Connect to main database with create_if_missing and WAL mode for better concurrency
    let db_options = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(30));

    let db = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(db_options)
        .await?;

    // Connect to logs database (separate for performance)
    let logs_db_options = SqliteConnectOptions::from_str(&config.logs_database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(30));

    let logs_db = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(logs_db_options)
        .await?;

    // Run migrations
    tracing::info!("Running database migrations...");
    db::run_migrations(&db).await?;
    db::run_logs_migrations(&logs_db).await?;

    // Create broadcast channel for log streaming
    let (log_tx, _) = broadcast::channel::<models::LogMessage>(1000);

    // Check for max_runs_per_job in database, default to 5 if not set
    let db_max_runs: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'max_runs_per_job'"
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let max_runs_per_job = db_max_runs
        .and_then(|(value,)| value.parse::<u32>().ok())
        .or(Some(5));

    // Check for search_timeout_seconds in database, default to 10 if not set
    let db_search_timeout: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'search_timeout_seconds'"
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let search_timeout_seconds = db_search_timeout
        .and_then(|(value,)| value.parse::<u32>().ok())
        .unwrap_or(10);

    // Check for show_directory_sizes in database, default to false if not set
    let db_show_dir_sizes: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'show_directory_sizes'"
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let show_directory_sizes = db_show_dir_sizes
        .map(|(value,)| value == "true")
        .unwrap_or(false);

    // Initialize services
    let auth_service = AuthService::new(config.jwt_secret.clone());
    let mount_service = MountService::new();
    let credential_service = Arc::new(CredentialService::new(&config.jwt_secret, db.clone()));

    // Migrate any legacy-encrypted credentials to v1 format (Argon2id)
    // This is idempotent and safe to run on every startup
    match credential_service.migrate_legacy_credentials(&db).await {
        Ok(result) => {
            if result.migrated > 0 {
                tracing::info!(
                    "Credential encryption migration: {} upgraded to v1 format",
                    result.migrated
                );
            }
            if !result.errors.is_empty() {
                tracing::warn!(
                    "Credential migration had {} errors: {:?}",
                    result.errors.len(),
                    result.errors
                );
            }
        }
        Err(e) => {
            tracing::warn!("Credential migration check failed: {}", e);
        }
    }

    let backup_service = Arc::new(BackupService::new(
        db.clone(),
        logs_db.clone(),
        log_tx.clone(),
        max_runs_per_job,
        Arc::clone(&credential_service),
    ));

    // Initialize rate limiting service
    let rate_limit_config = RateLimitConfig {
        max_attempts: config.rate_limit_max_attempts,
        window_secs: config.rate_limit_window_secs,
        lockout_secs: config.rate_limit_lockout_secs,
        max_lockout_secs: config.rate_limit_max_lockout_secs,
    };
    let rate_limit_service = RateLimitService::new(rate_limit_config);
    RateLimitService::start_cleanup_task(rate_limit_service.clone());

    let state = Arc::new(AppState {
        db: db.clone(),
        logs_db: logs_db.clone(),
        config: config.clone(),
        auth_service,
        backup_service: backup_service.clone(),
        mount_service,
        rate_limit_service,
        credential_service,
        log_tx,
        delete_verifications: RwLock::new(HashMap::new()),
        search_timeout_seconds: RwLock::new(search_timeout_seconds),
        show_directory_sizes: RwLock::new(show_directory_sizes),
        dir_size_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
    });

    // Start periodic cleanup of expired pending TOTP sessions
    let cleanup_db = db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let _ = sqlx::query("DELETE FROM pending_totp_sessions WHERE expires_at < datetime('now')")
                .execute(&cleanup_db)
                .await;
        }
    });

    // Start scheduler
    let scheduler = SchedulerService::new(db.clone(), backup_service, config.timezone);
    tokio::spawn(async move {
        scheduler.start().await;
    });

    // Build router - split into public and protected routes

    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/auth/setup-required", get(handlers::auth::setup_required))
        .route("/auth/setup", post(handlers::auth::setup))
        .route("/auth/setup-from-backup", post(handlers::auth::setup_from_backup))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/totp/validate", post(handlers::totp::validate))
        .route("/auth/totp/recovery", post(handlers::totp::recovery))
        .route("/system/health", get(handlers::system::health))
        .route("/system/version", get(handlers::system::version));

    // Protected routes (authentication required)
    let protected_routes = Router::new()
        // Auth routes (protected)
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/me", get(handlers::auth::me))
        .route("/auth/token", get(handlers::auth::get_token))
        .route("/auth/change-password", post(handlers::auth::change_password))
        // TOTP / 2FA routes (protected)
        .route("/auth/totp/setup", post(handlers::totp::setup))
        .route("/auth/totp/enable", post(handlers::totp::enable))
        .route("/auth/totp/disable", post(handlers::totp::disable))
        .route("/auth/totp/status", get(handlers::totp::status))
        // Job routes
        .route("/jobs", get(handlers::jobs::list_jobs).post(handlers::jobs::create_job))
        .route("/jobs/:id", get(handlers::jobs::get_job).put(handlers::jobs::update_job).delete(handlers::jobs::delete_job))
        .route("/jobs/:id/run", post(handlers::jobs::run_job))
        .route("/jobs/:id/cancel", post(handlers::jobs::cancel_job))
        .route("/jobs/:id/clone", post(handlers::jobs::clone_job))
        .route("/jobs/:id/check-space", post(handlers::jobs::check_job_space))
        // Schedule routes
        .route("/jobs/:id/schedules", get(handlers::schedules::list_schedules).post(handlers::schedules::create_schedule))
        .route("/schedules/:id", put(handlers::schedules::update_schedule).delete(handlers::schedules::delete_schedule))
        // History and logs
        .route("/jobs/:id/runs", get(handlers::logs::list_runs).delete(handlers::logs::delete_job_runs))
        .route("/runs", delete(handlers::logs::purge_all_runs))
        .route("/runs/:id", get(handlers::logs::get_run).delete(handlers::logs::delete_run))
        .route("/runs/:id/logs", get(handlers::logs::get_logs))
        // System routes
        .route("/system/drives", get(handlers::system::list_drives))
        .route("/system/mounts", get(handlers::system::list_mounts))
        .route("/system/mount", post(handlers::system::mount_drive))
        .route("/system/unmount", post(handlers::system::unmount_drive))
        .route("/system/browse", get(handlers::system::browse_path))
        .route("/system/search", get(handlers::system::search_path))
        .route("/system/mkdir", post(handlers::system::create_directory))
        .route("/system/allowed-paths", get(handlers::system::allowed_paths))
        .route("/system/download", get(handlers::system::download_file))
        .route("/system/generate-mount-point", post(handlers::system::generate_mount_point))
        .route("/system/verify-delete-access", post(handlers::system::verify_delete_access))
        .route("/system/delete", delete(handlers::system::delete_path))
        .route("/system/dir-sizes", post(handlers::system::dir_sizes))
        .route("/system/delete-status", get(handlers::system::delete_status))
        // Settings
        .route("/settings", get(handlers::settings::get_settings).put(handlers::settings::update_settings))
        // Config backup/restore
        .route("/config/export", post(handlers::config_backup::export_config))
        .route("/config/import", post(handlers::config_backup::import_config))
        .route("/config/import/preview", post(handlers::config_backup::preview_import))
        // Credentials
        .route("/credentials", get(handlers::credentials::list_credentials).post(handlers::credentials::create_credential))
        .route("/credentials/:id", get(handlers::credentials::get_credential).put(handlers::credentials::update_credential).delete(handlers::credentials::delete_credential))
        .route("/credentials/:id/usage", get(handlers::credentials::get_credential_usage))
        // Providers
        .route("/providers", get(handlers::providers::list_providers))
        .route("/providers/:type/capabilities", get(handlers::providers::get_provider_capabilities))
        .route("/providers/test", post(handlers::providers::test_connection))
        // Apply auth middleware to all protected routes
        .layer(axum_middleware::from_fn_with_state(state.clone(), middleware::require_auth));

    // WebSocket routes (handle their own auth via query param token)
    let ws_routes = Router::new()
        .route("/ws/logs/:run_id", get(handlers::websocket::ws_logs_handler))
        .route("/ws/status", get(handlers::websocket::ws_status_handler));

    let api_routes = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(ws_routes)
        // Prevent caching of sensitive API responses
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache"),
        ))
        // Apply request body size limit to prevent DoS attacks via large payloads
        .layer(RequestBodyLimitLayer::new(state.config.max_request_body_size));

    // Build security headers layers
    let security_headers = build_security_headers(&config);

    let app = Router::new()
        .nest("/api", api_routes)
        .fallback_service(ServeDir::new(&config.static_files_dir))
        .layer(TraceLayer::new_for_http())
        .layer(build_cors_layer(&config))
        .layer(security_headers)
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

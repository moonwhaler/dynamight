//! Server binary. Everything but startup lives in the library crate
//! (`src/lib.rs`), which the integration tests use too, so the server and the
//! tests share one route tree.

use axum::Router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use axum::http::{header, HeaderValue, Method};
use axum::http::{Extensions, HeaderMap, StatusCode, Version};
use tower_http::compression::{predicate::{DefaultPredicate, NotForContentType, Predicate}, CompressionLayer};
use tower_http::{cors::CorsLayer, services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use dynamight::config::{self, Config};
use dynamight::services::{
    AuthService, BackupService, CredentialService, MountService, RateLimitConfig, RateLimitService,
    SchedulerService,
};
use dynamight::{db, models, AppState};

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

/// Handle command-line arguments before anything else starts.
///
/// The server takes no arguments, so anything given is answered here and the
/// process exits. Without this, a call like `dynamight --version` from a
/// script would start a second server against the configured database.
fn handle_args() {
    let Some(arg) = std::env::args().nth(1) else {
        return;
    };
    match arg.as_str() {
        "-V" | "--version" => {
            println!(
                "dynamight {} ({}, {})",
                env!("CARGO_PKG_VERSION"),
                env!("BUILD_GIT_HASH"),
                env!("BUILD_DATE")
            );
            std::process::exit(0);
        }
        "-h" | "--help" => {
            println!(
                "Dynamight backup server\n\n\
                 Usage: dynamight [--version | --help]\n\n\
                 Without arguments the server starts. Configuration is read from\n\
                 $DYNAMIGHT_CONFIG, ./dynamight.toml or /etc/dynamight/dynamight.toml;\n\
                 environment variables override the file (see dynamight.toml.example)."
            );
            std::process::exit(0);
        }
        other => {
            eprintln!("dynamight: unknown argument '{other}' (try --help)");
            std::process::exit(2);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    handle_args();

    // Initialize tracing. RUST_LOG wins; otherwise start with the default
    // filter (so config loading can log) and switch to [logging] level below.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().ok();
    let from_env = env_filter.is_some();
    let (filter_layer, filter_handle) = tracing_subscriber::reload::Layer::new(
        env_filter.unwrap_or_else(|| config::default_log_level().into()),
    );
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::load();

    if !from_env {
        match tracing_subscriber::EnvFilter::try_new(&config.log_level) {
            Ok(filter) => {
                if let Err(e) = filter_handle.reload(filter) {
                    tracing::warn!("Could not apply log level '{}': {}", config.log_level, e);
                }
            }
            Err(e) => tracing::warn!(
                "Invalid [logging] level '{}' ({}), keeping '{}'",
                config.log_level,
                e,
                config::default_log_level()
            ),
        }
    }

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

    let max_runs_per_job =
        dynamight::handlers::settings::max_runs_from_setting(db_max_runs.as_ref().map(|(v,)| v.as_str()));

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

    // Check for show_directory_sizes in database, default to true if not set
    let db_show_dir_sizes: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'show_directory_sizes'"
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(None);

    let show_directory_sizes = db_show_dir_sizes
        .map(|(value,)| value == "true")
        .unwrap_or(true);

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

    let db_log_days: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'log_retention_days'")
            .fetch_optional(&db)
            .await
            .unwrap_or(None);
    backup_service
        .set_log_retention_days(dynamight::handlers::settings::log_days_from_setting(
            db_log_days.as_ref().map(|(v,)| v.as_str()),
        ))
        .await;

    // No run can be active yet: anything still marked pending/running was
    // interrupted by a crash or restart.
    match backup_service.mark_interrupted_runs().await {
        Ok(0) => {}
        Ok(n) => tracing::warn!("Marked {} interrupted job run(s) as failed", n),
        Err(e) => tracing::error!("Failed to mark interrupted job runs: {}", e),
    }

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
        file_ops: dynamight::services::file_ops::FileOps::new(config.allowed_browse_paths.clone()),
        thumbs_dir: dynamight::thumbs_dir_for(&config.database_url),
    });

    // Hourly cleanup of expired sessions and tokens; log retention every
    // 6 hours (the first tick runs right away, at startup)
    let cleanup_db = db.clone();
    let maintenance = backup_service.clone();
    let browse_paths = config.allowed_browse_paths.clone();
    let thumbs_dir = state.thumbs_dir.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        let mut hours: u64 = 0;
        loop {
            interval.tick().await;
            if hours.is_multiple_of(6) {
                maintenance.maintain_logs().await;
            }
            hours += 1;
            dynamight::handlers::files::purge_expired_trash(&cleanup_db, &browse_paths).await;
            dynamight::handlers::uploads::remove_stale(&cleanup_db).await;
            let thumbs = thumbs_dir.clone();
            let _ = tokio::task::spawn_blocking(move || {
                dynamight::handlers::file_preview::trim_thumbs(&thumbs, THUMB_CACHE_BYTES)
            })
            .await;
            let _ = sqlx::query("DELETE FROM pending_totp_sessions WHERE expires_at < datetime('now')")
                .execute(&cleanup_db)
                .await;
            let _ = sqlx::query("DELETE FROM revoked_tokens WHERE expires_at < ?")
                .bind(chrono::Utc::now())
                .execute(&cleanup_db)
                .await;
        }
    });

    // Start scheduler
    let scheduler = SchedulerService::new(db.clone(), backup_service.clone(), config.timezone);
    tokio::spawn(async move {
        scheduler.start().await;
    });

    // API routes (shared with the integration tests, see lib.rs)
    let api_routes = dynamight::build_api_router(state.clone());

    // Build security headers layers
    let security_headers = build_security_headers(&config);

    // Vite names the files in /assets after their content, so browsers may
    // keep them for good; everything else (index.html) is checked each time
    let assets = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(ServeDir::new(std::path::Path::new(&config.static_files_dir).join("assets")));
    let static_files = Router::new()
        .nest_service("/assets", assets)
        .fallback_service(ServeDir::new(&config.static_files_dir))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ));

    // Compress pages, scripts and JSON, but not file downloads (attachments):
    // those can be large or compressed already, and keep their Content-Length
    // Streamed folder sizes (NDJSON) must reach the browser line by line, and
    // compression would hold them back
    let compression = CompressionLayer::new().compress_when(
        DefaultPredicate::new()
            .and(NotForContentType::const_new("application/x-ndjson"))
            .and(|_: StatusCode, _: Version, headers: &HeaderMap, _: &Extensions| {
                !headers.contains_key(header::CONTENT_DISPOSITION)
            }),
    );

    let app = Router::new()
        .nest("/api", api_routes)
        .merge(static_files)
        .layer(compression)
        .layer(TraceLayer::new_for_http())
        .layer(build_cors_layer(&config))
        .layer(security_headers);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    // On SIGTERM/Ctrl+C: stop the backups cleanly first (systemd allows 30 s,
    // see TimeoutStopSec), then let the HTTP server finish its requests.
    let http_stop = Arc::new(tokio::sync::Notify::new());
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown({
        let http_stop = Arc::clone(&http_stop);
        async move { http_stop.notified().await }
    });

    let stop = async {
        shutdown_signal().await;
        backup_service.shutdown(SHUTDOWN_GRACE).await;
        http_stop.notify_one();
        // Open live-log WebSockets would keep the server waiting
        tokio::time::sleep(HTTP_SHUTDOWN_GRACE).await;
    };

    tokio::select! {
        result = server => result?,
        _ = stop => tracing::warn!("Connections still open after {:?}, exiting", HTTP_SHUTDOWN_GRACE),
    }

    tracing::info!("Server shut down gracefully");
    Ok(())
}

/// How long running backups get to stop on shutdown (kill rsync, unmount,
/// store the run as cancelled). Below systemd's TimeoutStopSec (30 s) and the
/// compose file's stop_grace_period, so it finishes before a SIGKILL.
/// Size the thumbnail cache is trimmed to every hour
const THUMB_CACHE_BYTES: u64 = 500 * 1024 * 1024;
const SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(20);
/// How long open HTTP connections may delay the exit after that.
const HTTP_SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down..."),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down..."),
    }
}

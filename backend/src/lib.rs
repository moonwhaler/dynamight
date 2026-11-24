//! Library crate for Dynamight backend.
//!
//! Re-exports internal modules so that integration tests (in `tests/`) can
//! construct an `AppState`, build the router, and exercise the API through
//! `axum_test::TestServer`.

pub mod config;
pub mod db;
pub mod errors;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod services;

#[cfg(test)]
mod test_helpers;

use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use axum::http::{header, HeaderValue};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

pub use crate::config::Config;
pub use crate::services::{
    AuthService, BackupService, CredentialService, MountService,
    RateLimitConfig, RateLimitService,
};

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
    pub delete_verifications: RwLock<HashMap<i64, Instant>>,
    pub search_timeout_seconds: RwLock<u32>,
    pub show_directory_sizes: RwLock<bool>,
    pub dir_size_cache: Arc<std::sync::RwLock<HashMap<PathBuf, (u64, Instant)>>>,
    /// Copy and move jobs of the file manager
    pub file_ops: Arc<services::file_ops::FileOps>,
    /// Cache of image and video thumbnails
    pub thumbs_dir: PathBuf,
}

/// Folder for cached thumbnails: next to the database file, or in the
/// system's temp folder for an in-memory database
pub fn thumbs_dir_for(database_url: &str) -> PathBuf {
    let path = database_url
        .trim_start_matches("sqlite://")
        .trim_start_matches("sqlite:")
        .split('?')
        .next()
        .unwrap_or_default();
    match std::path::Path::new(path).parent() {
        Some(parent) if !path.contains(":memory:") && !parent.as_os_str().is_empty() => parent.join("thumbs"),
        _ => std::env::temp_dir().join("dynamight-thumbs"),
    }
}

/// Build the API router (without static-file fallback or outer layers like
/// CORS / tracing / security headers).  This is the same route tree used in
/// `main()` but suitable for testing.
pub fn build_api_router(state: Arc<AppState>) -> Router {
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
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/me", get(handlers::auth::me))
        .route("/auth/change-password", post(handlers::auth::change_password))
        .route("/auth/totp/setup", post(handlers::totp::setup))
        .route("/auth/totp/enable", post(handlers::totp::enable))
        .route("/auth/totp/disable", post(handlers::totp::disable))
        .route("/auth/totp/status", get(handlers::totp::status))
        .route("/jobs", get(handlers::jobs::list_jobs).post(handlers::jobs::create_job))
        .route("/jobs/:id", get(handlers::jobs::get_job).put(handlers::jobs::update_job).delete(handlers::jobs::delete_job))
        .route("/jobs/:id/run", post(handlers::jobs::run_job))
        .route("/jobs/:id/cancel", post(handlers::jobs::cancel_job))
        .route("/jobs/:id/clone", post(handlers::jobs::clone_job))
        .route("/jobs/:id/check-space", post(handlers::jobs::check_job_space))
        .route("/jobs/:id/schedules", get(handlers::schedules::list_schedules).post(handlers::schedules::create_schedule))
        .route("/schedules/validate", post(handlers::schedules::validate_cron))
        .route("/schedules/:id", put(handlers::schedules::update_schedule).delete(handlers::schedules::delete_schedule))
        .route("/jobs/:id/runs", get(handlers::logs::list_runs).delete(handlers::logs::delete_job_runs))
        .route("/runs", get(handlers::logs::list_recent_runs).delete(handlers::logs::purge_all_runs))
        .route("/runs/:id", get(handlers::logs::get_run).delete(handlers::logs::delete_run))
        .route("/runs/:id/logs", get(handlers::logs::get_logs))
        .route("/system/info", get(handlers::system::info))
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
        .route("/files/places", get(handlers::files::places))
        .route("/files/raw", get(handlers::files::raw))
        .route(
            "/files/trash",
            get(handlers::files::list_trash)
                .post(handlers::files::move_to_trash)
                .delete(handlers::files::purge_trash),
        )
        .route("/files/trash/restore", post(handlers::files::restore_trash))
        .route("/files/dir-sizes", post(handlers::files::dir_sizes))
        .route("/files/tasks", get(handlers::file_actions::list_tasks).post(handlers::file_actions::start_task))
        .route("/files/tasks/:id/cancel", post(handlers::file_actions::cancel_task))
        .route("/files/tasks/:id/resolve", post(handlers::file_actions::resolve_task))
        .route("/files/rename", post(handlers::file_actions::rename))
        .route("/files/archive", post(handlers::file_actions::prepare_archive))
        .route("/files/archive/:token", get(handlers::file_actions::download_archive))
        .route("/files/text", get(handlers::file_preview::text))
        .route("/files/thumb", get(handlers::file_preview::thumb))
        .route("/files/uploads", post(handlers::uploads::start))
        .route(
            "/files/uploads/:id",
            get(handlers::uploads::status).put(handlers::uploads::chunk).delete(handlers::uploads::cancel),
        )
        .route("/settings", get(handlers::settings::get_settings).put(handlers::settings::update_settings))
        .route(
            "/settings/notifications",
            get(handlers::notifications::get_notification_settings)
                .put(handlers::notifications::update_notification_settings),
        )
        .route("/settings/notifications/test", post(handlers::notifications::test_notification_settings))
        .route("/config/export", post(handlers::config_backup::export_config))
        .route("/config/import", post(handlers::config_backup::import_config))
        .route("/config/import/preview", post(handlers::config_backup::preview_import))
        .route("/credentials", get(handlers::credentials::list_credentials).post(handlers::credentials::create_credential))
        .route("/credentials/:id", get(handlers::credentials::get_credential).put(handlers::credentials::update_credential).delete(handlers::credentials::delete_credential))
        .route("/credentials/:id/usage", get(handlers::credentials::get_credential_usage))
        .route("/providers", get(handlers::providers::list_providers))
        .route("/providers/:type/capabilities", get(handlers::providers::get_provider_capabilities))
        .route("/providers/test", post(handlers::providers::test_connection))
        .layer(axum_middleware::from_fn_with_state(state.clone(), middleware::require_auth));

    let ws_routes = Router::new()
        .route("/ws/logs/:run_id", get(handlers::websocket::ws_logs_handler))
        .route("/ws/status", get(handlers::websocket::ws_status_handler));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(ws_routes)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache"),
        ))
        .layer(RequestBodyLimitLayer::new(state.config.max_request_body_size))
        .with_state(state)
}

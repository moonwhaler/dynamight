use std::env;
use std::path::Path;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub logs_database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub static_files_dir: String,
    pub max_runs_per_job: Option<u32>,
    pub allowed_browse_paths: Vec<String>,
    pub cors_origins: Option<Vec<String>>,
    // Rate limiting
    pub rate_limit_max_attempts: u32,
    pub rate_limit_window_secs: u64,
    pub rate_limit_lockout_secs: u64,
    pub rate_limit_max_lockout_secs: u64,
    // Cookie security
    pub secure_cookies: bool,
    // File browser max download size (default: 2GB)
    pub max_download_size: u64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/dynamight.db".to_string());

        // Derive logs database URL from main database URL if not specified
        let logs_database_url = env::var("LOGS_DATABASE_URL").unwrap_or_else(|_| {
            // Extract the path from sqlite:path/to/db.db and create logs.db in same directory
            if let Some(path) = database_url.strip_prefix("sqlite:") {
                let db_path = Path::new(path);
                if let Some(parent) = db_path.parent() {
                    return format!("sqlite:{}/logs.db", parent.display());
                }
            }
            "sqlite:data/logs.db".to_string()
        });

        Self {
            database_url,
            logs_database_url,
            jwt_secret: env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set"),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("PORT must be a valid number"),
            static_files_dir: env::var("STATIC_FILES_DIR")
                .unwrap_or_else(|_| "static".to_string()),
            max_runs_per_job: env::var("MAX_RUNS_PER_JOB")
                .ok()
                .and_then(|v| v.parse().ok()),
            allowed_browse_paths: env::var("ALLOWED_BROWSE_PATHS")
                .unwrap_or_else(|_| "/mnt,/home,/media".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            cors_origins: env::var("CORS_ORIGINS").ok().map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }),
            // Rate limiting with sensible defaults
            rate_limit_max_attempts: env::var("RATE_LIMIT_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            rate_limit_window_secs: env::var("RATE_LIMIT_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            rate_limit_lockout_secs: env::var("RATE_LIMIT_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            rate_limit_max_lockout_secs: env::var("RATE_LIMIT_MAX_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            // Default to true for production safety; set SECURE_COOKIES=false for local HTTP development
            secure_cookies: env::var("SECURE_COOKIES")
                .map(|v| v.to_lowercase() != "false" && v != "0")
                .unwrap_or(true),
            // Max download size: default 2GB (2_147_483_648 bytes)
            max_download_size: env::var("MAX_DOWNLOAD_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2_147_483_648),
        }
    }

    /// Returns the directory containing the database files
    pub fn database_dir(&self) -> Option<&Path> {
        self.database_url
            .strip_prefix("sqlite:")
            .map(Path::new)
            .and_then(|p| p.parent())
    }
}

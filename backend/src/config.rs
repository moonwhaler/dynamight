use chrono_tz::Tz;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

/// TOML configuration file structure
#[derive(Deserialize, Default)]
struct TomlConfig {
    #[serde(default)]
    security: SecurityConfig,
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    database: DatabaseConfig,
    #[serde(default)]
    #[allow(dead_code)] // Parsed for documentation; logging uses RUST_LOG env var
    logging: LoggingConfig,
    #[serde(default)]
    rate_limit: RateLimitConfig,
    #[serde(default)]
    limits: LimitsConfig,
}

#[derive(Deserialize, Default)]
struct SecurityConfig {
    jwt_secret: Option<String>,
    #[serde(default = "default_secure_cookies")]
    secure_cookies: bool,
    #[serde(default = "default_browse_paths")]
    allowed_browse_paths: Vec<String>,
    #[serde(default)]
    cors_origins: Vec<String>,
    #[serde(default)]
    trusted_proxies: Vec<String>,
}

#[derive(Deserialize, Default)]
struct ServerConfig {
    #[serde(default = "default_host")]
    host: String,
    #[serde(default = "default_port")]
    port: u16,
    #[serde(default = "default_static_files_dir")]
    static_files_dir: String,
    #[serde(default = "default_timezone")]
    timezone: String,
}

#[derive(Deserialize, Default)]
struct DatabaseConfig {
    #[serde(default = "default_database_url")]
    url: String,
    logs_url: Option<String>,
}

#[allow(dead_code)] // Parsed for documentation; logging uses RUST_LOG env var
#[derive(Deserialize, Default)]
struct LoggingConfig {
    #[serde(default = "default_log_level")]
    level: String,
}

#[derive(Deserialize)]
struct RateLimitConfig {
    #[serde(default = "default_rate_limit_max_attempts")]
    max_attempts: u32,
    #[serde(default = "default_rate_limit_window_secs")]
    window_secs: u64,
    #[serde(default = "default_rate_limit_lockout_secs")]
    lockout_secs: u64,
    #[serde(default = "default_rate_limit_max_lockout_secs")]
    max_lockout_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_rate_limit_max_attempts(),
            window_secs: default_rate_limit_window_secs(),
            lockout_secs: default_rate_limit_lockout_secs(),
            max_lockout_secs: default_rate_limit_max_lockout_secs(),
        }
    }
}

#[derive(Deserialize)]
struct LimitsConfig {
    #[serde(default = "default_max_download_size")]
    max_download_size: u64,
    #[serde(default = "default_max_request_body_size")]
    max_request_body_size: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            max_download_size: default_max_download_size(),
            max_request_body_size: default_max_request_body_size(),
        }
    }
}

// Default value functions
fn default_secure_cookies() -> bool {
    true
}
fn default_browse_paths() -> Vec<String> {
    vec!["/mnt".to_string(), "/home".to_string(), "/media".to_string()]
}
fn default_host() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_static_files_dir() -> String {
    "static".to_string()
}
fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_database_url() -> String {
    "sqlite:data/dynamight.db".to_string()
}
fn default_log_level() -> String {
    "info,dynamight=debug".to_string()
}
fn default_rate_limit_max_attempts() -> u32 {
    5
}
fn default_rate_limit_window_secs() -> u64 {
    60
}
fn default_rate_limit_lockout_secs() -> u64 {
    60
}
fn default_rate_limit_max_lockout_secs() -> u64 {
    3600
}
fn default_max_download_size() -> u64 {
    2_147_483_648
}
fn default_max_request_body_size() -> usize {
    10 * 1024 * 1024
}

/// Application configuration
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub logs_database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub static_files_dir: String,
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
    // Trusted proxy IPs/CIDRs for X-Forwarded-For header trust
    // If empty, X-Forwarded-For is ignored (safest default)
    pub trusted_proxies: Vec<String>,
    // Maximum request body size in bytes (default: 10MB)
    pub max_request_body_size: usize,
    // Timezone for cron schedule interpretation (e.g. "Europe/Berlin")
    pub timezone: Tz,
}

impl Config {
    /// Load configuration from TOML file with environment variable overrides.
    ///
    /// Configuration priority (highest to lowest):
    /// 1. Environment variables
    /// 2. TOML config file (dynamight.toml)
    /// 3. Built-in defaults
    pub fn load() -> Self {
        // Try to load TOML config file
        let toml_config = Self::load_toml_config();

        // Build final config with env var overrides
        let database_url = env::var("DATABASE_URL").unwrap_or(toml_config.database.url);

        // Derive logs database URL from main database URL if not specified
        let logs_database_url =
            env::var("LOGS_DATABASE_URL").unwrap_or_else(|_| match &toml_config.database.logs_url {
                Some(url) => url.clone(),
                None => {
                    // Extract the path from sqlite:path/to/db.db and create logs.db in same directory
                    if let Some(path) = database_url.strip_prefix("sqlite:") {
                        let db_path = Path::new(path);
                        if let Some(parent) = db_path.parent() {
                            return format!("sqlite:{}/logs.db", parent.display());
                        }
                    }
                    "sqlite:data/logs.db".to_string()
                }
            });

        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            toml_config
                .security
                .jwt_secret
                .expect("jwt_secret must be set in dynamight.toml or JWT_SECRET environment variable")
        });

        let cors_origins = env::var("CORS_ORIGINS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .or((!toml_config.security.cors_origins.is_empty())
                .then_some(toml_config.security.cors_origins));

        let allowed_browse_paths = env::var("ALLOWED_BROWSE_PATHS")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or(toml_config.security.allowed_browse_paths);

        let trusted_proxies = env::var("TRUSTED_PROXIES")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or(toml_config.security.trusted_proxies);

        Self {
            database_url,
            logs_database_url,
            jwt_secret,
            host: env::var("HOST").unwrap_or(toml_config.server.host),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.server.port),
            static_files_dir: env::var("STATIC_FILES_DIR")
                .unwrap_or(toml_config.server.static_files_dir),
            allowed_browse_paths,
            cors_origins,
            rate_limit_max_attempts: env::var("RATE_LIMIT_MAX_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.rate_limit.max_attempts),
            rate_limit_window_secs: env::var("RATE_LIMIT_WINDOW_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.rate_limit.window_secs),
            rate_limit_lockout_secs: env::var("RATE_LIMIT_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.rate_limit.lockout_secs),
            rate_limit_max_lockout_secs: env::var("RATE_LIMIT_MAX_LOCKOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.rate_limit.max_lockout_secs),
            secure_cookies: env::var("SECURE_COOKIES")
                .map(|v| v.to_lowercase() != "false" && v != "0")
                .unwrap_or(toml_config.security.secure_cookies),
            max_download_size: env::var("MAX_DOWNLOAD_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.limits.max_download_size),
            trusted_proxies,
            max_request_body_size: env::var("MAX_REQUEST_BODY_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(toml_config.limits.max_request_body_size),
            timezone: env::var("TZ")
                .ok()
                .or_else(|| Some(toml_config.server.timezone.clone()))
                .and_then(|tz| {
                    tz.parse::<Tz>().map_err(|_| {
                        tracing::warn!("Unknown timezone '{}', falling back to UTC", tz);
                    }).ok()
                })
                .unwrap_or(Tz::UTC),
        }
    }

    /// Load TOML configuration file, returning defaults if not found
    fn load_toml_config() -> TomlConfig {
        // Search for config file in order of priority
        let config_paths = [
            env::var("DYNAMIGHT_CONFIG").ok(),
            Some("dynamight.toml".to_string()),
            Some("/etc/dynamight/dynamight.toml".to_string()),
        ];

        for path_opt in config_paths.iter().flatten() {
            let path = Path::new(path_opt);
            if path.exists() {
                match fs::read_to_string(path) {
                    Ok(contents) => match toml::from_str(&contents) {
                        Ok(config) => {
                            tracing::info!("Loaded configuration from {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to parse {}: {}. Using defaults.",
                                path.display(),
                                e
                            );
                        }
                    },
                    Err(e) => {
                        tracing::error!(
                            "Failed to read {}: {}. Using defaults.",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        tracing::info!("No config file found, using defaults (configure via environment variables or create dynamight.toml)");
        TomlConfig::default()
    }

    /// Backwards-compatible alias for load()
    #[deprecated(since = "0.2.0", note = "Use Config::load() instead")]
    pub fn from_env() -> Self {
        Self::load()
    }

    /// Returns the directory containing the database files
    pub fn database_dir(&self) -> Option<&Path> {
        self.database_url
            .strip_prefix("sqlite:")
            .map(Path::new)
            .and_then(|p| p.parent())
    }
}

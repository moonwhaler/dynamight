use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub static_files_dir: String,
    pub max_runs_per_job: Option<u32>,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data/dynamight.db".to_string()),
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
        }
    }
}

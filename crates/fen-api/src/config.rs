use std::env;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Address to bind the server to
    pub bind_address: String,

    /// Path to the redb database file
    pub database_path: String,

    /// Path to rule files (optional)
    pub rules_path: Option<PathBuf>,

    /// Maximum upload size in bytes
    pub max_upload_size: usize,
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            bind_address: env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            database_path: env::var("DATABASE_PATH").unwrap_or_else(|_| "data/fen.redb".to_string()),
            rules_path: env::var("RULES_PATH").ok().map(PathBuf::from),
            max_upload_size: env::var("MAX_UPLOAD_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50 * 1024 * 1024), // 50MB default
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

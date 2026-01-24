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

    /// Rate limit: requests per second
    pub rate_limit_rps: u32,

    /// Rate limit: burst size
    pub rate_limit_burst: u32,

    /// Statistical anomaly detection configuration
    pub statistical: StatisticalConfig,
}

/// Configuration for statistical anomaly detection
#[derive(Debug, Clone)]
pub struct StatisticalConfig {
    /// Enable statistical analysis
    pub enabled: bool,
    /// Enable during document ingestion (POST /documents)
    pub on_ingest: bool,
    /// Enable during validation (POST /validate)
    pub on_validate: bool,
    /// Default z-score threshold for outlier detection
    pub threshold: f64,
    /// Metrics to analyze
    pub metrics: Vec<String>,
    /// Rolling window in days for baseline computation
    pub window_days: u32,
    /// Enable seasonal baseline awareness (month-of-year patterns)
    pub seasonal: bool,
}

impl Default for StatisticalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            on_ingest: false,  // Disabled by default on ingest (performance)
            on_validate: true, // Enabled on explicit validation
            threshold: 2.0,
            metrics: vec!["total_amount".to_string()],
            window_days: 90,
            seasonal: true,
        }
    }
}

impl StatisticalConfig {
    /// Load statistical configuration from environment variables
    pub fn from_env() -> Self {
        let enabled = env::var("STATISTICAL_ENABLED")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(true);

        let on_ingest = env::var("STATISTICAL_ON_INGEST")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(false);

        let on_validate = env::var("STATISTICAL_ON_VALIDATE")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(true);

        let threshold = env::var("STATISTICAL_THRESHOLD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2.0);

        let metrics = env::var("STATISTICAL_METRICS")
            .map(|s| s.split(',').map(|m| m.trim().to_string()).collect())
            .unwrap_or_else(|_| vec!["total_amount".to_string()]);

        let window_days = env::var("STATISTICAL_WINDOW_DAYS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(90);

        let seasonal = env::var("STATISTICAL_SEASONAL")
            .map(|s| s.to_lowercase() == "true" || s == "1")
            .unwrap_or(true);

        Self {
            enabled,
            on_ingest,
            on_validate,
            threshold,
            metrics,
            window_days,
            seasonal,
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            bind_address: env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            database_path: env::var("DATABASE_PATH")
                .unwrap_or_else(|_| "data/fen.redb".to_string()),
            rules_path: env::var("RULES_PATH").ok().map(PathBuf::from),
            max_upload_size: env::var("MAX_UPLOAD_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50 * 1024 * 1024), // 50MB default
            rate_limit_rps: env::var("RATE_LIMIT_RPS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100), // 100 requests per second default
            rate_limit_burst: env::var("RATE_LIMIT_BURST")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(200), // 200 burst capacity default
            statistical: StatisticalConfig::from_env(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

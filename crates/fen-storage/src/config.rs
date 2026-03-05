//! Storage backend configuration for dev vs prod environments.
//!
//! # Feature Flags
//!
//! - `embedded` (default): Local filesystem storage for development
//! - `remote-storage`: Cloud/S3 storage backends for production
//!
//! # Usage
//!
//! ```text
//! // Development (default - embedded feature)
//! let config = WarmStorageConfig::embedded("/path/to/local/db");
//!
//! // Production (with remote-storage feature)
//! let config = WarmStorageConfig::s3("s3://bucket/path", S3Config { ... });
//! ```

use serde::{Deserialize, Serialize};

/// Storage backend type for warm tier (LanceDB)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WarmStorageBackend {
    /// Embedded local filesystem storage (development)
    /// Uses LanceDB in embedded mode with local directory
    Embedded {
        /// Path to local LanceDB directory
        path: String,
    },

    /// S3-compatible object storage (production)
    /// Uses LanceDB with S3 backend
    #[cfg(feature = "remote-storage")]
    S3 {
        /// S3 URI (e.g., "s3://bucket/path")
        uri: String,
        /// S3 configuration
        #[serde(flatten)]
        config: S3Config,
    },

    /// LanceDB Cloud (production)
    /// Managed LanceDB service
    #[cfg(feature = "remote-storage")]
    LanceCloud {
        /// LanceDB Cloud database URI
        db_uri: String,
        /// API key for authentication
        api_key: String,
        /// Region (optional)
        region: Option<String>,
    },
}

impl Default for WarmStorageBackend {
    fn default() -> Self {
        Self::Embedded {
            path: "data/warm".to_string(),
        }
    }
}

impl WarmStorageBackend {
    /// Create an embedded (local filesystem) backend configuration
    pub fn embedded(path: impl Into<String>) -> Self {
        Self::Embedded { path: path.into() }
    }

    /// Create an S3 backend configuration (requires `remote-storage` feature)
    #[cfg(feature = "remote-storage")]
    pub fn s3(uri: impl Into<String>, config: S3Config) -> Self {
        Self::S3 {
            uri: uri.into(),
            config,
        }
    }

    /// Create a LanceDB Cloud backend configuration (requires `remote-storage` feature)
    #[cfg(feature = "remote-storage")]
    pub fn lance_cloud(
        db_uri: impl Into<String>,
        api_key: impl Into<String>,
        region: Option<String>,
    ) -> Self {
        Self::LanceCloud {
            db_uri: db_uri.into(),
            api_key: api_key.into(),
            region,
        }
    }

    /// Check if this is an embedded (local) backend
    pub fn is_embedded(&self) -> bool {
        matches!(self, Self::Embedded { .. })
    }

    /// Check if this is a remote backend
    #[cfg(feature = "remote-storage")]
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::S3 { .. } | Self::LanceCloud { .. })
    }

    /// Get the connection URI for LanceDB
    pub fn connection_uri(&self) -> &str {
        match self {
            Self::Embedded { path } => path,
            #[cfg(feature = "remote-storage")]
            Self::S3 { uri, .. } => uri,
            #[cfg(feature = "remote-storage")]
            Self::LanceCloud { db_uri, .. } => db_uri,
        }
    }
}

/// S3-compatible storage configuration
#[cfg(feature = "remote-storage")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    /// AWS region (e.g., "us-east-1")
    pub region: String,
    /// AWS access key ID (optional - can use environment/IAM)
    pub access_key_id: Option<String>,
    /// AWS secret access key (optional - can use environment/IAM)
    pub secret_access_key: Option<String>,
    /// Custom endpoint URL (for MinIO, LocalStack, etc.)
    pub endpoint: Option<String>,
    /// Allow HTTP connections (for local development)
    #[serde(default)]
    pub allow_http: bool,
}

#[cfg(feature = "remote-storage")]
impl S3Config {
    /// Create a new S3 configuration
    pub fn new(region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            access_key_id: None,
            secret_access_key: None,
            endpoint: None,
            allow_http: false,
        }
    }

    /// Set explicit credentials
    pub fn with_credentials(
        mut self,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key_id.into());
        self.secret_access_key = Some(secret_access_key.into());
        self
    }

    /// Set custom endpoint (for MinIO, LocalStack)
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Allow HTTP connections
    pub fn with_allow_http(mut self, allow: bool) -> Self {
        self.allow_http = allow;
        self
    }
}

/// Hot tier storage backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HotStorageBackend {
    /// Embedded redb storage (development & production)
    /// redb is always embedded - this is the recommended mode
    Embedded {
        /// Path to redb database file
        path: String,
    },

    /// In-memory storage (testing only)
    InMemory,
}

impl Default for HotStorageBackend {
    fn default() -> Self {
        Self::Embedded {
            path: "data/hot.redb".to_string(),
        }
    }
}

impl HotStorageBackend {
    /// Create an embedded backend configuration
    pub fn embedded(path: impl Into<String>) -> Self {
        Self::Embedded { path: path.into() }
    }

    /// Create an in-memory backend (for testing)
    pub fn in_memory() -> Self {
        Self::InMemory
    }
}

/// Knowledge graph backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphBackend {
    /// Embedded graph database (RyuGraph) on local filesystem
    Embedded {
        /// Path to graph database directory
        path: String,
    },

    /// In-memory graph database (testing only)
    InMemory,

    /// Graph disabled
    Disabled,
}

impl Default for GraphBackend {
    fn default() -> Self {
        Self::Disabled
    }
}

impl GraphBackend {
    /// Create an embedded graph backend
    pub fn embedded(path: impl Into<String>) -> Self {
        Self::Embedded { path: path.into() }
    }

    /// Check if the graph is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

/// Environment detection helpers
pub mod env {
    use super::*;

    /// Detect if running in development mode
    pub fn is_development() -> bool {
        std::env::var("FEN_ENV")
            .map(|v| v.to_lowercase() == "development" || v.to_lowercase() == "dev")
            .unwrap_or(true) // Default to development
    }

    /// Detect if running in production mode
    pub fn is_production() -> bool {
        std::env::var("FEN_ENV")
            .map(|v| v.to_lowercase() == "production" || v.to_lowercase() == "prod")
            .unwrap_or(false)
    }

    /// Create default warm storage backend based on environment
    pub fn default_warm_backend() -> WarmStorageBackend {
        if is_production() {
            #[cfg(feature = "remote-storage")]
            {
                // In production, try to use S3 from environment
                if let Ok(bucket) = std::env::var("FEN_S3_BUCKET") {
                    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into());
                    return WarmStorageBackend::S3 {
                        uri: format!("s3://{}/fen-warm", bucket),
                        config: S3Config::new(region),
                    };
                }
            }
            // Fall back to embedded even in prod if no remote config
            tracing::warn!(
                "Production mode but no remote storage configured, falling back to embedded"
            );
        }
        WarmStorageBackend::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warm_backend_default() {
        let backend = WarmStorageBackend::default();
        assert!(backend.is_embedded());
        assert_eq!(backend.connection_uri(), "data/warm");
    }

    #[test]
    fn test_warm_backend_embedded() {
        let backend = WarmStorageBackend::embedded("/custom/path");
        assert!(backend.is_embedded());
        assert_eq!(backend.connection_uri(), "/custom/path");
    }

    #[cfg(feature = "remote-storage")]
    #[test]
    fn test_warm_backend_s3() {
        let config = S3Config::new("us-west-2")
            .with_credentials("key", "secret")
            .with_endpoint("http://localhost:9000")
            .with_allow_http(true);

        let backend = WarmStorageBackend::s3("s3://my-bucket/warm", config);
        assert!(backend.is_remote());
        assert_eq!(backend.connection_uri(), "s3://my-bucket/warm");
    }

    #[test]
    fn test_hot_backend_default() {
        let backend = HotStorageBackend::default();
        matches!(backend, HotStorageBackend::Embedded { .. });
    }

    #[test]
    fn test_serialization() {
        let backend = WarmStorageBackend::embedded("/test/path");
        let json = serde_json::to_string(&backend).unwrap();
        assert!(json.contains("embedded"));
        assert!(json.contains("/test/path"));

        let deserialized: WarmStorageBackend = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_embedded());
    }
}

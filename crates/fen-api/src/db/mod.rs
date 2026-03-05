//! Database access layer for user/tenant management and observability.
//!
//! This module provides repository implementations backed by:
//! - PostgreSQL for user/tenant management
//! - TimescaleDB for metrics and observability

pub mod metrics;
pub mod tenant;
pub mod user;

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub use metrics::MetricsRepository;
pub use tenant::TenantRepository;
pub use user::UserRepository;

/// Database configuration
#[derive(Clone, Debug)]
pub struct DbConfig {
    /// PostgreSQL connection URL for user/tenant data
    pub postgres_url: String,
    /// TimescaleDB connection URL for metrics
    pub timescale_url: String,
    /// Maximum connections per pool
    pub max_connections: u32,
    /// Connection timeout
    pub connect_timeout: Duration,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            postgres_url: String::new(),
            timescale_url: String::new(),
            max_connections: 10,
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl DbConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            postgres_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| Self::default().postgres_url),
            timescale_url: std::env::var("TIMESCALE_URL")
                .unwrap_or_else(|_| Self::default().timescale_url),
            max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            connect_timeout: Duration::from_secs(
                std::env::var("DB_CONNECT_TIMEOUT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
            ),
        }
    }
}

/// Database pools for application use
#[derive(Clone)]
pub struct DbPools {
    /// PostgreSQL pool for user/tenant data
    pub postgres: PgPool,
    /// TimescaleDB pool for metrics
    pub timescale: PgPool,
}

impl DbPools {
    /// Create new database pools from configuration
    pub async fn new(config: &DbConfig) -> Result<Self, sqlx::Error> {
        let postgres = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.connect_timeout)
            .connect(&config.postgres_url)
            .await?;

        tracing::info!("Connected to PostgreSQL");

        let timescale = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.connect_timeout)
            .connect(&config.timescale_url)
            .await?;

        tracing::info!("Connected to TimescaleDB");

        Ok(Self {
            postgres,
            timescale,
        })
    }

    /// Create repositories
    pub fn repositories(&self) -> Repositories {
        Repositories {
            users: UserRepository::new(self.postgres.clone()),
            tenants: TenantRepository::new(self.postgres.clone()),
            metrics: MetricsRepository::new(self.timescale.clone()),
        }
    }
}

/// All repositories
#[derive(Clone)]
pub struct Repositories {
    pub users: UserRepository,
    pub tenants: TenantRepository,
    pub metrics: MetricsRepository,
}

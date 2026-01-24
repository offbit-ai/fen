//! Tenant configuration types for multi-tenant deployments.
//!
//! This module provides per-tenant configuration including:
//! - Data retention policies (hot/warm/cold storage tiers)
//! - Rate limiting configuration
//! - Storage quotas
//! - Feature flags

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::cluster::TenantId;

/// Retention policy for a specific data type.
///
/// Controls how long data is kept in each storage tier before being
/// moved to the next tier or deleted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Days to keep in hot storage (redb) for fast access
    pub hot_retention_days: u32,
    /// Days to keep in warm storage (LanceDB) for vector search
    pub warm_retention_days: u32,
    /// Days to keep in cold/archive storage (S3/GCS)
    pub archive_retention_days: u32,
    /// Whether to permanently delete after archive retention expires
    /// If false, data is soft-deleted (marked but not removed)
    pub hard_delete: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            hot_retention_days: 60,
            warm_retention_days: 365,
            archive_retention_days: 2555, // ~7 years for financial compliance
            hard_delete: false,
        }
    }
}

impl RetentionPolicy {
    /// Create a retention policy for short-term data
    pub fn short_term() -> Self {
        Self {
            hot_retention_days: 30,
            warm_retention_days: 90,
            archive_retention_days: 365,
            hard_delete: true,
        }
    }

    /// Create a retention policy for long-term compliance data
    pub fn compliance() -> Self {
        Self {
            hot_retention_days: 90,
            warm_retention_days: 365 * 2,
            archive_retention_days: 365 * 10, // 10 years
            hard_delete: false,
        }
    }
}

/// Rate limit configuration for tenant API access.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum API requests per minute
    pub requests_per_minute: u32,
    /// Maximum document uploads per hour
    pub uploads_per_hour: u32,
    /// Maximum concurrent processing jobs
    pub max_concurrent_jobs: u32,
    /// Maximum batch size for bulk operations
    pub max_batch_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 1000,
            uploads_per_hour: 500,
            max_concurrent_jobs: 10,
            max_batch_size: 100,
        }
    }
}

impl RateLimitConfig {
    /// Create rate limits for a free/trial tier
    pub fn free_tier() -> Self {
        Self {
            requests_per_minute: 60,
            uploads_per_hour: 50,
            max_concurrent_jobs: 2,
            max_batch_size: 10,
        }
    }

    /// Create rate limits for an enterprise tier
    pub fn enterprise() -> Self {
        Self {
            requests_per_minute: 10000,
            uploads_per_hour: 5000,
            max_concurrent_jobs: 50,
            max_batch_size: 1000,
        }
    }
}

/// Storage quota configuration for tenant resources.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuotaConfig {
    /// Maximum storage in bytes across all tiers
    pub max_storage_bytes: u64,
    /// Maximum number of documents
    pub max_documents: u64,
    /// Maximum number of validation rules
    pub max_rules: u32,
    /// Maximum number of users per tenant
    pub max_users: u32,
}

impl Default for QuotaConfig {
    fn default() -> Self {
        Self {
            max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            max_documents: 100_000,
            max_rules: 1000,
            max_users: 100,
        }
    }
}

impl QuotaConfig {
    /// Create quotas for a free/trial tier
    pub fn free_tier() -> Self {
        Self {
            max_storage_bytes: 1024 * 1024 * 1024, // 1 GB
            max_documents: 1000,
            max_rules: 50,
            max_users: 5,
        }
    }

    /// Create quotas for an enterprise tier
    pub fn enterprise() -> Self {
        Self {
            max_storage_bytes: 1024 * 1024 * 1024 * 1024, // 1 TB
            max_documents: 10_000_000,
            max_rules: 10_000,
            max_users: 1000,
        }
    }
}

/// Feature flags controlling tenant capabilities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable ML-based anomaly detection
    pub ml_anomaly_detection: bool,
    /// Enable statistical baseline computation
    pub statistical_baselines: bool,
    /// Enable real-time WebSocket notifications
    pub real_time_notifications: bool,
    /// Enable webhook integrations
    pub api_webhooks: bool,
    /// Enable custom validation rules
    pub custom_rules: bool,
    /// Enable data export functionality
    pub export_enabled: bool,
    /// Enable ZIP (Zero Invoice Price) queries
    pub zip_queries: bool,
    /// Enable contract-invoice matching
    pub contract_matching: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            ml_anomaly_detection: true,
            statistical_baselines: true,
            real_time_notifications: true,
            api_webhooks: false,
            custom_rules: true,
            export_enabled: true,
            zip_queries: true,
            contract_matching: true,
        }
    }
}

impl FeatureFlags {
    /// Create feature flags for a free/trial tier
    pub fn free_tier() -> Self {
        Self {
            ml_anomaly_detection: false,
            statistical_baselines: true,
            real_time_notifications: false,
            api_webhooks: false,
            custom_rules: false,
            export_enabled: true,
            zip_queries: true,
            contract_matching: false,
        }
    }

    /// Create feature flags for an enterprise tier
    pub fn enterprise() -> Self {
        Self {
            ml_anomaly_detection: true,
            statistical_baselines: true,
            real_time_notifications: true,
            api_webhooks: true,
            custom_rules: true,
            export_enabled: true,
            zip_queries: true,
            contract_matching: true,
        }
    }
}

/// Tenant status for lifecycle management.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TenantStatus {
    /// Tenant is active and can perform all allowed operations
    #[default]
    Active,
    /// Tenant is in trial period
    Trial,
    /// Tenant is suspended (read-only access)
    Suspended,
    /// Tenant is being deprovisioned
    Deprovisioning,
    /// Tenant has been archived
    Archived,
}

impl std::fmt::Display for TenantStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TenantStatus::Active => write!(f, "active"),
            TenantStatus::Trial => write!(f, "trial"),
            TenantStatus::Suspended => write!(f, "suspended"),
            TenantStatus::Deprovisioning => write!(f, "deprovisioning"),
            TenantStatus::Archived => write!(f, "archived"),
        }
    }
}

/// Complete tenant configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Tenant identifier
    pub tenant_id: TenantId,
    /// Human-readable tenant name
    pub name: String,
    /// Tenant status
    pub status: TenantStatus,
    /// When the tenant was created
    pub created_at: DateTime<Utc>,
    /// When the configuration was last updated
    pub updated_at: DateTime<Utc>,

    /// Retention policies per resource type (e.g., "invoice", "contract", "anomaly")
    pub retention_policies: HashMap<String, RetentionPolicy>,

    /// API rate limiting configuration
    pub rate_limits: RateLimitConfig,

    /// Storage and resource quotas
    pub quotas: QuotaConfig,

    /// Feature flags
    pub features: FeatureFlags,

    /// Custom metadata (tenant-specific key-value pairs)
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TenantConfig {
    /// Create a new tenant configuration with default settings.
    pub fn new(tenant_id: TenantId, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            tenant_id,
            name: name.into(),
            status: TenantStatus::default(),
            created_at: now,
            updated_at: now,
            retention_policies: HashMap::new(),
            rate_limits: RateLimitConfig::default(),
            quotas: QuotaConfig::default(),
            features: FeatureFlags::default(),
            metadata: HashMap::new(),
        }
    }

    /// Create a tenant configuration for the free tier.
    pub fn free_tier(tenant_id: TenantId, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            tenant_id,
            name: name.into(),
            status: TenantStatus::Trial,
            created_at: now,
            updated_at: now,
            retention_policies: HashMap::new(),
            rate_limits: RateLimitConfig::free_tier(),
            quotas: QuotaConfig::free_tier(),
            features: FeatureFlags::free_tier(),
            metadata: HashMap::new(),
        }
    }

    /// Create a tenant configuration for the enterprise tier.
    pub fn enterprise(tenant_id: TenantId, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            tenant_id,
            name: name.into(),
            status: TenantStatus::Active,
            created_at: now,
            updated_at: now,
            retention_policies: HashMap::new(),
            rate_limits: RateLimitConfig::enterprise(),
            quotas: QuotaConfig::enterprise(),
            features: FeatureFlags::enterprise(),
            metadata: HashMap::new(),
        }
    }

    /// Set a retention policy for a specific resource type.
    pub fn with_retention_policy(
        mut self,
        resource_type: impl Into<String>,
        policy: RetentionPolicy,
    ) -> Self {
        self.retention_policies.insert(resource_type.into(), policy);
        self.updated_at = Utc::now();
        self
    }

    /// Set rate limits.
    pub fn with_rate_limits(mut self, rate_limits: RateLimitConfig) -> Self {
        self.rate_limits = rate_limits;
        self.updated_at = Utc::now();
        self
    }

    /// Set quotas.
    pub fn with_quotas(mut self, quotas: QuotaConfig) -> Self {
        self.quotas = quotas;
        self.updated_at = Utc::now();
        self
    }

    /// Set feature flags.
    pub fn with_features(mut self, features: FeatureFlags) -> Self {
        self.features = features;
        self.updated_at = Utc::now();
        self
    }

    /// Add custom metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self.updated_at = Utc::now();
        self
    }

    /// Get retention policy for a resource type, or default if not specified.
    pub fn get_retention_policy(&self, resource_type: &str) -> RetentionPolicy {
        self.retention_policies
            .get(resource_type)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if tenant is active (can perform write operations).
    pub fn is_active(&self) -> bool {
        matches!(self.status, TenantStatus::Active | TenantStatus::Trial)
    }

    /// Check if tenant can perform read operations.
    pub fn can_read(&self) -> bool {
        !matches!(self.status, TenantStatus::Archived)
    }

    /// Check if a specific feature is enabled.
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "ml_anomaly_detection" => self.features.ml_anomaly_detection,
            "statistical_baselines" => self.features.statistical_baselines,
            "real_time_notifications" => self.features.real_time_notifications,
            "api_webhooks" => self.features.api_webhooks,
            "custom_rules" => self.features.custom_rules,
            "export_enabled" => self.features.export_enabled,
            "zip_queries" => self.features.zip_queries,
            "contract_matching" => self.features.contract_matching,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_retention_policy() {
        let policy = RetentionPolicy::default();
        assert_eq!(policy.hot_retention_days, 60);
        assert_eq!(policy.warm_retention_days, 365);
        assert!(!policy.hard_delete);
    }

    #[test]
    fn test_tenant_config_creation() {
        let tenant_id = TenantId::new();
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        assert_eq!(config.tenant_id, tenant_id);
        assert_eq!(config.name, "Test Tenant");
        assert!(config.is_active());
    }

    #[test]
    fn test_tenant_config_with_retention() {
        let tenant_id = TenantId::new();
        let config = TenantConfig::new(tenant_id, "Test")
            .with_retention_policy("invoice", RetentionPolicy::compliance());

        let policy = config.get_retention_policy("invoice");
        assert_eq!(policy.hot_retention_days, 90);
        assert_eq!(policy.archive_retention_days, 365 * 10);
    }

    #[test]
    fn test_tenant_status_display() {
        assert_eq!(TenantStatus::Active.to_string(), "active");
        assert_eq!(TenantStatus::Suspended.to_string(), "suspended");
    }

    #[test]
    fn test_free_tier_config() {
        let config = TenantConfig::free_tier(TenantId::new(), "Free User");
        assert_eq!(config.status, TenantStatus::Trial);
        assert!(!config.features.ml_anomaly_detection);
        assert_eq!(config.quotas.max_documents, 1000);
    }

    #[test]
    fn test_enterprise_config() {
        let config = TenantConfig::enterprise(TenantId::new(), "Enterprise Corp");
        assert_eq!(config.status, TenantStatus::Active);
        assert!(config.features.ml_anomaly_detection);
        assert!(config.features.api_webhooks);
    }

    #[test]
    fn test_feature_check() {
        let config = TenantConfig::new(TenantId::new(), "Test");
        assert!(config.is_feature_enabled("ml_anomaly_detection"));
        assert!(!config.is_feature_enabled("api_webhooks"));
        assert!(!config.is_feature_enabled("unknown_feature"));
    }
}

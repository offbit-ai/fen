//! Ingestion service gate for rate limiting and quota enforcement.
//!
//! This module provides gates for the document ingestion service, including:
//! - Rate limiting (uploads per hour, requests per minute)
//! - Storage quota enforcement
//! - Document count limits

use async_trait::async_trait;
use dashmap::DashMap;
use fen_core::domain::{
    cluster::TenantId,
    tenant_config::{QuotaConfig, RateLimitConfig, TenantConfig},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use super::{GateError, ServiceGate};
use crate::middleware::AuthContext;

/// Sliding window entry for rate limiting.
#[derive(Debug, Clone)]
struct WindowEntry {
    timestamp: u64, // Unix timestamp in seconds
    count: u32,
}

/// Rate limiter using sliding window algorithm.
///
/// Tracks request counts per tenant within configurable time windows.
pub struct RateLimiter {
    /// Request counts per tenant and operation type
    /// Key format: "{tenant_id}:{operation}"
    windows: DashMap<String, Vec<WindowEntry>>,
    /// Window duration in seconds
    window_duration_secs: u64,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(window_duration_secs: u64) -> Self {
        Self {
            windows: DashMap::new(),
            window_duration_secs,
        }
    }

    /// Check if an operation is within rate limits.
    ///
    /// Returns the current count if under limit, or error if exceeded.
    pub fn check(
        &self,
        tenant_id: &TenantId,
        operation: &str,
        limit: u32,
    ) -> Result<u32, GateError> {
        let key = format!("{}:{}", tenant_id, operation);
        let now = current_timestamp();

        let mut entry = self.windows.entry(key).or_insert_with(Vec::new);
        let window = entry.value_mut();

        // Clean old entries outside the window
        let cutoff = now.saturating_sub(self.window_duration_secs);
        window.retain(|e| e.timestamp >= cutoff);

        // Calculate total count
        let count: u32 = window.iter().map(|e| e.count).sum();

        if count >= limit {
            let oldest = window.first().map(|e| e.timestamp).unwrap_or(now);
            let retry_after = self.window_duration_secs.saturating_sub(now - oldest);
            return Err(GateError::rate_limited(
                format!(
                    "Rate limit of {} requests per {} seconds exceeded",
                    limit, self.window_duration_secs
                ),
                Some(retry_after),
            ));
        }

        Ok(count)
    }

    /// Record an operation for rate limiting.
    pub fn record(&self, tenant_id: &TenantId, operation: &str) {
        let key = format!("{}:{}", tenant_id, operation);
        let now = current_timestamp();

        let mut entry = self.windows.entry(key).or_insert_with(Vec::new);
        let window = entry.value_mut();

        // Try to merge with existing entry for same second
        if let Some(last) = window.last_mut() {
            if last.timestamp == now {
                last.count += 1;
                return;
            }
        }

        window.push(WindowEntry {
            timestamp: now,
            count: 1,
        });
    }

    /// Check upload rate limit.
    pub fn check_upload(
        &self,
        tenant_id: &TenantId,
        limits: &RateLimitConfig,
    ) -> Result<(), GateError> {
        // Check uploads per hour (3600 seconds window)
        self.check(tenant_id, "upload", limits.uploads_per_hour)?;
        Ok(())
    }

    /// Check API request rate limit.
    pub fn check_request(
        &self,
        tenant_id: &TenantId,
        limits: &RateLimitConfig,
    ) -> Result<(), GateError> {
        // Check requests per minute (60 seconds window)
        let key = format!("{}:request", tenant_id);
        let now = current_timestamp();
        let minute_ago = now.saturating_sub(60);

        let entry = self.windows.entry(key).or_insert_with(Vec::new);
        let window = entry.value();

        let count: u32 = window
            .iter()
            .filter(|e| e.timestamp >= minute_ago)
            .map(|e| e.count)
            .sum();

        if count >= limits.requests_per_minute {
            return Err(GateError::rate_limited(
                format!(
                    "Rate limit of {} requests per minute exceeded",
                    limits.requests_per_minute
                ),
                Some(60),
            ));
        }

        Ok(())
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(3600) // 1 hour default window
    }
}

/// Quota tracker for tenant resource limits.
pub struct QuotaTracker {
    /// Document counts per tenant
    document_counts: DashMap<TenantId, u64>,
    /// Storage usage in bytes per tenant
    storage_bytes: DashMap<TenantId, u64>,
    /// Rule counts per tenant
    rule_counts: DashMap<TenantId, u32>,
    /// User counts per tenant
    user_counts: DashMap<TenantId, u32>,
}

impl QuotaTracker {
    /// Create a new quota tracker.
    pub fn new() -> Self {
        Self {
            document_counts: DashMap::new(),
            storage_bytes: DashMap::new(),
            rule_counts: DashMap::new(),
            user_counts: DashMap::new(),
        }
    }

    /// Get current document count for a tenant.
    pub fn get_document_count(&self, tenant_id: &TenantId) -> u64 {
        self.document_counts
            .get(tenant_id)
            .map(|v| *v.value())
            .unwrap_or(0)
    }

    /// Set document count for a tenant.
    pub fn set_document_count(&self, tenant_id: &TenantId, count: u64) {
        self.document_counts.insert(tenant_id.clone(), count);
    }

    /// Increment document count for a tenant.
    pub fn increment_documents(&self, tenant_id: &TenantId, count: u64) {
        self.document_counts
            .entry(tenant_id.clone())
            .and_modify(|v| *v += count)
            .or_insert(count);
    }

    /// Decrement document count for a tenant.
    pub fn decrement_documents(&self, tenant_id: &TenantId, count: u64) {
        self.document_counts
            .entry(tenant_id.clone())
            .and_modify(|v| {
                *v = v.saturating_sub(count);
            });
    }

    /// Get current storage usage for a tenant.
    pub fn get_storage_bytes(&self, tenant_id: &TenantId) -> u64 {
        self.storage_bytes
            .get(tenant_id)
            .map(|v| *v.value())
            .unwrap_or(0)
    }

    /// Add storage bytes for a tenant.
    pub fn add_storage(&self, tenant_id: &TenantId, bytes: u64) {
        self.storage_bytes
            .entry(tenant_id.clone())
            .and_modify(|v| *v += bytes)
            .or_insert(bytes);
    }

    /// Release storage bytes for a tenant.
    pub fn release_storage(&self, tenant_id: &TenantId, bytes: u64) {
        self.storage_bytes.entry(tenant_id.clone()).and_modify(|v| {
            *v = v.saturating_sub(bytes);
        });
    }

    /// Check document quota.
    pub fn check_document_quota(
        &self,
        tenant_id: &TenantId,
        quotas: &QuotaConfig,
    ) -> Result<(), GateError> {
        let current = self.get_document_count(tenant_id);
        if current >= quotas.max_documents {
            return Err(GateError::quota_exceeded(
                "documents",
                quotas.max_documents,
                current,
            ));
        }
        Ok(())
    }

    /// Check storage quota.
    pub fn check_storage_quota(
        &self,
        tenant_id: &TenantId,
        quotas: &QuotaConfig,
        additional_bytes: u64,
    ) -> Result<(), GateError> {
        let current = self.get_storage_bytes(tenant_id);
        let projected = current + additional_bytes;
        if projected > quotas.max_storage_bytes {
            return Err(GateError::quota_exceeded(
                "storage_bytes",
                quotas.max_storage_bytes,
                current,
            ));
        }
        Ok(())
    }

    /// Get rule count for a tenant.
    pub fn get_rule_count(&self, tenant_id: &TenantId) -> u32 {
        self.rule_counts
            .get(tenant_id)
            .map(|v| *v.value())
            .unwrap_or(0)
    }

    /// Check rule quota.
    pub fn check_rule_quota(
        &self,
        tenant_id: &TenantId,
        quotas: &QuotaConfig,
    ) -> Result<(), GateError> {
        let current = self.get_rule_count(tenant_id);
        if current >= quotas.max_rules {
            return Err(GateError::quota_exceeded(
                "rules",
                quotas.max_rules as u64,
                current as u64,
            ));
        }
        Ok(())
    }
}

impl Default for QuotaTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Gate for document ingestion operations.
///
/// Enforces:
/// - Tenant status (active vs suspended)
/// - Upload rate limits
/// - Document count quotas
/// - Storage quotas
pub struct IngestionGate {
    tenant_configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
    rate_limiter: Arc<RateLimiter>,
    quota_tracker: Arc<QuotaTracker>,
}

impl IngestionGate {
    /// Create a new ingestion gate.
    pub fn new(
        tenant_configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
        rate_limiter: Arc<RateLimiter>,
        quota_tracker: Arc<QuotaTracker>,
    ) -> Self {
        Self {
            tenant_configs,
            rate_limiter,
            quota_tracker,
        }
    }

    /// Check if upload is allowed and record the attempt.
    pub async fn check_upload(&self, ctx: &AuthContext, file_size: u64) -> Result<(), GateError> {
        let configs = self.tenant_configs.read().await;
        let config = configs
            .get(&ctx.tenant_id)
            .ok_or_else(|| GateError::tenant_suspended("Tenant not found"))?;

        // Check tenant status
        if !config.is_active() {
            return Err(GateError::tenant_suspended(format!(
                "Tenant is {}",
                config.status
            )));
        }

        // Check rate limits
        self.rate_limiter
            .check_upload(&ctx.tenant_id, &config.rate_limits)?;

        // Check document quota
        self.quota_tracker
            .check_document_quota(&ctx.tenant_id, &config.quotas)?;

        // Check storage quota
        self.quota_tracker
            .check_storage_quota(&ctx.tenant_id, &config.quotas, file_size)?;

        // Record the upload attempt for rate limiting
        self.rate_limiter.record(&ctx.tenant_id, "upload");

        Ok(())
    }
}

#[async_trait]
impl ServiceGate for IngestionGate {
    async fn check(&self, ctx: &AuthContext) -> Result<(), GateError> {
        let configs = self.tenant_configs.read().await;
        let config = configs
            .get(&ctx.tenant_id)
            .ok_or_else(|| GateError::tenant_suspended("Tenant not found"))?;

        // Check tenant status
        if !config.is_active() {
            return Err(GateError::tenant_suspended(format!(
                "Tenant is {}",
                config.status
            )));
        }

        // Check request rate limit
        self.rate_limiter
            .check_request(&ctx.tenant_id, &config.rate_limits)?;

        // Record the request for rate limiting
        self.rate_limiter.record(&ctx.tenant_id, "request");

        Ok(())
    }

    async fn check_operation(&self, ctx: &AuthContext, operation: &str) -> Result<(), GateError> {
        match operation {
            "upload" => self.check_upload(ctx, 0).await,
            _ => self.check(ctx).await,
        }
    }
}

/// Get the current Unix timestamp in seconds.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fen_core::domain::acl::TenantPermissions;
    use fen_core::domain::tenant_config::TenantStatus;

    fn create_test_context(tenant_id: TenantId) -> AuthContext {
        AuthContext {
            user_id: "test-user".to_string(),
            tenant_id: tenant_id.clone(),
            permissions: TenantPermissions::new(tenant_id, "test-user"),
        }
    }

    #[test]
    fn test_rate_limiter_under_limit() {
        let limiter = RateLimiter::new(3600);
        let tenant_id = TenantId::new();

        // Record a few requests
        for _ in 0..5 {
            limiter.record(&tenant_id, "upload");
        }

        // Should be under limit
        let result = limiter.check(&tenant_id, "upload", 10);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
    }

    #[test]
    fn test_rate_limiter_exceeds_limit() {
        let limiter = RateLimiter::new(3600);
        let tenant_id = TenantId::new();

        // Record up to limit
        for _ in 0..10 {
            limiter.record(&tenant_id, "upload");
        }

        // Should exceed limit
        let result = limiter.check(&tenant_id, "upload", 10);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::RateLimitExceeded { .. }
        ));
    }

    #[test]
    fn test_quota_tracker_documents() {
        let tracker = QuotaTracker::new();
        let tenant_id = TenantId::new();

        tracker.set_document_count(&tenant_id, 50);
        assert_eq!(tracker.get_document_count(&tenant_id), 50);

        tracker.increment_documents(&tenant_id, 10);
        assert_eq!(tracker.get_document_count(&tenant_id), 60);

        tracker.decrement_documents(&tenant_id, 5);
        assert_eq!(tracker.get_document_count(&tenant_id), 55);
    }

    #[test]
    fn test_quota_exceeded() {
        let tracker = QuotaTracker::new();
        let tenant_id = TenantId::new();

        let quotas = QuotaConfig {
            max_documents: 100,
            max_storage_bytes: 1024 * 1024,
            max_rules: 50,
            max_users: 10,
        };

        // Under quota
        tracker.set_document_count(&tenant_id, 50);
        assert!(tracker.check_document_quota(&tenant_id, &quotas).is_ok());

        // At quota
        tracker.set_document_count(&tenant_id, 100);
        let result = tracker.check_document_quota(&tenant_id, &quotas);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::QuotaExceeded { .. }
        ));
    }

    #[tokio::test]
    async fn test_ingestion_gate_active_tenant() {
        let tenant_id = TenantId::new();
        let config = TenantConfig::new(tenant_id.clone(), "Test Tenant");

        let mut configs = HashMap::new();
        configs.insert(tenant_id.clone(), config);

        let gate = IngestionGate::new(
            Arc::new(RwLock::new(configs)),
            Arc::new(RateLimiter::new(3600)),
            Arc::new(QuotaTracker::new()),
        );

        let ctx = create_test_context(tenant_id);
        let result = gate.check(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_ingestion_gate_suspended_tenant() {
        let tenant_id = TenantId::new();
        let mut config = TenantConfig::new(tenant_id.clone(), "Test Tenant");
        config.status = TenantStatus::Suspended;

        let mut configs = HashMap::new();
        configs.insert(tenant_id.clone(), config);

        let gate = IngestionGate::new(
            Arc::new(RwLock::new(configs)),
            Arc::new(RateLimiter::new(3600)),
            Arc::new(QuotaTracker::new()),
        );

        let ctx = create_test_context(tenant_id);
        let result = gate.check(&ctx).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            GateError::TenantSuspended { .. }
        ));
    }
}

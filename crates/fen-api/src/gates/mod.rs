//! Service-level gates for enforcing tenant policies.
//!
//! Gates provide a layer of policy enforcement before service operations,
//! including rate limiting, quota enforcement, and feature flag checks.

#![allow(dead_code)]

pub mod ingestion;

use async_trait::async_trait;
use fen_core::domain::cluster::TenantId;
use thiserror::Error;

use crate::middleware::AuthContext;

#[allow(unused_imports)]
pub use ingestion::{IngestionGate, QuotaTracker, RateLimiter};

/// Errors from service gate checks.
#[derive(Debug, Error)]
pub enum GateError {
    /// Rate limit has been exceeded.
    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded {
        message: String,
        retry_after_secs: Option<u64>,
    },

    /// Resource quota has been exceeded.
    #[error("Quota exceeded: {resource} limit of {limit} reached (current: {current})")]
    QuotaExceeded {
        resource: String,
        limit: u64,
        current: u64,
    },

    /// Feature is disabled for this tenant.
    #[error("Feature disabled: {feature}")]
    FeatureDisabled { feature: String },

    /// Tenant is suspended and cannot perform write operations.
    #[error("Tenant suspended: {reason}")]
    TenantSuspended { reason: String },

    /// Internal error during gate check.
    #[error("Gate check failed: {0}")]
    Internal(String),
}

impl GateError {
    /// Create a rate limit exceeded error.
    pub fn rate_limited(message: impl Into<String>, retry_after: Option<u64>) -> Self {
        Self::RateLimitExceeded {
            message: message.into(),
            retry_after_secs: retry_after,
        }
    }

    /// Create a quota exceeded error.
    pub fn quota_exceeded(resource: impl Into<String>, limit: u64, current: u64) -> Self {
        Self::QuotaExceeded {
            resource: resource.into(),
            limit,
            current,
        }
    }

    /// Create a feature disabled error.
    pub fn feature_disabled(feature: impl Into<String>) -> Self {
        Self::FeatureDisabled {
            feature: feature.into(),
        }
    }

    /// Create a tenant suspended error.
    pub fn tenant_suspended(reason: impl Into<String>) -> Self {
        Self::TenantSuspended {
            reason: reason.into(),
        }
    }
}

/// HTTP status code for gate errors.
impl GateError {
    /// Get the appropriate HTTP status code for this error.
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            GateError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            GateError::QuotaExceeded { .. } => StatusCode::PAYMENT_REQUIRED,
            GateError::FeatureDisabled { .. } => StatusCode::FORBIDDEN,
            GateError::TenantSuspended { .. } => StatusCode::FORBIDDEN,
            GateError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Service gate trait for enforcing tenant-level policies.
///
/// Implement this trait for each service that needs policy enforcement.
#[async_trait]
pub trait ServiceGate: Send + Sync {
    /// Check if an operation is allowed for the given authentication context.
    ///
    /// Returns `Ok(())` if the operation is allowed, or an appropriate
    /// `GateError` if it should be blocked.
    async fn check(&self, ctx: &AuthContext) -> Result<(), GateError>;

    /// Check if a specific operation is allowed.
    ///
    /// The `operation` parameter identifies the specific action being performed.
    async fn check_operation(&self, ctx: &AuthContext, _operation: &str) -> Result<(), GateError> {
        // Default implementation just calls the basic check
        self.check(ctx).await
    }
}

/// Combine multiple gates that all must pass.
pub struct CompositeGate {
    gates: Vec<Box<dyn ServiceGate>>,
}

impl CompositeGate {
    /// Create a new composite gate.
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }

    /// Add a gate to the composite.
    pub fn add_gate<G: ServiceGate + 'static>(mut self, gate: G) -> Self {
        self.gates.push(Box::new(gate));
        self
    }
}

impl Default for CompositeGate {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceGate for CompositeGate {
    async fn check(&self, ctx: &AuthContext) -> Result<(), GateError> {
        for gate in &self.gates {
            gate.check(ctx).await?;
        }
        Ok(())
    }
}

/// Gate that checks if a feature is enabled for the tenant.
pub struct FeatureGate {
    feature_name: String,
    tenant_features: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<TenantId, std::collections::HashSet<String>>>,
    >,
}

impl FeatureGate {
    /// Create a new feature gate.
    pub fn new(
        feature_name: impl Into<String>,
        tenant_features: std::sync::Arc<
            tokio::sync::RwLock<
                std::collections::HashMap<TenantId, std::collections::HashSet<String>>,
            >,
        >,
    ) -> Self {
        Self {
            feature_name: feature_name.into(),
            tenant_features,
        }
    }
}

#[async_trait]
impl ServiceGate for FeatureGate {
    async fn check(&self, ctx: &AuthContext) -> Result<(), GateError> {
        let features = self.tenant_features.read().await;
        let tenant_enabled = features
            .get(&ctx.tenant_id)
            .map(|f| f.contains(&self.feature_name))
            .unwrap_or(false);

        if !tenant_enabled {
            return Err(GateError::feature_disabled(&self.feature_name));
        }

        Ok(())
    }
}

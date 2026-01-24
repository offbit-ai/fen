//! Notification delivery provider trait and types.
//!
//! This module defines the core abstraction for notification delivery,
//! allowing multiple delivery channels (WebSocket, Email, Webhook, etc.)
//! to be used interchangeably.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use fen_core::domain::TenantId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during notification delivery.
#[derive(Debug, Error)]
pub enum DeliveryError {
    /// Failed to deliver notification.
    #[error("Delivery failed: {0}")]
    DeliveryFailed(String),

    /// Provider is not configured.
    #[error("Provider not configured: {0}")]
    NotConfigured(String),

    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Timeout error.
    #[error("Delivery timed out: {0}")]
    Timeout(String),

    /// Rate limited.
    #[error("Rate limited: retry after {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    /// Recipient not found.
    #[error("Recipient not found: {0}")]
    RecipientNotFound(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Configuration error (alias for InvalidConfiguration).
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Provider is unavailable.
    #[error("Provider unavailable: {0}")]
    ProviderUnavailable(String),
}

/// Severity level for notifications.
///
/// Ordered from lowest to highest: Info < Warning < Error < Critical
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational notification.
    Info,
    /// Warning notification.
    Warning,
    /// Error/high severity notification.
    Error,
    /// Critical notification requiring immediate attention.
    Critical,
}

impl Default for Severity {
    fn default() -> Self {
        Self::Info
    }
}

impl std::str::FromStr for Severity {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(Self::Info),
            "warning" | "warn" => Ok(Self::Warning),
            "error" | "high" => Ok(Self::Error),
            "critical" | "crit" => Ok(Self::Critical),
            _ => Err(()),
        }
    }
}

/// Type of notification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    /// Anomaly was detected in a document.
    AnomalyDetected,
    /// Document validation completed.
    ValidationComplete,
    /// Vendor baseline was updated.
    BaselineUpdated,
    /// System alert (health, performance, etc.).
    SystemAlert,
    /// Batched report (for email digests).
    BatchReport,
    /// Custom notification type.
    Custom(String),
}

impl NotificationType {
    /// Get the string representation of this notification type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::AnomalyDetected => "anomaly_detected",
            Self::ValidationComplete => "validation_complete",
            Self::BaselineUpdated => "baseline_updated",
            Self::SystemAlert => "system_alert",
            Self::BatchReport => "batch_report",
            Self::Custom(s) => s.as_str(),
        }
    }
}

/// Notification payload that can be delivered via any provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPayload {
    /// Unique notification ID.
    pub id: String,
    /// Tenant this notification belongs to.
    pub tenant_id: TenantId,
    /// Type of notification.
    pub notification_type: NotificationType,
    /// Severity level.
    pub severity: Severity,
    /// Short title/subject.
    pub title: String,
    /// Full notification body.
    pub body: String,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// When the notification was created.
    pub timestamp: DateTime<Utc>,
}

impl NotificationPayload {
    /// Create a new notification payload.
    pub fn new(
        tenant_id: TenantId,
        notification_type: NotificationType,
        severity: Severity,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tenant_id,
            notification_type,
            severity,
            title: title.into(),
            body: body.into(),
            metadata: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Add metadata to the notification.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Delivery mode for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// Deliver immediately (WebSocket, SSE).
    Immediate,
    /// Batch and deliver periodically (Email reports).
    Batched {
        /// Interval in seconds between batch deliveries.
        interval_secs: u64,
    },
    /// Fire and forget (Webhooks).
    Async,
}

/// Trait for notification delivery providers.
///
/// Implement this trait for each delivery channel (WebSocket, Email, Webhook, Slack, etc.)
/// to enable pluggable notification delivery.
///
/// # Example
///
/// ```ignore
/// use fen_notify::{NotificationDeliveryProvider, DeliveryMode, NotificationPayload, DeliveryError};
///
/// struct SlackProvider {
///     webhook_url: String,
/// }
///
/// #[async_trait]
/// impl NotificationDeliveryProvider for SlackProvider {
///     fn provider_id(&self) -> &str { "slack" }
///     fn delivery_mode(&self) -> DeliveryMode { DeliveryMode::Async }
///
///     async fn deliver(&self, payload: &NotificationPayload) -> Result<(), DeliveryError> {
///         // Send to Slack webhook...
///         Ok(())
///     }
///
///     async fn health_check(&self) -> Result<(), DeliveryError> {
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait NotificationDeliveryProvider: Send + Sync {
    /// Provider identifier (e.g., "websocket", "email", "slack", "webhook").
    fn provider_id(&self) -> &str;

    /// Delivery mode for this provider.
    fn delivery_mode(&self) -> DeliveryMode;

    /// Deliver a single notification immediately.
    ///
    /// # Arguments
    /// * `payload` - The notification to deliver
    ///
    /// # Returns
    /// Ok(()) if delivery was successful
    async fn deliver(&self, payload: &NotificationPayload) -> Result<(), DeliveryError>;

    /// Deliver a batch of notifications (for batched providers like email).
    ///
    /// Default implementation calls `deliver` for each payload.
    ///
    /// # Arguments
    /// * `payloads` - The notifications to deliver as a batch
    async fn deliver_batch(&self, payloads: &[NotificationPayload]) -> Result<(), DeliveryError> {
        for payload in payloads {
            self.deliver(payload).await?;
        }
        Ok(())
    }

    /// Check if this provider supports the given tenant.
    ///
    /// Override to implement tenant-specific provider restrictions.
    async fn supports_tenant(&self, _tenant_id: &TenantId) -> bool {
        true // Default: all tenants supported
    }

    /// Check if this provider supports the given notification type.
    ///
    /// Override to filter notifications by type.
    fn supports_notification_type(&self, _notification_type: &NotificationType) -> bool {
        true // Default: all types supported
    }

    /// Check if this provider supports the given severity level.
    ///
    /// Override to filter notifications by severity.
    fn supports_severity(&self, _severity: Severity) -> bool {
        true // Default: all severities supported
    }

    /// Health check for the provider.
    ///
    /// Returns Ok(()) if the provider is healthy and ready to deliver.
    async fn health_check(&self) -> Result<(), DeliveryError>;

    /// Get provider metrics (optional).
    ///
    /// Returns a map of metric name to value.
    fn metrics(&self) -> HashMap<String, f64> {
        HashMap::new()
    }
}

/// Configuration for a delivery provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled.
    pub enabled: bool,
    /// Provider type identifier.
    pub provider_type: String,
    /// Provider-specific settings.
    #[serde(default)]
    pub settings: HashMap<String, serde_json::Value>,
}

impl ProviderConfig {
    /// Create a new provider configuration.
    pub fn new(provider_type: impl Into<String>) -> Self {
        Self {
            enabled: true,
            provider_type: provider_type.into(),
            settings: HashMap::new(),
        }
    }

    /// Set a configuration setting.
    pub fn with_setting(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.settings.insert(key.into(), value);
        self
    }

    /// Get a string setting.
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.settings
            .get(key)
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Get a u64 setting.
    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.settings.get(key).and_then(|v| v.as_u64())
    }

    /// Get a bool setting.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.settings.get(key).and_then(|v| v.as_bool())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_payload_creation() {
        let payload = NotificationPayload::new(
            TenantId::new(),
            NotificationType::AnomalyDetected,
            Severity::Warning,
            "Test Title",
            "Test Body",
        );

        assert!(!payload.id.is_empty());
        assert_eq!(payload.title, "Test Title");
        assert_eq!(payload.severity, Severity::Warning);
    }

    #[test]
    fn test_notification_payload_with_metadata() {
        let payload = NotificationPayload::new(
            TenantId::new(),
            NotificationType::AnomalyDetected,
            Severity::Info,
            "Test",
            "Body",
        )
        .with_metadata("document_id", serde_json::json!("doc-123"))
        .with_metadata("z_score", serde_json::json!(2.5));

        assert_eq!(payload.metadata.len(), 2);
        assert_eq!(
            payload.metadata.get("document_id"),
            Some(&serde_json::json!("doc-123"))
        );
    }

    #[test]
    fn test_severity_from_str() {
        assert_eq!("info".parse::<Severity>(), Ok(Severity::Info));
        assert_eq!("warning".parse::<Severity>(), Ok(Severity::Warning));
        assert_eq!("warn".parse::<Severity>(), Ok(Severity::Warning));
        assert_eq!("error".parse::<Severity>(), Ok(Severity::Error));
        assert_eq!("critical".parse::<Severity>(), Ok(Severity::Critical));
    }

    #[test]
    fn test_provider_config() {
        let config = ProviderConfig::new("email")
            .with_setting("smtp_host", serde_json::json!("mail.example.com"))
            .with_setting("batch_interval", serde_json::json!(3600));

        assert!(config.enabled);
        assert_eq!(
            config.get_string("smtp_host"),
            Some("mail.example.com".to_string())
        );
        assert_eq!(config.get_u64("batch_interval"), Some(3600));
    }
}

//! Notification hub for routing to multiple delivery providers.
//!
//! The hub manages registered providers and routes notifications
//! based on tenant preferences and provider capabilities.

use dashmap::DashMap;
use fen_core::domain::TenantId;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::provider::{
    DeliveryError, DeliveryMode, NotificationDeliveryProvider, NotificationPayload,
    NotificationType, Severity,
};

/// Configuration for the notification hub.
#[derive(Debug, Clone)]
pub struct HubConfig {
    /// Default providers for tenants without specific preferences.
    pub default_providers: Vec<String>,
    /// Broadcast channel capacity.
    pub broadcast_capacity: usize,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            default_providers: vec!["websocket".to_string()],
            broadcast_capacity: 1000,
        }
    }
}

/// Notification hub that routes to multiple delivery providers.
///
/// The hub supports:
/// - Multiple registered providers (WebSocket, Email, Webhook, etc.)
/// - Per-tenant provider preferences
/// - Filtering by notification type and severity
/// - Broadcast channel for real-time consumers
pub struct NotificationHub {
    /// Registered delivery providers.
    providers: DashMap<String, Arc<dyn NotificationDeliveryProvider>>,
    /// Tenant-specific provider preferences.
    tenant_preferences: DashMap<TenantId, Vec<String>>,
    /// Default providers for tenants without preferences.
    default_providers: Vec<String>,
    /// Broadcast channel for real-time consumers.
    broadcast_tx: broadcast::Sender<NotificationPayload>,
}

impl NotificationHub {
    /// Create a new notification hub with default configuration.
    pub fn new() -> Self {
        Self::with_config(HubConfig::default())
    }

    /// Create a new notification hub with custom configuration.
    pub fn with_config(config: HubConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(config.broadcast_capacity);

        Self {
            providers: DashMap::new(),
            tenant_preferences: DashMap::new(),
            default_providers: config.default_providers,
            broadcast_tx,
        }
    }

    /// Register a delivery provider.
    ///
    /// # Arguments
    /// * `provider` - The provider to register
    pub fn register_provider(&self, provider: Arc<dyn NotificationDeliveryProvider>) {
        let id = provider.provider_id().to_string();
        info!(provider_id = %id, "Registering notification provider");
        self.providers.insert(id, provider);
    }

    /// Unregister a delivery provider.
    ///
    /// # Arguments
    /// * `provider_id` - The ID of the provider to unregister
    pub fn unregister_provider(&self, provider_id: &str) -> Option<Arc<dyn NotificationDeliveryProvider>> {
        self.providers.remove(provider_id).map(|(_, v)| v)
    }

    /// Get a registered provider by ID.
    pub fn get_provider(&self, provider_id: &str) -> Option<Arc<dyn NotificationDeliveryProvider>> {
        self.providers.get(provider_id).map(|v| v.clone())
    }

    /// List all registered provider IDs.
    pub fn list_providers(&self) -> Vec<String> {
        self.providers.iter().map(|r| r.key().clone()).collect()
    }

    /// Set tenant's preferred providers.
    ///
    /// # Arguments
    /// * `tenant_id` - The tenant ID
    /// * `provider_ids` - List of provider IDs (e.g., ["websocket", "email"])
    pub fn set_tenant_providers(&self, tenant_id: TenantId, provider_ids: Vec<String>) {
        self.tenant_preferences.insert(tenant_id, provider_ids);
    }

    /// Get tenant's preferred providers.
    pub fn get_tenant_providers(&self, tenant_id: &TenantId) -> Vec<String> {
        self.tenant_preferences
            .get(tenant_id)
            .map(|v| v.clone())
            .unwrap_or_else(|| self.default_providers.clone())
    }

    /// Subscribe to the broadcast channel for real-time notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<NotificationPayload> {
        self.broadcast_tx.subscribe()
    }

    /// Broadcast a notification to all applicable providers for the tenant.
    ///
    /// This method:
    /// 1. Gets the tenant's preferred providers
    /// 2. Filters by notification type and severity
    /// 3. Delivers to all matching providers
    /// 4. Broadcasts to the real-time channel
    ///
    /// # Arguments
    /// * `payload` - The notification to broadcast
    ///
    /// # Returns
    /// Ok(()) if at least one provider delivered successfully
    pub async fn broadcast(&self, payload: NotificationPayload) -> Result<(), DeliveryError> {
        let preferred = self.get_tenant_providers(&payload.tenant_id);
        let mut success = false;
        let mut last_error = None;

        // Broadcast to real-time channel
        let _ = self.broadcast_tx.send(payload.clone());

        for provider_id in &preferred {
            if let Some(provider) = self.providers.get(provider_id) {
                // Check if provider supports this notification
                if !provider.supports_notification_type(&payload.notification_type) {
                    continue;
                }
                if !provider.supports_severity(payload.severity) {
                    continue;
                }
                if !provider.supports_tenant(&payload.tenant_id).await {
                    continue;
                }

                match provider.deliver(&payload).await {
                    Ok(()) => {
                        success = true;
                    }
                    Err(e) => {
                        warn!(
                            provider = %provider_id,
                            error = %e,
                            "Provider delivery failed"
                        );
                        last_error = Some(e);
                    }
                }
            }
        }

        if success {
            Ok(())
        } else if let Some(e) = last_error {
            Err(e)
        } else {
            // No providers matched
            Ok(())
        }
    }

    /// Broadcast to a specific set of providers.
    ///
    /// # Arguments
    /// * `payload` - The notification to broadcast
    /// * `provider_ids` - Specific providers to use
    pub async fn broadcast_to(
        &self,
        payload: NotificationPayload,
        provider_ids: &[String],
    ) -> Result<(), DeliveryError> {
        let mut success = false;

        for provider_id in provider_ids {
            if let Some(provider) = self.providers.get(provider_id) {
                if let Err(e) = provider.deliver(&payload).await {
                    warn!(
                        provider = %provider_id,
                        error = %e,
                        "Provider delivery failed"
                    );
                } else {
                    success = true;
                }
            }
        }

        if success {
            Ok(())
        } else {
            Err(DeliveryError::DeliveryFailed(
                "All providers failed".to_string(),
            ))
        }
    }

    /// Create a convenience builder for anomaly notifications.
    pub fn anomaly_notification(
        &self,
        tenant_id: TenantId,
        document_id: &str,
        anomaly_type: &str,
        severity: Severity,
        description: &str,
    ) -> NotificationPayload {
        NotificationPayload::new(
            tenant_id,
            NotificationType::AnomalyDetected,
            severity,
            format!("Anomaly Detected: {}", anomaly_type),
            description.to_string(),
        )
        .with_metadata("document_id", serde_json::json!(document_id))
        .with_metadata("anomaly_type", serde_json::json!(anomaly_type))
    }

    /// Run health checks on all providers.
    pub async fn health_check_all(&self) -> Vec<(String, Result<(), DeliveryError>)> {
        let mut results = Vec::new();

        for entry in self.providers.iter() {
            let id = entry.key().clone();
            let result = entry.value().health_check().await;
            results.push((id, result));
        }

        results
    }

    /// Get providers that support batched delivery mode.
    pub fn get_batched_providers(&self) -> Vec<Arc<dyn NotificationDeliveryProvider>> {
        self.providers
            .iter()
            .filter(|entry| matches!(entry.value().delivery_mode(), DeliveryMode::Batched { .. }))
            .map(|entry| entry.value().clone())
            .collect()
    }
}

impl Default for NotificationHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::DeliveryMode;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test provider that counts deliveries.
    struct TestProvider {
        id: String,
        delivery_count: AtomicUsize,
    }

    impl TestProvider {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_string(),
                delivery_count: AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.delivery_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl NotificationDeliveryProvider for TestProvider {
        fn provider_id(&self) -> &str {
            &self.id
        }

        fn delivery_mode(&self) -> DeliveryMode {
            DeliveryMode::Immediate
        }

        async fn deliver(&self, _payload: &NotificationPayload) -> Result<(), DeliveryError> {
            self.delivery_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn health_check(&self) -> Result<(), DeliveryError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_hub_register_provider() {
        let hub = NotificationHub::new();
        let provider = Arc::new(TestProvider::new("test"));

        hub.register_provider(provider);

        assert!(hub.get_provider("test").is_some());
        assert_eq!(hub.list_providers(), vec!["test"]);
    }

    #[tokio::test]
    async fn test_hub_broadcast() {
        let hub = NotificationHub::with_config(HubConfig {
            default_providers: vec!["test".to_string()],
            broadcast_capacity: 10,
        });

        let provider = Arc::new(TestProvider::new("test"));
        hub.register_provider(provider.clone());

        let payload = NotificationPayload::new(
            TenantId::new(),
            NotificationType::AnomalyDetected,
            Severity::Warning,
            "Test",
            "Test body",
        );

        hub.broadcast(payload).await.unwrap();

        assert_eq!(provider.count(), 1);
    }

    #[tokio::test]
    async fn test_hub_tenant_preferences() {
        let hub = NotificationHub::new();
        let tenant = TenantId::new();

        hub.set_tenant_providers(tenant.clone(), vec!["email".to_string(), "slack".to_string()]);

        let providers = hub.get_tenant_providers(&tenant);
        assert_eq!(providers, vec!["email", "slack"]);
    }
}

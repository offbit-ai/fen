//! WebSocket delivery provider for real-time UI updates.
//!
//! This provider maintains connections per tenant and broadcasts
//! notifications immediately to all connected clients.

use async_trait::async_trait;
use dashmap::DashMap;
use fen_core::domain::TenantId;
use tokio::sync::broadcast;
use tracing::debug;

use crate::provider::{
    DeliveryError, DeliveryMode, NotificationDeliveryProvider, NotificationPayload,
    NotificationType, Severity,
};

/// Configuration for the WebSocket provider.
#[derive(Debug, Clone)]
pub struct WebSocketProviderConfig {
    /// Channel capacity per tenant.
    pub channel_capacity: usize,
}

impl Default for WebSocketProviderConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 1000,
        }
    }
}

/// WebSocket delivery provider for real-time notifications.
///
/// This provider maintains broadcast channels per tenant, allowing
/// multiple WebSocket connections to receive notifications in real-time.
pub struct WebSocketProvider {
    /// Broadcast channels per tenant.
    connections: DashMap<TenantId, broadcast::Sender<NotificationPayload>>,
    /// Channel capacity for new tenants.
    channel_capacity: usize,
}

impl WebSocketProvider {
    /// Create a new WebSocket provider with default configuration.
    pub fn new() -> Self {
        Self::with_config(WebSocketProviderConfig::default())
    }

    /// Create a new WebSocket provider with custom configuration.
    pub fn with_config(config: WebSocketProviderConfig) -> Self {
        Self {
            connections: DashMap::new(),
            channel_capacity: config.channel_capacity,
        }
    }

    /// Subscribe to notifications for a specific tenant.
    ///
    /// Returns a receiver that will receive all notifications for the tenant.
    pub fn subscribe(&self, tenant_id: &TenantId) -> broadcast::Receiver<NotificationPayload> {
        let sender = self
            .connections
            .entry(tenant_id.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(self.channel_capacity);
                tx
            });
        sender.subscribe()
    }

    /// Get the number of active subscribers for a tenant.
    pub fn subscriber_count(&self, tenant_id: &TenantId) -> usize {
        self.connections
            .get(tenant_id)
            .map(|sender| sender.receiver_count())
            .unwrap_or(0)
    }

    /// Get total number of tenants with active connections.
    pub fn tenant_count(&self) -> usize {
        self.connections.len()
    }

    /// Remove a tenant's channel (e.g., when no more subscribers).
    pub fn remove_tenant(&self, tenant_id: &TenantId) {
        self.connections.remove(tenant_id);
    }

    /// Clean up channels with no subscribers.
    pub fn cleanup_empty_channels(&self) {
        self.connections
            .retain(|_, sender| sender.receiver_count() > 0);
    }
}

impl Default for WebSocketProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationDeliveryProvider for WebSocketProvider {
    fn provider_id(&self) -> &str {
        "websocket"
    }

    fn delivery_mode(&self) -> DeliveryMode {
        DeliveryMode::Immediate
    }

    fn supports_notification_type(&self, _notification_type: &NotificationType) -> bool {
        true // WebSocket supports all notification types
    }

    fn supports_severity(&self, _severity: Severity) -> bool {
        true // WebSocket supports all severities
    }

    async fn supports_tenant(&self, _tenant_id: &TenantId) -> bool {
        true // All tenants can use WebSocket
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<(), DeliveryError> {
        if let Some(sender) = self.connections.get(&payload.tenant_id) {
            match sender.send(payload.clone()) {
                Ok(count) => {
                    debug!(
                        tenant_id = %payload.tenant_id,
                        receivers = count,
                        "Delivered notification via WebSocket"
                    );
                    Ok(())
                }
                Err(_) => {
                    // No active receivers - this is not necessarily an error
                    debug!(
                        tenant_id = %payload.tenant_id,
                        "No active WebSocket receivers for tenant"
                    );
                    Ok(())
                }
            }
        } else {
            // No channel for tenant - create one for future subscribers
            debug!(
                tenant_id = %payload.tenant_id,
                "No WebSocket channel for tenant, notification dropped"
            );
            Ok(())
        }
    }

    async fn health_check(&self) -> Result<(), DeliveryError> {
        // WebSocket provider is always healthy if it exists
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_websocket_subscribe_and_deliver() {
        let provider = WebSocketProvider::new();
        let tenant_id = TenantId::new();

        // Subscribe before sending
        let mut rx = provider.subscribe(&tenant_id);

        let payload = NotificationPayload::new(
            tenant_id.clone(),
            NotificationType::AnomalyDetected,
            Severity::Warning,
            "Test Alert",
            "Test body",
        );

        provider.deliver(&payload).await.unwrap();

        let received = rx.try_recv().unwrap();
        assert_eq!(received.title, "Test Alert");
    }

    #[tokio::test]
    async fn test_websocket_no_subscribers() {
        let provider = WebSocketProvider::new();
        let tenant_id = TenantId::new();

        let payload = NotificationPayload::new(
            tenant_id,
            NotificationType::SystemAlert,
            Severity::Info,
            "Test",
            "Body",
        );

        // Should succeed even with no subscribers
        let result = provider.deliver(&payload).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_websocket_subscriber_count() {
        let provider = WebSocketProvider::new();
        let tenant_id = TenantId::new();

        assert_eq!(provider.subscriber_count(&tenant_id), 0);

        let _rx1 = provider.subscribe(&tenant_id);
        assert_eq!(provider.subscriber_count(&tenant_id), 1);

        let _rx2 = provider.subscribe(&tenant_id);
        assert_eq!(provider.subscriber_count(&tenant_id), 2);
    }
}

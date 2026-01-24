//! Real-time notification system for Fen.
//!
//! This crate provides a pluggable notification delivery system with support
//! for multiple delivery channels including WebSocket, Email, and Webhooks.
//!
//! # Architecture
//!
//! The notification system is built around the `NotificationDeliveryProvider` trait,
//! which defines the interface for all delivery channels. The `NotificationHub`
//! coordinates delivery across multiple providers based on tenant preferences.
//!
//! # Features
//!
//! - `email` - Enable batched email report delivery via SMTP
//! - `webhook` - Enable webhook delivery to external endpoints
//! - `full` - Enable all features
//!
//! # Example
//!
//! ```ignore
//! use fen_notify::{NotificationHub, HubConfig};
//! use fen_notify::providers::WebSocketProvider;
//! use std::sync::Arc;
//!
//! let hub = NotificationHub::with_config(HubConfig {
//!     default_providers: vec!["websocket".to_string()],
//!     broadcast_capacity: 1000,
//! });
//!
//! // Register WebSocket provider
//! let ws_provider = Arc::new(WebSocketProvider::new());
//! hub.register_provider(ws_provider.clone());
//!
//! // Subscribe to notifications for a tenant
//! let mut rx = ws_provider.subscribe(&tenant_id);
//! ```

pub mod hub;
pub mod provider;
pub mod providers;

pub use hub::{HubConfig, NotificationHub};
pub use provider::{
    DeliveryError, DeliveryMode, NotificationDeliveryProvider, NotificationPayload,
    NotificationType, ProviderConfig, Severity,
};
pub use providers::WebSocketProvider;

#[cfg(feature = "email")]
pub use providers::EmailProvider;

#[cfg(feature = "webhook")]
pub use providers::WebhookProvider;

#[cfg(test)]
mod tests {
    use super::*;
    use fen_core::domain::TenantId;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_hub_with_websocket() {
        let hub = NotificationHub::new();
        let ws_provider = Arc::new(WebSocketProvider::new());

        hub.register_provider(ws_provider.clone());

        let tenant_id = TenantId::new();
        let mut rx = ws_provider.subscribe(&tenant_id);

        let payload = NotificationPayload::new(
            tenant_id.clone(),
            NotificationType::AnomalyDetected,
            Severity::Warning,
            "Test Anomaly",
            "A test anomaly was detected",
        );

        hub.broadcast(payload).await.unwrap();

        let received = rx.try_recv().unwrap();
        assert_eq!(received.title, "Test Anomaly");
    }

    #[tokio::test]
    async fn test_hub_tenant_preferences() {
        let hub = NotificationHub::new();
        let tenant_id = TenantId::new();

        hub.set_tenant_providers(tenant_id.clone(), vec!["email".to_string(), "webhook".to_string()]);

        let providers = hub.get_tenant_providers(&tenant_id);
        assert_eq!(providers, vec!["email", "webhook"]);
    }
}

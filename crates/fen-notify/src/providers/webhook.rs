//! Webhook delivery provider for external integrations.
//!
//! This provider sends notifications to configured HTTP endpoints,
//! with optional HMAC signature verification for security.

use async_trait::async_trait;
use dashmap::DashMap;
use fen_core::domain::TenantId;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, error, warn};

use crate::provider::{
    DeliveryError, DeliveryMode, NotificationDeliveryProvider, NotificationPayload,
    NotificationType, Severity,
};

/// Configuration for the webhook provider.
#[derive(Debug, Clone)]
pub struct WebhookProviderConfig {
    /// Request timeout.
    pub timeout: Duration,
    /// Maximum retries on failure.
    pub max_retries: u32,
    /// Retry delay.
    pub retry_delay: Duration,
}

impl Default for WebhookProviderConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_secs(1),
        }
    }
}

/// A webhook endpoint configuration.
#[derive(Debug, Clone)]
pub struct WebhookEndpoint {
    /// The URL to send notifications to.
    pub url: String,
    /// Optional secret for HMAC signature.
    pub secret: Option<String>,
    /// Filter by notification types (empty = all).
    pub type_filters: Vec<NotificationType>,
    /// Filter by minimum severity.
    pub min_severity: Option<Severity>,
    /// Whether this endpoint is enabled.
    pub enabled: bool,
}

/// Webhook delivery provider for external integrations.
///
/// This provider sends HTTP POST requests to configured endpoints
/// with notification payloads as JSON. It supports:
/// - Multiple endpoints per tenant
/// - HMAC-SHA256 signature verification
/// - Notification type and severity filtering
/// - Async fire-and-forget delivery with retries
pub struct WebhookProvider {
    /// HTTP client for requests.
    client: Client,
    /// Provider configuration.
    config: WebhookProviderConfig,
    /// Endpoints per tenant.
    endpoints: DashMap<TenantId, Vec<WebhookEndpoint>>,
}

impl WebhookProvider {
    /// Create a new webhook provider with default configuration.
    pub fn new() -> Self {
        Self::with_config(WebhookProviderConfig::default())
    }

    /// Create a new webhook provider with custom configuration.
    pub fn with_config(config: WebhookProviderConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            config,
            endpoints: DashMap::new(),
        }
    }

    /// Register a webhook endpoint for a tenant.
    pub fn register_endpoint(&self, tenant_id: TenantId, endpoint: WebhookEndpoint) {
        self.endpoints
            .entry(tenant_id)
            .or_insert_with(Vec::new)
            .push(endpoint);
    }

    /// Remove all endpoints for a tenant.
    pub fn remove_tenant(&self, tenant_id: &TenantId) {
        self.endpoints.remove(tenant_id);
    }

    /// Get endpoints for a tenant.
    pub fn get_endpoints(&self, tenant_id: &TenantId) -> Vec<WebhookEndpoint> {
        self.endpoints
            .get(tenant_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }

    /// Compute HMAC-SHA256 signature for a payload.
    fn compute_signature(&self, payload: &[u8], secret: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload);
        let result = mac.finalize();

        // Return hex-encoded signature
        hex::encode(result.into_bytes())
    }

    /// Check if an endpoint should receive a notification.
    fn endpoint_matches(&self, endpoint: &WebhookEndpoint, payload: &NotificationPayload) -> bool {
        if !endpoint.enabled {
            return false;
        }

        // Check type filter
        if !endpoint.type_filters.is_empty()
            && !endpoint.type_filters.contains(&payload.notification_type)
        {
            return false;
        }

        // Check severity filter
        if let Some(min_severity) = endpoint.min_severity {
            if payload.severity < min_severity {
                return false;
            }
        }

        true
    }

    /// Send a notification to an endpoint with retries.
    async fn send_to_endpoint(
        &self,
        endpoint: &WebhookEndpoint,
        payload: &NotificationPayload,
    ) -> Result<(), DeliveryError> {
        let body =
            serde_json::to_vec(payload).map_err(|e| DeliveryError::DeliveryFailed(e.to_string()))?;

        let mut attempt = 0;
        loop {
            attempt += 1;

            let mut request = self
                .client
                .post(&endpoint.url)
                .header("Content-Type", "application/json")
                .header("X-Fen-Event", payload.notification_type.as_str())
                .header("X-Fen-Notification-Id", &payload.id);

            // Add signature if secret is configured
            if let Some(secret) = &endpoint.secret {
                let signature = self.compute_signature(&body, secret);
                request = request.header("X-Fen-Signature", format!("sha256={}", signature));
            }

            match request.body(body.clone()).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!(
                            url = %endpoint.url,
                            status = %response.status(),
                            "Webhook delivered successfully"
                        );
                        return Ok(());
                    } else {
                        let status = response.status();
                        let error_body = response.text().await.unwrap_or_default();
                        warn!(
                            url = %endpoint.url,
                            status = %status,
                            body = %error_body,
                            attempt = attempt,
                            "Webhook request failed"
                        );

                        if attempt >= self.config.max_retries {
                            return Err(DeliveryError::DeliveryFailed(format!(
                                "Webhook failed after {} attempts: {} - {}",
                                attempt, status, error_body
                            )));
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        url = %endpoint.url,
                        error = %e,
                        attempt = attempt,
                        "Webhook request error"
                    );

                    if attempt >= self.config.max_retries {
                        return Err(DeliveryError::DeliveryFailed(format!(
                            "Webhook failed after {} attempts: {}",
                            attempt, e
                        )));
                    }
                }
            }

            // Wait before retry
            tokio::time::sleep(self.config.retry_delay).await;
        }
    }
}

impl Default for WebhookProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationDeliveryProvider for WebhookProvider {
    fn provider_id(&self) -> &str {
        "webhook"
    }

    fn delivery_mode(&self) -> DeliveryMode {
        DeliveryMode::Async
    }

    fn supports_notification_type(&self, _notification_type: &NotificationType) -> bool {
        true // Webhook supports all types (filtering is per-endpoint)
    }

    fn supports_severity(&self, _severity: Severity) -> bool {
        true // Webhook supports all severities (filtering is per-endpoint)
    }

    async fn supports_tenant(&self, tenant_id: &TenantId) -> bool {
        self.endpoints
            .get(tenant_id)
            .map(|endpoints| endpoints.iter().any(|e| e.enabled))
            .unwrap_or(false)
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<(), DeliveryError> {
        let endpoints = match self.endpoints.get(&payload.tenant_id) {
            Some(endpoints) => endpoints.clone(),
            None => return Ok(()), // No endpoints configured
        };

        let matching_endpoints: Vec<_> = endpoints
            .iter()
            .filter(|e| self.endpoint_matches(e, payload))
            .collect();

        if matching_endpoints.is_empty() {
            return Ok(());
        }

        // Fire and forget - spawn tasks for each endpoint
        for endpoint in matching_endpoints {
            let endpoint = endpoint.clone();
            let payload = payload.clone();
            let client = self.client.clone();
            let config = self.config.clone();

            tokio::spawn(async move {
                let provider = WebhookProvider {
                    client,
                    config,
                    endpoints: DashMap::new(),
                };

                if let Err(e) = provider.send_to_endpoint(&endpoint, &payload).await {
                    error!(
                        url = %endpoint.url,
                        error = %e,
                        "Failed to deliver webhook"
                    );
                }
            });
        }

        Ok(())
    }

    async fn health_check(&self) -> Result<(), DeliveryError> {
        // Webhook provider is healthy if we can create requests
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_signature() {
        let provider = WebhookProvider::new();
        let payload = b"test payload";
        let secret = "test_secret";

        let sig1 = provider.compute_signature(payload, secret);
        let sig2 = provider.compute_signature(payload, secret);

        assert_eq!(sig1, sig2);
        assert!(!sig1.is_empty());
    }

    #[test]
    fn test_endpoint_matches() {
        let provider = WebhookProvider::new();
        let tenant_id = TenantId::new();

        let endpoint = WebhookEndpoint {
            url: "https://example.com/webhook".to_string(),
            secret: None,
            type_filters: vec![NotificationType::AnomalyDetected],
            min_severity: Some(Severity::Warning),
            enabled: true,
        };

        // Matching payload
        let matching_payload = NotificationPayload::new(
            tenant_id.clone(),
            NotificationType::AnomalyDetected,
            Severity::Critical,
            "Test",
            "Body",
        );
        assert!(provider.endpoint_matches(&endpoint, &matching_payload));

        // Non-matching type
        let wrong_type = NotificationPayload::new(
            tenant_id.clone(),
            NotificationType::SystemAlert,
            Severity::Critical,
            "Test",
            "Body",
        );
        assert!(!provider.endpoint_matches(&endpoint, &wrong_type));

        // Non-matching severity
        let wrong_severity = NotificationPayload::new(
            tenant_id,
            NotificationType::AnomalyDetected,
            Severity::Info,
            "Test",
            "Body",
        );
        assert!(!provider.endpoint_matches(&endpoint, &wrong_severity));
    }

    #[test]
    fn test_disabled_endpoint() {
        let provider = WebhookProvider::new();
        let tenant_id = TenantId::new();

        let endpoint = WebhookEndpoint {
            url: "https://example.com/webhook".to_string(),
            secret: None,
            type_filters: vec![],
            min_severity: None,
            enabled: false,
        };

        let payload = NotificationPayload::new(
            tenant_id,
            NotificationType::AnomalyDetected,
            Severity::Critical,
            "Test",
            "Body",
        );

        assert!(!provider.endpoint_matches(&endpoint, &payload));
    }
}

//! Email delivery provider for batched notification reports.
//!
//! This provider collects notifications and sends periodic email digests
//! to tenant administrators. It supports HTML templating and configurable
//! batch intervals.

use async_trait::async_trait;
use dashmap::DashMap;
use fen_core::domain::TenantId;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::provider::{
    DeliveryError, DeliveryMode, NotificationDeliveryProvider, NotificationPayload,
    NotificationType, Severity,
};

/// SMTP configuration for email delivery.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// SMTP server hostname.
    pub host: String,
    /// SMTP server port.
    pub port: u16,
    /// SMTP username.
    pub username: String,
    /// SMTP password.
    pub password: String,
    /// Use TLS/STARTTLS.
    pub use_tls: bool,
}

/// Configuration for the email provider.
#[derive(Debug, Clone)]
pub struct EmailProviderConfig {
    /// SMTP configuration.
    pub smtp: SmtpConfig,
    /// From email address.
    pub from_address: String,
    /// From display name.
    pub from_name: String,
    /// Batch interval for sending digest emails.
    pub batch_interval: Duration,
    /// Minimum severity to include in emails.
    pub min_severity: Severity,
}

/// Recipient configuration for a tenant.
#[derive(Debug, Clone)]
pub struct TenantEmailConfig {
    /// Email addresses to receive notifications.
    pub recipients: Vec<String>,
    /// Whether email notifications are enabled.
    pub enabled: bool,
}

/// Email delivery provider for batched notification reports.
///
/// This provider collects notifications in memory and sends periodic
/// digest emails containing all notifications since the last send.
pub struct EmailProvider {
    /// SMTP transport for sending emails.
    smtp: AsyncSmtpTransport<Tokio1Executor>,
    /// From email address.
    from_address: Mailbox,
    /// Batch interval.
    batch_interval: Duration,
    /// Minimum severity to send.
    min_severity: Severity,
    /// Pending notifications per tenant.
    pending: Arc<RwLock<DashMap<TenantId, Vec<NotificationPayload>>>>,
    /// Tenant email configurations.
    tenant_configs: DashMap<TenantId, TenantEmailConfig>,
}

impl EmailProvider {
    /// Create a new email provider.
    pub fn new(config: EmailProviderConfig) -> Result<Self, DeliveryError> {
        let creds = Credentials::new(config.smtp.username, config.smtp.password);

        let smtp = if config.smtp.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp.host)
                .map_err(|e| DeliveryError::Configuration(e.to_string()))?
                .port(config.smtp.port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp.host)
                .port(config.smtp.port)
                .credentials(creds)
                .build()
        };

        let from_address = format!("{} <{}>", config.from_name, config.from_address)
            .parse()
            .map_err(|e: lettre::address::AddressError| {
                DeliveryError::Configuration(e.to_string())
            })?;

        Ok(Self {
            smtp,
            from_address,
            batch_interval: config.batch_interval,
            min_severity: config.min_severity,
            pending: Arc::new(RwLock::new(DashMap::new())),
            tenant_configs: DashMap::new(),
        })
    }

    /// Configure email settings for a tenant.
    pub fn configure_tenant(&self, tenant_id: TenantId, config: TenantEmailConfig) {
        self.tenant_configs.insert(tenant_id, config);
    }

    /// Remove tenant configuration.
    pub fn remove_tenant(&self, tenant_id: &TenantId) {
        self.tenant_configs.remove(tenant_id);
    }

    /// Get the batch interval.
    pub fn batch_interval(&self) -> Duration {
        self.batch_interval
    }

    /// Flush all pending notifications and send email digests.
    pub async fn flush(&self) -> Result<(), DeliveryError> {
        let pending = self.pending.write().await;
        let batches: Vec<_> = pending
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        pending.clear();
        drop(pending);

        for (tenant_id, payloads) in batches {
            if payloads.is_empty() {
                continue;
            }

            if let Err(e) = self.send_digest(&tenant_id, &payloads).await {
                error!(
                    tenant_id = %tenant_id,
                    error = %e,
                    count = payloads.len(),
                    "Failed to send email digest"
                );
            }
        }

        Ok(())
    }

    /// Run the batch flusher as a background task.
    pub async fn run_batch_flusher(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.batch_interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.flush().await {
                error!(error = %e, "Failed to flush email batches");
            }
        }
    }

    /// Send a digest email for a tenant.
    async fn send_digest(
        &self,
        tenant_id: &TenantId,
        payloads: &[NotificationPayload],
    ) -> Result<(), DeliveryError> {
        let config = match self.tenant_configs.get(tenant_id) {
            Some(config) if config.enabled => config.clone(),
            _ => return Ok(()), // No config or disabled
        };

        if config.recipients.is_empty() {
            return Ok(());
        }

        let html = self.render_digest(payloads)?;
        let subject = self.format_subject(payloads);

        for recipient in &config.recipients {
            let recipient_mailbox: Mailbox =
                recipient
                    .parse()
                    .map_err(|e: lettre::address::AddressError| {
                        DeliveryError::Configuration(format!("Invalid recipient: {}", e))
                    })?;

            let email = Message::builder()
                .from(self.from_address.clone())
                .to(recipient_mailbox)
                .subject(&subject)
                .header(ContentType::TEXT_HTML)
                .body(html.clone())
                .map_err(|e| DeliveryError::DeliveryFailed(e.to_string()))?;

            self.smtp
                .send(email)
                .await
                .map_err(|e| DeliveryError::DeliveryFailed(e.to_string()))?;

            info!(
                tenant_id = %tenant_id,
                recipient = %recipient,
                count = payloads.len(),
                "Sent email digest"
            );
        }

        Ok(())
    }

    /// Format the email subject based on notifications.
    fn format_subject(&self, payloads: &[NotificationPayload]) -> String {
        let critical_count = payloads
            .iter()
            .filter(|p| p.severity == Severity::Critical)
            .count();
        let warning_count = payloads
            .iter()
            .filter(|p| p.severity == Severity::Warning)
            .count();

        if critical_count > 0 {
            format!(
                "🚨 Fen Alert: {} critical, {} warning notifications",
                critical_count, warning_count
            )
        } else if warning_count > 0 {
            format!("⚠️ Fen Alert: {} warning notifications", warning_count)
        } else {
            format!("Fen Notification Digest: {} notifications", payloads.len())
        }
    }

    /// Render the HTML digest email.
    fn render_digest(&self, payloads: &[NotificationPayload]) -> Result<String, DeliveryError> {
        let mut rows = String::new();

        for payload in payloads {
            let severity_color = match payload.severity {
                Severity::Critical => "#dc3545",
                Severity::Error => "#e83e8c",
                Severity::Warning => "#ffc107",
                Severity::Info => "#17a2b8",
            };

            let severity_badge = match payload.severity {
                Severity::Critical => "🔴 Critical",
                Severity::Error => "🟠 Error",
                Severity::Warning => "🟡 Warning",
                Severity::Info => "🔵 Info",
            };

            rows.push_str(&format!(
                r#"
                <tr>
                    <td style="padding: 12px; border-bottom: 1px solid #eee;">
                        <span style="background-color: {}; color: white; padding: 2px 8px; border-radius: 4px; font-size: 12px;">{}</span>
                    </td>
                    <td style="padding: 12px; border-bottom: 1px solid #eee;">
                        <strong>{}</strong><br/>
                        <span style="color: #666; font-size: 14px;">{}</span>
                    </td>
                    <td style="padding: 12px; border-bottom: 1px solid #eee; color: #666; font-size: 12px;">
                        {}
                    </td>
                </tr>
                "#,
                severity_color,
                severity_badge,
                html_escape(&payload.title),
                html_escape(&payload.body),
                payload.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        let html = format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Fen Notification Digest</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background-color: #f5f5f5;">
    <div style="max-width: 800px; margin: 0 auto; background-color: white; border-radius: 8px; overflow: hidden; box-shadow: 0 2px 4px rgba(0,0,0,0.1);">
        <div style="background-color: #2563eb; color: white; padding: 20px;">
            <h1 style="margin: 0; font-size: 24px;">Fen Notification Digest</h1>
            <p style="margin: 8px 0 0 0; opacity: 0.9;">{} notifications</p>
        </div>
        <table style="width: 100%; border-collapse: collapse;">
            <thead>
                <tr style="background-color: #f8f9fa;">
                    <th style="padding: 12px; text-align: left; width: 120px;">Severity</th>
                    <th style="padding: 12px; text-align: left;">Details</th>
                    <th style="padding: 12px; text-align: left; width: 180px;">Time</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
        <div style="padding: 20px; text-align: center; color: #666; font-size: 12px; border-top: 1px solid #eee;">
            <p>This is an automated message from Fen Invoice Anomaly Detection System.</p>
        </div>
    </div>
</body>
</html>
"#,
            payloads.len(),
            rows
        );

        Ok(html)
    }
}

/// Basic HTML escaping for user content.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[async_trait]
impl NotificationDeliveryProvider for EmailProvider {
    fn provider_id(&self) -> &str {
        "email"
    }

    fn delivery_mode(&self) -> DeliveryMode {
        DeliveryMode::Batched {
            interval_secs: self.batch_interval.as_secs(),
        }
    }

    fn supports_notification_type(&self, _notification_type: &NotificationType) -> bool {
        true // Email supports all notification types
    }

    fn supports_severity(&self, severity: Severity) -> bool {
        severity >= self.min_severity
    }

    async fn supports_tenant(&self, tenant_id: &TenantId) -> bool {
        self.tenant_configs
            .get(tenant_id)
            .map(|c| c.enabled && !c.recipients.is_empty())
            .unwrap_or(false)
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<(), DeliveryError> {
        // Add to pending batch
        let pending = self.pending.read().await;
        pending
            .entry(payload.tenant_id.clone())
            .or_insert_with(Vec::new)
            .push(payload.clone());
        Ok(())
    }

    async fn deliver_batch(&self, payloads: &[NotificationPayload]) -> Result<(), DeliveryError> {
        // Group by tenant
        let mut by_tenant: std::collections::HashMap<TenantId, Vec<&NotificationPayload>> =
            std::collections::HashMap::new();

        for payload in payloads {
            by_tenant
                .entry(payload.tenant_id.clone())
                .or_default()
                .push(payload);
        }

        for (tenant_id, tenant_payloads) in by_tenant {
            let owned: Vec<NotificationPayload> = tenant_payloads.into_iter().cloned().collect();
            self.send_digest(&tenant_id, &owned).await?;
        }

        Ok(())
    }

    async fn health_check(&self) -> Result<(), DeliveryError> {
        self.smtp
            .test_connection()
            .await
            .map_err(|e| DeliveryError::ProviderUnavailable(e.to_string()))?;
        Ok(())
    }
}

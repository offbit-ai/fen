//! Background event workers for the API process.
//!
//! When running as a monolith (no external Kafka workers), the API itself
//! hosts background tasks that consume events from the local event bus and
//! drive the anomaly detection → notification pipeline.

use std::sync::Arc;

use fen_core::domain::Invoice;
use fen_events::{topics, EventProducer, LocalEventBus, LocalEventConsumer, EventConsumer};
use fen_notify::NotificationHub;
use fen_rules::RuleEngine;
use fen_storage::AnomalyStore;

use crate::conversions::persist_anomalies;

/// Serialized document event payload for the local bus.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DocumentProcessedEvent {
    pub document_type: String,
    pub invoice: Option<Invoice>,
}

/// Publish a DOCUMENT_PROCESSED event after successful ingestion.
pub async fn publish_document_processed(
    event_bus: &LocalEventBus,
    invoice: &Invoice,
) {
    let event = DocumentProcessedEvent {
        document_type: "invoice".to_string(),
        invoice: Some(invoice.clone()),
    };

    let key = format!("{}:{}", invoice.tenant_id, invoice.document_id);
    match serde_json::to_vec(&event) {
        Ok(payload) => {
            if let Err(e) = event_bus.publish(topics::DOCUMENT_PROCESSED, key.as_bytes(), &payload).await {
                tracing::warn!(error = %e, "Failed to publish DOCUMENT_PROCESSED event");
            } else {
                tracing::debug!(
                    invoice_id = %invoice.id,
                    "Published DOCUMENT_PROCESSED event"
                );
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to serialize DOCUMENT_PROCESSED event");
        }
    }
}

/// Spawn the anomaly detection worker as a background task.
///
/// This worker:
/// 1. Subscribes to `DOCUMENT_PROCESSED` events on the local bus
/// 2. Runs rule engine validation on each processed document
/// 3. Persists detected anomalies to the anomaly store
/// 4. Broadcasts anomaly notifications via the notification hub
/// 5. Publishes `ANOMALY_EVENTS` back to the bus for downstream consumers
pub fn spawn_anomaly_worker(
    event_bus: Arc<LocalEventBus>,
    rule_engine: Arc<RuleEngine>,
    anomaly_store: Arc<AnomalyStore>,
    notification_hub: Option<Arc<NotificationHub>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let consumer = LocalEventConsumer::new(event_bus.clone());

        if let Err(e) = consumer.subscribe(&[topics::DOCUMENT_PROCESSED]).await {
            tracing::error!(error = %e, "Anomaly worker failed to subscribe to DOCUMENT_PROCESSED");
            return;
        }

        tracing::info!("Anomaly detection worker started — listening on {}", topics::DOCUMENT_PROCESSED);

        loop {
            match consumer.poll(500).await {
                Ok(events) => {
                    for raw_event in events {
                        let event: DocumentProcessedEvent = match serde_json::from_slice(&raw_event.payload) {
                            Ok(e) => e,
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to deserialize DOCUMENT_PROCESSED event");
                                continue;
                            }
                        };

                        let Some(invoice) = event.invoice else {
                            continue;
                        };

                        // Run anomaly detection
                        match rule_engine.validate_invoice(&invoice).await {
                            Ok(result) => {
                                if result.anomalies.is_empty() {
                                    tracing::debug!(
                                        invoice_id = %invoice.id,
                                        "Async validation passed — no anomalies"
                                    );
                                    continue;
                                }

                                let anomaly_count = result.anomalies.len();
                                let has_critical = result.has_critical();

                                tracing::info!(
                                    invoice_id = %invoice.id,
                                    anomaly_count,
                                    has_critical,
                                    "Async anomaly detection found issues"
                                );

                                // Persist anomalies
                                persist_anomalies(&anomaly_store, &result.anomalies, &invoice).await;

                                // Publish ANOMALY_EVENTS for downstream consumers
                                for anomaly in &result.anomalies {
                                    let key = format!("{}", invoice.tenant_id);
                                    let anomaly_payload = serde_json::json!({
                                        "document_id": anomaly.document_id.to_string(),
                                        "tenant_id": invoice.tenant_id.to_string(),
                                        "invoice_id": invoice.id.to_string(),
                                        "vendor_name": invoice.vendor.name,
                                        "anomaly_type": format!("{}", anomaly.anomaly_type),
                                        "severity": format!("{:?}", anomaly.severity),
                                        "description": anomaly.description,
                                    });

                                    if let Ok(bytes) = serde_json::to_vec(&anomaly_payload) {
                                        let _ = event_bus.publish(
                                            topics::ANOMALY_EVENTS,
                                            key.as_bytes(),
                                            &bytes,
                                        ).await;
                                    }
                                }

                                // Broadcast notifications
                                if let Some(ref hub) = notification_hub {
                                    let severity = if has_critical {
                                        fen_notify::Severity::Critical
                                    } else if result.anomalies.iter().any(|a| a.severity == fen_core::domain::Severity::High) {
                                        fen_notify::Severity::Error
                                    } else if result.anomalies.iter().any(|a| a.severity == fen_core::domain::Severity::Medium) {
                                        fen_notify::Severity::Warning
                                    } else {
                                        fen_notify::Severity::Info
                                    };

                                    let payload = hub.anomaly_notification(
                                        invoice.tenant_id.clone(),
                                        &invoice.document_id.to_string(),
                                        &format!("{} anomalies detected", anomaly_count),
                                        severity,
                                        &format!(
                                            "Invoice {} from vendor '{}' has {} anomalies requiring review",
                                            invoice.invoice_number,
                                            invoice.vendor.name,
                                            anomaly_count,
                                        ),
                                    );

                                    if let Err(e) = hub.broadcast(payload).await {
                                        tracing::warn!(error = %e, "Failed to broadcast anomaly notification");
                                    }
                                }

                                // Publish VALIDATION_RESULTS summary
                                let validation_summary = serde_json::json!({
                                    "document_id": invoice.document_id.to_string(),
                                    "tenant_id": invoice.tenant_id.to_string(),
                                    "is_valid": result.is_valid,
                                    "anomaly_count": anomaly_count,
                                    "has_critical": has_critical,
                                    "validation_time_ms": result.validation_time_ms,
                                });
                                if let Ok(bytes) = serde_json::to_vec(&validation_summary) {
                                    let _ = event_bus.publish(
                                        topics::VALIDATION_RESULTS,
                                        invoice.document_id.to_string().as_bytes(),
                                        &bytes,
                                    ).await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    invoice_id = %invoice.id,
                                    error = %e,
                                    "Async anomaly detection failed"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Anomaly worker poll error");
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }
        }
    })
}

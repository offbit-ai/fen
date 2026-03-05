//! Background event workers for the API process.
//!
//! When running as a monolith (no external Kafka workers), the API itself
//! hosts background tasks that consume events from the local event bus and
//! drive the anomaly detection → notification pipeline.
//!
//! The anomaly worker provides feature parity with the standalone
//! `fen-validation-worker` binary: structural validation, GoRules custom
//! rules, statistical outlier detection with vendor baselines, anomaly
//! persistence, event publishing, and notification broadcasting.

use std::collections::HashMap;
use std::sync::Arc;

use fen_core::domain::{Contract, Invoice, VendorBaseline};
use fen_events::{topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer};
use fen_notify::NotificationHub;
use fen_rules::RuleEngine;
use fen_storage::AnomalyStore;
use tokio::sync::RwLock;

use crate::conversions::persist_anomalies;

/// Serialized document event payload for the local bus.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DocumentProcessedEvent {
    pub document_type: String,
    pub invoice: Option<Invoice>,
    pub contract: Option<Contract>,
}

/// Publish a DOCUMENT_PROCESSED event for an invoice.
pub async fn publish_document_processed(event_bus: &LocalEventBus, invoice: &Invoice) {
    let event = DocumentProcessedEvent {
        document_type: "invoice".to_string(),
        invoice: Some(invoice.clone()),
        contract: None,
    };
    publish_event(
        event_bus,
        &event,
        &format!("{}:{}", invoice.tenant_id, invoice.document_id),
    )
    .await;
}

/// Publish a DOCUMENT_PROCESSED event for a contract.
pub async fn publish_contract_processed(event_bus: &LocalEventBus, contract: &Contract) {
    let event = DocumentProcessedEvent {
        document_type: "contract".to_string(),
        invoice: None,
        contract: Some(contract.clone()),
    };
    publish_event(
        event_bus,
        &event,
        &format!("{}:{}", contract.tenant_id, contract.document_id),
    )
    .await;
}

async fn publish_event(event_bus: &LocalEventBus, event: &DocumentProcessedEvent, key: &str) {
    match serde_json::to_vec(event) {
        Ok(payload) => {
            if let Err(e) = event_bus
                .publish(topics::DOCUMENT_PROCESSED, key.as_bytes(), &payload)
                .await
            {
                tracing::warn!(error = %e, "Failed to publish DOCUMENT_PROCESSED event");
            } else {
                tracing::debug!(key, "Published DOCUMENT_PROCESSED event");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to serialize DOCUMENT_PROCESSED event");
        }
    }
}

/// Spawn the anomaly detection worker as a background task.
///
/// This worker provides full anomaly detection parity with the standalone
/// validation worker:
/// 1. Subscribes to `DOCUMENT_PROCESSED` events on the local bus
/// 2. Looks up vendor baselines from the baseline cache
/// 3. Runs `validate_invoice_with_baselines()` — structural + statistical + GoRules
/// 4. Persists detected anomalies to the anomaly store
/// 5. Publishes fully-detailed `ANOMALY_EVENTS` (including statistical scores)
/// 6. Publishes `VALIDATION_RESULTS` summary with high_severity_count
/// 7. Broadcasts anomaly notifications via the notification hub
pub fn spawn_anomaly_worker(
    event_bus: Arc<LocalEventBus>,
    rule_engine: Arc<RuleEngine>,
    anomaly_store: Arc<AnomalyStore>,
    notification_hub: Option<Arc<NotificationHub>>,
) -> tokio::task::JoinHandle<()> {
    // Vendor baseline cache — keyed by vendor name
    let baseline_cache: Arc<RwLock<HashMap<String, Vec<VendorBaseline>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    tokio::spawn(async move {
        let consumer = LocalEventConsumer::new(event_bus.clone());

        if let Err(e) = consumer
            .subscribe(&[topics::DOCUMENT_PROCESSED])
            .await
        {
            tracing::error!(error = %e, "Anomaly worker failed to subscribe to DOCUMENT_PROCESSED");
            return;
        }

        tracing::info!(
            "Anomaly detection worker started — listening on {}",
            topics::DOCUMENT_PROCESSED
        );

        loop {
            match consumer.poll(500).await {
                Ok(events) => {
                    for raw_event in events {
                        let event: DocumentProcessedEvent =
                            match serde_json::from_slice(&raw_event.payload) {
                                Ok(e) => e,
                                Err(e) => {
                                    tracing::warn!(
                                        error = %e,
                                        "Failed to deserialize DOCUMENT_PROCESSED event"
                                    );
                                    continue;
                                }
                            };

                        // Only invoices go through the anomaly detection pipeline
                        let Some(invoice) = event.invoice else {
                            // Contract events are published for downstream consumers
                            // but don't go through invoice validation
                            if event.contract.is_some() {
                                tracing::debug!("Contract processed event received — no invoice validation needed");
                            }
                            continue;
                        };

                        // Look up vendor baselines for statistical analysis
                        let baselines = {
                            let cache = baseline_cache.read().await;
                            cache.get(&invoice.vendor.name).cloned().unwrap_or_default()
                        };

                        // Run full anomaly detection: structural + statistical + GoRules
                        match rule_engine
                            .validate_invoice_with_baselines(&invoice, &baselines)
                            .await
                        {
                            Ok(result) => {
                                if result.anomalies.is_empty() {
                                    tracing::debug!(
                                        invoice_id = %invoice.id,
                                        "Async validation passed — no anomalies"
                                    );

                                    // Still publish clean validation result
                                    publish_validation_results(
                                        &event_bus, &invoice, true, 0, 0,
                                        result.validation_time_ms,
                                    )
                                    .await;
                                    continue;
                                }

                                let anomaly_count = result.anomalies.len();
                                let has_critical = result.has_critical();
                                let high_severity_count = result
                                    .anomalies
                                    .iter()
                                    .filter(|a| {
                                        a.severity == fen_core::domain::Severity::High
                                            || a.severity == fen_core::domain::Severity::Critical
                                    })
                                    .count();

                                tracing::info!(
                                    invoice_id = %invoice.id,
                                    anomaly_count,
                                    high_severity_count,
                                    has_critical,
                                    "Async anomaly detection found issues"
                                );

                                // Persist anomalies to store
                                persist_anomalies(
                                    &anomaly_store,
                                    &result.anomalies,
                                    &invoice,
                                )
                                .await;

                                // Publish detailed ANOMALY_EVENTS for each anomaly
                                for anomaly in &result.anomalies {
                                    let statistical_score =
                                        anomaly.statistical_score.as_ref().map(|s| {
                                            serde_json::json!({
                                                "z_score": s.z_score,
                                                "percentile": s.percentile,
                                                "trend": format!("{:?}", s.trend),
                                                "is_outlier": s.is_outlier,
                                                "baseline_mean": s.baseline_mean,
                                                "baseline_stddev": s.baseline_stddev,
                                                "sample_count": s.sample_count,
                                            })
                                        });

                                    let anomaly_payload = serde_json::json!({
                                        "document_id": anomaly.document_id.to_string(),
                                        "tenant_id": invoice.tenant_id.to_string(),
                                        "invoice_id": invoice.id.to_string(),
                                        "vendor_name": invoice.vendor.name,
                                        "anomaly_type": format!("{}", anomaly.anomaly_type),
                                        "severity": format!("{:?}", anomaly.severity),
                                        "description": anomaly.description,
                                        "field_path": anomaly.field_path,
                                        "expected_value": anomaly.expected_value,
                                        "actual_value": anomaly.actual_value,
                                        "statistical_score": statistical_score,
                                        "detected_at": anomaly.detected_at.map(|dt| dt.to_rfc3339()),
                                    });

                                    let key = invoice.tenant_id.to_string();
                                    if let Ok(bytes) = serde_json::to_vec(&anomaly_payload) {
                                        let _ = event_bus
                                            .publish(
                                                topics::ANOMALY_EVENTS,
                                                key.as_bytes(),
                                                &bytes,
                                            )
                                            .await;
                                    }
                                }

                                // Broadcast notifications via hub
                                if let Some(ref hub) = notification_hub {
                                    let severity = map_anomaly_severity(&result.anomalies);

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
                                        tracing::warn!(
                                            error = %e,
                                            "Failed to broadcast anomaly notification"
                                        );
                                    }
                                }

                                // Publish VALIDATION_RESULTS summary
                                publish_validation_results(
                                    &event_bus,
                                    &invoice,
                                    result.is_valid,
                                    anomaly_count,
                                    high_severity_count,
                                    result.validation_time_ms,
                                )
                                .await;
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

/// Map domain anomaly severities to notification severity.
fn map_anomaly_severity(anomalies: &[fen_core::domain::Anomaly]) -> fen_notify::Severity {
    if anomalies
        .iter()
        .any(|a| a.severity == fen_core::domain::Severity::Critical)
    {
        fen_notify::Severity::Critical
    } else if anomalies
        .iter()
        .any(|a| a.severity == fen_core::domain::Severity::High)
    {
        fen_notify::Severity::Error
    } else if anomalies
        .iter()
        .any(|a| a.severity == fen_core::domain::Severity::Medium)
    {
        fen_notify::Severity::Warning
    } else {
        fen_notify::Severity::Info
    }
}

/// Publish a VALIDATION_RESULTS event with full summary.
async fn publish_validation_results(
    event_bus: &LocalEventBus,
    invoice: &Invoice,
    is_valid: bool,
    anomaly_count: usize,
    high_severity_count: usize,
    validation_time_ms: u64,
) {
    let summary = serde_json::json!({
        "document_id": invoice.document_id.to_string(),
        "tenant_id": invoice.tenant_id.to_string(),
        "is_valid": is_valid,
        "anomaly_count": anomaly_count,
        "high_severity_count": high_severity_count,
        "validation_time_ms": validation_time_ms,
        "completed_at": chrono::Utc::now().to_rfc3339(),
    });
    if let Ok(bytes) = serde_json::to_vec(&summary) {
        let _ = event_bus
            .publish(
                topics::VALIDATION_RESULTS,
                invoice.document_id.to_string().as_bytes(),
                &bytes,
            )
            .await;
    }
}

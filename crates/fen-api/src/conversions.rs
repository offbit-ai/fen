//! Conversion utilities between domain types and storage record types.

use fen_core::domain::{Anomaly, Invoice};
use fen_storage::{AnomalyRecord, AnomalyStore};

/// Convert a domain `Anomaly` (from validation) into an `AnomalyRecord` (for persistence),
/// using context from the associated invoice.
pub fn anomaly_to_record(anomaly: &Anomaly, invoice: &Invoice) -> AnomalyRecord {
    let mut record = AnomalyRecord::new(
        anomaly.document_id.clone(),
        &invoice.vendor.name,
        anomaly.anomaly_type.clone(),
        anomaly.severity,
        &anomaly.description,
    )
    .with_invoice_id(invoice.id)
    .with_confidence(anomaly.confidence)
    .with_invoice_date(invoice.invoice_date);

    if let Some(ref field) = anomaly.field_path {
        record = record.with_field(field);
    }

    if let (Some(ref expected), Some(ref actual)) = (&anomaly.expected_value, &anomaly.actual_value)
    {
        record = record.with_values(expected, actual);
    }

    if let Some(ref score) = anomaly.statistical_score {
        record = record.with_statistical_score(score);
    }

    record
}

/// Persist all anomalies from a validation result to the anomaly store.
/// Non-fatal — logs warnings on individual failures.
pub async fn persist_anomalies(
    anomaly_store: &AnomalyStore,
    anomalies: &[Anomaly],
    invoice: &Invoice,
) {
    for anomaly in anomalies {
        let record = anomaly_to_record(anomaly, invoice);
        if let Err(e) = anomaly_store.store_anomaly(&record).await {
            tracing::warn!(
                anomaly_type = %anomaly.anomaly_type,
                invoice_id = %invoice.id,
                error = %e,
                "Failed to persist anomaly"
            );
        }
    }
}

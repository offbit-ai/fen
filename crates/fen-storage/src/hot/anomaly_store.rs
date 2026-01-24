//! Anomaly persistence store for historical analysis.
//!
//! This module provides redb-based storage for anomaly records,
//! enabling historical accumulation for statistical analysis.

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};

use fen_core::domain::{AnomalyId, AnomalyType, DocumentId, InvoiceId, Severity, StatisticalScore};

use crate::error::StorageError;

/// Table for anomaly history records
const ANOMALY_HISTORY_TABLE: TableDefinition<&[u8], &[u8]> =
    TableDefinition::new("anomaly_history");

/// Secondary index: vendor_name -> list of anomaly IDs (for vendor queries)
const VENDOR_ANOMALY_INDEX: TableDefinition<&str, &[u8]> =
    TableDefinition::new("vendor_anomaly_index");

/// Persisted anomaly record with full context for historical analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyRecord {
    /// Unique anomaly ID
    pub id: AnomalyId,
    /// Associated document ID
    pub document_id: DocumentId,
    /// Associated invoice ID (if from invoice validation)
    pub invoice_id: Option<InvoiceId>,
    /// Vendor name for grouping
    pub vendor_name: String,
    /// Type of anomaly detected
    pub anomaly_type: AnomalyType,
    /// Severity level
    pub severity: Severity,
    /// Human-readable description
    pub description: String,
    /// Field path (e.g., "total_amount", "line_items[0].price")
    pub field_path: Option<String>,
    /// Expected value (for comparison anomalies)
    pub expected_value: Option<String>,
    /// Actual value found
    pub actual_value: Option<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Z-score (for statistical outliers)
    pub z_score: Option<f64>,
    /// Percentile ranking (for statistical outliers)
    pub percentile: Option<f64>,
    /// Baseline mean at time of detection
    pub baseline_mean: Option<f64>,
    /// Baseline standard deviation at time of detection
    pub baseline_stddev: Option<f64>,
    /// When the anomaly was detected
    pub detected_at: DateTime<Utc>,
    /// Invoice date (for time-based analysis)
    pub invoice_date: Option<NaiveDate>,
}

impl AnomalyRecord {
    /// Create a new anomaly record
    pub fn new(
        document_id: DocumentId,
        vendor_name: impl Into<String>,
        anomaly_type: AnomalyType,
        severity: Severity,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: AnomalyId::new(),
            document_id,
            invoice_id: None,
            vendor_name: vendor_name.into(),
            anomaly_type,
            severity,
            description: description.into(),
            field_path: None,
            expected_value: None,
            actual_value: None,
            confidence: 1.0,
            z_score: None,
            percentile: None,
            baseline_mean: None,
            baseline_stddev: None,
            detected_at: Utc::now(),
            invoice_date: None,
        }
    }

    /// Set invoice ID
    pub fn with_invoice_id(mut self, id: InvoiceId) -> Self {
        self.invoice_id = Some(id);
        self
    }

    /// Set field path
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field_path = Some(field.into());
        self
    }

    /// Set expected and actual values
    pub fn with_values(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.expected_value = Some(expected.into());
        self.actual_value = Some(actual.into());
        self
    }

    /// Set statistical score from analysis
    pub fn with_statistical_score(mut self, score: &StatisticalScore) -> Self {
        self.z_score = Some(score.z_score);
        self.percentile = Some(score.percentile);
        self.baseline_mean = Some(score.baseline_mean);
        self.baseline_stddev = Some(score.baseline_stddev);
        self
    }

    /// Set invoice date
    pub fn with_invoice_date(mut self, date: NaiveDate) -> Self {
        self.invoice_date = Some(date);
        self
    }

    /// Set confidence
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }
}

/// Index entry for vendor -> anomaly mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VendorAnomalyIndex {
    anomaly_ids: Vec<[u8; 16]>,
}

/// Store for anomaly persistence
pub struct AnomalyStore {
    db: Arc<Database>,
}

impl AnomalyStore {
    /// Create a new anomaly store with the given database
    pub fn new(db: Arc<Database>) -> Result<Self, StorageError> {
        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(ANOMALY_HISTORY_TABLE)?;
            let _ = write_txn.open_table(VENDOR_ANOMALY_INDEX)?;
        }
        write_txn.commit()?;

        tracing::info!("Initialized anomaly store");
        Ok(Self { db })
    }

    /// Store an anomaly record
    pub async fn store_anomaly(&self, record: &AnomalyRecord) -> Result<(), StorageError> {
        let key = record.id.as_bytes();
        let value =
            serde_json::to_vec(record).map_err(|e| StorageError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            // Store the anomaly record
            let mut table = write_txn.open_table(ANOMALY_HISTORY_TABLE)?;
            table.insert(key.as_slice(), value.as_slice())?;

            // Update vendor index
            let mut index_table = write_txn.open_table(VENDOR_ANOMALY_INDEX)?;
            let vendor_key = record.vendor_name.as_str();

            let mut index = match index_table.get(vendor_key)? {
                Some(data) => serde_json::from_slice::<VendorAnomalyIndex>(data.value()).unwrap_or(
                    VendorAnomalyIndex {
                        anomaly_ids: vec![],
                    },
                ),
                None => VendorAnomalyIndex {
                    anomaly_ids: vec![],
                },
            };

            index.anomaly_ids.push(*key);

            let index_value = serde_json::to_vec(&index)
                .map_err(|e| StorageError::Serialization(e.to_string()))?;
            index_table.insert(vendor_key, index_value.as_slice())?;
        }
        write_txn.commit()?;

        tracing::debug!(
            anomaly_id = %record.id,
            vendor = %record.vendor_name,
            anomaly_type = ?record.anomaly_type,
            "Stored anomaly record"
        );

        Ok(())
    }

    /// Get an anomaly by ID
    pub async fn get_anomaly(&self, id: &AnomalyId) -> Result<Option<AnomalyRecord>, StorageError> {
        let key = id.as_bytes();

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ANOMALY_HISTORY_TABLE)?;

        match table.get(key.as_slice())? {
            Some(value) => {
                let record = serde_json::from_slice(value.value())
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    /// Get anomalies for a vendor within a time window
    pub async fn get_anomalies_for_vendor(
        &self,
        vendor: &str,
        days: u32,
    ) -> Result<Vec<AnomalyRecord>, StorageError> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        let read_txn = self.db.begin_read()?;
        let index_table = read_txn.open_table(VENDOR_ANOMALY_INDEX)?;
        let anomaly_table = read_txn.open_table(ANOMALY_HISTORY_TABLE)?;

        // Get anomaly IDs for this vendor
        let anomaly_ids = match index_table.get(vendor)? {
            Some(data) => {
                let index: VendorAnomalyIndex = serde_json::from_slice(data.value())
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;
                index.anomaly_ids
            }
            None => return Ok(vec![]),
        };

        // Fetch and filter anomalies
        let mut results = Vec::new();
        for id_bytes in anomaly_ids {
            if let Some(value) = anomaly_table.get(id_bytes.as_slice())? {
                let record: AnomalyRecord = serde_json::from_slice(value.value())
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;

                if record.detected_at >= cutoff {
                    results.push(record);
                }
            }
        }

        // Sort by detected_at descending
        results.sort_by(|a, b| b.detected_at.cmp(&a.detected_at));

        Ok(results)
    }

    /// Get anomalies by type within a time window
    pub async fn get_anomalies_by_type(
        &self,
        anomaly_type: AnomalyType,
        days: u32,
    ) -> Result<Vec<AnomalyRecord>, StorageError> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ANOMALY_HISTORY_TABLE)?;

        let results: Result<Vec<AnomalyRecord>, StorageError> = table
            .iter()?
            .filter_map(|result| {
                let (_, value) = result.ok()?;
                let record: AnomalyRecord = serde_json::from_slice(value.value()).ok()?;

                if record.anomaly_type == anomaly_type && record.detected_at >= cutoff {
                    Some(Ok(record))
                } else {
                    None
                }
            })
            .collect();

        let mut results = results?;
        results.sort_by(|a, b| b.detected_at.cmp(&a.detected_at));

        Ok(results)
    }

    /// Count anomalies for a vendor within a time window
    pub async fn count_anomalies(&self, vendor: &str, days: u32) -> Result<usize, StorageError> {
        let anomalies = self.get_anomalies_for_vendor(vendor, days).await?;
        Ok(anomalies.len())
    }

    /// Get all anomalies within a time window
    pub async fn get_recent_anomalies(
        &self,
        days: u32,
    ) -> Result<Vec<AnomalyRecord>, StorageError> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ANOMALY_HISTORY_TABLE)?;

        let results: Result<Vec<AnomalyRecord>, StorageError> = table
            .iter()?
            .filter_map(|result| {
                let (_, value) = result.ok()?;
                let record: AnomalyRecord = serde_json::from_slice(value.value()).ok()?;

                if record.detected_at >= cutoff {
                    Some(Ok(record))
                } else {
                    None
                }
            })
            .collect();

        let mut results = results?;
        results.sort_by(|a, b| b.detected_at.cmp(&a.detected_at));

        Ok(results)
    }

    /// Get statistical outlier anomalies for a vendor
    pub async fn get_statistical_outliers(
        &self,
        vendor: &str,
        days: u32,
    ) -> Result<Vec<AnomalyRecord>, StorageError> {
        let anomalies = self.get_anomalies_for_vendor(vendor, days).await?;
        Ok(anomalies
            .into_iter()
            .filter(|a| a.anomaly_type == AnomalyType::StatisticalOutlier)
            .collect())
    }

    /// Delete anomalies older than the specified number of days
    pub async fn cleanup_old_anomalies(&self, retention_days: u32) -> Result<usize, StorageError> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let mut deleted_count = 0;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(ANOMALY_HISTORY_TABLE)?;

            // Collect IDs to delete
            let to_delete: Vec<[u8; 16]> = table
                .iter()?
                .filter_map(|result| {
                    let (key, value) = result.ok()?;
                    let record: AnomalyRecord = serde_json::from_slice(value.value()).ok()?;
                    if record.detected_at < cutoff {
                        let mut id = [0u8; 16];
                        id.copy_from_slice(key.value());
                        Some(id)
                    } else {
                        None
                    }
                })
                .collect();

            // Delete old records
            for id in to_delete {
                if table.remove(id.as_slice())?.is_some() {
                    deleted_count += 1;
                }
            }
        }
        write_txn.commit()?;

        tracing::info!(
            deleted_count,
            retention_days,
            "Cleaned up old anomaly records"
        );

        Ok(deleted_count)
    }

    /// Get total anomaly count
    pub async fn total_count(&self) -> Result<usize, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(ANOMALY_HISTORY_TABLE)?;
        Ok(table.len()? as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redb::backends::InMemoryBackend;

    fn create_test_db() -> Arc<Database> {
        Arc::new(
            Database::builder()
                .create_with_backend(InMemoryBackend::new())
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn test_store_and_retrieve_anomaly() {
        let db = create_test_db();
        let store = AnomalyStore::new(db).unwrap();

        let record = AnomalyRecord::new(
            DocumentId::new(),
            "Acme Corp",
            AnomalyType::StatisticalOutlier,
            Severity::Medium,
            "Invoice amount is 3.5 standard deviations from mean",
        )
        .with_field("total_amount")
        .with_values("1000.00", "5000.00");

        store.store_anomaly(&record).await.unwrap();

        let loaded = store.get_anomaly(&record.id).await.unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.vendor_name, "Acme Corp");
        assert_eq!(loaded.anomaly_type, AnomalyType::StatisticalOutlier);
    }

    #[tokio::test]
    async fn test_get_anomalies_for_vendor() {
        let db = create_test_db();
        let store = AnomalyStore::new(db).unwrap();

        // Store multiple anomalies for same vendor
        for i in 0..5 {
            let record = AnomalyRecord::new(
                DocumentId::new(),
                "Acme Corp",
                AnomalyType::MathMismatch,
                Severity::Low,
                format!("Test anomaly {}", i),
            );
            store.store_anomaly(&record).await.unwrap();
        }

        // Store one for different vendor
        let other = AnomalyRecord::new(
            DocumentId::new(),
            "Other Corp",
            AnomalyType::MissingField,
            Severity::Medium,
            "Other anomaly",
        );
        store.store_anomaly(&other).await.unwrap();

        let acme_anomalies = store
            .get_anomalies_for_vendor("Acme Corp", 30)
            .await
            .unwrap();
        assert_eq!(acme_anomalies.len(), 5);

        let other_anomalies = store
            .get_anomalies_for_vendor("Other Corp", 30)
            .await
            .unwrap();
        assert_eq!(other_anomalies.len(), 1);
    }

    #[tokio::test]
    async fn test_count_anomalies() {
        let db = create_test_db();
        let store = AnomalyStore::new(db).unwrap();

        for i in 0..3 {
            let record = AnomalyRecord::new(
                DocumentId::new(),
                "Test Vendor",
                AnomalyType::OutOfRange,
                Severity::High,
                format!("Anomaly {}", i),
            );
            store.store_anomaly(&record).await.unwrap();
        }

        let count = store.count_anomalies("Test Vendor", 30).await.unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_total_count() {
        let db = create_test_db();
        let store = AnomalyStore::new(db).unwrap();

        assert_eq!(store.total_count().await.unwrap(), 0);

        let record = AnomalyRecord::new(
            DocumentId::new(),
            "Vendor",
            AnomalyType::ValidationFailure,
            Severity::Low,
            "Test",
        );
        store.store_anomaly(&record).await.unwrap();

        assert_eq!(store.total_count().await.unwrap(), 1);
    }
}

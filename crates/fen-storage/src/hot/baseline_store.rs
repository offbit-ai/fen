//! Baseline storage for statistical anomaly detection.
//!
//! This module provides storage and caching for vendor baselines,
//! enabling efficient z-score computation and trend analysis.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use dashmap::DashMap;
use redb::{Database, ReadableTable, TableDefinition};

use fen_core::domain::{BaselineId, BaselinePeriod, BaselineStats, VendorBaseline};

use crate::error::StorageError;

/// Table for vendor baselines
const VENDOR_BASELINES_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("vendor_baselines");

/// Cached baseline with expiration tracking
struct CachedBaseline {
    baseline: VendorBaseline,
    cached_at: Instant,
}

/// Store for vendor baselines with in-memory caching
pub struct BaselineStore {
    db: Arc<Database>,
    cache: DashMap<String, CachedBaseline>,
    cache_ttl: Duration,
}

impl BaselineStore {
    /// Create a new baseline store with the given database
    pub fn new(db: Arc<Database>) -> Result<Self, StorageError> {
        Self::with_cache_ttl(db, Duration::from_secs(3600)) // 1 hour default
    }

    /// Create a new baseline store with custom cache TTL
    pub fn with_cache_ttl(db: Arc<Database>, cache_ttl: Duration) -> Result<Self, StorageError> {
        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(VENDOR_BASELINES_TABLE)?;
        }
        write_txn.commit()?;

        tracing::info!(cache_ttl_secs = cache_ttl.as_secs(), "Initialized baseline store");

        Ok(Self {
            db,
            cache: DashMap::new(),
            cache_ttl,
        })
    }

    /// Get a baseline for a vendor/metric/period combination
    pub async fn get_baseline(
        &self,
        vendor: &str,
        metric: &str,
        period: BaselinePeriod,
    ) -> Result<Option<VendorBaseline>, StorageError> {
        let cache_key = format_cache_key(vendor, metric, &period);

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key) {
            if cached.cached_at.elapsed() < self.cache_ttl && !cached.baseline.is_expired() {
                tracing::trace!(cache_key, "Baseline cache hit");
                return Ok(Some(cached.baseline.clone()));
            }
        }

        // Load from database
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VENDOR_BASELINES_TABLE)?;

        match table.get(cache_key.as_str())? {
            Some(value) => {
                let baseline: VendorBaseline = serde_json::from_slice(value.value())
                    .map_err(|e| StorageError::Deserialization(e.to_string()))?;

                // Update cache if not expired
                if !baseline.is_expired() {
                    self.cache.insert(
                        cache_key,
                        CachedBaseline {
                            baseline: baseline.clone(),
                            cached_at: Instant::now(),
                        },
                    );
                }

                Ok(Some(baseline))
            }
            None => Ok(None),
        }
    }

    /// Store a computed baseline
    pub async fn store_baseline(&self, baseline: &VendorBaseline) -> Result<(), StorageError> {
        let cache_key = baseline.cache_key();
        let value = serde_json::to_vec(baseline)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VENDOR_BASELINES_TABLE)?;
            table.insert(cache_key.as_str(), value.as_slice())?;
        }
        write_txn.commit()?;

        // Update cache
        self.cache.insert(
            cache_key.clone(),
            CachedBaseline {
                baseline: baseline.clone(),
                cached_at: Instant::now(),
            },
        );

        tracing::debug!(
            vendor = %baseline.vendor_name,
            metric = %baseline.metric_name,
            period = %baseline.period,
            sample_count = baseline.stats.count,
            "Stored baseline"
        );

        Ok(())
    }

    /// Compute and store a baseline from values
    pub async fn compute_and_store_baseline(
        &self,
        vendor: &str,
        metric: &str,
        period: BaselinePeriod,
        values: &[f64],
        recent_values: Vec<f64>,
        expiry_hours: u32,
    ) -> Result<VendorBaseline, StorageError> {
        let stats = BaselineStats::from_values(values);

        let baseline = VendorBaseline {
            id: BaselineId::new(),
            vendor_name: vendor.to_string(),
            metric_name: metric.to_string(),
            period,
            stats,
            recent_values,
            computed_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(expiry_hours as i64),
        };

        self.store_baseline(&baseline).await?;

        Ok(baseline)
    }

    /// Get all baselines for a vendor
    pub async fn get_baselines_for_vendor(
        &self,
        vendor: &str,
    ) -> Result<Vec<VendorBaseline>, StorageError> {
        let prefix = format!("{}:", vendor);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VENDOR_BASELINES_TABLE)?;

        let baselines: Result<Vec<VendorBaseline>, StorageError> = table
            .iter()?
            .filter_map(|result| {
                let (key, value) = result.ok()?;
                let key_str = key.value();
                if key_str.starts_with(&prefix) {
                    let baseline: VendorBaseline = serde_json::from_slice(value.value()).ok()?;
                    Some(Ok(baseline))
                } else {
                    None
                }
            })
            .collect();

        baselines
    }

    /// Invalidate cache for a vendor
    pub fn invalidate_cache(&self, vendor: &str) {
        self.cache.retain(|key, _| !key.starts_with(vendor));
        tracing::debug!(vendor, "Invalidated baseline cache");
    }

    /// Invalidate entire cache
    pub fn invalidate_all_cache(&self) {
        self.cache.clear();
        tracing::debug!("Invalidated all baseline cache");
    }

    /// Delete a baseline
    pub async fn delete_baseline(
        &self,
        vendor: &str,
        metric: &str,
        period: BaselinePeriod,
    ) -> Result<bool, StorageError> {
        let cache_key = format_cache_key(vendor, metric, &period);

        let write_txn = self.db.begin_write()?;
        let deleted = {
            let mut table = write_txn.open_table(VENDOR_BASELINES_TABLE)?;
            let result = table.remove(cache_key.as_str())?.is_some();
            result
        };
        write_txn.commit()?;

        // Remove from cache
        self.cache.remove(&cache_key);

        Ok(deleted)
    }

    /// Delete all baselines for a vendor
    pub async fn delete_baselines_for_vendor(&self, vendor: &str) -> Result<usize, StorageError> {
        let prefix = format!("{}:", vendor);
        let mut deleted_count = 0;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VENDOR_BASELINES_TABLE)?;

            // Collect keys to delete
            let to_delete: Vec<String> = table
                .iter()?
                .filter_map(|result| {
                    let (key, _) = result.ok()?;
                    let key_str = key.value().to_string();
                    if key_str.starts_with(&prefix) {
                        Some(key_str)
                    } else {
                        None
                    }
                })
                .collect();

            for key in to_delete {
                if table.remove(key.as_str())?.is_some() {
                    deleted_count += 1;
                }
            }
        }
        write_txn.commit()?;

        // Invalidate cache
        self.invalidate_cache(vendor);

        Ok(deleted_count)
    }

    /// Clean up expired baselines
    pub async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        let now = Utc::now();
        let mut deleted_count = 0;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VENDOR_BASELINES_TABLE)?;

            // Collect expired keys
            let to_delete: Vec<String> = table
                .iter()?
                .filter_map(|result| {
                    let (key, value) = result.ok()?;
                    let baseline: VendorBaseline = serde_json::from_slice(value.value()).ok()?;
                    if baseline.expires_at < now {
                        Some(key.value().to_string())
                    } else {
                        None
                    }
                })
                .collect();

            for key in to_delete {
                if table.remove(key.as_str())?.is_some() {
                    deleted_count += 1;
                }
            }
        }
        write_txn.commit()?;

        // Clear cache to force refresh
        self.invalidate_all_cache();

        tracing::info!(deleted_count, "Cleaned up expired baselines");

        Ok(deleted_count)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            ttl_secs: self.cache_ttl.as_secs(),
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub ttl_secs: u64,
}

/// Format a cache key for vendor/metric/period
fn format_cache_key(vendor: &str, metric: &str, period: &BaselinePeriod) -> String {
    format!("{}:{}:{}", vendor, metric, period.cache_key())
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
    async fn test_store_and_retrieve_baseline() {
        let db = create_test_db();
        let store = BaselineStore::new(db).unwrap();

        let values = vec![100.0, 150.0, 200.0, 180.0, 220.0];
        let recent = vec![180.0, 200.0, 190.0];

        let baseline = store
            .compute_and_store_baseline(
                "Acme Corp",
                "total_amount",
                BaselinePeriod::Rolling90Days,
                &values,
                recent,
                24,
            )
            .await
            .unwrap();

        assert_eq!(baseline.stats.count, 5);
        assert!((baseline.stats.mean - 170.0).abs() < 0.1);

        // Retrieve
        let loaded = store
            .get_baseline("Acme Corp", "total_amount", BaselinePeriod::Rolling90Days)
            .await
            .unwrap();

        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.vendor_name, "Acme Corp");
        assert_eq!(loaded.metric_name, "total_amount");
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let db = create_test_db();
        let store = BaselineStore::new(db).unwrap();

        let values = vec![1000.0, 1100.0, 900.0];
        store
            .compute_and_store_baseline(
                "Test Corp",
                "amount",
                BaselinePeriod::Rolling30Days,
                &values,
                vec![],
                24,
            )
            .await
            .unwrap();

        // First call loads from DB
        let _b1 = store
            .get_baseline("Test Corp", "amount", BaselinePeriod::Rolling30Days)
            .await
            .unwrap();

        // Second call should hit cache
        let _b2 = store
            .get_baseline("Test Corp", "amount", BaselinePeriod::Rolling30Days)
            .await
            .unwrap();

        let stats = store.cache_stats();
        assert_eq!(stats.entries, 1);
    }

    #[tokio::test]
    async fn test_invalidate_cache() {
        let db = create_test_db();
        let store = BaselineStore::new(db).unwrap();

        store
            .compute_and_store_baseline(
                "Vendor A",
                "metric",
                BaselinePeriod::Rolling30Days,
                &[100.0],
                vec![],
                24,
            )
            .await
            .unwrap();

        store
            .compute_and_store_baseline(
                "Vendor B",
                "metric",
                BaselinePeriod::Rolling30Days,
                &[200.0],
                vec![],
                24,
            )
            .await
            .unwrap();

        assert_eq!(store.cache_stats().entries, 2);

        store.invalidate_cache("Vendor A");
        assert_eq!(store.cache_stats().entries, 1);

        store.invalidate_all_cache();
        assert_eq!(store.cache_stats().entries, 0);
    }

    #[tokio::test]
    async fn test_delete_baseline() {
        let db = create_test_db();
        let store = BaselineStore::new(db).unwrap();

        store
            .compute_and_store_baseline(
                "Vendor",
                "metric",
                BaselinePeriod::Rolling90Days,
                &[100.0],
                vec![],
                24,
            )
            .await
            .unwrap();

        let deleted = store
            .delete_baseline("Vendor", "metric", BaselinePeriod::Rolling90Days)
            .await
            .unwrap();
        assert!(deleted);

        let loaded = store
            .get_baseline("Vendor", "metric", BaselinePeriod::Rolling90Days)
            .await
            .unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_get_baselines_for_vendor() {
        let db = create_test_db();
        let store = BaselineStore::new(db).unwrap();

        // Create multiple baselines for same vendor
        store
            .compute_and_store_baseline(
                "Multi Corp",
                "total_amount",
                BaselinePeriod::Rolling30Days,
                &[100.0],
                vec![],
                24,
            )
            .await
            .unwrap();

        store
            .compute_and_store_baseline(
                "Multi Corp",
                "total_amount",
                BaselinePeriod::Rolling90Days,
                &[100.0, 150.0],
                vec![],
                24,
            )
            .await
            .unwrap();

        store
            .compute_and_store_baseline(
                "Multi Corp",
                "line_item_count",
                BaselinePeriod::Rolling30Days,
                &[5.0, 8.0, 6.0],
                vec![],
                24,
            )
            .await
            .unwrap();

        let baselines = store.get_baselines_for_vendor("Multi Corp").await.unwrap();
        assert_eq!(baselines.len(), 3);
    }
}

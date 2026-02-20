use std::path::Path;
use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use tokio::sync::RwLock;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::config::{GraphBackend, HotStorageBackend, WarmStorageBackend};
use crate::error::StorageError;
use crate::hot::RedbStorage;
use crate::location::{DocumentLocationIndex, TierDistribution};
use crate::query::QueryCache;
use crate::traits::{InvoiceFilter, StorageTier};
use crate::warm::LanceStorage;

/// Configuration for tiered storage
///
/// # Feature Flags
///
/// The storage backends are controlled by feature flags:
/// - `embedded` (default): Local filesystem storage for development
/// - `remote-storage`: Cloud/S3 storage backends for production
///
/// # Example
///
/// ```rust,ignore
/// // Development configuration (default)
/// let config = TieredStorageConfig::default();
///
/// // Production configuration with S3
/// let config = TieredStorageConfig {
///     warm_backend: WarmStorageBackend::s3("s3://bucket/warm", s3_config),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct TieredStorageConfig {
    /// Hot tier storage backend (redb - always embedded)
    pub hot_backend: HotStorageBackend,
    /// Warm tier storage backend (LanceDB - embedded or remote)
    pub warm_backend: WarmStorageBackend,
    /// Knowledge graph backend (RyuGraph)
    pub graph_backend: GraphBackend,
    /// Age threshold (days) for hot tier data
    pub hot_tier_days: u32,
    /// Age threshold (days) for warm tier data
    pub warm_tier_days: u32,
    /// Enable automatic tier migration
    pub auto_migration: bool,
    /// Enable query caching
    pub cache_enabled: bool,
    /// Maximum cache entries
    pub cache_max_entries: usize,
}

impl Default for TieredStorageConfig {
    fn default() -> Self {
        Self {
            hot_backend: HotStorageBackend::default(),
            warm_backend: WarmStorageBackend::default(),
            graph_backend: GraphBackend::default(),
            hot_tier_days: 30,
            warm_tier_days: 365,
            auto_migration: true,
            cache_enabled: true,
            cache_max_entries: 10000,
        }
    }
}

impl TieredStorageConfig {
    /// Create a development configuration with local filesystem storage
    pub fn development() -> Self {
        Self::default()
    }

    /// Create a development configuration with custom paths
    pub fn development_with_paths(
        hot_path: impl Into<String>,
        warm_path: impl Into<String>,
    ) -> Self {
        Self {
            hot_backend: HotStorageBackend::embedded(hot_path),
            warm_backend: WarmStorageBackend::embedded(warm_path),
            ..Default::default()
        }
    }

    /// Create a production configuration with S3 backend for warm tier
    #[cfg(feature = "remote-storage")]
    pub fn production_s3(
        hot_path: impl Into<String>,
        s3_uri: impl Into<String>,
        s3_config: crate::config::S3Config,
    ) -> Self {
        Self {
            hot_backend: HotStorageBackend::embedded(hot_path),
            warm_backend: WarmStorageBackend::s3(s3_uri, s3_config),
            ..Default::default()
        }
    }

    /// Create a production configuration with LanceDB Cloud backend
    #[cfg(feature = "remote-storage")]
    pub fn production_lance_cloud(
        hot_path: impl Into<String>,
        db_uri: impl Into<String>,
        api_key: impl Into<String>,
        region: Option<String>,
    ) -> Self {
        Self {
            hot_backend: HotStorageBackend::embedded(hot_path),
            warm_backend: WarmStorageBackend::lance_cloud(db_uri, api_key, region),
            ..Default::default()
        }
    }

    /// Check if this is a development (embedded) configuration
    pub fn is_development(&self) -> bool {
        self.warm_backend.is_embedded()
    }

    /// Check if this is a production (remote) configuration
    #[cfg(feature = "remote-storage")]
    pub fn is_production(&self) -> bool {
        self.warm_backend.is_remote()
    }

    /// Get the hot tier backend URI for logging
    pub fn hot_backend_uri(&self) -> &str {
        match &self.hot_backend {
            HotStorageBackend::Embedded { path } => path,
            HotStorageBackend::InMemory => "<in-memory>",
        }
    }
}

/// Tiered storage manager combining hot (redb) and warm (LanceDB) tiers
pub struct TieredStorage {
    config: TieredStorageConfig,
    hot: Arc<RedbStorage>,
    warm: Arc<RwLock<LanceStorage>>,
    cache: Arc<QueryCache>,
    /// Location index tracking which tier each document resides in.
    /// This is the single source of truth for document counts.
    location_index: Arc<DocumentLocationIndex>,
}

impl TieredStorage {
    /// Create a new tiered storage from configuration
    ///
    /// # Feature Flags
    ///
    /// Storage backends are determined by feature flags:
    /// - `embedded` (default): Local filesystem storage
    /// - `remote-storage`: S3/cloud storage backends
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Development (default embedded storage)
    /// let config = TieredStorageConfig::default();
    /// let storage = TieredStorage::new(config).await?;
    ///
    /// // Production with S3 (requires remote-storage feature)
    /// let config = TieredStorageConfig::production_s3(
    ///     "/data/hot.redb",
    ///     "s3://bucket/warm",
    ///     S3Config::new("us-east-1"),
    /// );
    /// let storage = TieredStorage::new(config).await?;
    /// ```
    pub async fn new(config: TieredStorageConfig) -> Result<Self, StorageError> {
        // Initialize hot tier (always redb)
        let hot = match &config.hot_backend {
            HotStorageBackend::Embedded { path } => {
                if let Some(parent) = Path::new(path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
                Arc::new(RedbStorage::new(path)?)
            }
            HotStorageBackend::InMemory => Arc::new(RedbStorage::in_memory()?),
        };

        // Initialize warm tier (embedded or remote based on config)
        let warm = Arc::new(RwLock::new(
            LanceStorage::from_backend(&config.warm_backend).await?,
        ));

        let cache = Arc::new(QueryCache::new(config.cache_max_entries));
        let location_index = Arc::new(DocumentLocationIndex::new());

        let backend_mode = if config.is_development() {
            "development (embedded)"
        } else {
            "production (remote)"
        };

        tracing::info!(
            hot_uri = %config.hot_backend_uri(),
            warm_uri = %config.warm_backend.connection_uri(),
            mode = %backend_mode,
            "Initialized tiered storage"
        );

        Ok(Self {
            config,
            hot,
            warm,
            cache,
            location_index,
        })
    }

    /// Create an in-memory tiered storage (for testing)
    pub async fn in_memory() -> Result<Self, StorageError> {
        let temp_dir = std::env::temp_dir();
        let warm_path = temp_dir.join(format!("fen_test_{}", uuid::Uuid::new_v4()));

        let config = TieredStorageConfig {
            hot_backend: HotStorageBackend::in_memory(),
            warm_backend: WarmStorageBackend::embedded(warm_path.to_string_lossy().to_string()),
            cache_max_entries: 1000,
            ..Default::default()
        };

        let hot = Arc::new(RedbStorage::in_memory()?);
        let warm = Arc::new(RwLock::new(
            LanceStorage::from_backend(&config.warm_backend).await?,
        ));
        let cache = Arc::new(QueryCache::new(config.cache_max_entries));
        let location_index = Arc::new(DocumentLocationIndex::new());

        Ok(Self {
            config,
            hot,
            warm,
            cache,
            location_index,
        })
    }

    /// Determine the appropriate tier for a date
    fn determine_tier(&self, date: NaiveDate) -> StorageTier {
        let today = Utc::now().date_naive();
        let age_days = (today - date).num_days() as u32;

        if age_days <= self.config.hot_tier_days {
            StorageTier::Hot
        } else if age_days <= self.config.warm_tier_days {
            StorageTier::Warm
        } else {
            StorageTier::Cold
        }
    }

    /// Store an invoice with optional embedding
    pub async fn store_invoice(
        &self,
        invoice: &Invoice,
        embedding: Option<&[f32]>,
    ) -> Result<(), StorageError> {
        let tier = self.determine_tier(invoice.invoice_date);

        match tier {
            StorageTier::Hot => {
                // Store in hot tier
                self.hot.store_invoice(invoice).await?;

                // Register in location index (Hot is the primary location)
                self.location_index
                    .register_invoice(invoice.id, StorageTier::Hot);

                // Also index in warm tier if embedding provided (for vector search)
                // Note: This is for search indexing only, not primary storage
                if embedding.is_some() {
                    let mut warm = self.warm.write().await;
                    warm.store_invoice_with_embedding(invoice, embedding)
                        .await?;
                }
            }
            StorageTier::Warm | StorageTier::Cold => {
                // Store in warm tier
                let mut warm = self.warm.write().await;
                warm.store_invoice_with_embedding(invoice, embedding)
                    .await?;

                // Register in location index
                self.location_index.register_invoice(invoice.id, tier);
            }
        }

        // Invalidate cache
        self.cache.invalidate_invoice(&invoice.id);

        tracing::debug!(
            invoice_id = %invoice.id,
            tier = ?tier,
            "Stored invoice"
        );

        Ok(())
    }

    /// Get an invoice by ID
    pub async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError> {
        // Check cache
        if self.config.cache_enabled {
            if let Some(cached) = self.cache.get_invoice(id) {
                return Ok(Some(cached));
            }
        }

        // Check hot tier
        if let Some(invoice) = self.hot.get_invoice(id).await? {
            if self.config.cache_enabled {
                self.cache.put_invoice(invoice.clone());
            }
            return Ok(Some(invoice));
        }

        // Check warm tier
        let warm = self.warm.read().await;
        if let Some(invoice) = warm.get_invoice(id).await? {
            if self.config.cache_enabled {
                self.cache.put_invoice(invoice.clone());
            }
            return Ok(Some(invoice));
        }

        Ok(None)
    }

    /// Delete an invoice from all tiers
    ///
    /// Removes the invoice from both hot and warm tiers, plus the location index.
    pub async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
        self.cache.invalidate_invoice(id);

        // Remove from location index
        let was_tracked = self.location_index.remove_invoice(id).is_some();

        // Delete from hot tier
        let hot_deleted = self.hot.delete_invoice(id).await?;

        // Delete from warm tier (LanceDB)
        let warm = self.warm.read().await;
        let warm_deleted = warm.delete_invoice(id).await?;

        tracing::debug!(
            invoice_id = %id,
            hot_deleted = hot_deleted,
            warm_deleted = warm_deleted,
            "Deleted invoice from storage"
        );

        Ok(hot_deleted || warm_deleted || was_tracked)
    }

    /// List invoices from hot tier
    pub async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        self.hot.list_invoices(limit, offset).await
    }

    /// Count invoices across all tiers (deduplicated via location index)
    pub async fn count_invoices(&self) -> Result<usize, StorageError> {
        // Use location index for accurate, deduplicated count
        Ok(self.location_index.count_invoices())
    }

    /// Search invoices by embedding similarity
    pub async fn search_invoices_by_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<(Invoice, f32)>, StorageError> {
        let warm = self.warm.read().await;
        warm.search_invoices_by_embedding(query_embedding, limit)
            .await
    }

    /// Hybrid search with filters
    pub async fn search_invoices_hybrid(
        &self,
        query_embedding: Option<&[f32]>,
        filter: &InvoiceFilter,
        limit: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        let warm = self.warm.read().await;
        let filter_sql = filter.to_sql_filter().unwrap_or_default();

        if filter_sql.is_empty() {
            if let Some(emb) = query_embedding {
                Ok(warm
                    .search_invoices_by_embedding(emb, limit)
                    .await?
                    .into_iter()
                    .map(|(inv, _)| inv)
                    .collect())
            } else {
                warm.list_invoices(limit, 0).await
            }
        } else {
            warm.search_invoices_filtered(query_embedding, &filter_sql, limit)
                .await
        }
    }

    /// Store a contract
    pub async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError> {
        self.hot.store_contract(contract).await?;
        // Contracts are always stored in hot tier for now
        self.location_index
            .register_contract(contract.id, StorageTier::Hot);
        self.cache.invalidate_contract(&contract.id);
        Ok(())
    }

    /// Get a contract by ID
    pub async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError> {
        if self.config.cache_enabled {
            if let Some(cached) = self.cache.get_contract(id) {
                return Ok(Some(cached));
            }
        }

        let contract = self.hot.get_contract(id).await?;

        if let Some(ref c) = contract {
            if self.config.cache_enabled {
                self.cache.put_contract(c.clone());
            }
        }

        Ok(contract)
    }

    /// Delete a contract
    pub async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError> {
        self.cache.invalidate_contract(id);
        self.location_index.remove_contract(id);
        self.hot.delete_contract(id).await
    }

    /// List contracts
    pub async fn list_contracts(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Contract>, StorageError> {
        self.hot.list_contracts(limit, offset).await
    }

    /// Count contracts (deduplicated via location index)
    pub async fn count_contracts(&self) -> Result<usize, StorageError> {
        Ok(self.location_index.count_contracts())
    }

    /// Create vector indices
    pub async fn create_indices(&self) -> Result<(), StorageError> {
        let warm = self.warm.read().await;
        warm.create_vector_index().await
    }

    /// Migrate old data from hot to warm tier
    ///
    /// This uses atomic tier transitions in the location index to prevent
    /// duplicate counting during migration.
    pub async fn migrate_to_warm(&self) -> Result<usize, StorageError> {
        if !self.config.auto_migration {
            return Ok(0);
        }

        let cutoff_date =
            Utc::now().date_naive() - chrono::Duration::days(self.config.hot_tier_days as i64);

        let mut migrated = 0;

        // Get invoices that are in hot tier according to location index
        let hot_invoice_ids = self.location_index.invoices_in_tier(StorageTier::Hot);

        for invoice_id in hot_invoice_ids {
            // Fetch the invoice from hot tier
            let invoice = match self.hot.get_invoice(&invoice_id).await? {
                Some(inv) => inv,
                None => {
                    // Invoice no longer in hot tier, clean up index
                    self.location_index.remove_invoice(&invoice_id);
                    continue;
                }
            };

            if invoice.invoice_date < cutoff_date {
                // Step 1: Store in warm tier first
                let mut warm = self.warm.write().await;
                warm.store_invoice_with_embedding(&invoice, None).await?;
                drop(warm);

                // Step 2: Atomically update location index (single source of truth)
                // This ensures no duplicate counting during the transition
                self.location_index
                    .move_invoice(&invoice.id, StorageTier::Warm);

                // Step 3: Delete from hot tier (safe now since index updated)
                self.hot.delete_invoice(&invoice.id).await?;
                self.cache.invalidate_invoice(&invoice.id);

                migrated += 1;
            }
        }

        if migrated > 0 {
            tracing::info!(count = migrated, "Migrated invoices from hot to warm tier");
        }

        Ok(migrated)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        self.cache.stats()
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get hot storage reference
    pub fn hot(&self) -> &Arc<RedbStorage> {
        &self.hot
    }

    /// Get tier distribution for invoices
    pub fn invoice_tier_distribution(&self) -> TierDistribution {
        self.location_index.invoice_tier_distribution()
    }

    /// Get tier distribution for contracts
    pub fn contract_tier_distribution(&self) -> TierDistribution {
        self.location_index.contract_tier_distribution()
    }

    /// Get the location index for advanced queries
    pub fn location_index(&self) -> &Arc<DocumentLocationIndex> {
        &self.location_index
    }
}

use crate::traits::DocumentStore;

#[async_trait::async_trait]
impl DocumentStore for TieredStorage {
    async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError> {
        self.store_invoice(invoice, None).await
    }

    async fn store_invoice_with_embedding(
        &self,
        invoice: &Invoice,
        embedding: Option<&[f32]>,
    ) -> Result<(), StorageError> {
        self.store_invoice(invoice, embedding).await
    }

    async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError> {
        TieredStorage::get_invoice(self, id).await
    }

    async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
        TieredStorage::delete_invoice(self, id).await
    }

    async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError> {
        TieredStorage::store_contract(self, contract).await
    }

    async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError> {
        TieredStorage::get_contract(self, id).await
    }

    async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError> {
        TieredStorage::delete_contract(self, id).await
    }

    async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        TieredStorage::list_invoices(self, limit, offset).await
    }

    async fn list_contracts(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Contract>, StorageError> {
        TieredStorage::list_contracts(self, limit, offset).await
    }

    async fn count_invoices(&self) -> Result<usize, StorageError> {
        TieredStorage::count_invoices(self).await
    }

    async fn count_contracts(&self) -> Result<usize, StorageError> {
        TieredStorage::count_contracts(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tiered_storage_basic() {
        let storage = TieredStorage::in_memory().await.unwrap();

        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        storage.store_invoice(&invoice, None).await.unwrap();

        let loaded = storage.get_invoice(&invoice.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().invoice_number, "INV-001");
    }

    #[tokio::test]
    async fn test_tiered_storage_with_embedding() {
        let storage = TieredStorage::in_memory().await.unwrap();

        let invoice = Invoice::new("INV-002", NaiveDate::from_ymd_opt(2024, 2, 1).unwrap());
        let embedding: Vec<f32> = (0..384).map(|i| i as f32 / 1000.0).collect();

        storage
            .store_invoice(&invoice, Some(&embedding))
            .await
            .unwrap();

        // Search by embedding
        let query: Vec<f32> = (0..384).map(|i| i as f32 / 1000.0 + 0.001).collect();
        let results = storage
            .search_invoices_by_embedding(&query, 10)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.invoice_number, "INV-002");
    }

    #[tokio::test]
    async fn test_tiered_storage_cache() {
        let storage = TieredStorage::in_memory().await.unwrap();

        let invoice = Invoice::new("INV-003", NaiveDate::from_ymd_opt(2024, 3, 1).unwrap());
        storage.store_invoice(&invoice, None).await.unwrap();

        // First get
        let _ = storage.get_invoice(&invoice.id).await.unwrap();
        let (_hits, misses) = storage.cache_stats();
        assert_eq!(misses, 1);

        // Second get - should be cached
        let _ = storage.get_invoice(&invoice.id).await.unwrap();
        let (hits, _) = storage.cache_stats();
        assert_eq!(hits, 1);
    }
}

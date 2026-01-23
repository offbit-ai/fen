use std::path::Path;
use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use tokio::sync::RwLock;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;
use crate::hot::RedbStorage;
use crate::query::QueryCache;
use crate::traits::{InvoiceFilter, StorageTier};
use crate::warm::LanceStorage;

/// Configuration for tiered storage
#[derive(Debug, Clone)]
pub struct TieredStorageConfig {
    /// Path to hot storage (redb)
    pub hot_path: String,
    /// Path to warm storage (LanceDB)
    pub warm_path: String,
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
            hot_path: "data/hot.redb".to_string(),
            warm_path: "data/warm".to_string(),
            hot_tier_days: 30,
            warm_tier_days: 365,
            auto_migration: true,
            cache_enabled: true,
            cache_max_entries: 10000,
        }
    }
}

/// Tiered storage manager combining hot (redb) and warm (LanceDB) tiers
pub struct TieredStorage {
    config: TieredStorageConfig,
    hot: Arc<RedbStorage>,
    warm: Arc<RwLock<LanceStorage>>,
    cache: Arc<QueryCache>,
}

impl TieredStorage {
    /// Create a new tiered storage from configuration
    pub async fn new(config: TieredStorageConfig) -> Result<Self, StorageError> {
        // Ensure directories exist
        if let Some(parent) = Path::new(&config.hot_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(&config.warm_path)?;

        let hot = Arc::new(RedbStorage::new(&config.hot_path)?);
        let warm = Arc::new(RwLock::new(LanceStorage::new(&config.warm_path).await?));
        let cache = Arc::new(QueryCache::new(config.cache_max_entries));

        tracing::info!(
            hot_path = %config.hot_path,
            warm_path = %config.warm_path,
            "Initialized tiered storage"
        );

        Ok(Self {
            config,
            hot,
            warm,
            cache,
        })
    }

    /// Create an in-memory tiered storage (for testing)
    pub async fn in_memory() -> Result<Self, StorageError> {
        let temp_dir = std::env::temp_dir();
        let warm_path = temp_dir.join(format!("fen_test_{}", uuid::Uuid::new_v4()));

        let hot = Arc::new(RedbStorage::in_memory()?);
        let warm = Arc::new(RwLock::new(LanceStorage::new(&warm_path).await?));
        let cache = Arc::new(QueryCache::new(1000));

        Ok(Self {
            config: TieredStorageConfig::default(),
            hot,
            warm,
            cache,
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

                // Also index in warm tier if embedding provided (for vector search)
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

    /// Delete an invoice
    pub async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
        self.cache.invalidate_invoice(id);

        // Try to delete from hot tier
        let hot_deleted = self.hot.delete_invoice(id).await?;

        // Note: LanceDB doesn't support direct deletes easily, would need to implement
        // For now, just return hot tier result
        Ok(hot_deleted)
    }

    /// List invoices from hot tier
    pub async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        self.hot.list_invoices(limit, offset).await
    }

    /// Count invoices across all tiers
    pub async fn count_invoices(&self) -> Result<usize, StorageError> {
        let hot_count = self.hot.count_invoices().await?;
        let warm = self.warm.read().await;
        let warm_count = warm.count_invoices().await?;

        // Note: May have duplicates during migration window
        Ok(hot_count + warm_count)
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

    /// Count contracts
    pub async fn count_contracts(&self) -> Result<usize, StorageError> {
        self.hot.count_contracts().await
    }

    /// Create vector indices
    pub async fn create_indices(&self) -> Result<(), StorageError> {
        let warm = self.warm.read().await;
        warm.create_vector_index().await
    }

    /// Migrate old data from hot to warm tier
    pub async fn migrate_to_warm(&self) -> Result<usize, StorageError> {
        if !self.config.auto_migration {
            return Ok(0);
        }

        let cutoff_date = Utc::now().date_naive()
            - chrono::Duration::days(self.config.hot_tier_days as i64);

        let mut migrated = 0;
        let invoices = self.hot.list_invoices(1000, 0).await?;

        for invoice in invoices {
            if invoice.invoice_date < cutoff_date {
                // Move to warm tier
                let mut warm = self.warm.write().await;
                warm.store_invoice_with_embedding(&invoice, None).await?;
                drop(warm);

                // Delete from hot tier
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
}

use crate::traits::DocumentStore;

#[async_trait::async_trait]
impl DocumentStore for TieredStorage {
    async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError> {
        self.store_invoice(invoice, None).await
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
        let embedding: Vec<f32> = (0..768).map(|i| i as f32 / 1000.0).collect();

        storage
            .store_invoice(&invoice, Some(&embedding))
            .await
            .unwrap();

        // Search by embedding
        let query: Vec<f32> = (0..768).map(|i| i as f32 / 1000.0 + 0.001).collect();
        let results = storage.search_invoices_by_embedding(&query, 10).await.unwrap();

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

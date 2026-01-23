use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use chrono::{NaiveDate, Utc};
use dashmap::DashMap;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;
use crate::hot::RedbStorage;
use crate::traits::{DocumentStore, InvoiceFilter, QueryMetrics, StorageTier, VectorSearchResult};
use crate::warm::LanceStorage;

use super::cache::QueryCache;

/// Document location entry tracking which tier(s) a document exists in
#[derive(Debug, Clone)]
pub struct DocumentLocation {
    /// Tiers where this document is stored
    pub tiers: HashSet<StorageTier>,
    /// Primary (authoritative) tier for this document
    pub primary_tier: StorageTier,
    /// Last update timestamp
    pub last_updated: std::time::Instant,
}

impl DocumentLocation {
    fn new(tier: StorageTier) -> Self {
        let mut tiers = HashSet::new();
        tiers.insert(tier);
        Self {
            tiers,
            primary_tier: tier,
            last_updated: Instant::now(),
        }
    }

    fn add_tier(&mut self, tier: StorageTier) {
        self.tiers.insert(tier);
        self.last_updated = Instant::now();
    }

    fn remove_tier(&mut self, tier: StorageTier) {
        self.tiers.remove(&tier);
        self.last_updated = Instant::now();
    }

    #[allow(dead_code)]
    fn set_primary(&mut self, tier: StorageTier) {
        self.primary_tier = tier;
        self.last_updated = Instant::now();
    }
}

/// Document location index for tracking documents across tiers
pub struct DocumentLocationIndex {
    invoices: DashMap<InvoiceId, DocumentLocation>,
    contracts: DashMap<ContractId, DocumentLocation>,
}

impl DocumentLocationIndex {
    pub fn new() -> Self {
        Self {
            invoices: DashMap::new(),
            contracts: DashMap::new(),
        }
    }

    /// Register an invoice in a tier
    pub fn register_invoice(&self, id: &InvoiceId, tier: StorageTier) {
        self.invoices
            .entry(id.clone())
            .and_modify(|loc| loc.add_tier(tier))
            .or_insert_with(|| DocumentLocation::new(tier));
    }

    /// Unregister an invoice from a tier
    pub fn unregister_invoice(&self, id: &InvoiceId, tier: StorageTier) {
        if let Some(mut loc) = self.invoices.get_mut(id) {
            loc.remove_tier(tier);
            if loc.tiers.is_empty() {
                drop(loc);
                self.invoices.remove(id);
            }
        }
    }

    /// Get the location of an invoice
    pub fn get_invoice_location(&self, id: &InvoiceId) -> Option<DocumentLocation> {
        self.invoices.get(id).map(|loc| loc.clone())
    }

    /// Count invoices per tier (each invoice counted only in its primary tier)
    pub fn count_invoices_by_primary_tier(&self) -> (usize, usize, usize) {
        let mut hot = 0;
        let mut warm = 0;
        let mut cold = 0;

        for entry in self.invoices.iter() {
            match entry.value().primary_tier {
                StorageTier::Hot => hot += 1,
                StorageTier::Warm => warm += 1,
                StorageTier::Cold => cold += 1,
            }
        }

        (hot, warm, cold)
    }

    /// Total unique invoices across all tiers
    pub fn total_invoices(&self) -> usize {
        self.invoices.len()
    }

    /// Register a contract in a tier
    pub fn register_contract(&self, id: &ContractId, tier: StorageTier) {
        self.contracts
            .entry(id.clone())
            .and_modify(|loc| loc.add_tier(tier))
            .or_insert_with(|| DocumentLocation::new(tier));
    }

    /// Total unique contracts
    pub fn total_contracts(&self) -> usize {
        self.contracts.len()
    }
}

impl Default for DocumentLocationIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Query result with metadata
#[derive(Debug)]
pub struct QueryResult<T> {
    pub data: T,
    pub metrics: QueryMetrics,
}

impl<T> QueryResult<T> {
    pub fn new(data: T, metrics: QueryMetrics) -> Self {
        Self { data, metrics }
    }
}

/// Configuration for the query engine
#[derive(Debug, Clone)]
pub struct QueryEngineConfig {
    /// Age threshold (days) for hot tier data
    pub hot_tier_days: u32,
    /// Age threshold (days) for warm tier data (beyond this is cold)
    pub warm_tier_days: u32,
    /// Enable query caching
    pub cache_enabled: bool,
    /// Default limit for list queries
    pub default_limit: usize,
}

impl Default for QueryEngineConfig {
    fn default() -> Self {
        Self {
            hot_tier_days: 30,
            warm_tier_days: 365,
            cache_enabled: true,
            default_limit: 100,
        }
    }
}

/// Hybrid query engine that routes queries to appropriate storage tiers
pub struct QueryEngine {
    config: QueryEngineConfig,
    hot_storage: Arc<RedbStorage>,
    warm_storage: Arc<tokio::sync::RwLock<LanceStorage>>,
    cache: Arc<QueryCache>,
    /// Document location index for tracking documents across tiers
    location_index: Arc<DocumentLocationIndex>,
}

impl QueryEngine {
    /// Create a new query engine
    pub fn new(
        hot_storage: Arc<RedbStorage>,
        warm_storage: LanceStorage,
        config: QueryEngineConfig,
    ) -> Self {
        Self {
            config,
            hot_storage,
            warm_storage: Arc::new(tokio::sync::RwLock::new(warm_storage)),
            cache: Arc::new(QueryCache::new(10000)),
            location_index: Arc::new(DocumentLocationIndex::new()),
        }
    }

    /// Get a reference to the document location index
    pub fn location_index(&self) -> &Arc<DocumentLocationIndex> {
        &self.location_index
    }

    /// Determine the storage tier for a given date
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

    /// Store an invoice in the appropriate tier
    ///
    /// Documents are stored based on age:
    /// - Hot tier: < hot_tier_days old (stored in redb for fast access)
    /// - Warm tier: hot_tier_days to warm_tier_days old (stored in LanceDB with embeddings)
    /// - Cold tier: > warm_tier_days old (also uses warm tier storage)
    ///
    /// If an embedding is provided for a hot tier document, it's also indexed
    /// in the warm tier for vector search capability.
    pub async fn store_invoice(
        &self,
        invoice: &Invoice,
        embedding: Option<&[f32]>,
    ) -> Result<(), StorageError> {
        let tier = self.determine_tier(invoice.invoice_date);

        match tier {
            StorageTier::Hot => {
                // Store in hot tier (redb)
                self.hot_storage.store_invoice(invoice).await?;
                self.location_index.register_invoice(&invoice.id, StorageTier::Hot);

                // Also index in warm tier for vector search capability
                if embedding.is_some() {
                    let mut warm = self.warm_storage.write().await;
                    warm.store_invoice_with_embedding(invoice, embedding)
                        .await?;
                    // Mark as existing in warm tier too, but hot remains primary
                    self.location_index.register_invoice(&invoice.id, StorageTier::Warm);
                }
            }
            StorageTier::Warm | StorageTier::Cold => {
                // Store directly in warm tier
                let mut warm = self.warm_storage.write().await;
                warm.store_invoice_with_embedding(invoice, embedding)
                    .await?;
                self.location_index.register_invoice(&invoice.id, tier);
            }
        }

        // Invalidate cache for this invoice
        self.cache.invalidate_invoice(&invoice.id);

        Ok(())
    }

    /// Get an invoice by ID, checking all tiers
    pub async fn get_invoice(
        &self,
        id: &InvoiceId,
    ) -> Result<QueryResult<Option<Invoice>>, StorageError> {
        let start = Instant::now();

        // Check cache first
        if self.config.cache_enabled {
            if let Some(cached) = self.cache.get_invoice(id) {
                return Ok(QueryResult::new(
                    Some(cached),
                    QueryMetrics {
                        query_time_ms: start.elapsed().as_millis() as u64,
                        rows_scanned: 0,
                        rows_returned: 1,
                        tier_used: None,
                        cache_hit: true,
                    },
                ));
            }
        }

        // Check hot tier first
        if let Some(invoice) = self.hot_storage.get_invoice(id).await? {
            if self.config.cache_enabled {
                self.cache.put_invoice(invoice.clone());
            }
            return Ok(QueryResult::new(
                Some(invoice),
                QueryMetrics {
                    query_time_ms: start.elapsed().as_millis() as u64,
                    rows_scanned: 1,
                    rows_returned: 1,
                    tier_used: Some(StorageTier::Hot),
                    cache_hit: false,
                },
            ));
        }

        // Check warm tier
        let warm = self.warm_storage.read().await;
        if let Some(invoice) = warm.get_invoice(id).await? {
            if self.config.cache_enabled {
                self.cache.put_invoice(invoice.clone());
            }
            return Ok(QueryResult::new(
                Some(invoice),
                QueryMetrics {
                    query_time_ms: start.elapsed().as_millis() as u64,
                    rows_scanned: 1,
                    rows_returned: 1,
                    tier_used: Some(StorageTier::Warm),
                    cache_hit: false,
                },
            ));
        }

        Ok(QueryResult::new(
            None,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: 0,
                rows_returned: 0,
                tier_used: None,
                cache_hit: false,
            },
        ))
    }

    /// List invoices from hot tier with pagination
    pub async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<QueryResult<Vec<Invoice>>, StorageError> {
        let start = Instant::now();
        let limit = if limit == 0 {
            self.config.default_limit
        } else {
            limit
        };

        let invoices = self.hot_storage.list_invoices(limit, offset).await?;
        let count = invoices.len();

        Ok(QueryResult::new(
            invoices,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: count,
                rows_returned: count,
                tier_used: Some(StorageTier::Hot),
                cache_hit: false,
            },
        ))
    }

    /// Search invoices by semantic similarity
    pub async fn search_invoices_by_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<QueryResult<Vec<VectorSearchResult<Invoice>>>, StorageError> {
        let start = Instant::now();

        let warm = self.warm_storage.read().await;
        let results = warm.search_invoices_by_embedding(query_embedding, limit).await?;

        let count = results.len();
        let search_results: Vec<VectorSearchResult<Invoice>> = results
            .into_iter()
            .map(|(invoice, score)| VectorSearchResult {
                item: invoice,
                score,
            })
            .collect();

        Ok(QueryResult::new(
            search_results,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: count,
                rows_returned: count,
                tier_used: Some(StorageTier::Warm),
                cache_hit: false,
            },
        ))
    }

    /// Hybrid search: semantic similarity + filters
    pub async fn search_invoices_hybrid(
        &self,
        query_embedding: Option<&[f32]>,
        filter: &InvoiceFilter,
        limit: usize,
    ) -> Result<QueryResult<Vec<Invoice>>, StorageError> {
        let start = Instant::now();

        // For hybrid search, use warm tier (LanceDB) which supports vector + SQL
        let warm = self.warm_storage.read().await;
        let filter_sql = filter.to_sql_filter().unwrap_or_default();

        let invoices = if filter_sql.is_empty() {
            if let Some(emb) = query_embedding {
                warm.search_invoices_by_embedding(emb, limit)
                    .await?
                    .into_iter()
                    .map(|(inv, _)| inv)
                    .collect()
            } else {
                warm.list_invoices(limit, 0).await?
            }
        } else {
            warm.search_invoices_filtered(query_embedding, &filter_sql, limit)
                .await?
        };

        let count = invoices.len();

        Ok(QueryResult::new(
            invoices,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: count,
                rows_returned: count,
                tier_used: Some(StorageTier::Warm),
                cache_hit: false,
            },
        ))
    }

    /// Get invoice count across tiers with proper deduplication
    ///
    /// During tier migration, documents may temporarily exist in multiple tiers.
    /// This method returns accurate counts by:
    /// 1. Counting hot tier documents (authoritative for recent documents)
    /// 2. Counting warm tier documents that are NOT in hot tier
    ///
    /// Uses the document location index to provide accurate, deduplicated counts.
    /// Each document is counted only once, based on its primary tier.
    pub async fn count_invoices(&self) -> Result<QueryResult<usize>, StorageError> {
        let start = Instant::now();

        // Use the location index for accurate counts (each doc counted once)
        let total = self.location_index.total_invoices();

        // Also get tier breakdown for metrics
        let (hot_count, warm_count, _cold_count) = self.location_index.count_invoices_by_primary_tier();

        Ok(QueryResult::new(
            total,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: hot_count + warm_count,
                rows_returned: 1,
                tier_used: None,
                cache_hit: false,
            },
        ))
    }

    /// Get detailed invoice counts by tier
    pub fn count_invoices_by_tier(&self) -> (usize, usize, usize) {
        self.location_index.count_invoices_by_primary_tier()
    }

    /// Store a contract
    pub async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError> {
        // Contracts go to hot tier by default
        self.hot_storage.store_contract(contract).await?;
        self.location_index.register_contract(&contract.id, StorageTier::Hot);
        self.cache.invalidate_contract(&contract.id);
        Ok(())
    }

    /// Get a contract by ID
    pub async fn get_contract(
        &self,
        id: &ContractId,
    ) -> Result<QueryResult<Option<Contract>>, StorageError> {
        let start = Instant::now();

        // Check cache
        if self.config.cache_enabled {
            if let Some(cached) = self.cache.get_contract(id) {
                return Ok(QueryResult::new(
                    Some(cached),
                    QueryMetrics {
                        query_time_ms: start.elapsed().as_millis() as u64,
                        rows_scanned: 0,
                        rows_returned: 1,
                        tier_used: None,
                        cache_hit: true,
                    },
                ));
            }
        }

        // Check hot tier
        let contract = self.hot_storage.get_contract(id).await?;
        let has_contract = contract.is_some();

        if let Some(ref c) = contract {
            if self.config.cache_enabled {
                self.cache.put_contract(c.clone());
            }
        }

        Ok(QueryResult::new(
            contract,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: 1,
                rows_returned: if has_contract { 1 } else { 0 },
                tier_used: Some(StorageTier::Hot),
                cache_hit: false,
            },
        ))
    }

    /// List contracts with pagination
    pub async fn list_contracts(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<QueryResult<Vec<Contract>>, StorageError> {
        let start = Instant::now();

        let contracts = self.hot_storage.list_contracts(limit, offset).await?;
        let count = contracts.len();

        Ok(QueryResult::new(
            contracts,
            QueryMetrics {
                query_time_ms: start.elapsed().as_millis() as u64,
                rows_scanned: count,
                rows_returned: count,
                tier_used: Some(StorageTier::Hot),
                cache_hit: false,
            },
        ))
    }

    /// Create vector indices for faster search
    pub async fn create_indices(&self) -> Result<(), StorageError> {
        let warm = self.warm_storage.read().await;
        warm.create_vector_index().await
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        self.cache.stats()
    }

    /// Clear the query cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_engine() -> QueryEngine {
        let _hot_dir = tempdir().unwrap();
        let warm_dir = tempdir().unwrap();

        let hot_storage = Arc::new(RedbStorage::in_memory().unwrap());
        let warm_storage: LanceStorage = LanceStorage::new(warm_dir.path()).await.unwrap();

        QueryEngine::new(hot_storage, warm_storage, QueryEngineConfig::default())
    }

    /// Get a recent date that will be in hot tier (today - 5 days)
    fn recent_date() -> NaiveDate {
        Utc::now().date_naive() - chrono::Duration::days(5)
    }

    #[tokio::test]
    async fn test_query_engine_store_and_get() {
        let engine = create_test_engine().await;

        // Use a recent date so it goes to hot tier
        let invoice = Invoice::new("INV-001", recent_date());
        engine.store_invoice(&invoice, None).await.unwrap();

        let result = engine.get_invoice(&invoice.id).await.unwrap();
        assert!(result.data.is_some());
        assert_eq!(result.data.unwrap().invoice_number, "INV-001");
        assert_eq!(result.metrics.tier_used, Some(StorageTier::Hot));
    }

    #[tokio::test]
    async fn test_query_engine_cache() {
        let engine = create_test_engine().await;

        let invoice = Invoice::new("INV-002", recent_date());
        engine.store_invoice(&invoice, None).await.unwrap();

        // First get - cache miss
        let result1 = engine.get_invoice(&invoice.id).await.unwrap();
        assert!(!result1.metrics.cache_hit);

        // Second get - cache hit
        let result2 = engine.get_invoice(&invoice.id).await.unwrap();
        assert!(result2.metrics.cache_hit);
    }

    #[tokio::test]
    async fn test_query_engine_tier_determination() {
        let engine = create_test_engine().await;

        // Recent date should go to hot tier
        let today = Utc::now().date_naive();
        assert_eq!(engine.determine_tier(today), StorageTier::Hot);

        // Old date should go to warm tier
        let old_date = today - chrono::Duration::days(60);
        assert_eq!(engine.determine_tier(old_date), StorageTier::Warm);

        // Very old date should go to cold tier
        let very_old_date = today - chrono::Duration::days(400);
        assert_eq!(engine.determine_tier(very_old_date), StorageTier::Cold);
    }
}

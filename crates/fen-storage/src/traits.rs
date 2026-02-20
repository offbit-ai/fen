use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;

/// Generic document storage operations
#[async_trait]
pub trait DocumentStore: Send + Sync {
    /// Store an invoice
    async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError>;

    /// Store an invoice with its embedding vector for warm-tier indexing.
    ///
    /// Default implementation drops the embedding and delegates to `store_invoice`.
    async fn store_invoice_with_embedding(
        &self,
        invoice: &Invoice,
        _embedding: Option<&[f32]>,
    ) -> Result<(), StorageError> {
        self.store_invoice(invoice).await
    }

    /// Retrieve an invoice by ID
    async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError>;

    /// Delete an invoice by ID
    async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError>;

    /// Store a contract
    async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError>;

    /// Retrieve a contract by ID
    async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError>;

    /// Delete a contract by ID
    async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError>;

    /// List invoices with pagination
    async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError>;

    /// List contracts with pagination
    async fn list_contracts(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Contract>, StorageError>;

    /// Count total invoices
    async fn count_invoices(&self) -> Result<usize, StorageError>;

    /// Count total contracts
    async fn count_contracts(&self) -> Result<usize, StorageError>;
}

/// Vector search result with similarity score
#[derive(Debug, Clone)]
pub struct VectorSearchResult<T> {
    pub item: T,
    pub score: f32,
}

/// Invoice search filters
#[derive(Debug, Clone, Default)]
pub struct InvoiceFilter {
    /// Filter by vendor name (partial match)
    pub vendor_name: Option<String>,
    /// Filter by date range
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    /// Filter by minimum amount
    pub min_amount: Option<Decimal>,
    /// Filter by maximum amount
    pub max_amount: Option<Decimal>,
    /// Filter by currency
    pub currency: Option<String>,
}

impl InvoiceFilter {
    /// Build a SQL filter string for LanceDB
    pub fn to_sql_filter(&self) -> Option<String> {
        let mut conditions = Vec::new();

        if let Some(ref name) = self.vendor_name {
            conditions.push(format!("vendor_name LIKE '%{}%'", name.replace('\'', "''")));
        }

        if let Some(date) = self.date_from {
            conditions.push(format!("invoice_date >= '{}'", date));
        }

        if let Some(date) = self.date_to {
            conditions.push(format!("invoice_date <= '{}'", date));
        }

        if let Some(min) = self.min_amount {
            conditions.push(format!("total_amount >= {}", min));
        }

        if let Some(max) = self.max_amount {
            conditions.push(format!("total_amount <= {}", max));
        }

        if let Some(ref curr) = self.currency {
            conditions.push(format!("currency = '{}'", curr));
        }

        if conditions.is_empty() {
            None
        } else {
            Some(conditions.join(" AND "))
        }
    }
}

/// Vector store operations for semantic search
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Store an invoice with its embedding vector
    async fn store_invoice_with_embedding(
        &mut self,
        invoice: &Invoice,
        embedding: Option<&[f32]>,
    ) -> Result<(), StorageError>;

    /// Search invoices by semantic similarity
    async fn search_invoices_by_embedding(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult<Invoice>>, StorageError>;

    /// Hybrid search: semantic + filters
    async fn search_invoices_hybrid(
        &self,
        query_embedding: Option<&[f32]>,
        filter: &InvoiceFilter,
        limit: usize,
    ) -> Result<Vec<Invoice>, StorageError>;

    /// Create vector index for faster search
    async fn create_vector_index(&self) -> Result<(), StorageError>;
}

/// Storage tier for tiered architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageTier {
    /// Hot tier (redb) - recent data, sub-ms latency
    Hot,
    /// Warm tier (LanceDB) - 30-365 days, vector search
    Warm,
    /// Cold tier - archived data
    Cold,
}

/// Metrics for query performance
#[derive(Debug, Clone, Default)]
pub struct QueryMetrics {
    pub query_time_ms: u64,
    pub rows_scanned: usize,
    pub rows_returned: usize,
    pub tier_used: Option<StorageTier>,
    pub cache_hit: bool,
}

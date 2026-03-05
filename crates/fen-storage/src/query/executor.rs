//! Unified query executor for the Fen query language
//!
//! Executes parsed queries against the storage layer, combining:
//! - Vector similarity search (VECTOR_DISTANCE)
//! - Full-text BM25 search (BM25_SCORE)
//! - Standard SQL-like filters
//! - Score fusion for hybrid ranking
//!
//! # Execution Model
//!
//! The executor delegates CPU-bound stages (filter, score, sort, project)
//! to the [`morsel`](super::morsel) module for parallel execution. The
//! executor itself handles I/O: candidate retrieval from hot/warm storage,
//! vector search via LanceDB, and full-text search via Tantivy.
//!
//! ```text
//! Executor (async I/O)           Morsel (parallel CPU)
//! ┌─────────────────┐           ┌───────────────────┐
//! │ vector search   │──────────>│ filter + score    │
//! │ text search     │           │ sort              │
//! │ hot/warm scan   │           │ project           │
//! └─────────────────┘           └───────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;
use crate::fulltext::FullTextIndex;
use crate::hot::RedbStorage;
use crate::query::lang::{
    Expr, FenQuery, FunctionCall, FunctionName, QueryParams, QueryTarget, SelectItem,
};
use crate::traits::DocumentStore;
use crate::warm::LanceStorage;

/// Query execution result
#[derive(Debug)]
pub struct ExecutionResult {
    /// Result rows
    pub rows: Vec<ResultRow>,
    /// Execution metadata
    pub metadata: ExecutionMetadata,
}

/// A single result row
#[derive(Debug, Clone)]
pub struct ResultRow {
    /// Column values by name
    pub columns: HashMap<String, ColumnValue>,
    /// Combined relevance score (if applicable)
    pub score: Option<f64>,
}

/// Column value types
#[derive(Debug, Clone)]
pub enum ColumnValue {
    String(String),
    Integer(i64),
    Float(f64),
    Decimal(Decimal),
    Date(NaiveDate),
    Boolean(bool),
    Null,
}

impl ColumnValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ColumnValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ColumnValue::Float(f) => Some(*f),
            ColumnValue::Integer(i) => Some(*i as f64),
            ColumnValue::Decimal(d) => d.to_string().parse().ok(),
            _ => None,
        }
    }
}

/// Execution metadata
#[derive(Debug, Default)]
pub struct ExecutionMetadata {
    /// Time taken to execute (ms)
    pub execution_time_ms: u64,
    /// Number of rows scanned
    pub rows_scanned: usize,
    /// Number of rows returned
    pub rows_returned: usize,
    /// Whether vector search was used
    pub used_vector_search: bool,
    /// Whether text search was used
    pub used_text_search: bool,
}

/// Configuration for the query executor
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Default limit for queries without LIMIT clause
    pub default_limit: usize,
    /// Maximum allowed limit
    pub max_limit: usize,
    /// Weight for vector distance in score fusion (0.0 - 1.0)
    pub vector_weight: f64,
    /// Weight for BM25 score in score fusion (0.0 - 1.0)
    pub text_weight: f64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            default_limit: 100,
            max_limit: 10000,
            vector_weight: 0.7,
            text_weight: 0.3,
        }
    }
}

/// Pre-computed query feature flags to avoid repeated AST walks
struct QueryFlags {
    uses_vector: bool,
    uses_text: bool,
    uses_contains: bool,
}

/// Unified query executor
pub struct QueryExecutor {
    config: ExecutorConfig,
    hot_storage: Arc<RedbStorage>,
    warm_storage: Arc<tokio::sync::RwLock<LanceStorage>>,
    fulltext_index: Arc<FullTextIndex>,
    /// Morsel context for parallel execution
    morsel_ctx: super::morsel::MorselContext,
}

impl QueryExecutor {
    /// Create a new query executor
    pub fn new(
        hot_storage: Arc<RedbStorage>,
        warm_storage: Arc<tokio::sync::RwLock<LanceStorage>>,
        fulltext_index: Arc<FullTextIndex>,
        config: ExecutorConfig,
    ) -> Self {
        let morsel_ctx = super::morsel::MorselContext {
            fulltext_index: Arc::clone(&fulltext_index),
            config: super::morsel::MorselConfig {
                vector_weight: config.vector_weight,
                text_weight: config.text_weight,
                ..Default::default()
            },
        };
        Self {
            config,
            hot_storage,
            warm_storage,
            fulltext_index,
            morsel_ctx,
        }
    }

    /// Execute a query
    pub async fn execute(
        &self,
        query: &FenQuery,
        params: &QueryParams,
    ) -> Result<ExecutionResult, StorageError> {
        let start = std::time::Instant::now();

        // Pre-compute feature flags once instead of walking AST per-row
        let flags = QueryFlags {
            uses_vector: query.uses_vector_search(),
            uses_text: query.uses_text_search(),
            uses_contains: query.uses_contains(),
        };

        let result = match query.from {
            QueryTarget::Invoices => self.execute_invoice_query(query, params, &flags).await?,
            QueryTarget::Contracts => self.execute_contract_query(query, params, &flags).await?,
        };

        let execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            metadata: ExecutionMetadata {
                execution_time_ms,
                rows_returned: result.len(),
                used_vector_search: flags.uses_vector,
                used_text_search: flags.uses_text,
                ..Default::default()
            },
            rows: result,
        })
    }

    async fn execute_invoice_query(
        &self,
        query: &FenQuery,
        params: &QueryParams,
        flags: &QueryFlags,
    ) -> Result<Vec<ResultRow>, StorageError> {
        // Determine the limit
        let limit = query
            .limit
            .map(|l| l as usize)
            .unwrap_or(self.config.default_limit)
            .min(self.config.max_limit);

        let offset = query.offset.map(|o| o as usize).unwrap_or(0);

        // Determine search strategy based on pre-computed flags
        let uses_vector = flags.uses_vector;
        let uses_text = flags.uses_text || flags.uses_contains;

        // Get candidate invoices based on search strategy
        let candidates = if uses_vector {
            self.vector_search_invoices(query, params, limit * 3)
                .await?
        } else if uses_text {
            self.text_search_invoices(query, params, limit * 3).await?
        } else {
            self.scan_invoices(limit * 3, offset).await?
        };

        // Morsel-parallel filter + score (auto-selects sequential for small sets)
        let morsel_features = super::morsel::QueryFeatures {
            uses_vector: flags.uses_vector,
            uses_text: flags.uses_text,
            uses_contains: flags.uses_contains,
        };
        let mut scored_rows = super::morsel::morsel_filter_score_invoices(
            candidates,
            query.filter.as_ref(),
            query,
            params,
            &morsel_features,
            &self.morsel_ctx,
        );

        // Morsel-parallel sort (auto-selects sequential for small sets)
        if let Some(order_by) = &query.order_by {
            super::morsel::morsel_sort_invoices(&mut scored_rows, order_by);
        }

        // Apply OFFSET and LIMIT with parallel projection
        let results = super::morsel::morsel_project_invoices(
            scored_rows,
            &query.select,
            offset,
            limit,
        );

        Ok(results)
    }

    async fn execute_contract_query(
        &self,
        query: &FenQuery,
        params: &QueryParams,
        flags: &QueryFlags,
    ) -> Result<Vec<ResultRow>, StorageError> {
        // Determine the limit
        let limit = query
            .limit
            .map(|l| l as usize)
            .unwrap_or(self.config.default_limit)
            .min(self.config.max_limit);

        let offset = query.offset.map(|o| o as usize).unwrap_or(0);

        // For contracts, we primarily use hot storage (no vector search for contracts yet)
        let uses_text = flags.uses_text || flags.uses_contains;

        // Get candidate contracts
        let candidates = if uses_text {
            self.text_search_contracts(query, params, limit * 3).await?
        } else {
            self.scan_contracts(limit * 3, offset).await?
        };

        // Morsel-parallel filter (auto-selects sequential for small sets)
        let morsel_features = super::morsel::QueryFeatures {
            uses_vector: flags.uses_vector,
            uses_text: flags.uses_text,
            uses_contains: flags.uses_contains,
        };
        let mut candidates = super::morsel::morsel_filter_score_contracts(
            candidates,
            query.filter.as_ref(),
            query,
            params,
            &morsel_features,
            &self.morsel_ctx,
        );

        // Morsel-parallel sort
        if let Some(order_by) = &query.order_by {
            super::morsel::morsel_sort_contracts(&mut candidates, order_by);
        }

        // Apply OFFSET and LIMIT
        let results: Vec<ResultRow> = candidates
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(contract, score)| self.project_contract(&contract, &query.select, score))
            .collect();

        Ok(results)
    }

    async fn text_search_contracts(
        &self,
        query: &FenQuery,
        params: &QueryParams,
        limit: usize,
    ) -> Result<Vec<(Contract, f64)>, StorageError> {
        // Extract text search parameter
        let search_terms = self.find_text_param(query, params)?;

        let search_results = self.fulltext_index.search_contracts(&search_terms, limit)?;

        // Fetch full contracts for each result
        let mut contracts = Vec::new();
        for result in search_results {
            if let Ok(uuid) = result.id.parse::<uuid::Uuid>() {
                let id = ContractId(uuid);
                if let Ok(Some(contract)) = self.hot_storage.get_contract(&id).await {
                    contracts.push((contract, result.score as f64));
                }
            }
        }

        Ok(contracts)
    }

    async fn scan_contracts(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(Contract, f64)>, StorageError> {
        let contracts = self.hot_storage.list_contracts(limit, offset).await?;
        Ok(contracts.into_iter().map(|c| (c, 0.0)).collect())
    }

    fn get_contract_column(&self, contract: &Contract, column: &str) -> ColumnValue {
        if column.eq_ignore_ascii_case("id") {
            ColumnValue::String(contract.id.0.to_string())
        } else if column.eq_ignore_ascii_case("document_id") {
            ColumnValue::String(contract.document_id.0.to_string())
        } else if column.eq_ignore_ascii_case("contract_number") {
            contract.contract_number.clone().map(ColumnValue::String).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("title") {
            ColumnValue::String(contract.title.clone())
        } else if column.eq_ignore_ascii_case("contract_type") {
            ColumnValue::String(format!("{:?}", contract.contract_type))
        } else if column.eq_ignore_ascii_case("effective_date") {
            ColumnValue::Date(contract.effective_date)
        } else if column.eq_ignore_ascii_case("expiration_date") {
            contract.expiration_date.map(ColumnValue::Date).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("execution_date") {
            contract.execution_date.map(ColumnValue::Date).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("total_value") {
            contract.total_value.map(ColumnValue::Decimal).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("currency") {
            contract.currency.as_ref().map(|c| ColumnValue::String(format!("{:?}", c))).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("validation_status") {
            ColumnValue::String(format!("{:?}", contract.validation_status))
        } else if column.eq_ignore_ascii_case("confidence_score") {
            ColumnValue::Float(contract.confidence_score as f64)
        } else if column.eq_ignore_ascii_case("extracted_text") {
            ColumnValue::String(contract.extracted_text.clone())
        } else if column.eq_ignore_ascii_case("party_name") || column.eq_ignore_ascii_case("vendor_name") {
            contract.parties.first().map(|p| ColumnValue::String(p.name.clone())).unwrap_or(ColumnValue::Null)
        } else {
            ColumnValue::Null
        }
    }

    fn project_contract(
        &self,
        contract: &Contract,
        select: &[SelectItem],
        score: f64,
    ) -> ResultRow {
        let mut columns = HashMap::new();

        for item in select {
            match &item.expr {
                Expr::Wildcard => {
                    // Add all columns
                    columns.insert(
                        "id".to_string(),
                        ColumnValue::String(contract.id.0.to_string()),
                    );
                    columns.insert(
                        "contract_number".to_string(),
                        contract
                            .contract_number
                            .clone()
                            .map(ColumnValue::String)
                            .unwrap_or(ColumnValue::Null),
                    );
                    columns.insert(
                        "title".to_string(),
                        ColumnValue::String(contract.title.clone()),
                    );
                    columns.insert(
                        "contract_type".to_string(),
                        ColumnValue::String(format!("{:?}", contract.contract_type)),
                    );
                    columns.insert(
                        "effective_date".to_string(),
                        ColumnValue::Date(contract.effective_date),
                    );
                    columns.insert(
                        "expiration_date".to_string(),
                        contract
                            .expiration_date
                            .map(ColumnValue::Date)
                            .unwrap_or(ColumnValue::Null),
                    );
                    columns.insert(
                        "total_value".to_string(),
                        contract
                            .total_value
                            .map(ColumnValue::Decimal)
                            .unwrap_or(ColumnValue::Null),
                    );
                    columns.insert(
                        "confidence_score".to_string(),
                        ColumnValue::Float(contract.confidence_score as f64),
                    );
                    // Add first party name if available
                    if let Some(party) = contract.parties.first() {
                        columns.insert(
                            "party_name".to_string(),
                            ColumnValue::String(party.name.clone()),
                        );
                    }
                }
                Expr::Column(col) => {
                    let name = item.alias.clone().unwrap_or_else(|| col.column.clone());
                    let value = self.get_contract_column(contract, &col.column);
                    columns.insert(name, value);
                }
                Expr::Function(FunctionCall {
                    name: FunctionName::Bm25Score,
                    ..
                }) => {
                    let name = item
                        .alias
                        .clone()
                        .unwrap_or_else(|| "bm25_score".to_string());
                    columns.insert(name, ColumnValue::Float(score));
                }
                _ => {}
            }
        }

        ResultRow {
            columns,
            score: Some(score),
        }
    }

    async fn vector_search_invoices(
        &self,
        query: &FenQuery,
        params: &QueryParams,
        limit: usize,
    ) -> Result<Vec<(Invoice, f64)>, StorageError> {
        // Extract vector parameter from query
        let vector_param = self.find_vector_param(query, params)?;

        let warm = self.warm_storage.read().await;
        let results = warm
            .search_invoices_by_embedding(&vector_param, limit)
            .await?;

        Ok(results
            .into_iter()
            .map(|(inv, score)| (inv, score as f64))
            .collect())
    }

    async fn text_search_invoices(
        &self,
        query: &FenQuery,
        params: &QueryParams,
        limit: usize,
    ) -> Result<Vec<(Invoice, f64)>, StorageError> {
        // Extract text search parameter
        let search_terms = self.find_text_param(query, params)?;

        let search_results = self.fulltext_index.search_invoices(&search_terms, limit)?;

        // Fetch full invoices for each result — try hot first, batch warm misses
        let mut invoices = Vec::new();
        let mut warm_pending: Vec<(InvoiceId, f64)> = Vec::new();

        for result in search_results {
            if let Ok(uuid) = result.id.parse::<uuid::Uuid>() {
                let id = InvoiceId(uuid);
                if let Ok(Some(inv)) = self.hot_storage.get_invoice(&id).await {
                    invoices.push((inv, result.score as f64));
                } else {
                    warm_pending.push((id, result.score as f64));
                }
            }
        }

        // Single warm storage lock for all misses
        if !warm_pending.is_empty() {
            let warm = self.warm_storage.read().await;
            for (id, score) in warm_pending {
                if let Ok(Some(inv)) = warm.get_invoice(&id).await {
                    invoices.push((inv, score));
                }
            }
        }

        Ok(invoices)
    }

    async fn scan_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<(Invoice, f64)>, StorageError> {
        let invoices = self.hot_storage.list_invoices(limit, offset).await?;
        Ok(invoices.into_iter().map(|inv| (inv, 0.0)).collect())
    }

    fn find_vector_param(
        &self,
        query: &FenQuery,
        params: &QueryParams,
    ) -> Result<Vec<f32>, StorageError> {
        // Look for VECTOR_DISTANCE function calls and extract the parameter name
        fn find_in_expr(expr: &Expr) -> Option<String> {
            match expr {
                Expr::Function(FunctionCall {
                    name: FunctionName::VectorDistance,
                    args,
                }) => args.get(1).and_then(|arg| {
                    if let Expr::Parameter(name) = arg {
                        Some(name.clone())
                    } else {
                        None
                    }
                }),
                Expr::BinaryOp { left, right, .. } => {
                    find_in_expr(left).or_else(|| find_in_expr(right))
                }
                _ => None,
            }
        }

        // Search in SELECT, WHERE, ORDER BY
        let param_name = query
            .select
            .iter()
            .find_map(|s| find_in_expr(&s.expr))
            .or_else(|| query.filter.as_ref().and_then(|f| find_in_expr(&f.expr)))
            .or_else(|| {
                query
                    .order_by
                    .as_ref()
                    .and_then(|o| o.items.iter().find_map(|i| find_in_expr(&i.expr)))
            })
            .ok_or_else(|| StorageError::Query("No vector parameter found in query".to_string()))?;

        params
            .get_vector(&param_name)
            .map(|v| v.to_vec())
            .ok_or_else(|| {
                StorageError::Query(format!("Vector parameter '{}' not provided", param_name))
            })
    }

    fn find_text_param(
        &self,
        query: &FenQuery,
        params: &QueryParams,
    ) -> Result<String, StorageError> {
        // Look for BM25_SCORE or CONTAINS function calls and extract the parameter name
        fn find_in_expr(expr: &Expr) -> Option<String> {
            match expr {
                Expr::Function(FunctionCall { name, args }) => {
                    if matches!(name, FunctionName::Bm25Score | FunctionName::Contains) {
                        args.get(1).and_then(|arg| {
                            if let Expr::Parameter(name) = arg {
                                Some(name.clone())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                }
                Expr::BinaryOp { left, right, .. } => {
                    find_in_expr(left).or_else(|| find_in_expr(right))
                }
                _ => None,
            }
        }

        let param_name = query
            .select
            .iter()
            .find_map(|s| find_in_expr(&s.expr))
            .or_else(|| query.filter.as_ref().and_then(|f| find_in_expr(&f.expr)))
            .or_else(|| {
                query
                    .order_by
                    .as_ref()
                    .and_then(|o| o.items.iter().find_map(|i| find_in_expr(&i.expr)))
            })
            .ok_or_else(|| {
                StorageError::Query("No text search parameter found in query".to_string())
            })?;

        params
            .get_string(&param_name)
            .map(|s| s.to_string())
            .ok_or_else(|| {
                StorageError::Query(format!("Text parameter '{}' not provided", param_name))
            })
    }

}

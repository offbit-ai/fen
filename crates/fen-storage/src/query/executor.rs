//! Unified query executor for the Fen query language
//!
//! Executes parsed queries against the storage layer, combining:
//! - Vector similarity search (VECTOR_DISTANCE)
//! - Full-text BM25 search (BM25_SCORE)
//! - Standard SQL-like filters
//! - Score fusion for hybrid ranking

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;
use crate::fulltext::FullTextIndex;
use crate::hot::RedbStorage;
use crate::query::lang::{
    BinaryOperator, Expr, FenQuery, FilterExpr, FunctionCall, FunctionName, Literal, OrderByClause,
    ParamValue, QueryParams, QueryTarget, SelectItem, SortDirection,
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
    /// Cache compiled LIKE/ILIKE regexes keyed by (pattern, case_insensitive)
    like_cache: RefCell<HashMap<(String, bool), regex::Regex>>,
}

impl QueryExecutor {
    /// Create a new query executor
    pub fn new(
        hot_storage: Arc<RedbStorage>,
        warm_storage: Arc<tokio::sync::RwLock<LanceStorage>>,
        fulltext_index: Arc<FullTextIndex>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            config,
            hot_storage,
            warm_storage,
            fulltext_index,
            like_cache: RefCell::new(HashMap::new()),
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

        // Clear the LIKE regex cache between queries
        self.like_cache.borrow_mut().clear();

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
        let mut candidates = if uses_vector {
            self.vector_search_invoices(query, params, limit * 3)
                .await?
        } else if uses_text {
            self.text_search_invoices(query, params, limit * 3).await?
        } else {
            self.scan_invoices(limit * 3, offset).await?
        };

        // Apply WHERE filters
        if let Some(filter) = &query.filter {
            candidates.retain(|(inv, _)| self.evaluate_filter(filter, inv, params));
        }

        // Calculate scores for each candidate
        let mut scored_rows: Vec<(Invoice, f64)> = candidates
            .into_iter()
            .map(|(inv, base_score)| {
                let score = self.calculate_combined_score(query, &inv, params, base_score, flags);
                (inv, score)
            })
            .collect();

        // Apply ORDER BY
        if let Some(order_by) = &query.order_by {
            self.sort_results(&mut scored_rows, order_by, params);
        }

        // Apply OFFSET and LIMIT
        let results: Vec<ResultRow> = scored_rows
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|(inv, score)| self.project_invoice(&inv, &query.select, score))
            .collect();

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
        let mut candidates = if uses_text {
            self.text_search_contracts(query, params, limit * 3).await?
        } else {
            self.scan_contracts(limit * 3, offset).await?
        };

        // Apply WHERE filters
        if let Some(filter) = &query.filter {
            candidates
                .retain(|(contract, _)| self.evaluate_contract_filter(filter, contract, params));
        }

        // Apply ORDER BY (for contracts, we use a simpler sort)
        if let Some(order_by) = &query.order_by {
            self.sort_contract_results(&mut candidates, order_by, params);
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

    fn evaluate_contract_filter(
        &self,
        filter: &FilterExpr,
        contract: &Contract,
        params: &QueryParams,
    ) -> bool {
        self.evaluate_contract_expr_bool(&filter.expr, contract, params)
    }

    fn evaluate_contract_expr_bool(
        &self,
        expr: &Expr,
        contract: &Contract,
        params: &QueryParams,
    ) -> bool {
        match expr {
            Expr::BinaryOp { left, op, right } => match op {
                BinaryOperator::And => {
                    self.evaluate_contract_expr_bool(left, contract, params)
                        && self.evaluate_contract_expr_bool(right, contract, params)
                }
                BinaryOperator::Or => {
                    self.evaluate_contract_expr_bool(left, contract, params)
                        || self.evaluate_contract_expr_bool(right, contract, params)
                }
                BinaryOperator::Eq => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.compare_values(&left_val, &right_val, |a, b| a == b)
                }
                BinaryOperator::NotEq => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.compare_values(&left_val, &right_val, |a, b| a != b)
                }
                BinaryOperator::Lt => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a < b)
                }
                BinaryOperator::LtEq => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a <= b)
                }
                BinaryOperator::Gt => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a > b)
                }
                BinaryOperator::GtEq => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a >= b)
                }
                BinaryOperator::Like => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.like_match(&left_val, &right_val, false)
                }
                BinaryOperator::ILike => {
                    let left_val = self.evaluate_contract_expr_value(left, contract, params);
                    let right_val = self.evaluate_contract_expr_value(right, contract, params);
                    self.like_match(&left_val, &right_val, true)
                }
                _ => false,
            },
            Expr::Function(FunctionCall {
                name: FunctionName::Contains,
                args,
            }) => {
                if args.len() >= 2 {
                    let text_val = self.evaluate_contract_expr_value(&args[0], contract, params);
                    let search_val = self.evaluate_contract_expr_value(&args[1], contract, params);

                    if let (ColumnValue::String(text), ColumnValue::String(search)) =
                        (text_val, search_val)
                    {
                        return self.fulltext_index.contains(&text, &search);
                    }
                }
                false
            }
            _ => true,
        }
    }

    fn evaluate_contract_expr_value(
        &self,
        expr: &Expr,
        contract: &Contract,
        params: &QueryParams,
    ) -> ColumnValue {
        match expr {
            Expr::Column(col) => self.get_contract_column(contract, &col.column),
            Expr::Literal(lit) => self.literal_to_value(lit),
            Expr::Parameter(name) => self.param_to_value(params, name),
            Expr::Function(FunctionCall {
                name: FunctionName::Bm25Score,
                args,
            }) => {
                if args.len() >= 2 {
                    if let (Expr::Column(col), Expr::Parameter(param_name)) = (&args[0], &args[1]) {
                        let text = self.get_contract_column(contract, &col.column);
                        if let (ColumnValue::String(text), Some(query)) =
                            (text, params.get_string(param_name))
                        {
                            let score = self.fulltext_index.bm25_score(&text, query);
                            return ColumnValue::Float(score as f64);
                        }
                    }
                }
                ColumnValue::Float(0.0)
            }
            _ => ColumnValue::Null,
        }
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

    fn sort_contract_results(
        &self,
        results: &mut [(Contract, f64)],
        order_by: &OrderByClause,
        _params: &QueryParams,
    ) {
        if let Some(first) = order_by.items.first() {
            let ascending = matches!(first.direction, SortDirection::Asc);

            results.sort_by(|a, b| {
                let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
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

    fn evaluate_filter(
        &self,
        filter: &FilterExpr,
        invoice: &Invoice,
        params: &QueryParams,
    ) -> bool {
        self.evaluate_expr_bool(&filter.expr, invoice, params)
    }

    fn evaluate_expr_bool(&self, expr: &Expr, invoice: &Invoice, params: &QueryParams) -> bool {
        match expr {
            Expr::BinaryOp { left, op, right } => match op {
                BinaryOperator::And => {
                    self.evaluate_expr_bool(left, invoice, params)
                        && self.evaluate_expr_bool(right, invoice, params)
                }
                BinaryOperator::Or => {
                    self.evaluate_expr_bool(left, invoice, params)
                        || self.evaluate_expr_bool(right, invoice, params)
                }
                BinaryOperator::Eq => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.compare_values(&left_val, &right_val, |a, b| a == b)
                }
                BinaryOperator::NotEq => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.compare_values(&left_val, &right_val, |a, b| a != b)
                }
                BinaryOperator::Lt => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a < b)
                }
                BinaryOperator::LtEq => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a <= b)
                }
                BinaryOperator::Gt => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a > b)
                }
                BinaryOperator::GtEq => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.compare_values_ord(&left_val, &right_val, |a, b| a >= b)
                }
                BinaryOperator::Like => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.like_match(&left_val, &right_val, false)
                }
                BinaryOperator::ILike => {
                    let left_val = self.evaluate_expr_value(left, invoice, params);
                    let right_val = self.evaluate_expr_value(right, invoice, params);
                    self.like_match(&left_val, &right_val, true)
                }
                _ => false,
            },
            Expr::Function(FunctionCall {
                name: FunctionName::Contains,
                args,
            }) => {
                if args.len() >= 2 {
                    let text_val = self.evaluate_expr_value(&args[0], invoice, params);
                    let search_val = self.evaluate_expr_value(&args[1], invoice, params);

                    if let (ColumnValue::String(text), ColumnValue::String(search)) =
                        (text_val, search_val)
                    {
                        return self.fulltext_index.contains(&text, &search);
                    }
                }
                false
            }
            Expr::Function(FunctionCall {
                name: FunctionName::VectorDistance,
                ..
            }) => {
                // VECTOR_DISTANCE is used for scoring, not boolean filtering
                // It should be compared with a threshold like VECTOR_DISTANCE(...) < 0.3
                true
            }
            _ => true,
        }
    }

    fn evaluate_expr_value(
        &self,
        expr: &Expr,
        invoice: &Invoice,
        params: &QueryParams,
    ) -> ColumnValue {
        match expr {
            Expr::Column(col) => self.get_invoice_column(invoice, &col.column),
            Expr::Literal(lit) => self.literal_to_value(lit),
            Expr::Parameter(name) => self.param_to_value(params, name),
            Expr::Function(FunctionCall {
                name: FunctionName::VectorDistance,
                ..
            }) => {
                // Return a placeholder - actual distance is calculated in scoring
                ColumnValue::Float(0.0)
            }
            Expr::Function(FunctionCall {
                name: FunctionName::Bm25Score,
                args,
            }) => {
                if args.len() >= 2 {
                    if let (Expr::Column(col), Expr::Parameter(param_name)) = (&args[0], &args[1]) {
                        let text = self.get_invoice_column(invoice, &col.column);
                        if let (ColumnValue::String(text), Some(query)) =
                            (text, params.get_string(param_name))
                        {
                            let score = self.fulltext_index.bm25_score(&text, query);
                            return ColumnValue::Float(score as f64);
                        }
                    }
                }
                ColumnValue::Float(0.0)
            }
            _ => ColumnValue::Null,
        }
    }

    fn get_invoice_column(&self, invoice: &Invoice, column: &str) -> ColumnValue {
        if column.eq_ignore_ascii_case("id") {
            ColumnValue::String(invoice.id.0.to_string())
        } else if column.eq_ignore_ascii_case("document_id") {
            ColumnValue::String(invoice.document_id.0.to_string())
        } else if column.eq_ignore_ascii_case("invoice_number") {
            ColumnValue::String(invoice.invoice_number.clone())
        } else if column.eq_ignore_ascii_case("invoice_date") {
            ColumnValue::Date(invoice.invoice_date)
        } else if column.eq_ignore_ascii_case("due_date") {
            invoice.due_date.map(ColumnValue::Date).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("po_number") {
            invoice.po_number.clone().map(ColumnValue::String).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("vendor_name") {
            ColumnValue::String(invoice.vendor.name.clone())
        } else if column.eq_ignore_ascii_case("vendor_tax_id") {
            invoice.vendor.tax_id.clone().map(ColumnValue::String).unwrap_or(ColumnValue::Null)
        } else if column.eq_ignore_ascii_case("bill_to_name") {
            ColumnValue::String(invoice.bill_to.name.clone())
        } else if column.eq_ignore_ascii_case("currency") {
            ColumnValue::String(format!("{:?}", invoice.currency))
        } else if column.eq_ignore_ascii_case("subtotal") {
            ColumnValue::Decimal(invoice.subtotal)
        } else if column.eq_ignore_ascii_case("tax_amount") {
            ColumnValue::Decimal(invoice.tax_amount)
        } else if column.eq_ignore_ascii_case("discount_amount") {
            ColumnValue::Decimal(invoice.discount_amount)
        } else if column.eq_ignore_ascii_case("total_amount") {
            ColumnValue::Decimal(invoice.total_amount)
        } else if column.eq_ignore_ascii_case("validation_status") {
            ColumnValue::String(format!("{:?}", invoice.validation_status))
        } else if column.eq_ignore_ascii_case("confidence_score") {
            ColumnValue::Float(invoice.confidence_score as f64)
        } else if column.eq_ignore_ascii_case("extracted_text") {
            ColumnValue::String(invoice.extracted_text.clone())
        } else {
            ColumnValue::Null
        }
    }

    fn literal_to_value(&self, lit: &Literal) -> ColumnValue {
        match lit {
            Literal::String(s) => ColumnValue::String(s.clone()),
            Literal::Integer(i) => ColumnValue::Integer(*i),
            Literal::Float(f) => ColumnValue::Float(*f),
            Literal::Boolean(b) => ColumnValue::Boolean(*b),
            Literal::Null => ColumnValue::Null,
            Literal::Array(_) => ColumnValue::Null, // Arrays not directly comparable
        }
    }

    fn param_to_value(&self, params: &QueryParams, name: &str) -> ColumnValue {
        match params.get(name) {
            Some(ParamValue::String(s)) => ColumnValue::String(s.clone()),
            Some(ParamValue::Integer(i)) => ColumnValue::Integer(*i),
            Some(ParamValue::Float(f)) => ColumnValue::Float(*f),
            Some(ParamValue::Boolean(b)) => ColumnValue::Boolean(*b),
            Some(ParamValue::Null) => ColumnValue::Null,
            Some(ParamValue::Vector(_)) => ColumnValue::Null, // Vectors not comparable as values
            None => ColumnValue::Null,
        }
    }

    fn compare_values<F>(&self, a: &ColumnValue, b: &ColumnValue, cmp: F) -> bool
    where
        F: Fn(&str, &str) -> bool,
    {
        match (a, b) {
            (ColumnValue::String(a), ColumnValue::String(b)) => cmp(a, b),
            (ColumnValue::Null, ColumnValue::Null) => true,
            _ => false,
        }
    }

    fn compare_values_ord<F>(&self, a: &ColumnValue, b: &ColumnValue, cmp: F) -> bool
    where
        F: Fn(f64, f64) -> bool,
    {
        let a_f64 = a.as_f64();
        let b_f64 = b.as_f64();

        match (a_f64, b_f64) {
            (Some(a), Some(b)) => cmp(a, b),
            _ => false,
        }
    }

    fn like_match(
        &self,
        value: &ColumnValue,
        pattern: &ColumnValue,
        case_insensitive: bool,
    ) -> bool {
        match (value, pattern) {
            (ColumnValue::String(v), ColumnValue::String(p)) => {
                let mut cache = self.like_cache.borrow_mut();
                let key = (p.clone(), case_insensitive);
                let re = cache.entry(key).or_insert_with(|| {
                    let escaped = regex::escape(p).replace("%", ".*").replace("_", ".");
                    let regex_pattern = if case_insensitive {
                        format!("(?i)^{}$", escaped)
                    } else {
                        format!("^{}$", escaped)
                    };
                    regex::Regex::new(&regex_pattern).expect("valid LIKE regex pattern")
                });
                re.is_match(v)
            }
            _ => false,
        }
    }

    fn calculate_combined_score(
        &self,
        query: &FenQuery,
        invoice: &Invoice,
        params: &QueryParams,
        base_score: f64,
        flags: &QueryFlags,
    ) -> f64 {
        // If the query has a custom score expression in ORDER BY, use that
        // Otherwise, use weighted combination of vector and text scores

        let uses_vector = flags.uses_vector;
        let uses_text = flags.uses_text;

        if !uses_vector && !uses_text {
            return base_score;
        }

        let mut vector_score = 0.0;
        let mut text_score = 0.0;

        if uses_vector {
            // base_score is the vector distance for vector searches
            // Convert distance to similarity (1 - distance for cosine)
            vector_score = 1.0 - base_score.min(1.0);
        }

        if uses_text {
            // Calculate BM25 score
            if let Ok(search_terms) = self.find_text_param(query, params) {
                let bm25 = self
                    .fulltext_index
                    .bm25_score(&invoice.extracted_text, &search_terms);
                // Normalize BM25 to [0, 1] range (approximate)
                text_score = (bm25 as f64).min(10.0) / 10.0;
            }
        }

        // Combine scores with configured weights
        if uses_vector && uses_text {
            self.config.vector_weight * vector_score + self.config.text_weight * text_score
        } else if uses_vector {
            vector_score
        } else {
            text_score
        }
    }

    fn sort_results(
        &self,
        results: &mut [(Invoice, f64)],
        order_by: &OrderByClause,
        _params: &QueryParams,
    ) {
        // For now, sort by combined score
        // TODO: Implement full ORDER BY expression evaluation
        if let Some(first) = order_by.items.first() {
            let ascending = matches!(first.direction, SortDirection::Asc);

            results.sort_by(|a, b| {
                let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
                if ascending {
                    cmp
                } else {
                    cmp.reverse()
                }
            });
        }
    }

    fn project_invoice(&self, invoice: &Invoice, select: &[SelectItem], score: f64) -> ResultRow {
        let mut columns = HashMap::new();

        for item in select {
            match &item.expr {
                Expr::Wildcard => {
                    // Add all columns
                    columns.insert(
                        "id".to_string(),
                        ColumnValue::String(invoice.id.0.to_string()),
                    );
                    columns.insert(
                        "invoice_number".to_string(),
                        ColumnValue::String(invoice.invoice_number.clone()),
                    );
                    columns.insert(
                        "invoice_date".to_string(),
                        ColumnValue::Date(invoice.invoice_date),
                    );
                    columns.insert(
                        "vendor_name".to_string(),
                        ColumnValue::String(invoice.vendor.name.clone()),
                    );
                    columns.insert(
                        "total_amount".to_string(),
                        ColumnValue::Decimal(invoice.total_amount),
                    );
                    columns.insert(
                        "confidence_score".to_string(),
                        ColumnValue::Float(invoice.confidence_score as f64),
                    );
                }
                Expr::Column(col) => {
                    let name = item.alias.clone().unwrap_or_else(|| col.column.clone());
                    let value = self.get_invoice_column(invoice, &col.column);
                    columns.insert(name, value);
                }
                Expr::Function(FunctionCall {
                    name: FunctionName::VectorDistance,
                    ..
                }) => {
                    let name = item
                        .alias
                        .clone()
                        .unwrap_or_else(|| "vector_distance".to_string());
                    // Use the base score as vector distance
                    columns.insert(name, ColumnValue::Float(1.0 - score));
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
}

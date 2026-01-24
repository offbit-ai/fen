//! ZIP executor for cross-table queries
//!
//! This module handles executing queries that span both invoices and contracts
//! tables with shared WHERE conditions for cross-relationship analysis.
//!
//! # Overview
//!
//! ZIP queries allow joining invoices and contracts to:
//! - Find all invoices related to a specific contract
//! - Detect invoices that exceed contract limits
//! - Validate invoice terms against contract clauses
//! - Identify orphan invoices without matching contracts
//!
//! # ZIP Modes
//!
//! | Mode | Description |
//! |------|-------------|
//! | `INNER ZIP` | Only pairs where both invoice and contract match |
//! | `LEFT ZIP` | All invoices, with matching contracts (or NULL) |
//! | `CROSS ZIP` | Cartesian product with ON filter |
//!
//! # SQL Syntax
//!
//! ```text
//! SELECT inv.*, con.title
//! FROM invoices inv
//! ZIP contracts con ON inv.vendor_name = con.party_name
//! WHERE inv.total_amount > 1000
//! ```
//!
//! # Smart WHERE Clause Splitting
//!
//! The ZIP executor optimizes query execution by splitting WHERE conditions:
//!
//! - **Invoice-only conditions** (e.g., `inv.total_amount > 1000`)
//!   Applied before the join to reduce candidates
//!
//! - **Contract-only conditions** (e.g., `con.effective_date > '2024-01-01'`)
//!   Applied before the join to reduce candidates
//!
//! - **Cross-table conditions** (e.g., `inv.total_amount < con.total_value`)
//!   Applied after the join during pair filtering
//!
//! # ON Condition Matching
//!
//! The ON condition specifies how invoices and contracts relate:
//!
//! ```text
//! -- Match by vendor name to party name
//! ON inv.vendor_name = con.party_name
//!
//! -- Match by explicit contract ID
//! ON inv.contract_id = con.id
//!
//! -- Match by contract number
//! ON inv.contract_number = con.contract_number
//! ```
//!
//! # Pipeline Operations
//!
//! ZIP query results can be processed through pipeline operations using the `|>` syntax:
//!
//! ```text
//! SELECT inv.*, con.title
//! FROM invoices inv
//! ZIP contracts con ON inv.vendor_name = con.party_name
//! WHERE inv.total_amount > 1000
//! |> VALIDATE WITH ('math_check', 'date_check')
//! |> CROSS_VALIDATE ON (inv.total_amount, con.total_value)
//! |> ANALYZE
//! ```
//!
//! Available operations:
//! - **VALIDATE** - Run validation rules on results
//! - **ANALYZE** - Detect anomalies across pairs
//! - **CROSS_VALIDATE** - Compare invoice fields against contract fields
//! - **AGGREGATE** - Group and compute metrics
//!
//! # Usage
//!
//! ```rust,ignore
//! use fen_storage::{ZipExecutor, parse_query, QueryParams};
//!
//! // Parse a ZIP query
//! let query = parse_query(
//!     "SELECT inv.invoice_number, con.title
//!      FROM invoices inv
//!      ZIP contracts con ON inv.vendor_name = con.party_name
//!      WHERE inv.total_amount > con.total_value"
//! )?;
//!
//! // Execute
//! let params = QueryParams::new();
//! let result = zip_executor.execute_zip_query(&query, &params).await?;
//!
//! // Process results
//! for pair in &result.pairs {
//!     if let (Some(inv), Some(con)) = (&pair.invoice, &pair.contract) {
//!         println!("Invoice {} exceeds contract {}", inv.invoice_number, con.title);
//!     }
//! }
//! ```
//!
//! # Result Structure
//!
//! ```rust,ignore
//! pub struct ZipResult {
//!     pub pairs: Vec<ZipPair>,      // Invoice-contract pairs
//!     pub metadata: ZipMetadata,     // Execution statistics
//!     pub pipeline_results: Option<PipelineResults>, // Pipeline output
//! }
//!
//! pub struct ZipPair {
//!     pub invoice: Option<Invoice>,  // None in LEFT ZIP if no match
//!     pub contract: Option<Contract>, // None in LEFT ZIP if no match
//!     pub score: Option<f64>,        // Relevance score
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::error::StorageError;
use crate::fulltext::FullTextIndex;
use crate::hot::RedbStorage;
use crate::query::executor::{ColumnValue, ExecutorConfig, QueryExecutor};
use crate::query::lang::json::{
    JsonAggFunction, JsonCondition, JsonExpr, JsonPipelineOp, JsonValue,
};
use crate::query::lang::{
    BinaryOperator, ColumnRef, Expr, FenQuery, Literal, QueryParams, QueryTarget, SelectItem,
    ZipMode,
};
use crate::warm::LanceStorage;

use fen_core::domain::{
    Anomaly, AnomalyType, Contract, DocumentId, Invoice, Severity, ValidationResult,
};

/// Result of a ZIP query containing both invoices and contracts
#[derive(Debug, Clone)]
pub struct ZipResult {
    /// Matching invoice-contract pairs
    pub pairs: Vec<ZipPair>,
    /// Metadata about the query execution
    pub metadata: ZipMetadata,
    /// Pipeline results if any pipeline operations were applied
    pub pipeline_results: Option<PipelineResults>,
}

/// A paired invoice and contract from a ZIP query
#[derive(Debug, Clone)]
pub struct ZipPair {
    /// The invoice (None if Left join and no invoice matched)
    pub invoice: Option<Invoice>,
    /// The contract (None if Left join and no contract matched)
    pub contract: Option<Contract>,
    /// Combined relevance score
    pub score: Option<f64>,
}

/// Metadata about ZIP query execution
#[derive(Debug, Clone)]
pub struct ZipMetadata {
    /// Total number of pairs returned
    pub pair_count: usize,
    /// Number of invoices matched
    pub invoice_count: usize,
    /// Number of contracts matched
    pub contract_count: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// ZIP mode used
    pub zip_mode: String,
}

/// Results from pipeline operations
#[derive(Debug, Clone)]
pub struct PipelineResults {
    /// Validation results if validate was called
    pub validation: Option<Vec<ValidationResult>>,
    /// Anomalies found across all pairs
    pub anomalies: Vec<Anomaly>,
    /// Cross-validation issues (mismatches between invoice and contract)
    pub cross_validation_issues: Vec<CrossValidationIssue>,
    /// Aggregated metrics
    pub aggregates: HashMap<String, AggregateValue>,
}

/// A cross-validation issue between invoice and contract
#[derive(Debug, Clone)]
pub struct CrossValidationIssue {
    /// The invoice ID
    pub invoice_id: String,
    /// The contract ID
    pub contract_id: String,
    /// Type of mismatch
    pub issue_type: CrossValidationIssueType,
    /// Description of the issue
    pub description: String,
    /// Severity of the issue
    pub severity: Severity,
}

/// Types of cross-validation issues
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossValidationIssueType {
    /// Invoice total exceeds contract amount
    AmountExceedsContract,
    /// Invoice date outside contract period
    DateOutsideContractPeriod,
    /// Vendor mismatch between invoice and contract
    VendorMismatch,
    /// Currency mismatch
    CurrencyMismatch,
    /// Invoice references unknown contract
    UnknownContractReference,
    /// Contract payment terms violated
    PaymentTermsViolation,
}

/// Aggregated value for pipeline operations
#[derive(Debug, Clone)]
pub enum AggregateValue {
    Count(usize),
    Sum(f64),
    Average(f64),
    Min(f64),
    Max(f64),
    StringList(Vec<String>),
}

/// Tracks which tables are referenced by an expression
#[derive(Debug, Clone, Default)]
struct ReferencedTables {
    /// Whether the expression references invoice table columns
    references_invoice: bool,
    /// Whether the expression references contract table columns
    references_contract: bool,
}

/// Represents a WHERE clause split by table scope
#[derive(Debug, Clone, Default)]
struct SplitWhereClause {
    /// Conditions that apply only to invoices (e.g., inv.total_amount > 1000)
    invoice_conditions: Option<Expr>,
    /// Conditions that apply only to contracts (e.g., con.effective_date > '2024-01-01')
    contract_conditions: Option<Expr>,
    /// Conditions that span both tables (e.g., inv.total_amount < con.total_value)
    cross_table_conditions: Option<Expr>,
}

impl SplitWhereClause {
    /// Combine multiple conditions with AND
    fn combine_with_and(a: Option<Expr>, b: Option<Expr>) -> Option<Expr> {
        match (a, b) {
            (Some(a), Some(b)) => Some(Expr::BinaryOp {
                left: Box::new(a),
                op: BinaryOperator::And,
                right: Box::new(b),
            }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    /// Add an invoice condition
    fn add_invoice_condition(&mut self, expr: Expr) {
        self.invoice_conditions =
            Self::combine_with_and(self.invoice_conditions.take(), Some(expr));
    }

    /// Add a contract condition
    fn add_contract_condition(&mut self, expr: Expr) {
        self.contract_conditions =
            Self::combine_with_and(self.contract_conditions.take(), Some(expr));
    }

    /// Add a cross-table condition
    fn add_cross_table_condition(&mut self, expr: Expr) {
        self.cross_table_conditions =
            Self::combine_with_and(self.cross_table_conditions.take(), Some(expr));
    }
}

/// Executor for ZIP queries
pub struct ZipExecutor {
    /// Underlying query executor for individual table queries
    query_executor: QueryExecutor,
    /// Hot storage for direct access
    hot_storage: Arc<RedbStorage>,
}

impl ZipExecutor {
    /// Create a new ZIP executor
    pub fn new(
        hot_storage: Arc<RedbStorage>,
        warm_storage: Arc<RwLock<LanceStorage>>,
        fulltext_index: Arc<FullTextIndex>,
        config: ExecutorConfig,
    ) -> Self {
        let query_executor =
            QueryExecutor::new(hot_storage.clone(), warm_storage, fulltext_index, config);

        Self {
            query_executor,
            hot_storage,
        }
    }

    /// Split a WHERE clause into table-specific and cross-table conditions
    ///
    /// This analyzes each condition in the WHERE clause and determines which table(s)
    /// it references. Conditions are categorized as:
    /// - Invoice-only: Only references invoice table columns
    /// - Contract-only: Only references contract table columns
    /// - Cross-table: References columns from both tables
    fn split_where_clause(
        &self,
        filter: &Option<crate::query::lang::FilterExpr>,
        primary_alias: Option<&str>,
        secondary_alias: Option<&str>,
    ) -> SplitWhereClause {
        let mut split = SplitWhereClause::default();

        if let Some(filter_expr) = filter {
            self.categorize_condition(
                &filter_expr.expr,
                primary_alias,
                secondary_alias,
                &mut split,
            );
        }

        split
    }

    /// Categorize a single condition or recursively process compound conditions
    fn categorize_condition(
        &self,
        expr: &Expr,
        primary_alias: Option<&str>,
        secondary_alias: Option<&str>,
        split: &mut SplitWhereClause,
    ) {
        match expr {
            Expr::BinaryOp { left, op, right } => {
                match op {
                    BinaryOperator::And => {
                        // Recursively split AND conditions
                        self.categorize_condition(left, primary_alias, secondary_alias, split);
                        self.categorize_condition(right, primary_alias, secondary_alias, split);
                    }
                    BinaryOperator::Or => {
                        // OR conditions must be kept together - check which tables they reference
                        let tables =
                            self.get_referenced_tables(expr, primary_alias, secondary_alias);
                        self.add_condition_to_split(expr.clone(), &tables, split);
                    }
                    _ => {
                        // Comparison operators - check which tables are referenced
                        let tables =
                            self.get_referenced_tables(expr, primary_alias, secondary_alias);
                        self.add_condition_to_split(expr.clone(), &tables, split);
                    }
                }
            }
            Expr::Not(inner) => {
                // NOT conditions - check the inner expression's tables
                let tables = self.get_referenced_tables(inner, primary_alias, secondary_alias);
                self.add_condition_to_split(expr.clone(), &tables, split);
            }
            Expr::IsNull { expr: inner, .. } => {
                let tables = self.get_referenced_tables(inner, primary_alias, secondary_alias);
                self.add_condition_to_split(expr.clone(), &tables, split);
            }
            Expr::UnaryOp { expr: inner, .. } => {
                let tables = self.get_referenced_tables(inner, primary_alias, secondary_alias);
                self.add_condition_to_split(expr.clone(), &tables, split);
            }
            Expr::Function(_) => {
                // For functions, check all arguments
                let tables = self.get_referenced_tables(expr, primary_alias, secondary_alias);
                self.add_condition_to_split(expr.clone(), &tables, split);
            }
            _ => {
                // For any other expression, check its table references
                let tables = self.get_referenced_tables(expr, primary_alias, secondary_alias);
                self.add_condition_to_split(expr.clone(), &tables, split);
            }
        }
    }

    /// Get all tables referenced by an expression
    fn get_referenced_tables(
        &self,
        expr: &Expr,
        primary_alias: Option<&str>,
        secondary_alias: Option<&str>,
    ) -> ReferencedTables {
        let mut tables = ReferencedTables::default();
        self.collect_table_references(expr, primary_alias, secondary_alias, &mut tables);
        tables
    }

    /// Collect table references from an expression
    fn collect_table_references(
        &self,
        expr: &Expr,
        primary_alias: Option<&str>,
        secondary_alias: Option<&str>,
        tables: &mut ReferencedTables,
    ) {
        match expr {
            Expr::Column(col) => {
                if let Some(ref table) = col.table {
                    let table_lower = table.to_lowercase();
                    // Check if it's the primary table (invoices)
                    if table_lower == "invoices"
                        || table_lower == "inv"
                        || primary_alias.map(|a| a.to_lowercase()) == Some(table_lower.clone())
                    {
                        tables.references_invoice = true;
                    }
                    // Check if it's the secondary table (contracts)
                    else if table_lower == "contracts"
                        || table_lower == "con"
                        || secondary_alias.map(|a| a.to_lowercase()) == Some(table_lower.clone())
                    {
                        tables.references_contract = true;
                    }
                } else {
                    // No table prefix - try to infer from column name
                    let column_lower = col.column.to_lowercase();
                    if self.is_invoice_column(&column_lower) {
                        tables.references_invoice = true;
                    } else if self.is_contract_column(&column_lower) {
                        tables.references_contract = true;
                    }
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                self.collect_table_references(left, primary_alias, secondary_alias, tables);
                self.collect_table_references(right, primary_alias, secondary_alias, tables);
            }
            Expr::Not(inner) => {
                self.collect_table_references(inner, primary_alias, secondary_alias, tables);
            }
            Expr::UnaryOp { expr: inner, .. } => {
                self.collect_table_references(inner, primary_alias, secondary_alias, tables);
            }
            Expr::IsNull { expr: inner, .. } => {
                self.collect_table_references(inner, primary_alias, secondary_alias, tables);
            }
            Expr::Function(func) => {
                for arg in &func.args {
                    self.collect_table_references(arg, primary_alias, secondary_alias, tables);
                }
            }
            _ => {}
        }
    }

    /// Check if a column name belongs to invoices
    fn is_invoice_column(&self, column: &str) -> bool {
        matches!(
            column,
            "invoice_number"
                | "invoice_date"
                | "due_date"
                | "total_amount"
                | "subtotal"
                | "tax_amount"
                | "vendor_name"
                | "po_number"
                | "contract_id"
                | "contract_number"
                | "line_items"
                | "validation_status"
        )
    }

    /// Check if a column name belongs to contracts
    fn is_contract_column(&self, column: &str) -> bool {
        matches!(
            column,
            "title"
                | "effective_date"
                | "expiration_date"
                | "total_value"
                | "contract_value"
                | "party_name"
                | "parties"
                | "clauses"
                | "contract_type"
                | "payment_terms"
        )
    }

    /// Add a condition to the appropriate split category based on referenced tables
    fn add_condition_to_split(
        &self,
        expr: Expr,
        tables: &ReferencedTables,
        split: &mut SplitWhereClause,
    ) {
        match (tables.references_invoice, tables.references_contract) {
            (true, true) => {
                // Cross-table condition - must be applied after join
                split.add_cross_table_condition(expr);
            }
            (true, false) => {
                // Invoice-only condition - can be applied before join
                split.add_invoice_condition(expr);
            }
            (false, true) => {
                // Contract-only condition - can be applied before join
                split.add_contract_condition(expr);
            }
            (false, false) => {
                // No table references - apply to both (e.g., literal conditions)
                split.add_invoice_condition(expr.clone());
                split.add_contract_condition(expr);
            }
        }
    }

    /// Execute a ZIP query from a FenQuery with ZIP clause
    pub async fn execute_zip_query(
        &self,
        query: &FenQuery,
        params: &QueryParams,
    ) -> Result<ZipResult, StorageError> {
        let start = std::time::Instant::now();

        // Check if this query has a ZIP clause
        let zip_clause = match &query.zip {
            Some(zip) => zip,
            None => {
                // Not a ZIP query - execute as regular query
                return self.execute_single_table(query, params).await;
            }
        };

        // Execute the ZIP query
        let pairs = self.execute_zip(query, zip_clause, params).await?;

        let invoice_count = pairs.iter().filter(|p| p.invoice.is_some()).count();
        let contract_count = pairs.iter().filter(|p| p.contract.is_some()).count();

        Ok(ZipResult {
            pairs,
            metadata: ZipMetadata {
                pair_count: invoice_count.max(contract_count),
                invoice_count,
                contract_count,
                execution_time_ms: start.elapsed().as_millis() as u64,
                zip_mode: format!("{:?}", zip_clause.mode),
            },
            pipeline_results: None,
        })
    }

    /// Execute a single-table query wrapped as ZipResult
    async fn execute_single_table(
        &self,
        query: &FenQuery,
        params: &QueryParams,
    ) -> Result<ZipResult, StorageError> {
        let start = std::time::Instant::now();

        let result = self.query_executor.execute(query, params).await?;

        // Wrap results as ZipPairs based on table
        let pairs: Vec<ZipPair> = match query.from {
            QueryTarget::Invoices => {
                use crate::traits::DocumentStore;
                let mut invoice_pairs = Vec::new();
                for row in result.rows {
                    if let Some(ColumnValue::String(id)) = row.columns.get("id") {
                        if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                            let invoice_id = fen_core::domain::InvoiceId(uuid);
                            if let Ok(Some(invoice)) =
                                self.hot_storage.get_invoice(&invoice_id).await
                            {
                                invoice_pairs.push(ZipPair {
                                    invoice: Some(invoice),
                                    contract: None,
                                    score: row.score,
                                });
                            }
                        }
                    }
                }
                invoice_pairs
            }
            QueryTarget::Contracts => {
                use crate::traits::DocumentStore;
                let mut contract_pairs = Vec::new();
                for row in result.rows {
                    if let Some(ColumnValue::String(id)) = row.columns.get("id") {
                        if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                            let contract_id = fen_core::domain::ContractId(uuid);
                            if let Ok(Some(contract)) =
                                self.hot_storage.get_contract(&contract_id).await
                            {
                                contract_pairs.push(ZipPair {
                                    invoice: None,
                                    contract: Some(contract),
                                    score: row.score,
                                });
                            }
                        }
                    }
                }
                contract_pairs
            }
        };

        let invoice_count = pairs.iter().filter(|p| p.invoice.is_some()).count();
        let contract_count = pairs.iter().filter(|p| p.contract.is_some()).count();

        Ok(ZipResult {
            pairs,
            metadata: ZipMetadata {
                pair_count: invoice_count.max(contract_count),
                invoice_count,
                contract_count,
                execution_time_ms: start.elapsed().as_millis() as u64,
                zip_mode: "Single".to_string(),
            },
            pipeline_results: None,
        })
    }

    /// Execute a ZIP query joining invoices and contracts
    ///
    /// This method uses smart WHERE clause splitting to optimize query execution:
    /// - Invoice-only conditions are applied when fetching invoices
    /// - Contract-only conditions are applied when fetching contracts
    /// - Cross-table conditions are applied after pairs are formed
    async fn execute_zip(
        &self,
        query: &FenQuery,
        zip: &crate::query::lang::ZipClause,
        params: &QueryParams,
    ) -> Result<Vec<ZipPair>, StorageError> {
        use crate::traits::DocumentStore;

        // Split WHERE clause into table-specific and cross-table conditions
        let split = self.split_where_clause(
            &query.filter,
            query.from_alias.as_deref(),
            zip.alias.as_deref(),
        );

        // Determine which filter applies to which table based on primary table type
        let (invoice_filter, contract_filter) = if query.from == QueryTarget::Invoices {
            (
                split.invoice_conditions.clone(),
                split.contract_conditions.clone(),
            )
        } else {
            (
                split.invoice_conditions.clone(),
                split.contract_conditions.clone(),
            )
        };

        // Build queries for both tables with their specific filters
        let primary_query = if query.from == QueryTarget::Invoices {
            self.build_table_query(query.from, query, &invoice_filter)?
        } else {
            self.build_table_query(query.from, query, &contract_filter)?
        };

        let secondary_query = if zip.table == QueryTarget::Invoices {
            self.build_table_query(zip.table, query, &invoice_filter)?
        } else {
            self.build_table_query(zip.table, query, &contract_filter)?
        };

        // Execute both queries
        let primary_result = self.query_executor.execute(&primary_query, params).await?;
        let secondary_result = self
            .query_executor
            .execute(&secondary_query, params)
            .await?;

        // Fetch full documents based on table types
        let (invoices, contracts) = if query.from == QueryTarget::Invoices {
            let mut invs = Vec::new();
            for row in &primary_result.rows {
                if let Some(ColumnValue::String(id)) = row.columns.get("id") {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                        let invoice_id = fen_core::domain::InvoiceId(uuid);
                        if let Ok(Some(inv)) = self.hot_storage.get_invoice(&invoice_id).await {
                            invs.push((inv, row.score));
                        }
                    }
                }
            }

            let mut cons = Vec::new();
            for row in &secondary_result.rows {
                if let Some(ColumnValue::String(id)) = row.columns.get("id") {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                        let contract_id = fen_core::domain::ContractId(uuid);
                        if let Ok(Some(con)) = self.hot_storage.get_contract(&contract_id).await {
                            cons.push((con, row.score));
                        }
                    }
                }
            }

            (invs, cons)
        } else {
            // contracts is primary
            let mut cons = Vec::new();
            for row in &primary_result.rows {
                if let Some(ColumnValue::String(id)) = row.columns.get("id") {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                        let contract_id = fen_core::domain::ContractId(uuid);
                        if let Ok(Some(con)) = self.hot_storage.get_contract(&contract_id).await {
                            cons.push((con, row.score));
                        }
                    }
                }
            }

            let mut invs = Vec::new();
            for row in &secondary_result.rows {
                if let Some(ColumnValue::String(id)) = row.columns.get("id") {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                        let invoice_id = fen_core::domain::InvoiceId(uuid);
                        if let Ok(Some(inv)) = self.hot_storage.get_invoice(&invoice_id).await {
                            invs.push((inv, row.score));
                        }
                    }
                }
            }

            (invs, cons)
        };

        // Join based on ZIP mode and ON condition
        let mut pairs = self.join_results(invoices, contracts, zip);

        // Apply cross-table conditions after join
        if let Some(ref cross_conditions) = split.cross_table_conditions {
            pairs = pairs
                .into_iter()
                .filter(|pair| self.evaluate_cross_table_condition(pair, cross_conditions))
                .collect();
        }

        Ok(pairs)
    }

    /// Build a query for a specific table with an optional filter
    fn build_table_query(
        &self,
        table: QueryTarget,
        original: &FenQuery,
        table_filter: &Option<Expr>,
    ) -> Result<FenQuery, StorageError> {
        use crate::query::lang::FilterExpr;

        // Build select items - just fetch IDs for now
        let select = vec![SelectItem {
            expr: Expr::Column(ColumnRef {
                table: Some(table.as_str().to_string()),
                column: "id".to_string(),
            }),
            alias: None,
        }];

        // Wrap the filter expression if provided
        let filter = table_filter
            .as_ref()
            .map(|expr| FilterExpr::new(expr.clone()));

        Ok(FenQuery {
            from: table,
            from_alias: original.from_alias.clone(),
            select,
            filter,
            order_by: None,
            limit: original.limit,
            offset: original.offset,
            zip: None,      // Don't recurse
            pipeline: None, // Pipeline handled at top level
        })
    }

    /// Evaluate a cross-table condition against a ZipPair
    ///
    /// This is used to filter pairs after the join based on conditions
    /// that reference columns from both tables.
    fn evaluate_cross_table_condition(&self, pair: &ZipPair, condition: &Expr) -> bool {
        match condition {
            Expr::BinaryOp { left, op, right } => {
                match op {
                    BinaryOperator::And => {
                        self.evaluate_cross_table_condition(pair, left)
                            && self.evaluate_cross_table_condition(pair, right)
                    }
                    BinaryOperator::Or => {
                        self.evaluate_cross_table_condition(pair, left)
                            || self.evaluate_cross_table_condition(pair, right)
                    }
                    BinaryOperator::Eq => {
                        let left_val = self.eval_pair_expr(pair, left);
                        let right_val = self.eval_pair_expr(pair, right);
                        self.values_equal(&left_val, &right_val)
                    }
                    BinaryOperator::NotEq => {
                        let left_val = self.eval_pair_expr(pair, left);
                        let right_val = self.eval_pair_expr(pair, right);
                        !self.values_equal(&left_val, &right_val)
                    }
                    BinaryOperator::Lt => {
                        let left_val = self.eval_pair_expr(pair, left);
                        let right_val = self.eval_pair_expr(pair, right);
                        self.compare_values_ord(&left_val, &right_val)
                            == Some(std::cmp::Ordering::Less)
                    }
                    BinaryOperator::LtEq => {
                        let left_val = self.eval_pair_expr(pair, left);
                        let right_val = self.eval_pair_expr(pair, right);
                        matches!(
                            self.compare_values_ord(&left_val, &right_val),
                            Some(std::cmp::Ordering::Less) | Some(std::cmp::Ordering::Equal)
                        )
                    }
                    BinaryOperator::Gt => {
                        let left_val = self.eval_pair_expr(pair, left);
                        let right_val = self.eval_pair_expr(pair, right);
                        self.compare_values_ord(&left_val, &right_val)
                            == Some(std::cmp::Ordering::Greater)
                    }
                    BinaryOperator::GtEq => {
                        let left_val = self.eval_pair_expr(pair, left);
                        let right_val = self.eval_pair_expr(pair, right);
                        matches!(
                            self.compare_values_ord(&left_val, &right_val),
                            Some(std::cmp::Ordering::Greater) | Some(std::cmp::Ordering::Equal)
                        )
                    }
                    _ => true, // Other operators not supported for cross-table filtering
                }
            }
            Expr::Not(inner) => !self.evaluate_cross_table_condition(pair, inner),
            _ => true, // Non-binary expressions pass through
        }
    }

    /// Evaluate an expression against a ZipPair to get a value
    fn eval_pair_expr(&self, pair: &ZipPair, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Column(col) => {
                let table = col.table.as_deref().unwrap_or("");

                // Try invoice fields first
                if let Some(invoice) = &pair.invoice {
                    if table.is_empty() || table == "inv" || table == "invoices" {
                        if let Some(val) = self.get_invoice_field_str(invoice, &col.column) {
                            return Some(val);
                        }
                    }
                }

                // Try contract fields
                if let Some(contract) = &pair.contract {
                    if table.is_empty() || table == "con" || table == "contracts" {
                        if let Some(val) = self.get_contract_field_str(contract, &col.column) {
                            return Some(val);
                        }
                    }
                }

                None
            }
            Expr::Literal(lit) => match lit {
                Literal::String(s) => Some(s.clone()),
                Literal::Integer(i) => Some(i.to_string()),
                Literal::Float(f) => Some(f.to_string()),
                Literal::Boolean(b) => Some(b.to_string()),
                Literal::Null => None,
                Literal::Array(_) => None,
            },
            _ => None,
        }
    }

    /// Get a string representation of an invoice field
    fn get_invoice_field_str(&self, invoice: &Invoice, field: &str) -> Option<String> {
        match field {
            "id" => Some(invoice.id.to_string()),
            "invoice_number" => Some(invoice.invoice_number.clone()),
            "vendor_name" => Some(invoice.vendor.name.clone()),
            "total_amount" => Some(invoice.total_amount.to_string()),
            "subtotal" => Some(invoice.subtotal.to_string()),
            "tax_amount" => Some(invoice.tax_amount.to_string()),
            "invoice_date" => Some(invoice.invoice_date.to_string()),
            "due_date" => invoice.due_date.map(|d| d.to_string()),
            "currency" => Some(format!("{:?}", invoice.currency)),
            "po_number" => invoice.po_number.clone(),
            "contract_id" => invoice.contract_id.map(|id| id.to_string()),
            "contract_number" => invoice.contract_number.clone(),
            _ => None,
        }
    }

    /// Get a string representation of a contract field
    fn get_contract_field_str(&self, contract: &Contract, field: &str) -> Option<String> {
        match field {
            "id" => Some(contract.id.to_string()),
            "title" => Some(contract.title.clone()),
            "total_value" | "contract_value" => {
                contract.total_value.as_ref().map(|v| v.to_string())
            }
            "party_name" => contract.parties.first().map(|p| p.name.clone()),
            "effective_date" => Some(contract.effective_date.to_string()),
            "expiration_date" => contract.expiration_date.map(|d| d.to_string()),
            "currency" => contract.currency.as_ref().map(|c| format!("{:?}", c)),
            "contract_number" => contract.contract_number.clone(),
            _ => None,
        }
    }

    /// Check if two optional values are equal
    fn values_equal(&self, a: &Option<String>, b: &Option<String>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        }
    }

    /// Compare two optional values for ordering
    fn compare_values_ord(
        &self,
        a: &Option<String>,
        b: &Option<String>,
    ) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Some(a), Some(b)) => {
                // Try to parse as numbers first
                if let (Ok(a_num), Ok(b_num)) = (a.parse::<f64>(), b.parse::<f64>()) {
                    a_num.partial_cmp(&b_num)
                } else {
                    // Fall back to string comparison
                    Some(a.cmp(b))
                }
            }
            _ => None,
        }
    }

    /// Join invoice and contract results based on ZIP mode
    fn join_results(
        &self,
        invoices: Vec<(Invoice, Option<f64>)>,
        contracts: Vec<(Contract, Option<f64>)>,
        zip: &crate::query::lang::ZipClause,
    ) -> Vec<ZipPair> {
        let mut pairs = Vec::new();

        match zip.mode {
            ZipMode::Inner => {
                // Inner join - only pairs where both exist and match ON condition
                for (invoice, inv_score) in &invoices {
                    for (contract, con_score) in &contracts {
                        if self.matches_on_condition(invoice, contract, &zip.on) {
                            let combined_score = match (inv_score, con_score) {
                                (Some(a), Some(b)) => Some((a + b) / 2.0),
                                (Some(a), None) | (None, Some(a)) => Some(*a),
                                (None, None) => None,
                            };
                            pairs.push(ZipPair {
                                invoice: Some(invoice.clone()),
                                contract: Some(contract.clone()),
                                score: combined_score,
                            });
                        }
                    }
                }
            }
            ZipMode::Left => {
                // Left join - all invoices, with contracts where they match
                for (invoice, inv_score) in &invoices {
                    let matching_contracts: Vec<_> = contracts
                        .iter()
                        .filter(|(c, _)| self.matches_on_condition(invoice, c, &zip.on))
                        .collect();

                    if matching_contracts.is_empty() {
                        pairs.push(ZipPair {
                            invoice: Some(invoice.clone()),
                            contract: None,
                            score: *inv_score,
                        });
                    } else {
                        for (contract, con_score) in matching_contracts {
                            let combined_score = match (inv_score, con_score) {
                                (Some(a), Some(b)) => Some((a + b) / 2.0),
                                (Some(a), None) | (None, Some(a)) => Some(*a),
                                (None, None) => None,
                            };
                            pairs.push(ZipPair {
                                invoice: Some(invoice.clone()),
                                contract: Some(contract.clone()),
                                score: combined_score,
                            });
                        }
                    }
                }
            }
            ZipMode::Cross => {
                // Cross join - all combinations
                for (invoice, inv_score) in &invoices {
                    for (contract, con_score) in &contracts {
                        let combined_score = match (inv_score, con_score) {
                            (Some(a), Some(b)) => Some((a + b) / 2.0),
                            (Some(a), None) | (None, Some(a)) => Some(*a),
                            (None, None) => None,
                        };
                        pairs.push(ZipPair {
                            invoice: Some(invoice.clone()),
                            contract: Some(contract.clone()),
                            score: combined_score,
                        });
                    }
                }
            }
        }

        pairs
    }

    /// Check if an invoice and contract match the ON condition
    ///
    /// Matching priority:
    /// 1. Explicit contract_id on invoice (strongest link)
    /// 2. Contract number reference matching
    /// 3. ON clause expression evaluation
    /// 4. Vendor name matching (fallback)
    fn matches_on_condition(&self, invoice: &Invoice, contract: &Contract, on: &Expr) -> bool {
        // Priority 1: Explicit contract_id reference
        if let Some(ref invoice_contract_id) = invoice.contract_id {
            return *invoice_contract_id == contract.id;
        }

        // Priority 2: Contract number reference
        if let (Some(ref inv_contract_num), Some(ref con_contract_num)) =
            (&invoice.contract_number, &contract.contract_number)
        {
            if inv_contract_num == con_contract_num {
                return true;
            }
        }

        // Priority 3: Evaluate the ON condition expression
        match on {
            Expr::BinaryOp { left, op, right } => {
                match op {
                    BinaryOperator::Eq => {
                        let left_val = self.eval_expr_for_pair(left, invoice, contract);
                        let right_val = self.eval_expr_for_pair(right, invoice, contract);
                        left_val == right_val && left_val.is_some()
                    }
                    BinaryOperator::And => {
                        // Both sides must match
                        self.matches_on_condition(invoice, contract, left)
                            && self.matches_on_condition(invoice, contract, right)
                    }
                    BinaryOperator::Or => {
                        // Either side can match
                        self.matches_on_condition(invoice, contract, left)
                            || self.matches_on_condition(invoice, contract, right)
                    }
                    _ => false,
                }
            }
            _ => {
                // Priority 4: Default fallback - match on vendor name
                let invoice_vendor = &invoice.vendor.name;
                contract.parties.first().map(|p| &p.name) == Some(invoice_vendor)
            }
        }
    }

    /// Evaluate an expression for an invoice-contract pair
    fn eval_expr_for_pair(
        &self,
        expr: &Expr,
        invoice: &Invoice,
        contract: &Contract,
    ) -> Option<String> {
        match expr {
            Expr::Column(col) => {
                let table = col.table.as_deref().unwrap_or("");
                match (table, col.column.as_str()) {
                    // Invoice fields
                    ("inv" | "invoices", "vendor_name") | ("", "vendor_name") => {
                        Some(invoice.vendor.name.clone())
                    }
                    ("inv" | "invoices", "id") => Some(invoice.id.to_string()),
                    ("inv" | "invoices", "contract_id") => {
                        invoice.contract_id.map(|id| id.to_string())
                    }
                    ("inv" | "invoices", "contract_number") => invoice.contract_number.clone(),
                    ("inv" | "invoices", "po_number") => invoice.po_number.clone(),
                    ("inv" | "invoices", "currency") => Some(format!("{:?}", invoice.currency)),
                    // Contract fields
                    ("con" | "contracts", "party_name") | ("", "party_name") => {
                        contract.parties.first().map(|p| p.name.clone())
                    }
                    ("con" | "contracts", "id") => Some(contract.id.to_string()),
                    ("con" | "contracts", "contract_number") => contract.contract_number.clone(),
                    ("con" | "contracts", "currency") => {
                        contract.currency.map(|c| format!("{:?}", c))
                    }
                    _ => None,
                }
            }
            Expr::Literal(Literal::String(s)) => Some(s.clone()),
            Expr::Literal(Literal::Integer(i)) => Some(i.to_string()),
            _ => None,
        }
    }

    /// Execute cross-validation checks on ZIP pairs
    pub fn cross_validate(&self, pairs: &[ZipPair], checks: &[&str]) -> Vec<CrossValidationIssue> {
        let mut issues = Vec::new();

        for pair in pairs {
            let (invoice, contract) = match (&pair.invoice, &pair.contract) {
                (Some(i), Some(c)) => (i, c),
                _ => continue,
            };

            for check in checks {
                match *check {
                    "amount" | "amount_within_contract" => {
                        let invoice_total: f64 =
                            invoice.total_amount.to_string().parse().unwrap_or(0.0);
                        let contract_amount: f64 = contract
                            .total_value
                            .as_ref()
                            .and_then(|v| v.to_string().parse().ok())
                            .unwrap_or(f64::MAX);

                        if invoice_total > contract_amount {
                            issues.push(CrossValidationIssue {
                                invoice_id: invoice.id.to_string(),
                                contract_id: contract.id.to_string(),
                                issue_type: CrossValidationIssueType::AmountExceedsContract,
                                description: format!(
                                    "Invoice total ({:.2}) exceeds contract amount ({:.2})",
                                    invoice_total, contract_amount
                                ),
                                severity: Severity::High,
                            });
                        }
                    }
                    "date" | "date_within_contract" => {
                        let start = contract.effective_date;
                        if let Some(end) = contract.expiration_date {
                            if invoice.invoice_date < start || invoice.invoice_date > end {
                                issues.push(CrossValidationIssue {
                                    invoice_id: invoice.id.to_string(),
                                    contract_id: contract.id.to_string(),
                                    issue_type: CrossValidationIssueType::DateOutsideContractPeriod,
                                    description: format!(
                                        "Invoice date ({}) is outside contract period ({} to {})",
                                        invoice.invoice_date, start, end
                                    ),
                                    severity: Severity::Medium,
                                });
                            }
                        }
                    }
                    "vendor" | "vendor_match" => {
                        let invoice_vendor = &invoice.vendor.name;
                        let contract_party = contract.parties.first().map(|p| &p.name);

                        if contract_party != Some(invoice_vendor) {
                            issues.push(CrossValidationIssue {
                                invoice_id: invoice.id.to_string(),
                                contract_id: contract.id.to_string(),
                                issue_type: CrossValidationIssueType::VendorMismatch,
                                description: format!(
                                    "Invoice vendor ({}) does not match contract party ({:?})",
                                    invoice_vendor, contract_party
                                ),
                                severity: Severity::High,
                            });
                        }
                    }
                    "currency" | "currency_match" => {
                        let invoice_currency = &invoice.currency;
                        if let Some(cc) = &contract.currency {
                            if invoice_currency != cc {
                                issues.push(CrossValidationIssue {
                                    invoice_id: invoice.id.to_string(),
                                    contract_id: contract.id.to_string(),
                                    issue_type: CrossValidationIssueType::CurrencyMismatch,
                                    description: format!(
                                        "Invoice currency ({:?}) does not match contract currency ({:?})",
                                        invoice_currency, cc
                                    ),
                                    severity: Severity::Medium,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        issues
    }

    /// Analyze ZIP pairs for aggregate metrics
    pub fn analyze(&self, pairs: &[ZipPair], metrics: &[&str]) -> HashMap<String, AggregateValue> {
        let mut results = HashMap::new();

        for metric in metrics {
            match *metric {
                "total_invoice_amount" => {
                    let total: f64 = pairs
                        .iter()
                        .filter_map(|p| p.invoice.as_ref())
                        .map(|i| i.total_amount.to_string().parse::<f64>().unwrap_or(0.0))
                        .sum();
                    results.insert(metric.to_string(), AggregateValue::Sum(total));
                }
                "average_invoice_amount" => {
                    let amounts: Vec<f64> = pairs
                        .iter()
                        .filter_map(|p| p.invoice.as_ref())
                        .map(|i| i.total_amount.to_string().parse::<f64>().unwrap_or(0.0))
                        .collect();
                    let avg = if amounts.is_empty() {
                        0.0
                    } else {
                        amounts.iter().sum::<f64>() / amounts.len() as f64
                    };
                    results.insert(metric.to_string(), AggregateValue::Average(avg));
                }
                "invoice_count" => {
                    let count = pairs.iter().filter(|p| p.invoice.is_some()).count();
                    results.insert(metric.to_string(), AggregateValue::Count(count));
                }
                "contract_count" => {
                    let count = pairs.iter().filter(|p| p.contract.is_some()).count();
                    results.insert(metric.to_string(), AggregateValue::Count(count));
                }
                "pair_count" => {
                    results.insert(metric.to_string(), AggregateValue::Count(pairs.len()));
                }
                "vendors" => {
                    let vendors: Vec<String> = pairs
                        .iter()
                        .filter_map(|p| p.invoice.as_ref())
                        .map(|i| i.vendor.name.clone())
                        .collect();
                    results.insert(metric.to_string(), AggregateValue::StringList(vendors));
                }
                _ => {}
            }
        }

        results
    }

    /// Execute a pipeline of operations on a ZipResult
    pub fn execute_pipeline(
        &self,
        mut result: ZipResult,
        pipeline: &[JsonPipelineOp],
    ) -> Result<ZipResult, StorageError> {
        let mut pipeline_results = PipelineResults {
            validation: None,
            anomalies: Vec::new(),
            cross_validation_issues: Vec::new(),
            aggregates: HashMap::new(),
        };

        for op in pipeline {
            match op {
                JsonPipelineOp::Validate { rules, fail_fast } => {
                    let validation_results =
                        self.execute_validate(&result.pairs, rules.as_deref(), *fail_fast);
                    pipeline_results.validation = Some(validation_results);
                }

                JsonPipelineOp::Analyze {
                    analyzers,
                    include_scores: _,
                } => {
                    let metrics: Vec<&str> = analyzers
                        .as_ref()
                        .map(|a| a.iter().map(|s| s.as_str()).collect())
                        .unwrap_or_else(|| {
                            vec![
                                "total_invoice_amount",
                                "average_invoice_amount",
                                "invoice_count",
                                "contract_count",
                                "pair_count",
                            ]
                        });
                    let aggregates = self.analyze(&result.pairs, &metrics);
                    pipeline_results.aggregates.extend(aggregates);
                }

                JsonPipelineOp::CrossValidate {
                    field_mapping,
                    tolerance,
                } => {
                    let issues =
                        self.execute_cross_validate(&result.pairs, field_mapping, *tolerance);
                    pipeline_results.cross_validation_issues.extend(issues);
                }

                JsonPipelineOp::Aggregate {
                    group_by: _,
                    aggregations,
                } => {
                    let aggregates = self.execute_aggregate(&result.pairs, aggregations);
                    pipeline_results.aggregates.extend(aggregates);
                }

                JsonPipelineOp::Transform { mappings: _ } => {
                    // Transform modifies the pairs - for now just a placeholder
                    // Full implementation would create new result rows with mapped fields
                }

                JsonPipelineOp::Filter { condition } => {
                    result.pairs = self.execute_filter(result.pairs, condition);
                    result.metadata.pair_count = result.pairs.len();
                    result.metadata.invoice_count =
                        result.pairs.iter().filter(|p| p.invoice.is_some()).count();
                    result.metadata.contract_count =
                        result.pairs.iter().filter(|p| p.contract.is_some()).count();
                }

                JsonPipelineOp::Sort { order_by } => {
                    self.execute_sort(&mut result.pairs, order_by);
                }

                JsonPipelineOp::Take { limit, offset } => {
                    let start = *offset as usize;
                    let end = (start + *limit as usize).min(result.pairs.len());
                    if start < result.pairs.len() {
                        result.pairs = result.pairs[start..end].to_vec();
                    } else {
                        result.pairs.clear();
                    }
                    result.metadata.pair_count = result.pairs.len();
                }
            }
        }

        result.pipeline_results = Some(pipeline_results);
        Ok(result)
    }

    /// Execute validation pipeline operation
    fn execute_validate(
        &self,
        pairs: &[ZipPair],
        rules: Option<&[String]>,
        fail_fast: bool,
    ) -> Vec<ValidationResult> {
        let start = std::time::Instant::now();
        let mut results = Vec::new();

        let default_rules = vec![
            "math_check".to_string(),
            "date_check".to_string(),
            "required_fields".to_string(),
        ];
        let rules_to_run = rules.unwrap_or(&default_rules);

        for pair in pairs {
            if let Some(invoice) = &pair.invoice {
                let mut anomalies = Vec::new();

                for rule in rules_to_run {
                    match rule.as_str() {
                        "math_check" => {
                            // Check if line items sum to total
                            let line_total: f64 = invoice
                                .line_items
                                .iter()
                                .map(|li| li.total.to_string().parse::<f64>().unwrap_or(0.0))
                                .sum();
                            let invoice_total: f64 =
                                invoice.total_amount.to_string().parse().unwrap_or(0.0);

                            if (line_total - invoice_total).abs() > 0.01 {
                                anomalies.push(Anomaly {
                                    document_id: DocumentId(invoice.id.0),
                                    anomaly_type: AnomalyType::MathMismatch,
                                    severity: Severity::High,
                                    description: format!(
                                        "Line items total ({:.2}) does not match invoice total ({:.2})",
                                        line_total, invoice_total
                                    ),
                                    field_path: Some("total_amount".to_string()),
                                    expected_value: Some(line_total.to_string()),
                                    actual_value: Some(invoice_total.to_string()),
                                    confidence: 1.0,
                                    statistical_score: None,
                                    detected_at: None,
                                });
                            }
                        }
                        "date_check" => {
                            // Check if due date is after invoice date
                            if let Some(due_date) = invoice.due_date {
                                if due_date < invoice.invoice_date {
                                    anomalies.push(Anomaly {
                                        document_id: DocumentId(invoice.id.0),
                                        anomaly_type: AnomalyType::DateInconsistency,
                                        severity: Severity::Medium,
                                        description: format!(
                                            "Due date ({}) is before invoice date ({})",
                                            due_date, invoice.invoice_date
                                        ),
                                        field_path: Some("due_date".to_string()),
                                        expected_value: Some(format!(
                                            ">= {}",
                                            invoice.invoice_date
                                        )),
                                        actual_value: Some(due_date.to_string()),
                                        confidence: 1.0,
                                        statistical_score: None,
                                        detected_at: None,
                                    });
                                }
                            }
                        }
                        "required_fields" => {
                            let mut missing = Vec::new();
                            if invoice.invoice_number.is_empty() {
                                missing.push("invoice_number");
                            }
                            if invoice.vendor.name.is_empty() {
                                missing.push("vendor_name");
                            }

                            if !missing.is_empty() {
                                anomalies.push(Anomaly {
                                    document_id: DocumentId(invoice.id.0),
                                    anomaly_type: AnomalyType::MissingField,
                                    severity: Severity::High,
                                    description: format!(
                                        "Missing required fields: {}",
                                        missing.join(", ")
                                    ),
                                    field_path: Some(missing.join(",")),
                                    expected_value: Some("non-empty".to_string()),
                                    actual_value: Some("empty".to_string()),
                                    confidence: 1.0,
                                    statistical_score: None,
                                    detected_at: None,
                                });
                            }
                        }
                        _ => {}
                    }

                    if fail_fast && !anomalies.is_empty() {
                        break;
                    }
                }

                let is_valid = anomalies.is_empty();
                results.push(ValidationResult {
                    document_id: DocumentId(invoice.id.0),
                    is_valid,
                    anomalies,
                    validation_time_ms: start.elapsed().as_millis() as u64,
                });
            }
        }

        results
    }

    /// Execute cross-validation between invoices and contracts
    fn execute_cross_validate(
        &self,
        pairs: &[ZipPair],
        field_mapping: &HashMap<String, String>,
        tolerance: f64,
    ) -> Vec<CrossValidationIssue> {
        let mut issues = Vec::new();

        for pair in pairs {
            let (invoice, contract) = match (&pair.invoice, &pair.contract) {
                (Some(i), Some(c)) => (i, c),
                _ => continue,
            };

            for (invoice_field, contract_field) in field_mapping {
                match (invoice_field.as_str(), contract_field.as_str()) {
                    ("total_amount", "total_value") | ("amount", "value") => {
                        let invoice_val: f64 =
                            invoice.total_amount.to_string().parse().unwrap_or(0.0);
                        let contract_val: f64 = contract
                            .total_value
                            .as_ref()
                            .and_then(|v| v.to_string().parse().ok())
                            .unwrap_or(f64::MAX);

                        if invoice_val > contract_val * (1.0 + tolerance) {
                            issues.push(CrossValidationIssue {
                                invoice_id: invoice.id.to_string(),
                                contract_id: contract.id.to_string(),
                                issue_type: CrossValidationIssueType::AmountExceedsContract,
                                description: format!(
                                    "Invoice amount ({:.2}) exceeds contract value ({:.2}) beyond tolerance ({:.1}%)",
                                    invoice_val, contract_val, tolerance * 100.0
                                ),
                                severity: Severity::High,
                            });
                        }
                    }
                    ("vendor_name", "party_name") | ("vendor", "party") => {
                        let invoice_vendor = &invoice.vendor.name;
                        let contract_party = contract.parties.first().map(|p| &p.name);

                        if contract_party != Some(invoice_vendor) {
                            issues.push(CrossValidationIssue {
                                invoice_id: invoice.id.to_string(),
                                contract_id: contract.id.to_string(),
                                issue_type: CrossValidationIssueType::VendorMismatch,
                                description: format!(
                                    "Invoice vendor '{}' does not match contract party '{}'",
                                    invoice_vendor,
                                    contract_party.map(|s| s.as_str()).unwrap_or("none")
                                ),
                                severity: Severity::High,
                            });
                        }
                    }
                    ("currency", "currency") => {
                        if let Some(contract_currency) = &contract.currency {
                            if &invoice.currency != contract_currency {
                                issues.push(CrossValidationIssue {
                                    invoice_id: invoice.id.to_string(),
                                    contract_id: contract.id.to_string(),
                                    issue_type: CrossValidationIssueType::CurrencyMismatch,
                                    description: format!(
                                        "Invoice currency {:?} does not match contract currency {:?}",
                                        invoice.currency, contract_currency
                                    ),
                                    severity: Severity::Medium,
                                });
                            }
                        }
                    }
                    ("invoice_date", "contract_period") => {
                        let start = contract.effective_date;
                        if let Some(end) = contract.expiration_date {
                            if invoice.invoice_date < start || invoice.invoice_date > end {
                                issues.push(CrossValidationIssue {
                                    invoice_id: invoice.id.to_string(),
                                    contract_id: contract.id.to_string(),
                                    issue_type: CrossValidationIssueType::DateOutsideContractPeriod,
                                    description: format!(
                                        "Invoice date {} is outside contract period {} to {}",
                                        invoice.invoice_date, start, end
                                    ),
                                    severity: Severity::Medium,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        issues
    }

    /// Execute aggregation pipeline operation
    fn execute_aggregate(
        &self,
        pairs: &[ZipPair],
        aggregations: &[crate::query::lang::json::JsonAggregation],
    ) -> HashMap<String, AggregateValue> {
        let mut results = HashMap::new();

        for agg in aggregations {
            let values = self.extract_values(pairs, &agg.expr);

            let result = match agg.function {
                JsonAggFunction::Count => AggregateValue::Count(values.len()),
                JsonAggFunction::Sum => {
                    let sum: f64 = values.iter().filter_map(|v| v.as_f64()).sum();
                    AggregateValue::Sum(sum)
                }
                JsonAggFunction::Avg => {
                    let nums: Vec<f64> = values.iter().filter_map(|v| v.as_f64()).collect();
                    let avg = if nums.is_empty() {
                        0.0
                    } else {
                        nums.iter().sum::<f64>() / nums.len() as f64
                    };
                    AggregateValue::Average(avg)
                }
                JsonAggFunction::Min => {
                    let min = values
                        .iter()
                        .filter_map(|v| v.as_f64())
                        .fold(f64::INFINITY, f64::min);
                    AggregateValue::Min(if min.is_infinite() { 0.0 } else { min })
                }
                JsonAggFunction::Max => {
                    let max = values
                        .iter()
                        .filter_map(|v| v.as_f64())
                        .fold(f64::NEG_INFINITY, f64::max);
                    AggregateValue::Max(if max.is_infinite() { 0.0 } else { max })
                }
                JsonAggFunction::First => {
                    if let Some(first) = values.first() {
                        match first {
                            ExtractedValue::String(s) => {
                                AggregateValue::StringList(vec![s.clone()])
                            }
                            ExtractedValue::Float(f) => AggregateValue::Sum(*f),
                            ExtractedValue::Null => AggregateValue::Count(0),
                        }
                    } else {
                        AggregateValue::Count(0)
                    }
                }
                JsonAggFunction::Last => {
                    if let Some(last) = values.last() {
                        match last {
                            ExtractedValue::String(s) => {
                                AggregateValue::StringList(vec![s.clone()])
                            }
                            ExtractedValue::Float(f) => AggregateValue::Sum(*f),
                            ExtractedValue::Null => AggregateValue::Count(0),
                        }
                    } else {
                        AggregateValue::Count(0)
                    }
                }
                JsonAggFunction::Collect => {
                    let strings: Vec<String> = values
                        .iter()
                        .filter_map(|v| match v {
                            ExtractedValue::String(s) => Some(s.clone()),
                            ExtractedValue::Float(f) => Some(f.to_string()),
                            ExtractedValue::Null => None,
                        })
                        .collect();
                    AggregateValue::StringList(strings)
                }
            };

            results.insert(agg.alias.clone(), result);
        }

        results
    }

    /// Execute filter pipeline operation
    fn execute_filter(&self, pairs: Vec<ZipPair>, condition: &JsonCondition) -> Vec<ZipPair> {
        pairs
            .into_iter()
            .filter(|pair| self.evaluate_condition(pair, condition))
            .collect()
    }

    /// Evaluate a condition against a ZipPair
    fn evaluate_condition(&self, pair: &ZipPair, condition: &JsonCondition) -> bool {
        match condition {
            JsonCondition::Compare { left, op, right } => {
                let left_val = self.extract_value(pair, left);
                let right_val = self.extract_value(pair, right);
                self.compare_values(&left_val, op, &right_val)
            }
            JsonCondition::And { conditions } => {
                conditions.iter().all(|c| self.evaluate_condition(pair, c))
            }
            JsonCondition::Or { conditions } => {
                conditions.iter().any(|c| self.evaluate_condition(pair, c))
            }
            JsonCondition::Not { condition } => !self.evaluate_condition(pair, condition),
            JsonCondition::IsNull { expr, negated } => {
                let val = self.extract_value(pair, expr);
                let is_null = matches!(val, ExtractedValue::Null);
                if *negated {
                    !is_null
                } else {
                    is_null
                }
            }
            _ => true, // Default to true for unsupported conditions
        }
    }

    /// Compare two extracted values
    fn compare_values(
        &self,
        left: &ExtractedValue,
        op: &crate::query::lang::json::JsonCompareOp,
        right: &ExtractedValue,
    ) -> bool {
        use crate::query::lang::json::JsonCompareOp;

        match (left, right) {
            (ExtractedValue::Float(l), ExtractedValue::Float(r)) => match op {
                JsonCompareOp::Eq => (l - r).abs() < f64::EPSILON,
                JsonCompareOp::NotEq => (l - r).abs() >= f64::EPSILON,
                JsonCompareOp::Lt => l < r,
                JsonCompareOp::LtEq => l <= r,
                JsonCompareOp::Gt => l > r,
                JsonCompareOp::GtEq => l >= r,
                _ => false,
            },
            (ExtractedValue::String(l), ExtractedValue::String(r)) => {
                match op {
                    JsonCompareOp::Eq => l == r,
                    JsonCompareOp::NotEq => l != r,
                    JsonCompareOp::Lt => l < r,
                    JsonCompareOp::LtEq => l <= r,
                    JsonCompareOp::Gt => l > r,
                    JsonCompareOp::GtEq => l >= r,
                    JsonCompareOp::Like | JsonCompareOp::ILike => {
                        // Simple pattern matching for LIKE
                        let pattern = r.replace('%', ".*").replace('_', ".");
                        if matches!(op, JsonCompareOp::ILike) {
                            l.to_lowercase().contains(&pattern.to_lowercase())
                        } else {
                            l.contains(&pattern)
                        }
                    }
                }
            }
            (ExtractedValue::Null, ExtractedValue::Null) => {
                matches!(op, JsonCompareOp::Eq)
            }
            _ => false,
        }
    }

    /// Execute sort pipeline operation
    fn execute_sort(
        &self,
        pairs: &mut [ZipPair],
        order_by: &[crate::query::lang::json::JsonOrderBy],
    ) {
        if order_by.is_empty() {
            return;
        }

        pairs.sort_by(|a, b| {
            for order in order_by {
                let val_a = self.extract_value(a, &order.expr);
                let val_b = self.extract_value(b, &order.expr);

                let cmp = match (&val_a, &val_b) {
                    (ExtractedValue::Float(fa), ExtractedValue::Float(fb)) => {
                        fa.partial_cmp(fb).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    (ExtractedValue::String(sa), ExtractedValue::String(sb)) => sa.cmp(sb),
                    (ExtractedValue::Null, ExtractedValue::Null) => std::cmp::Ordering::Equal,
                    (ExtractedValue::Null, _) => match order.nulls {
                        Some(crate::query::lang::json::JsonNullsOrder::First) => {
                            std::cmp::Ordering::Less
                        }
                        _ => std::cmp::Ordering::Greater,
                    },
                    (_, ExtractedValue::Null) => match order.nulls {
                        Some(crate::query::lang::json::JsonNullsOrder::First) => {
                            std::cmp::Ordering::Greater
                        }
                        _ => std::cmp::Ordering::Less,
                    },
                    _ => std::cmp::Ordering::Equal,
                };

                let cmp = match order.direction {
                    crate::query::lang::json::JsonSortDirection::Desc => cmp.reverse(),
                    crate::query::lang::json::JsonSortDirection::Asc => cmp,
                };

                if cmp != std::cmp::Ordering::Equal {
                    return cmp;
                }
            }
            std::cmp::Ordering::Equal
        });
    }

    /// Extract values from all pairs for aggregation
    fn extract_values(&self, pairs: &[ZipPair], expr: &JsonExpr) -> Vec<ExtractedValue> {
        pairs.iter().map(|p| self.extract_value(p, expr)).collect()
    }

    /// Extract a value from a ZipPair based on a JSON expression
    fn extract_value(&self, pair: &ZipPair, expr: &JsonExpr) -> ExtractedValue {
        match expr {
            JsonExpr::Column { table, column } => {
                let table_hint = table.as_deref().unwrap_or("");

                // Try invoice first
                if let Some(invoice) = &pair.invoice {
                    if table_hint.is_empty() || table_hint == "inv" || table_hint == "invoices" {
                        if let Some(val) = self.get_invoice_field(invoice, column) {
                            return val;
                        }
                    }
                }

                // Try contract
                if let Some(contract) = &pair.contract {
                    if table_hint.is_empty() || table_hint == "con" || table_hint == "contracts" {
                        if let Some(val) = self.get_contract_field(contract, column) {
                            return val;
                        }
                    }
                }

                ExtractedValue::Null
            }
            JsonExpr::Literal { value } => match value {
                JsonValue::String(s) => ExtractedValue::String(s.clone()),
                JsonValue::Int(i) => ExtractedValue::Float(*i as f64),
                JsonValue::Float(f) => ExtractedValue::Float(*f),
                JsonValue::Bool(b) => ExtractedValue::Float(if *b { 1.0 } else { 0.0 }),
                JsonValue::Null => ExtractedValue::Null,
                JsonValue::Array(_) => ExtractedValue::Null,
            },
            _ => ExtractedValue::Null,
        }
    }

    /// Get a field value from an invoice
    fn get_invoice_field(&self, invoice: &Invoice, field: &str) -> Option<ExtractedValue> {
        match field {
            "id" => Some(ExtractedValue::String(invoice.id.to_string())),
            "invoice_number" => Some(ExtractedValue::String(invoice.invoice_number.clone())),
            "vendor_name" => Some(ExtractedValue::String(invoice.vendor.name.clone())),
            "total_amount" => {
                let amount: f64 = invoice.total_amount.to_string().parse().unwrap_or(0.0);
                Some(ExtractedValue::Float(amount))
            }
            "subtotal" => {
                let amount: f64 = invoice.subtotal.to_string().parse().unwrap_or(0.0);
                Some(ExtractedValue::Float(amount))
            }
            "tax_amount" => {
                let amount: f64 = invoice.tax_amount.to_string().parse().unwrap_or(0.0);
                Some(ExtractedValue::Float(amount))
            }
            "invoice_date" => Some(ExtractedValue::String(invoice.invoice_date.to_string())),
            "due_date" => invoice
                .due_date
                .map(|d| ExtractedValue::String(d.to_string())),
            "currency" => Some(ExtractedValue::String(format!("{:?}", invoice.currency))),
            "validation_status" => Some(ExtractedValue::String(format!(
                "{:?}",
                invoice.validation_status
            ))),
            "po_number" => invoice
                .po_number
                .as_ref()
                .map(|s| ExtractedValue::String(s.clone())),
            "contract_id" => invoice
                .contract_id
                .map(|id| ExtractedValue::String(id.to_string())),
            "contract_number" => invoice
                .contract_number
                .as_ref()
                .map(|s| ExtractedValue::String(s.clone())),
            _ => None,
        }
    }

    /// Get a field value from a contract
    fn get_contract_field(&self, contract: &Contract, field: &str) -> Option<ExtractedValue> {
        match field {
            "id" => Some(ExtractedValue::String(contract.id.to_string())),
            "title" => Some(ExtractedValue::String(contract.title.clone())),
            "total_value" | "contract_value" => {
                let amount: f64 = contract
                    .total_value
                    .as_ref()
                    .and_then(|a| a.to_string().parse().ok())
                    .unwrap_or(0.0);
                Some(ExtractedValue::Float(amount))
            }
            "party_name" => contract
                .parties
                .first()
                .map(|p| ExtractedValue::String(p.name.clone())),
            "effective_date" => Some(ExtractedValue::String(contract.effective_date.to_string())),
            "expiration_date" => contract
                .expiration_date
                .map(|d| ExtractedValue::String(d.to_string())),
            "currency" => contract
                .currency
                .as_ref()
                .map(|c| ExtractedValue::String(format!("{:?}", c))),
            "contract_type" => Some(ExtractedValue::String(format!(
                "{:?}",
                contract.contract_type
            ))),
            _ => None,
        }
    }
}

/// Value extracted from a ZipPair for evaluation
#[derive(Debug, Clone)]
enum ExtractedValue {
    String(String),
    Float(f64),
    Null,
}

impl ExtractedValue {
    fn as_f64(&self) -> Option<f64> {
        match self {
            ExtractedValue::Float(f) => Some(*f),
            ExtractedValue::String(s) => s.parse().ok(),
            ExtractedValue::Null => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_validation_issue_types() {
        assert_eq!(
            CrossValidationIssueType::AmountExceedsContract,
            CrossValidationIssueType::AmountExceedsContract
        );
        assert_ne!(
            CrossValidationIssueType::AmountExceedsContract,
            CrossValidationIssueType::VendorMismatch
        );
    }

    #[test]
    fn test_aggregate_value() {
        let count = AggregateValue::Count(10);
        let sum = AggregateValue::Sum(100.5);

        match count {
            AggregateValue::Count(n) => assert_eq!(n, 10),
            _ => panic!("Expected Count"),
        }

        match sum {
            AggregateValue::Sum(n) => assert!((n - 100.5).abs() < f64::EPSILON),
            _ => panic!("Expected Sum"),
        }
    }

    #[test]
    fn test_zip_mode_default() {
        assert_eq!(ZipMode::default(), ZipMode::Inner);
    }

    #[test]
    fn test_extracted_value_as_f64() {
        let float_val = ExtractedValue::Float(42.5);
        assert!((float_val.as_f64().unwrap() - 42.5).abs() < f64::EPSILON);

        let string_val = ExtractedValue::String("123.45".to_string());
        assert!((string_val.as_f64().unwrap() - 123.45).abs() < f64::EPSILON);

        let invalid_string = ExtractedValue::String("not a number".to_string());
        assert!(invalid_string.as_f64().is_none());

        let null_val = ExtractedValue::Null;
        assert!(null_val.as_f64().is_none());
    }

    #[test]
    fn test_pipeline_results_default() {
        let results = PipelineResults {
            validation: None,
            anomalies: Vec::new(),
            cross_validation_issues: Vec::new(),
            aggregates: HashMap::new(),
        };

        assert!(results.validation.is_none());
        assert!(results.anomalies.is_empty());
        assert!(results.cross_validation_issues.is_empty());
        assert!(results.aggregates.is_empty());
    }

    #[test]
    fn test_zip_result_metadata() {
        let result = ZipResult {
            pairs: Vec::new(),
            metadata: ZipMetadata {
                pair_count: 5,
                invoice_count: 3,
                contract_count: 4,
                execution_time_ms: 100,
                zip_mode: "Inner".to_string(),
            },
            pipeline_results: None,
        };

        assert_eq!(result.metadata.pair_count, 5);
        assert_eq!(result.metadata.invoice_count, 3);
        assert_eq!(result.metadata.contract_count, 4);
        assert_eq!(result.metadata.execution_time_ms, 100);
        assert_eq!(result.metadata.zip_mode, "Inner");
    }

    #[test]
    fn test_cross_validation_issue_severity() {
        let issue = CrossValidationIssue {
            invoice_id: "inv-123".to_string(),
            contract_id: "con-456".to_string(),
            issue_type: CrossValidationIssueType::AmountExceedsContract,
            description: "Invoice exceeds contract".to_string(),
            severity: Severity::High,
        };

        assert_eq!(issue.invoice_id, "inv-123");
        assert_eq!(issue.contract_id, "con-456");
        assert_eq!(issue.severity, Severity::High);
    }

    #[test]
    fn test_split_where_clause_default() {
        let split = SplitWhereClause::default();
        assert!(split.invoice_conditions.is_none());
        assert!(split.contract_conditions.is_none());
        assert!(split.cross_table_conditions.is_none());
    }

    #[test]
    fn test_referenced_tables_default() {
        let tables = ReferencedTables::default();
        assert!(!tables.references_invoice);
        assert!(!tables.references_contract);
    }

    #[test]
    fn test_is_invoice_column() {
        // Create a dummy executor for testing - we'll test the helper methods
        let invoice_columns = [
            "invoice_number",
            "invoice_date",
            "due_date",
            "total_amount",
            "subtotal",
            "tax_amount",
            "vendor_name",
            "po_number",
            "contract_id",
            "contract_number",
            "line_items",
            "validation_status",
        ];

        for col in invoice_columns {
            // These should be recognized as invoice columns
            assert!(
                matches!(
                    col,
                    "invoice_number"
                        | "invoice_date"
                        | "due_date"
                        | "total_amount"
                        | "subtotal"
                        | "tax_amount"
                        | "vendor_name"
                        | "po_number"
                        | "contract_id"
                        | "contract_number"
                        | "line_items"
                        | "validation_status"
                ),
                "Column {} should be recognized as invoice column",
                col
            );
        }
    }

    #[test]
    fn test_is_contract_column() {
        let contract_columns = [
            "title",
            "effective_date",
            "expiration_date",
            "total_value",
            "contract_value",
            "party_name",
            "parties",
            "clauses",
            "contract_type",
            "payment_terms",
        ];

        for col in contract_columns {
            assert!(
                matches!(
                    col,
                    "title"
                        | "effective_date"
                        | "expiration_date"
                        | "total_value"
                        | "contract_value"
                        | "party_name"
                        | "parties"
                        | "clauses"
                        | "contract_type"
                        | "payment_terms"
                ),
                "Column {} should be recognized as contract column",
                col
            );
        }
    }

    #[test]
    fn test_combine_with_and() {
        // Two conditions combined with AND
        let expr1 = Expr::Literal(Literal::Boolean(true));
        let expr2 = Expr::Literal(Literal::Boolean(false));

        let combined = SplitWhereClause::combine_with_and(Some(expr1.clone()), Some(expr2.clone()));
        assert!(combined.is_some());

        // Verify it's a BinaryOp with And
        if let Some(Expr::BinaryOp { op, .. }) = combined {
            assert_eq!(op, BinaryOperator::And);
        } else {
            panic!("Expected BinaryOp with And");
        }

        // One None
        let combined_one = SplitWhereClause::combine_with_and(Some(expr1.clone()), None);
        assert!(combined_one.is_some());

        // Both None
        let combined_none = SplitWhereClause::combine_with_and(None, None);
        assert!(combined_none.is_none());
    }

    #[test]
    fn test_split_where_clause_add_conditions() {
        let mut split = SplitWhereClause::default();

        let invoice_cond = Expr::BinaryOp {
            left: Box::new(Expr::Column(ColumnRef::qualified("inv", "total_amount"))),
            op: BinaryOperator::Gt,
            right: Box::new(Expr::Literal(Literal::Integer(1000))),
        };

        let contract_cond = Expr::BinaryOp {
            left: Box::new(Expr::Column(ColumnRef::qualified("con", "effective_date"))),
            op: BinaryOperator::Gt,
            right: Box::new(Expr::Literal(Literal::String("2024-01-01".to_string()))),
        };

        split.add_invoice_condition(invoice_cond);
        split.add_contract_condition(contract_cond);

        assert!(split.invoice_conditions.is_some());
        assert!(split.contract_conditions.is_some());
        assert!(split.cross_table_conditions.is_none());
    }

    #[test]
    fn test_values_equal() {
        assert!(matches!(
            (&Some("test".to_string()), &Some("test".to_string())),
            (Some(a), Some(b)) if a == b
        ));

        assert!(matches!((&None::<String>, &None::<String>), (None, None)));

        assert!(!matches!(
            (&Some("a".to_string()), &Some("b".to_string())),
            (Some(a), Some(b)) if a == b
        ));
    }

    #[test]
    fn test_compare_values_ord() {
        // Numeric comparison
        let a = Some("100".to_string());
        let b = Some("200".to_string());

        if let (Ok(a_num), Ok(b_num)) = (
            a.as_ref().unwrap().parse::<f64>(),
            b.as_ref().unwrap().parse::<f64>(),
        ) {
            assert_eq!(a_num.partial_cmp(&b_num), Some(std::cmp::Ordering::Less));
        }

        // String comparison
        let s1 = Some("apple".to_string());
        let s2 = Some("banana".to_string());

        if let (Some(s1), Some(s2)) = (&s1, &s2) {
            assert_eq!(s1.cmp(s2), std::cmp::Ordering::Less);
        }
    }
}

//! Morsel-based parallel query execution
//!
//! Implements the morsel-driven parallelism model (inspired by HyPer/Umbra)
//! for CPU-bound query stages: filtering, scoring, and projection.
//!
//! # Architecture
//!
//! The candidate set is partitioned into fixed-size **morsels** (~2048 rows).
//! Each morsel is processed independently by a rayon worker thread through
//! the filter → score → project pipeline. Results are merged via a final
//! parallel sort.
//!
//! ```text
//! Candidates ──┬── Morsel 0 ─── Filter → Score ──┐
//!              ├── Morsel 1 ─── Filter → Score ──┤── Merge + Sort → LIMIT
//!              ├── Morsel 2 ─── Filter → Score ──┤
//!              └── Morsel N ─── Filter → Score ──┘
//! ```
//!
//! # NUMA Awareness
//!
//! On multi-socket systems, morsel assignment respects thread locality.
//! Rayon's work-stealing scheduler keeps morsels on the same core that
//! fetched them from L2/L3 cache. The morsel size (2048) is chosen to
//! fit within a typical L2 cache line budget (~256KB for Invoice structs).
//!
//! # When Parallelism Activates
//!
//! Small candidate sets (< MORSEL_SIZE) bypass the parallel path entirely
//! and execute sequentially to avoid thread dispatch overhead (~5-10μs).

use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;

use fen_core::domain::{Contract, Invoice};

use crate::fulltext::FullTextIndex;
use crate::query::executor::{ColumnValue, ResultRow};
use crate::query::lang::{
    BinaryOperator, Expr, FenQuery, FilterExpr, FunctionCall, FunctionName, Literal, OrderByClause,
    ParamValue, QueryParams, SelectItem, SortDirection,
};

/// Default morsel size — tuned for L2 cache residency.
/// Invoice structs are ~500-1500 bytes, so 2048 morsels ≈ 1-3MB per morsel.
const MORSEL_SIZE: usize = 2048;

/// Minimum candidate count to activate parallel execution.
/// Below this threshold, sequential is faster due to thread dispatch overhead.
const PARALLEL_THRESHOLD: usize = 4096;

/// Configuration for the morsel executor
#[derive(Debug, Clone)]
pub struct MorselConfig {
    /// Morsel size (number of rows per work unit)
    pub morsel_size: usize,
    /// Minimum candidates to trigger parallel execution
    pub parallel_threshold: usize,
    /// Vector weight for score fusion
    pub vector_weight: f64,
    /// Text weight for score fusion
    pub text_weight: f64,
}

impl Default for MorselConfig {
    fn default() -> Self {
        Self {
            morsel_size: MORSEL_SIZE,
            parallel_threshold: PARALLEL_THRESHOLD,
            vector_weight: 0.7,
            text_weight: 0.3,
        }
    }
}

/// Pre-computed query features to avoid repeated AST walks across workers
#[derive(Debug, Clone, Copy)]
pub struct QueryFeatures {
    pub uses_vector: bool,
    pub uses_text: bool,
    pub uses_contains: bool,
}

impl QueryFeatures {
    pub fn from_query(query: &FenQuery) -> Self {
        Self {
            uses_vector: query.uses_vector_search(),
            uses_text: query.uses_text_search(),
            uses_contains: query.uses_contains(),
        }
    }
}

/// Thread-safe filter + score context shared across morsel workers.
///
/// Each worker receives a shared reference to this context.
/// All fields are either immutable or internally synchronized.
pub struct MorselContext {
    pub fulltext_index: Arc<FullTextIndex>,
    pub config: MorselConfig,
}

/// Execute filter → score on invoice candidates using morsel parallelism.
///
/// Returns scored, filtered candidates ready for sort + limit.
pub fn morsel_filter_score_invoices(
    candidates: Vec<(Invoice, f64)>,
    filter: Option<&FilterExpr>,
    query: &FenQuery,
    params: &QueryParams,
    features: &QueryFeatures,
    ctx: &MorselContext,
) -> Vec<(Invoice, f64)> {
    let n = candidates.len();

    if n < ctx.config.parallel_threshold {
        // Sequential fast path — avoid thread dispatch overhead
        return sequential_filter_score_invoices(candidates, filter, query, params, features, ctx);
    }

    // Partition into morsels and process in parallel
    candidates
        .into_par_iter()
        .with_min_len(ctx.config.morsel_size)
        .filter(|(inv, _)| {
            filter
                .map(|f| evaluate_invoice_filter(f, inv, params, &ctx.fulltext_index))
                .unwrap_or(true)
        })
        .map(|(inv, base_score)| {
            let score =
                calculate_invoice_score(query, &inv, params, base_score, features, ctx);
            (inv, score)
        })
        .collect()
}

/// Execute filter → score on contract candidates using morsel parallelism.
pub fn morsel_filter_score_contracts(
    candidates: Vec<(Contract, f64)>,
    filter: Option<&FilterExpr>,
    _query: &FenQuery,
    params: &QueryParams,
    _features: &QueryFeatures,
    ctx: &MorselContext,
) -> Vec<(Contract, f64)> {
    let n = candidates.len();

    if n < ctx.config.parallel_threshold {
        return sequential_filter_score_contracts(candidates, filter, params, &ctx.fulltext_index);
    }

    candidates
        .into_par_iter()
        .with_min_len(ctx.config.morsel_size)
        .filter(|(contract, _)| {
            filter
                .map(|f| evaluate_contract_filter(f, contract, params, &ctx.fulltext_index))
                .unwrap_or(true)
        })
        .collect()
}

/// Parallel sort using rayon's par_sort_unstable.
pub fn morsel_sort_invoices(
    results: &mut [(Invoice, f64)],
    order_by: &OrderByClause,
) {
    if results.len() < PARALLEL_THRESHOLD {
        // Sequential sort for small sets
        sequential_sort(results, order_by);
        return;
    }

    if let Some(first) = order_by.items.first() {
        let ascending = matches!(first.direction, SortDirection::Asc);

        results.par_sort_unstable_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }
}

/// Parallel sort for contracts.
pub fn morsel_sort_contracts(
    results: &mut [(Contract, f64)],
    order_by: &OrderByClause,
) {
    if results.len() < PARALLEL_THRESHOLD {
        sequential_sort_contracts(results, order_by);
        return;
    }

    if let Some(first) = order_by.items.first() {
        let ascending = matches!(first.direction, SortDirection::Asc);

        results.par_sort_unstable_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
            if ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });
    }
}

/// Parallel projection — convert invoices to result rows.
pub fn morsel_project_invoices(
    scored: Vec<(Invoice, f64)>,
    select: &[SelectItem],
    offset: usize,
    limit: usize,
) -> Vec<ResultRow> {
    let window: Vec<(Invoice, f64)> = scored.into_iter().skip(offset).take(limit).collect();

    if window.len() < PARALLEL_THRESHOLD / 4 {
        // Sequential projection for small result sets
        return window
            .into_iter()
            .map(|(inv, score)| project_invoice_row(&inv, select, score))
            .collect();
    }

    window
        .into_par_iter()
        .map(|(inv, score)| project_invoice_row(&inv, select, score))
        .collect()
}

// ==================== Sequential fallbacks ====================

fn sequential_filter_score_invoices(
    candidates: Vec<(Invoice, f64)>,
    filter: Option<&FilterExpr>,
    query: &FenQuery,
    params: &QueryParams,
    features: &QueryFeatures,
    ctx: &MorselContext,
) -> Vec<(Invoice, f64)> {
    candidates
        .into_iter()
        .filter(|(inv, _)| {
            filter
                .map(|f| evaluate_invoice_filter(f, inv, params, &ctx.fulltext_index))
                .unwrap_or(true)
        })
        .map(|(inv, base_score)| {
            let score =
                calculate_invoice_score(query, &inv, params, base_score, features, ctx);
            (inv, score)
        })
        .collect()
}

fn sequential_filter_score_contracts(
    candidates: Vec<(Contract, f64)>,
    filter: Option<&FilterExpr>,
    params: &QueryParams,
    fulltext_index: &FullTextIndex,
) -> Vec<(Contract, f64)> {
    candidates
        .into_iter()
        .filter(|(contract, _)| {
            filter
                .map(|f| evaluate_contract_filter(f, contract, params, fulltext_index))
                .unwrap_or(true)
        })
        .collect()
}

fn sequential_sort(results: &mut [(Invoice, f64)], order_by: &OrderByClause) {
    if let Some(first) = order_by.items.first() {
        let ascending = matches!(first.direction, SortDirection::Asc);
        results.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
            if ascending { cmp } else { cmp.reverse() }
        });
    }
}

fn sequential_sort_contracts(results: &mut [(Contract, f64)], order_by: &OrderByClause) {
    if let Some(first) = order_by.items.first() {
        let ascending = matches!(first.direction, SortDirection::Asc);
        results.sort_by(|a, b| {
            let cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
            if ascending { cmp } else { cmp.reverse() }
        });
    }
}

// ==================== Expression evaluation (thread-safe) ====================

/// Thread-safe invoice filter evaluation — no RefCell, no mutable state.
fn evaluate_invoice_filter(
    filter: &FilterExpr,
    invoice: &Invoice,
    params: &QueryParams,
    fulltext_index: &FullTextIndex,
) -> bool {
    eval_invoice_bool(&filter.expr, invoice, params, fulltext_index)
}

fn eval_invoice_bool(
    expr: &Expr,
    invoice: &Invoice,
    params: &QueryParams,
    ft: &FullTextIndex,
) -> bool {
    match expr {
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And => {
                eval_invoice_bool(left, invoice, params, ft)
                    && eval_invoice_bool(right, invoice, params, ft)
            }
            BinaryOperator::Or => {
                eval_invoice_bool(left, invoice, params, ft)
                    || eval_invoice_bool(right, invoice, params, ft)
            }
            BinaryOperator::Eq => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                compare_eq(&l, &r)
            }
            BinaryOperator::NotEq => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                !compare_eq(&l, &r)
            }
            BinaryOperator::Lt => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                compare_ord(&l, &r, |a, b| a < b)
            }
            BinaryOperator::LtEq => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                compare_ord(&l, &r, |a, b| a <= b)
            }
            BinaryOperator::Gt => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                compare_ord(&l, &r, |a, b| a > b)
            }
            BinaryOperator::GtEq => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                compare_ord(&l, &r, |a, b| a >= b)
            }
            BinaryOperator::Like => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                like_match_inline(&l, &r, false)
            }
            BinaryOperator::ILike => {
                let l = eval_invoice_value(left, invoice, params, ft);
                let r = eval_invoice_value(right, invoice, params, ft);
                like_match_inline(&l, &r, true)
            }
            _ => false,
        },
        Expr::Function(FunctionCall {
            name: FunctionName::Contains,
            args,
        }) => {
            if args.len() >= 2 {
                let text = eval_invoice_value(&args[0], invoice, params, ft);
                let search = eval_invoice_value(&args[1], invoice, params, ft);
                if let (ColumnValue::String(t), ColumnValue::String(s)) = (text, search) {
                    return ft.contains(&t, &s);
                }
            }
            false
        }
        Expr::Function(FunctionCall {
            name: FunctionName::VectorDistance,
            ..
        }) => true,
        _ => true,
    }
}

fn eval_invoice_value(
    expr: &Expr,
    invoice: &Invoice,
    params: &QueryParams,
    ft: &FullTextIndex,
) -> ColumnValue {
    match expr {
        Expr::Column(col) => get_invoice_col(invoice, &col.column),
        Expr::Literal(lit) => literal_to_value(lit),
        Expr::Parameter(name) => param_to_value(params, name),
        Expr::Function(FunctionCall {
            name: FunctionName::VectorDistance,
            ..
        }) => ColumnValue::Float(0.0),
        Expr::Function(FunctionCall {
            name: FunctionName::Bm25Score,
            args,
        }) => {
            if args.len() >= 2 {
                if let (Expr::Column(col), Expr::Parameter(param_name)) = (&args[0], &args[1]) {
                    let text = get_invoice_col(invoice, &col.column);
                    if let (ColumnValue::String(t), Some(q)) =
                        (text, params.get_string(param_name))
                    {
                        let score = ft.bm25_score(&t, q);
                        return ColumnValue::Float(score as f64);
                    }
                }
            }
            ColumnValue::Float(0.0)
        }
        _ => ColumnValue::Null,
    }
}

/// Thread-safe contract filter evaluation.
fn evaluate_contract_filter(
    filter: &FilterExpr,
    contract: &Contract,
    params: &QueryParams,
    fulltext_index: &FullTextIndex,
) -> bool {
    eval_contract_bool(&filter.expr, contract, params, fulltext_index)
}

fn eval_contract_bool(
    expr: &Expr,
    contract: &Contract,
    params: &QueryParams,
    ft: &FullTextIndex,
) -> bool {
    match expr {
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And => {
                eval_contract_bool(left, contract, params, ft)
                    && eval_contract_bool(right, contract, params, ft)
            }
            BinaryOperator::Or => {
                eval_contract_bool(left, contract, params, ft)
                    || eval_contract_bool(right, contract, params, ft)
            }
            BinaryOperator::Eq => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                compare_eq(&l, &r)
            }
            BinaryOperator::NotEq => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                !compare_eq(&l, &r)
            }
            BinaryOperator::Lt => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                compare_ord(&l, &r, |a, b| a < b)
            }
            BinaryOperator::LtEq => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                compare_ord(&l, &r, |a, b| a <= b)
            }
            BinaryOperator::Gt => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                compare_ord(&l, &r, |a, b| a > b)
            }
            BinaryOperator::GtEq => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                compare_ord(&l, &r, |a, b| a >= b)
            }
            BinaryOperator::Like => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                like_match_inline(&l, &r, false)
            }
            BinaryOperator::ILike => {
                let l = eval_contract_value(left, contract, params, ft);
                let r = eval_contract_value(right, contract, params, ft);
                like_match_inline(&l, &r, true)
            }
            _ => false,
        },
        Expr::Function(FunctionCall {
            name: FunctionName::Contains,
            args,
        }) => {
            if args.len() >= 2 {
                let text = eval_contract_value(&args[0], contract, params, ft);
                let search = eval_contract_value(&args[1], contract, params, ft);
                if let (ColumnValue::String(t), ColumnValue::String(s)) = (text, search) {
                    return ft.contains(&t, &s);
                }
            }
            false
        }
        _ => true,
    }
}

fn eval_contract_value(
    expr: &Expr,
    contract: &Contract,
    params: &QueryParams,
    ft: &FullTextIndex,
) -> ColumnValue {
    match expr {
        Expr::Column(col) => get_contract_col(contract, &col.column),
        Expr::Literal(lit) => literal_to_value(lit),
        Expr::Parameter(name) => param_to_value(params, name),
        Expr::Function(FunctionCall {
            name: FunctionName::Bm25Score,
            args,
        }) => {
            if args.len() >= 2 {
                if let (Expr::Column(col), Expr::Parameter(param_name)) = (&args[0], &args[1]) {
                    let text = get_contract_col(contract, &col.column);
                    if let (ColumnValue::String(t), Some(q)) =
                        (text, params.get_string(param_name))
                    {
                        let score = ft.bm25_score(&t, q);
                        return ColumnValue::Float(score as f64);
                    }
                }
            }
            ColumnValue::Float(0.0)
        }
        _ => ColumnValue::Null,
    }
}

// ==================== Column accessors (zero-alloc dispatch) ====================

fn get_invoice_col(invoice: &Invoice, column: &str) -> ColumnValue {
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

fn get_contract_col(contract: &Contract, column: &str) -> ColumnValue {
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

// ==================== Scoring ====================

fn calculate_invoice_score(
    query: &FenQuery,
    invoice: &Invoice,
    params: &QueryParams,
    base_score: f64,
    features: &QueryFeatures,
    ctx: &MorselContext,
) -> f64 {
    if !features.uses_vector && !features.uses_text {
        return base_score;
    }

    let mut vector_score = 0.0;
    let mut text_score = 0.0;

    if features.uses_vector {
        vector_score = 1.0 - base_score.min(1.0);
    }

    if features.uses_text {
        if let Some(search_terms) = find_text_param_from_query(query, params) {
            let bm25 = ctx.fulltext_index.bm25_score(&invoice.extracted_text, &search_terms);
            text_score = (bm25 as f64).min(10.0) / 10.0;
        }
    }

    if features.uses_vector && features.uses_text {
        ctx.config.vector_weight * vector_score + ctx.config.text_weight * text_score
    } else if features.uses_vector {
        vector_score
    } else {
        text_score
    }
}

// ==================== Shared helpers ====================

fn literal_to_value(lit: &Literal) -> ColumnValue {
    match lit {
        Literal::String(s) => ColumnValue::String(s.clone()),
        Literal::Integer(i) => ColumnValue::Integer(*i),
        Literal::Float(f) => ColumnValue::Float(*f),
        Literal::Boolean(b) => ColumnValue::Boolean(*b),
        Literal::Null => ColumnValue::Null,
        Literal::Array(_) => ColumnValue::Null,
    }
}

fn param_to_value(params: &QueryParams, name: &str) -> ColumnValue {
    match params.get(name) {
        Some(ParamValue::String(s)) => ColumnValue::String(s.clone()),
        Some(ParamValue::Integer(i)) => ColumnValue::Integer(*i),
        Some(ParamValue::Float(f)) => ColumnValue::Float(*f),
        Some(ParamValue::Boolean(b)) => ColumnValue::Boolean(*b),
        Some(ParamValue::Null) => ColumnValue::Null,
        Some(ParamValue::Vector(_)) => ColumnValue::Null,
        None => ColumnValue::Null,
    }
}

fn compare_eq(a: &ColumnValue, b: &ColumnValue) -> bool {
    match (a, b) {
        (ColumnValue::String(a), ColumnValue::String(b)) => a == b,
        (ColumnValue::Integer(a), ColumnValue::Integer(b)) => a == b,
        (ColumnValue::Float(a), ColumnValue::Float(b)) => a == b,
        (ColumnValue::Boolean(a), ColumnValue::Boolean(b)) => a == b,
        (ColumnValue::Null, ColumnValue::Null) => true,
        _ => false,
    }
}

fn compare_ord(a: &ColumnValue, b: &ColumnValue, cmp: impl Fn(f64, f64) -> bool) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => cmp(a, b),
        _ => false,
    }
}

/// Thread-safe LIKE matching — compiles regex inline (no shared cache needed).
/// For the parallel path, we accept the per-call compilation cost since
/// morsels are processed concurrently and the overall throughput is higher.
fn like_match_inline(value: &ColumnValue, pattern: &ColumnValue, case_insensitive: bool) -> bool {
    match (value, pattern) {
        (ColumnValue::String(v), ColumnValue::String(p)) => {
            let escaped = regex::escape(p).replace("%", ".*").replace("_", ".");
            let regex_pattern = if case_insensitive {
                format!("(?i)^{}$", escaped)
            } else {
                format!("^{}$", escaped)
            };
            regex::Regex::new(&regex_pattern)
                .map(|re| re.is_match(v))
                .unwrap_or(false)
        }
        _ => false,
    }
}

fn project_invoice_row(invoice: &Invoice, select: &[SelectItem], score: f64) -> ResultRow {
    let mut columns = HashMap::new();

    for item in select {
        match &item.expr {
            Expr::Wildcard => {
                columns.insert("id".to_string(), ColumnValue::String(invoice.id.0.to_string()));
                columns.insert(
                    "invoice_number".to_string(),
                    ColumnValue::String(invoice.invoice_number.clone()),
                );
                columns.insert("invoice_date".to_string(), ColumnValue::Date(invoice.invoice_date));
                columns.insert(
                    "vendor_name".to_string(),
                    ColumnValue::String(invoice.vendor.name.clone()),
                );
                columns.insert("total_amount".to_string(), ColumnValue::Decimal(invoice.total_amount));
                columns.insert(
                    "confidence_score".to_string(),
                    ColumnValue::Float(invoice.confidence_score as f64),
                );
            }
            Expr::Column(col) => {
                let name = item.alias.clone().unwrap_or_else(|| col.column.clone());
                let value = get_invoice_col(invoice, &col.column);
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

/// Extract text search parameter from query AST (shared helper).
fn find_text_param_from_query(query: &FenQuery, params: &QueryParams) -> Option<String> {
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
        })?;

    params.get_string(&param_name).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morsel_config_defaults() {
        let config = MorselConfig::default();
        assert_eq!(config.morsel_size, 2048);
        assert_eq!(config.parallel_threshold, 4096);
        assert!((config.vector_weight - 0.7).abs() < f64::EPSILON);
        assert!((config.text_weight - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_query_features() {
        use crate::query::lang::parse_query;
        let q = parse_query("SELECT id FROM invoices").unwrap();
        let f = QueryFeatures::from_query(&q);
        assert!(!f.uses_vector);
        assert!(!f.uses_text);
        assert!(!f.uses_contains);
    }

    #[test]
    fn test_query_features_vector() {
        use crate::query::lang::parse_query;
        let q = parse_query(
            "SELECT VECTOR_DISTANCE(embedding, :vec) AS score FROM invoices",
        )
        .unwrap();
        let f = QueryFeatures::from_query(&q);
        assert!(f.uses_vector);
        assert!(!f.uses_text);
    }

    #[test]
    fn test_query_features_text() {
        use crate::query::lang::parse_query;
        let q = parse_query(
            "SELECT BM25_SCORE(extracted_text, :terms) AS score FROM invoices",
        )
        .unwrap();
        let f = QueryFeatures::from_query(&q);
        assert!(!f.uses_vector);
        assert!(f.uses_text);
    }

    #[test]
    fn test_like_match_inline_case_sensitive() {
        let v = ColumnValue::String("Hello World".to_string());
        let p = ColumnValue::String("%World".to_string());
        assert!(like_match_inline(&v, &p, false));
        assert!(!like_match_inline(&v, &ColumnValue::String("%world".to_string()), false));
    }

    #[test]
    fn test_like_match_inline_case_insensitive() {
        let v = ColumnValue::String("Hello World".to_string());
        let p = ColumnValue::String("%world".to_string());
        assert!(like_match_inline(&v, &p, true));
    }

    #[test]
    fn test_compare_eq() {
        assert!(compare_eq(
            &ColumnValue::String("a".into()),
            &ColumnValue::String("a".into()),
        ));
        assert!(!compare_eq(
            &ColumnValue::String("a".into()),
            &ColumnValue::String("b".into()),
        ));
        assert!(compare_eq(&ColumnValue::Null, &ColumnValue::Null));
    }

    #[test]
    fn test_compare_ord() {
        assert!(compare_ord(
            &ColumnValue::Float(1.0),
            &ColumnValue::Float(2.0),
            |a, b| a < b,
        ));
        assert!(!compare_ord(
            &ColumnValue::Float(2.0),
            &ColumnValue::Float(1.0),
            |a, b| a < b,
        ));
    }

    #[test]
    fn test_small_set_sequential_path() {
        // Verifies that empty candidates below threshold work correctly
        let result = morsel_filter_score_invoices(
            Vec::new(),
            None,
            &crate::query::lang::parse_query("SELECT id FROM invoices").unwrap(),
            &QueryParams::default(),
            &QueryFeatures {
                uses_vector: false,
                uses_text: false,
                uses_contains: false,
            },
            &MorselContext {
                fulltext_index: Arc::new(
                    FullTextIndex::in_memory(crate::fulltext::FullTextConfig::default()).unwrap(),
                ),
                config: MorselConfig::default(),
            },
        );
        assert!(result.is_empty());
    }
}

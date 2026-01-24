//! Abstract Syntax Tree types for the Fen query language
//!
//! These types represent the semantic structure of queries after parsing
//! from SQL syntax.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parsed and validated Fen query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FenQuery {
    /// Target table (invoices, contracts)
    pub from: QueryTarget,
    /// Table alias (e.g., "inv" for "invoices inv")
    pub alias: Option<String>,
    /// Columns/expressions to select
    pub select: Vec<SelectItem>,
    /// WHERE clause conditions
    pub filter: Option<FilterExpr>,
    /// ORDER BY clause
    pub order_by: Option<OrderByClause>,
    /// LIMIT clause
    pub limit: Option<u64>,
    /// OFFSET clause
    pub offset: Option<u64>,
}

impl FenQuery {
    /// Check if this query uses vector search
    pub fn uses_vector_search(&self) -> bool {
        self.select.iter().any(|s| s.uses_vector_distance())
            || self.filter.as_ref().map_or(false, |f| f.uses_vector_distance())
            || self.order_by.as_ref().map_or(false, |o| o.uses_vector_distance())
    }

    /// Check if this query uses BM25 text search
    pub fn uses_text_search(&self) -> bool {
        self.select.iter().any(|s| s.uses_bm25())
            || self.filter.as_ref().map_or(false, |f| f.uses_bm25())
            || self.order_by.as_ref().map_or(false, |o| o.uses_bm25())
    }

    /// Check if this query uses CONTAINS
    pub fn uses_contains(&self) -> bool {
        self.filter.as_ref().map_or(false, |f| f.uses_contains())
    }

    /// Extract all parameter names used in this query
    pub fn parameter_names(&self) -> Vec<String> {
        let mut params = Vec::new();
        for item in &self.select {
            item.collect_parameters(&mut params);
        }
        if let Some(filter) = &self.filter {
            filter.collect_parameters(&mut params);
        }
        if let Some(order) = &self.order_by {
            order.collect_parameters(&mut params);
        }
        params.sort();
        params.dedup();
        params
    }
}

/// Query target table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryTarget {
    Invoices,
    Contracts,
}

impl QueryTarget {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryTarget::Invoices => "invoices",
            QueryTarget::Contracts => "contracts",
        }
    }
}

/// A single item in the SELECT clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectItem {
    /// The expression being selected
    pub expr: Expr,
    /// Optional alias (AS name)
    pub alias: Option<String>,
}

impl SelectItem {
    pub fn uses_vector_distance(&self) -> bool {
        self.expr.uses_vector_distance()
    }

    pub fn uses_bm25(&self) -> bool {
        self.expr.uses_bm25()
    }

    fn collect_parameters(&self, params: &mut Vec<String>) {
        self.expr.collect_parameters(params);
    }
}

/// Expression types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    /// Column reference (e.g., inv.id, invoice_number)
    Column(ColumnRef),
    /// Literal value
    Literal(Literal),
    /// Parameter reference (e.g., :query_vector)
    Parameter(String),
    /// Binary operation (e.g., a + b, a < 0.3)
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    /// Unary operation (e.g., NOT, -)
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expr>,
    },
    /// Function call
    Function(FunctionCall),
    /// Wildcard (*)
    Wildcard,
    /// Qualified wildcard (table.*)
    QualifiedWildcard(String),
}

impl Expr {
    pub fn uses_vector_distance(&self) -> bool {
        match self {
            Expr::Function(f) => matches!(f.name, FunctionName::VectorDistance),
            Expr::BinaryOp { left, right, .. } => {
                left.uses_vector_distance() || right.uses_vector_distance()
            }
            Expr::UnaryOp { expr, .. } => expr.uses_vector_distance(),
            _ => false,
        }
    }

    pub fn uses_bm25(&self) -> bool {
        match self {
            Expr::Function(f) => matches!(f.name, FunctionName::Bm25Score),
            Expr::BinaryOp { left, right, .. } => left.uses_bm25() || right.uses_bm25(),
            Expr::UnaryOp { expr, .. } => expr.uses_bm25(),
            _ => false,
        }
    }

    pub fn uses_contains(&self) -> bool {
        match self {
            Expr::Function(f) => matches!(f.name, FunctionName::Contains),
            Expr::BinaryOp { left, right, .. } => left.uses_contains() || right.uses_contains(),
            Expr::UnaryOp { expr, .. } => expr.uses_contains(),
            _ => false,
        }
    }

    fn collect_parameters(&self, params: &mut Vec<String>) {
        match self {
            Expr::Parameter(name) => params.push(name.clone()),
            Expr::BinaryOp { left, right, .. } => {
                left.collect_parameters(params);
                right.collect_parameters(params);
            }
            Expr::UnaryOp { expr, .. } => expr.collect_parameters(params),
            Expr::Function(f) => {
                for arg in &f.args {
                    arg.collect_parameters(params);
                }
            }
            _ => {}
        }
    }
}

/// Column reference with optional table qualifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnRef {
    /// Table qualifier (e.g., "inv" in inv.id)
    pub table: Option<String>,
    /// Column name
    pub column: String,
}

impl ColumnRef {
    pub fn new(column: impl Into<String>) -> Self {
        Self {
            table: None,
            column: column.into(),
        }
    }

    pub fn qualified(table: impl Into<String>, column: impl Into<String>) -> Self {
        Self {
            table: Some(table.into()),
            column: column.into(),
        }
    }
}

/// Literal values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    /// String literal
    String(String),
    /// Integer literal
    Integer(i64),
    /// Float literal
    Float(f64),
    /// Boolean literal
    Boolean(bool),
    /// NULL literal
    Null,
    /// Array literal (for vector values)
    Array(Vec<Literal>),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,

    // Comparison
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // Logical
    And,
    Or,

    // String
    Like,
    ILike,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Minus,
    Plus,
}

/// Function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name
    pub name: FunctionName,
    /// Function arguments
    pub args: Vec<Expr>,
}

/// Known function names with special semantics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FunctionName {
    /// VECTOR_DISTANCE(column, vector) - compute cosine distance
    VectorDistance,
    /// BM25_SCORE(column, search_terms) - compute BM25 relevance score
    Bm25Score,
    /// CONTAINS(column, search_terms) - check if text contains terms
    Contains,
    /// Other function (passed through)
    Other(String),
}

impl FunctionName {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "VECTOR_DISTANCE" => FunctionName::VectorDistance,
            "BM25_SCORE" => FunctionName::Bm25Score,
            "CONTAINS" => FunctionName::Contains,
            _ => FunctionName::Other(s.to_string()),
        }
    }
}

/// Filter expression (WHERE clause)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterExpr {
    pub expr: Expr,
}

impl FilterExpr {
    pub fn new(expr: Expr) -> Self {
        Self { expr }
    }

    pub fn uses_vector_distance(&self) -> bool {
        self.expr.uses_vector_distance()
    }

    pub fn uses_bm25(&self) -> bool {
        self.expr.uses_bm25()
    }

    pub fn uses_contains(&self) -> bool {
        self.expr.uses_contains()
    }

    fn collect_parameters(&self, params: &mut Vec<String>) {
        self.expr.collect_parameters(params);
    }
}

/// ORDER BY clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderByClause {
    pub items: Vec<OrderByItem>,
}

impl OrderByClause {
    pub fn uses_vector_distance(&self) -> bool {
        self.items.iter().any(|i| i.expr.uses_vector_distance())
    }

    pub fn uses_bm25(&self) -> bool {
        self.items.iter().any(|i| i.expr.uses_bm25())
    }

    fn collect_parameters(&self, params: &mut Vec<String>) {
        for item in &self.items {
            item.expr.collect_parameters(params);
        }
    }
}

/// Single ORDER BY item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderByItem {
    pub expr: Expr,
    pub direction: SortDirection,
    pub nulls: Option<NullsOrder>,
}

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

/// NULLS ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NullsOrder {
    First,
    Last,
}

/// Query parameters for execution
#[derive(Debug, Clone, Default)]
pub struct QueryParams {
    /// Named parameters
    pub values: HashMap<String, ParamValue>,
}

impl QueryParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_vector(mut self, name: impl Into<String>, vector: Vec<f32>) -> Self {
        self.values.insert(name.into(), ParamValue::Vector(vector));
        self
    }

    pub fn with_string(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(name.into(), ParamValue::String(value.into()));
        self
    }

    pub fn with_int(mut self, name: impl Into<String>, value: i64) -> Self {
        self.values.insert(name.into(), ParamValue::Integer(value));
        self
    }

    pub fn with_float(mut self, name: impl Into<String>, value: f64) -> Self {
        self.values.insert(name.into(), ParamValue::Float(value));
        self
    }

    pub fn get(&self, name: &str) -> Option<&ParamValue> {
        self.values.get(name)
    }

    pub fn get_vector(&self, name: &str) -> Option<&[f32]> {
        match self.values.get(name) {
            Some(ParamValue::Vector(v)) => Some(v),
            _ => None,
        }
    }

    pub fn get_string(&self, name: &str) -> Option<&str> {
        match self.values.get(name) {
            Some(ParamValue::String(s)) => Some(s),
            _ => None,
        }
    }
}

/// Parameter value types
#[derive(Debug, Clone)]
pub enum ParamValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Vector(Vec<f32>),
    Null,
}

/// Known columns for invoices table
pub mod invoice_columns {
    pub const ID: &str = "id";
    pub const DOCUMENT_ID: &str = "document_id";
    pub const INVOICE_NUMBER: &str = "invoice_number";
    pub const INVOICE_DATE: &str = "invoice_date";
    pub const DUE_DATE: &str = "due_date";
    pub const PO_NUMBER: &str = "po_number";
    pub const VENDOR_NAME: &str = "vendor_name";
    pub const VENDOR_TAX_ID: &str = "vendor_tax_id";
    pub const BILL_TO_NAME: &str = "bill_to_name";
    pub const CURRENCY: &str = "currency";
    pub const SUBTOTAL: &str = "subtotal";
    pub const TAX_AMOUNT: &str = "tax_amount";
    pub const DISCOUNT_AMOUNT: &str = "discount_amount";
    pub const TOTAL_AMOUNT: &str = "total_amount";
    pub const VALIDATION_STATUS: &str = "validation_status";
    pub const CONFIDENCE_SCORE: &str = "confidence_score";
    pub const EXTRACTED_TEXT: &str = "extracted_text";
    pub const EMBEDDING: &str = "embedding";

    pub fn all() -> &'static [&'static str] {
        &[
            ID, DOCUMENT_ID, INVOICE_NUMBER, INVOICE_DATE, DUE_DATE, PO_NUMBER,
            VENDOR_NAME, VENDOR_TAX_ID, BILL_TO_NAME, CURRENCY, SUBTOTAL,
            TAX_AMOUNT, DISCOUNT_AMOUNT, TOTAL_AMOUNT, VALIDATION_STATUS,
            CONFIDENCE_SCORE, EXTRACTED_TEXT, EMBEDDING,
        ]
    }
}

/// Known columns for contracts table
pub mod contract_columns {
    pub const ID: &str = "id";
    pub const DOCUMENT_ID: &str = "document_id";
    pub const CONTRACT_NUMBER: &str = "contract_number";
    pub const TITLE: &str = "title";
    pub const CONTRACT_TYPE: &str = "contract_type";
    pub const EFFECTIVE_DATE: &str = "effective_date";
    pub const EXPIRATION_DATE: &str = "expiration_date";
    pub const EXECUTION_DATE: &str = "execution_date";
    pub const TOTAL_VALUE: &str = "total_value";
    pub const CURRENCY: &str = "currency";
    pub const VALIDATION_STATUS: &str = "validation_status";
    pub const CONFIDENCE_SCORE: &str = "confidence_score";
    pub const EXTRACTED_TEXT: &str = "extracted_text";
    pub const EMBEDDING: &str = "embedding";

    pub fn all() -> &'static [&'static str] {
        &[
            ID, DOCUMENT_ID, CONTRACT_NUMBER, TITLE, CONTRACT_TYPE,
            EFFECTIVE_DATE, EXPIRATION_DATE, EXECUTION_DATE, TOTAL_VALUE,
            CURRENCY, VALIDATION_STATUS, CONFIDENCE_SCORE, EXTRACTED_TEXT,
            EMBEDDING,
        ]
    }
}

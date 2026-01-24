//! JSON-based query format for visual query builders
//!
//! This module provides a JSON representation of queries that is easy to
//! construct programmatically and edit visually. It supports:
//! - Nested conditions (AND/OR with arbitrary depth)
//! - ZIP semantics for cross-table relationships
//! - Pipeline operations (validate, analyze)
//! - Full SQL-like query capabilities

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::ast::{
    BinaryOperator, ColumnRef, Expr, FenQuery, FilterExpr, FunctionCall, FunctionName,
    Literal, OrderByClause, OrderByItem, ParamValue, QueryParams, QueryTarget, SelectItem,
    SortDirection, NullsOrder,
};
use super::error::{ParseError, ParseErrorKind};
use super::span::Span;

/// JSON query root structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonQuery {
    /// SELECT clause - columns to return
    pub select: Vec<JsonSelectItem>,

    /// FROM clause - primary table
    pub from: JsonFromClause,

    /// Optional ZIP join with another table
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<JsonZipClause>,

    /// WHERE clause - filter conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#where: Option<JsonCondition>,

    /// ORDER BY clause
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<Vec<JsonOrderBy>>,

    /// LIMIT clause
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,

    /// OFFSET clause
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,

    /// Pipeline operations to apply to results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<Vec<JsonPipelineOp>>,

    /// Query parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<HashMap<String, JsonParamValue>>,
}

/// SELECT item (column, function, or wildcard)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JsonSelectItem {
    /// All columns (*)
    Wildcard,

    /// Table wildcard (table.*)
    TableWildcard { table: String },

    /// Single column reference
    Column {
        #[serde(skip_serializing_if = "Option::is_none")]
        table: Option<String>,
        column: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
    },

    /// Function call
    Function {
        name: JsonFunctionName,
        args: Vec<JsonExpr>,
        #[serde(skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
    },

    /// Arithmetic expression
    Expression {
        expr: JsonExpr,
        #[serde(skip_serializing_if = "Option::is_none")]
        alias: Option<String>,
    },
}

/// Function names supported in queries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JsonFunctionName {
    VectorDistance,
    Bm25Score,
    Contains,
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

/// FROM clause with table and optional alias
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonFromClause {
    /// Table name
    pub table: JsonTableName,

    /// Optional alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

/// Supported table names
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonTableName {
    Invoices,
    Contracts,
}

/// ZIP clause for cross-table relationships
///
/// ZIP semantics allow querying invoices and contracts together
/// with shared filter conditions, useful for:
/// - Finding contracts that relate to specific invoices
/// - Cross-validating invoice terms against contract clauses
/// - Analyzing relationships between documents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonZipClause {
    /// Table to zip with
    pub table: JsonTableName,

    /// Alias for the zipped table
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// Join condition (how tables relate)
    pub on: JsonCondition,

    /// ZIP mode
    #[serde(default)]
    pub mode: JsonZipMode,
}

/// ZIP modes for cross-table queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonZipMode {
    /// Inner join - only matching pairs
    #[default]
    Inner,

    /// Left join - all from primary, matching from secondary
    Left,

    /// Cross product with filter
    Cross,

    /// Cartesian product (all combinations)
    Cartesian,
}

/// Condition (WHERE clause) with nested AND/OR support
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JsonCondition {
    /// Simple comparison
    Compare {
        left: JsonExpr,
        op: JsonCompareOp,
        right: JsonExpr,
    },

    /// BETWEEN condition
    Between {
        expr: JsonExpr,
        low: JsonExpr,
        high: JsonExpr,
        #[serde(default)]
        negated: bool,
    },

    /// IN list condition
    In {
        expr: JsonExpr,
        values: Vec<JsonExpr>,
        #[serde(default)]
        negated: bool,
    },

    /// IS NULL / IS NOT NULL
    IsNull {
        expr: JsonExpr,
        #[serde(default)]
        negated: bool,
    },

    /// LIKE pattern matching
    Like {
        expr: JsonExpr,
        pattern: String,
        #[serde(default)]
        case_insensitive: bool,
        #[serde(default)]
        negated: bool,
    },

    /// Function-based condition (e.g., CONTAINS)
    Function {
        name: JsonFunctionName,
        args: Vec<JsonExpr>,
    },

    /// AND of multiple conditions
    And {
        conditions: Vec<JsonCondition>,
    },

    /// OR of multiple conditions
    Or {
        conditions: Vec<JsonCondition>,
    },

    /// NOT of a condition
    Not {
        condition: Box<JsonCondition>,
    },

    /// Nested subquery condition (EXISTS, IN subquery)
    Exists {
        subquery: Box<JsonQuery>,
        #[serde(default)]
        negated: bool,
    },
}

/// Comparison operators
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonCompareOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Like,
    ILike,
}

/// Expression types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JsonExpr {
    /// Column reference
    Column {
        #[serde(skip_serializing_if = "Option::is_none")]
        table: Option<String>,
        column: String,
    },

    /// Literal value
    Literal {
        value: JsonValue,
    },

    /// Parameter reference
    Param {
        name: String,
    },

    /// Function call
    Function {
        name: JsonFunctionName,
        args: Vec<JsonExpr>,
    },

    /// Binary arithmetic operation
    BinaryOp {
        left: Box<JsonExpr>,
        op: JsonBinaryOp,
        right: Box<JsonExpr>,
    },

    /// Unary operation (e.g., negation)
    UnaryOp {
        op: JsonUnaryOp,
        expr: Box<JsonExpr>,
    },
}

/// Binary operators for expressions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// Unary operators
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonUnaryOp {
    Neg,
    Not,
}

/// JSON value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<JsonValue>),
}

/// ORDER BY item
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonOrderBy {
    /// Expression to sort by
    pub expr: JsonExpr,

    /// Sort direction
    #[serde(default)]
    pub direction: JsonSortDirection,

    /// NULLS FIRST/LAST
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nulls: Option<JsonNullsOrder>,
}

/// Sort direction
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonSortDirection {
    #[default]
    Asc,
    Desc,
}

/// Nulls ordering
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonNullsOrder {
    First,
    Last,
}

/// Pipeline operation for post-processing results
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum JsonPipelineOp {
    /// Validate documents against rules
    Validate {
        #[serde(skip_serializing_if = "Option::is_none")]
        rules: Option<Vec<String>>,
        #[serde(default)]
        fail_fast: bool,
    },

    /// Analyze documents for anomalies
    Analyze {
        #[serde(skip_serializing_if = "Option::is_none")]
        analyzers: Option<Vec<String>>,
        #[serde(default)]
        include_scores: bool,
    },

    /// Compare invoices against contracts
    CrossValidate {
        /// Field mapping between invoice and contract
        field_mapping: HashMap<String, String>,
        /// Tolerance for numeric comparisons
        #[serde(default)]
        tolerance: f64,
    },

    /// Aggregate results
    Aggregate {
        /// Group by expressions
        group_by: Vec<JsonExpr>,
        /// Aggregations to compute
        aggregations: Vec<JsonAggregation>,
    },

    /// Transform results
    Transform {
        /// Output field mappings
        mappings: HashMap<String, JsonExpr>,
    },

    /// Filter results (post-query)
    Filter {
        condition: JsonCondition,
    },

    /// Sort results (post-query)
    Sort {
        order_by: Vec<JsonOrderBy>,
    },

    /// Limit results (post-query)
    Take {
        limit: u64,
        #[serde(default)]
        offset: u64,
    },
}

/// Aggregation specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonAggregation {
    /// Aggregation function
    pub function: JsonAggFunction,
    /// Expression to aggregate
    pub expr: JsonExpr,
    /// Output alias
    pub alias: String,
}

/// Aggregation functions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonAggFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    First,
    Last,
    Collect,
}

/// Parameter value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum JsonParamValue {
    String { value: String },
    Int { value: i64 },
    Float { value: f64 },
    Bool { value: bool },
    Vector { value: Vec<f32> },
    Array { value: Vec<JsonValue> },
    Null,
}

// ============================================================================
// Conversion: JSON Query -> AST Query
// ============================================================================

impl JsonQuery {
    /// Parse a JSON query from a string
    pub fn from_json(json: &str) -> Result<Self, ParseError> {
        serde_json::from_str(json).map_err(|e| {
            ParseError::new(
                ParseErrorKind::InvalidSyntax(format!("Invalid JSON query: {}", e)),
                Span::default(),
                json,
            )
        })
    }

    /// Serialize query to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Convert to FenQuery AST (for non-ZIP queries)
    pub fn to_ast(&self) -> Result<FenQuery, ParseError> {
        // Convert FROM clause
        let from = match self.from.table {
            JsonTableName::Invoices => QueryTarget::Invoices,
            JsonTableName::Contracts => QueryTarget::Contracts,
        };

        // Convert SELECT items
        let select = self
            .select
            .iter()
            .map(|item| item.to_ast())
            .collect::<Result<Vec<_>, _>>()?;

        // Convert WHERE clause
        let filter = self
            .r#where
            .as_ref()
            .map(|cond| cond.to_filter_expr())
            .transpose()?;

        // Convert ORDER BY
        let order_by = self
            .order_by
            .as_ref()
            .map(|items| {
                let order_items = items
                    .iter()
                    .map(|item| item.to_ast())
                    .collect::<Result<Vec<_>, _>>()?;
                Ok::<_, ParseError>(OrderByClause { items: order_items })
            })
            .transpose()?;

        Ok(FenQuery {
            select,
            from,
            from_alias: self.from.alias.clone(),
            filter,
            order_by,
            limit: self.limit,
            offset: self.offset,
            zip: None, // ZIP conversion handled separately
            pipeline: None, // Pipeline conversion handled separately
        })
    }

    /// Convert parameters to QueryParams
    pub fn to_params(&self) -> QueryParams {
        let mut params = QueryParams::new();

        if let Some(param_map) = &self.params {
            for (name, value) in param_map {
                let param_value = match value {
                    JsonParamValue::String { value } => ParamValue::String(value.clone()),
                    JsonParamValue::Int { value } => ParamValue::Integer(*value),
                    JsonParamValue::Float { value } => ParamValue::Float(*value),
                    JsonParamValue::Bool { value } => ParamValue::Boolean(*value),
                    JsonParamValue::Vector { value } => ParamValue::Vector(value.clone()),
                    JsonParamValue::Array { .. } => continue, // Skip arrays for now
                    JsonParamValue::Null => ParamValue::Null,
                };
                params = params.with_param(name, param_value);
            }
        }

        params
    }

    /// Check if this query uses ZIP semantics
    pub fn is_zip_query(&self) -> bool {
        self.zip.is_some()
    }

    /// Check if this query has pipeline operations
    pub fn has_pipeline(&self) -> bool {
        self.pipeline.as_ref().map(|p| !p.is_empty()).unwrap_or(false)
    }

    /// Get the primary table name
    pub fn primary_table(&self) -> &JsonTableName {
        &self.from.table
    }

    /// Get the ZIP table name if present
    pub fn zip_table(&self) -> Option<&JsonTableName> {
        self.zip.as_ref().map(|z| &z.table)
    }
}

impl JsonSelectItem {
    fn to_ast(&self) -> Result<SelectItem, ParseError> {
        match self {
            JsonSelectItem::Wildcard => Ok(SelectItem {
                expr: Expr::Wildcard,
                alias: None,
            }),

            JsonSelectItem::TableWildcard { table } => Ok(SelectItem {
                expr: Expr::Column(ColumnRef {
                    table: Some(table.clone()),
                    column: "*".to_string(),
                }),
                alias: None,
            }),

            JsonSelectItem::Column { table, column, alias } => Ok(SelectItem {
                expr: Expr::Column(ColumnRef {
                    table: table.clone(),
                    column: column.clone(),
                }),
                alias: alias.clone(),
            }),

            JsonSelectItem::Function { name, args, alias } => {
                let func_name = name.to_ast();
                let func_args = args
                    .iter()
                    .map(|arg| arg.to_ast())
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(SelectItem {
                    expr: Expr::Function(FunctionCall {
                        name: func_name,
                        args: func_args,
                    }),
                    alias: alias.clone(),
                })
            }

            JsonSelectItem::Expression { expr, alias } => Ok(SelectItem {
                expr: expr.to_ast()?,
                alias: alias.clone(),
            }),
        }
    }
}

impl JsonFunctionName {
    fn to_ast(&self) -> FunctionName {
        match self {
            JsonFunctionName::VectorDistance => FunctionName::VectorDistance,
            JsonFunctionName::Bm25Score => FunctionName::Bm25Score,
            JsonFunctionName::Contains => FunctionName::Contains,
            JsonFunctionName::Count => FunctionName::Count,
            JsonFunctionName::Sum => FunctionName::Sum,
            JsonFunctionName::Avg => FunctionName::Avg,
            JsonFunctionName::Min => FunctionName::Min,
            JsonFunctionName::Max => FunctionName::Max,
        }
    }
}

impl JsonCondition {
    fn to_filter_expr(&self) -> Result<FilterExpr, ParseError> {
        Ok(FilterExpr {
            expr: self.to_ast()?,
        })
    }

    fn to_ast(&self) -> Result<Expr, ParseError> {
        match self {
            JsonCondition::Compare { left, op, right } => {
                let left_expr = left.to_ast()?;
                let right_expr = right.to_ast()?;
                let binary_op = op.to_ast();

                Ok(Expr::BinaryOp {
                    left: Box::new(left_expr),
                    op: binary_op,
                    right: Box::new(right_expr),
                })
            }

            JsonCondition::Between { expr, low, high, negated } => {
                let expr_ast = expr.to_ast()?;
                let low_ast = low.to_ast()?;
                let high_ast = high.to_ast()?;

                // expr >= low AND expr <= high
                let between = Expr::BinaryOp {
                    left: Box::new(Expr::BinaryOp {
                        left: Box::new(expr_ast.clone()),
                        op: BinaryOperator::GtEq,
                        right: Box::new(low_ast),
                    }),
                    op: BinaryOperator::And,
                    right: Box::new(Expr::BinaryOp {
                        left: Box::new(expr_ast),
                        op: BinaryOperator::LtEq,
                        right: Box::new(high_ast),
                    }),
                };

                if *negated {
                    Ok(Expr::Not(Box::new(between)))
                } else {
                    Ok(between)
                }
            }

            JsonCondition::In { expr, values, negated } => {
                let expr_ast = expr.to_ast()?;
                let value_asts: Vec<Expr> = values
                    .iter()
                    .map(|v| v.to_ast())
                    .collect::<Result<_, _>>()?;

                // Build OR chain: expr = v1 OR expr = v2 OR ...
                let in_expr = value_asts
                    .into_iter()
                    .map(|v| Expr::BinaryOp {
                        left: Box::new(expr_ast.clone()),
                        op: BinaryOperator::Eq,
                        right: Box::new(v),
                    })
                    .reduce(|acc, e| Expr::BinaryOp {
                        left: Box::new(acc),
                        op: BinaryOperator::Or,
                        right: Box::new(e),
                    })
                    .unwrap_or(Expr::Literal(Literal::Boolean(false)));

                if *negated {
                    Ok(Expr::Not(Box::new(in_expr)))
                } else {
                    Ok(in_expr)
                }
            }

            JsonCondition::IsNull { expr, negated } => {
                let expr_ast = expr.to_ast()?;
                let is_null = Expr::IsNull {
                    expr: Box::new(expr_ast),
                    negated: *negated,
                };
                Ok(is_null)
            }

            JsonCondition::Like { expr, pattern, case_insensitive, negated } => {
                let expr_ast = expr.to_ast()?;
                let op = if *case_insensitive {
                    BinaryOperator::ILike
                } else {
                    BinaryOperator::Like
                };

                let like_expr = Expr::BinaryOp {
                    left: Box::new(expr_ast),
                    op,
                    right: Box::new(Expr::Literal(Literal::String(pattern.clone()))),
                };

                if *negated {
                    Ok(Expr::Not(Box::new(like_expr)))
                } else {
                    Ok(like_expr)
                }
            }

            JsonCondition::Function { name, args } => {
                let func_name = name.to_ast();
                let func_args = args
                    .iter()
                    .map(|arg| arg.to_ast())
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Expr::Function(FunctionCall {
                    name: func_name,
                    args: func_args,
                }))
            }

            JsonCondition::And { conditions } => {
                let exprs: Vec<Expr> = conditions
                    .iter()
                    .map(|c| c.to_ast())
                    .collect::<Result<_, _>>()?;

                exprs
                    .into_iter()
                    .reduce(|acc, e| Expr::BinaryOp {
                        left: Box::new(acc),
                        op: BinaryOperator::And,
                        right: Box::new(e),
                    })
                    .ok_or_else(|| {
                        ParseError::new(
                            ParseErrorKind::InvalidSyntax("Empty AND condition".to_string()),
                            Span::default(),
                            "",
                        )
                    })
            }

            JsonCondition::Or { conditions } => {
                let exprs: Vec<Expr> = conditions
                    .iter()
                    .map(|c| c.to_ast())
                    .collect::<Result<_, _>>()?;

                exprs
                    .into_iter()
                    .reduce(|acc, e| Expr::BinaryOp {
                        left: Box::new(acc),
                        op: BinaryOperator::Or,
                        right: Box::new(e),
                    })
                    .ok_or_else(|| {
                        ParseError::new(
                            ParseErrorKind::InvalidSyntax("Empty OR condition".to_string()),
                            Span::default(),
                            "",
                        )
                    })
            }

            JsonCondition::Not { condition } => {
                let inner = condition.to_ast()?;
                Ok(Expr::Not(Box::new(inner)))
            }

            JsonCondition::Exists { subquery: _, negated: _ } => {
                // Subqueries not fully supported in AST yet
                Err(ParseError::new(
                    ParseErrorKind::UnsupportedFeature("EXISTS subquery".to_string()),
                    Span::default(),
                    "",
                ))
            }
        }
    }
}

impl JsonCompareOp {
    fn to_ast(&self) -> BinaryOperator {
        match self {
            JsonCompareOp::Eq => BinaryOperator::Eq,
            JsonCompareOp::NotEq => BinaryOperator::NotEq,
            JsonCompareOp::Lt => BinaryOperator::Lt,
            JsonCompareOp::LtEq => BinaryOperator::LtEq,
            JsonCompareOp::Gt => BinaryOperator::Gt,
            JsonCompareOp::GtEq => BinaryOperator::GtEq,
            JsonCompareOp::Like => BinaryOperator::Like,
            JsonCompareOp::ILike => BinaryOperator::ILike,
        }
    }
}

impl JsonExpr {
    fn to_ast(&self) -> Result<Expr, ParseError> {
        match self {
            JsonExpr::Column { table, column } => Ok(Expr::Column(ColumnRef {
                table: table.clone(),
                column: column.clone(),
            })),

            JsonExpr::Literal { value } => Ok(Expr::Literal(value.to_ast())),

            JsonExpr::Param { name } => Ok(Expr::Parameter(name.clone())),

            JsonExpr::Function { name, args } => {
                let func_name = name.to_ast();
                let func_args = args
                    .iter()
                    .map(|arg| arg.to_ast())
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Expr::Function(FunctionCall {
                    name: func_name,
                    args: func_args,
                }))
            }

            JsonExpr::BinaryOp { left, op, right } => {
                let left_ast = left.to_ast()?;
                let right_ast = right.to_ast()?;
                let binary_op = match op {
                    JsonBinaryOp::Add => BinaryOperator::Add,
                    JsonBinaryOp::Sub => BinaryOperator::Sub,
                    JsonBinaryOp::Mul => BinaryOperator::Mul,
                    JsonBinaryOp::Div => BinaryOperator::Div,
                    JsonBinaryOp::Mod => BinaryOperator::Mod,
                };

                Ok(Expr::BinaryOp {
                    left: Box::new(left_ast),
                    op: binary_op,
                    right: Box::new(right_ast),
                })
            }

            JsonExpr::UnaryOp { op, expr } => {
                let inner = expr.to_ast()?;
                match op {
                    JsonUnaryOp::Neg => Ok(Expr::BinaryOp {
                        left: Box::new(Expr::Literal(Literal::Integer(0))),
                        op: BinaryOperator::Sub,
                        right: Box::new(inner),
                    }),
                    JsonUnaryOp::Not => Ok(Expr::Not(Box::new(inner))),
                }
            }
        }
    }
}

impl JsonValue {
    fn to_ast(&self) -> Literal {
        match self {
            JsonValue::Null => Literal::Null,
            JsonValue::Bool(b) => Literal::Boolean(*b),
            JsonValue::Int(i) => Literal::Integer(*i),
            JsonValue::Float(f) => Literal::Float(*f),
            JsonValue::String(s) => Literal::String(s.clone()),
            JsonValue::Array(arr) => {
                let values: Vec<Literal> = arr.iter().map(|v| v.to_ast()).collect();
                Literal::Array(values)
            }
        }
    }
}

impl JsonOrderBy {
    fn to_ast(&self) -> Result<OrderByItem, ParseError> {
        let expr = self.expr.to_ast()?;
        let direction = match self.direction {
            JsonSortDirection::Asc => SortDirection::Asc,
            JsonSortDirection::Desc => SortDirection::Desc,
        };
        let nulls = self.nulls.as_ref().map(|n| match n {
            JsonNullsOrder::First => NullsOrder::First,
            JsonNullsOrder::Last => NullsOrder::Last,
        });

        Ok(OrderByItem {
            expr,
            direction,
            nulls,
        })
    }
}

// ============================================================================
// Builder API for programmatic query construction
// ============================================================================

impl JsonQuery {
    /// Create a new query builder for invoices
    pub fn invoices() -> JsonQueryBuilder {
        JsonQueryBuilder::new(JsonTableName::Invoices)
    }

    /// Create a new query builder for contracts
    pub fn contracts() -> JsonQueryBuilder {
        JsonQueryBuilder::new(JsonTableName::Contracts)
    }
}

/// Builder for constructing JSON queries programmatically
#[derive(Debug, Clone)]
pub struct JsonQueryBuilder {
    query: JsonQuery,
}

impl JsonQueryBuilder {
    /// Create a new builder
    pub fn new(table: JsonTableName) -> Self {
        Self {
            query: JsonQuery {
                select: vec![JsonSelectItem::Wildcard],
                from: JsonFromClause { table, alias: None },
                zip: None,
                r#where: None,
                order_by: None,
                limit: None,
                offset: None,
                pipeline: None,
                params: None,
            },
        }
    }

    /// Set table alias
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.query.from.alias = Some(alias.into());
        self
    }

    /// Select specific columns
    pub fn select(mut self, items: Vec<JsonSelectItem>) -> Self {
        self.query.select = items;
        self
    }

    /// Select all columns
    pub fn select_all(mut self) -> Self {
        self.query.select = vec![JsonSelectItem::Wildcard];
        self
    }

    /// Add a column to select
    pub fn select_column(mut self, column: impl Into<String>) -> Self {
        self.query.select.push(JsonSelectItem::Column {
            table: None,
            column: column.into(),
            alias: None,
        });
        self
    }

    /// Add a column with alias
    pub fn select_column_as(mut self, column: impl Into<String>, alias: impl Into<String>) -> Self {
        self.query.select.push(JsonSelectItem::Column {
            table: None,
            column: column.into(),
            alias: Some(alias.into()),
        });
        self
    }

    /// Add a function to select
    pub fn select_function(
        mut self,
        name: JsonFunctionName,
        args: Vec<JsonExpr>,
        alias: Option<String>,
    ) -> Self {
        self.query.select.push(JsonSelectItem::Function {
            name,
            args,
            alias,
        });
        self
    }

    /// Set WHERE condition
    pub fn filter(mut self, condition: JsonCondition) -> Self {
        self.query.r#where = Some(condition);
        self
    }

    /// Add ZIP join
    pub fn zip(mut self, table: JsonTableName, on: JsonCondition) -> Self {
        self.query.zip = Some(JsonZipClause {
            table,
            alias: None,
            on,
            mode: JsonZipMode::Inner,
        });
        self
    }

    /// Add ZIP join with alias and mode
    pub fn zip_with(
        mut self,
        table: JsonTableName,
        alias: impl Into<String>,
        on: JsonCondition,
        mode: JsonZipMode,
    ) -> Self {
        self.query.zip = Some(JsonZipClause {
            table,
            alias: Some(alias.into()),
            on,
            mode,
        });
        self
    }

    /// Set ORDER BY
    pub fn order_by(mut self, items: Vec<JsonOrderBy>) -> Self {
        self.query.order_by = Some(items);
        self
    }

    /// Order by a single column ascending
    pub fn order_by_asc(mut self, column: impl Into<String>) -> Self {
        let items = self.query.order_by.get_or_insert_with(Vec::new);
        items.push(JsonOrderBy {
            expr: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            direction: JsonSortDirection::Asc,
            nulls: None,
        });
        self
    }

    /// Order by a single column descending
    pub fn order_by_desc(mut self, column: impl Into<String>) -> Self {
        let items = self.query.order_by.get_or_insert_with(Vec::new);
        items.push(JsonOrderBy {
            expr: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            direction: JsonSortDirection::Desc,
            nulls: None,
        });
        self
    }

    /// Set LIMIT
    pub fn limit(mut self, limit: u64) -> Self {
        self.query.limit = Some(limit);
        self
    }

    /// Set OFFSET
    pub fn offset(mut self, offset: u64) -> Self {
        self.query.offset = Some(offset);
        self
    }

    /// Add a pipeline operation
    pub fn pipe(mut self, op: JsonPipelineOp) -> Self {
        self.query.pipeline.get_or_insert_with(Vec::new).push(op);
        self
    }

    /// Add validate pipeline operation
    pub fn validate(self) -> Self {
        self.pipe(JsonPipelineOp::Validate {
            rules: None,
            fail_fast: false,
        })
    }

    /// Add analyze pipeline operation
    pub fn analyze(self) -> Self {
        self.pipe(JsonPipelineOp::Analyze {
            analyzers: None,
            include_scores: true,
        })
    }

    /// Add a parameter
    pub fn param(mut self, name: impl Into<String>, value: JsonParamValue) -> Self {
        self.query
            .params
            .get_or_insert_with(HashMap::new)
            .insert(name.into(), value);
        self
    }

    /// Build the query
    pub fn build(self) -> JsonQuery {
        self.query
    }
}

// ============================================================================
// Condition builder helpers
// ============================================================================

impl JsonCondition {
    /// Create an equality condition
    pub fn eq(column: impl Into<String>, value: JsonValue) -> Self {
        JsonCondition::Compare {
            left: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            op: JsonCompareOp::Eq,
            right: JsonExpr::Literal { value },
        }
    }

    /// Create a parameter equality condition
    pub fn eq_param(column: impl Into<String>, param: impl Into<String>) -> Self {
        JsonCondition::Compare {
            left: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            op: JsonCompareOp::Eq,
            right: JsonExpr::Param { name: param.into() },
        }
    }

    /// Create a greater-than condition
    pub fn gt(column: impl Into<String>, value: JsonValue) -> Self {
        JsonCondition::Compare {
            left: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            op: JsonCompareOp::Gt,
            right: JsonExpr::Literal { value },
        }
    }

    /// Create a less-than condition
    pub fn lt(column: impl Into<String>, value: JsonValue) -> Self {
        JsonCondition::Compare {
            left: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            op: JsonCompareOp::Lt,
            right: JsonExpr::Literal { value },
        }
    }

    /// Create a LIKE condition
    pub fn like(column: impl Into<String>, pattern: impl Into<String>) -> Self {
        JsonCondition::Like {
            expr: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            pattern: pattern.into(),
            case_insensitive: false,
            negated: false,
        }
    }

    /// Create an IS NULL condition
    pub fn is_null(column: impl Into<String>) -> Self {
        JsonCondition::IsNull {
            expr: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            negated: false,
        }
    }

    /// Create an IS NOT NULL condition
    pub fn is_not_null(column: impl Into<String>) -> Self {
        JsonCondition::IsNull {
            expr: JsonExpr::Column {
                table: None,
                column: column.into(),
            },
            negated: true,
        }
    }

    /// Create a CONTAINS condition
    pub fn contains(column: impl Into<String>, search: impl Into<String>) -> Self {
        JsonCondition::Function {
            name: JsonFunctionName::Contains,
            args: vec![
                JsonExpr::Column {
                    table: None,
                    column: column.into(),
                },
                JsonExpr::Literal {
                    value: JsonValue::String(search.into()),
                },
            ],
        }
    }

    /// Create a CONTAINS with parameter condition
    pub fn contains_param(column: impl Into<String>, param: impl Into<String>) -> Self {
        JsonCondition::Function {
            name: JsonFunctionName::Contains,
            args: vec![
                JsonExpr::Column {
                    table: None,
                    column: column.into(),
                },
                JsonExpr::Param { name: param.into() },
            ],
        }
    }

    /// Combine with AND
    pub fn and(self, other: JsonCondition) -> Self {
        match self {
            JsonCondition::And { mut conditions } => {
                conditions.push(other);
                JsonCondition::And { conditions }
            }
            _ => JsonCondition::And {
                conditions: vec![self, other],
            },
        }
    }

    /// Combine with OR
    pub fn or(self, other: JsonCondition) -> Self {
        match self {
            JsonCondition::Or { mut conditions } => {
                conditions.push(other);
                JsonCondition::Or { conditions }
            }
            _ => JsonCondition::Or {
                conditions: vec![self, other],
            },
        }
    }

    /// Negate condition
    pub fn not(self) -> Self {
        JsonCondition::Not {
            condition: Box::new(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_query_parsing() {
        let json = r#"{
            "select": [{ "type": "wildcard" }],
            "from": { "table": "invoices" },
            "where": {
                "type": "compare",
                "left": { "type": "column", "column": "vendor_name" },
                "op": "eq",
                "right": { "type": "literal", "value": "Acme Corp" }
            },
            "limit": 10
        }"#;

        let query = JsonQuery::from_json(json).unwrap();
        assert_eq!(query.limit, Some(10));
        assert!(query.r#where.is_some());
    }

    #[test]
    fn test_nested_conditions() {
        let json = r#"{
            "select": [{ "type": "wildcard" }],
            "from": { "table": "invoices" },
            "where": {
                "type": "and",
                "conditions": [
                    {
                        "type": "compare",
                        "left": { "type": "column", "column": "vendor_name" },
                        "op": "eq",
                        "right": { "type": "literal", "value": "Acme" }
                    },
                    {
                        "type": "or",
                        "conditions": [
                            {
                                "type": "compare",
                                "left": { "type": "column", "column": "total_amount" },
                                "op": "gt",
                                "right": { "type": "literal", "value": 1000 }
                            },
                            {
                                "type": "isNull",
                                "expr": { "type": "column", "column": "po_number" }
                            }
                        ]
                    }
                ]
            }
        }"#;

        let query = JsonQuery::from_json(json).unwrap();
        let ast = query.to_ast().unwrap();
        assert!(ast.filter.is_some());
    }

    #[test]
    fn test_builder_api() {
        let query = JsonQuery::invoices()
            .alias("inv")
            .select_column("id")
            .select_column_as("invoice_number", "num")
            .filter(
                JsonCondition::eq("vendor_name", JsonValue::String("Acme".to_string()))
                    .and(JsonCondition::gt("total_amount", JsonValue::Float(1000.0))),
            )
            .order_by_desc("invoice_date")
            .limit(10)
            .validate()
            .build();

        assert_eq!(query.select.len(), 3); // wildcard + 2 columns
        assert!(query.r#where.is_some());
        assert!(query.pipeline.is_some());
    }

    #[test]
    fn test_zip_query() {
        let query = JsonQuery::invoices()
            .alias("inv")
            .zip_with(
                JsonTableName::Contracts,
                "con",
                JsonCondition::Compare {
                    left: JsonExpr::Column {
                        table: Some("inv".to_string()),
                        column: "vendor_name".to_string(),
                    },
                    op: JsonCompareOp::Eq,
                    right: JsonExpr::Column {
                        table: Some("con".to_string()),
                        column: "party_name".to_string(),
                    },
                },
                JsonZipMode::Inner,
            )
            .build();

        assert!(query.is_zip_query());
        assert_eq!(
            query.zip.as_ref().unwrap().alias,
            Some("con".to_string())
        );
    }

    #[test]
    fn test_to_json() {
        let query = JsonQuery::invoices()
            .filter(JsonCondition::eq(
                "vendor_name",
                JsonValue::String("Acme".to_string()),
            ))
            .limit(10)
            .build();

        let json = query.to_json().unwrap();
        assert!(json.contains("invoices"));
        assert!(json.contains("Acme"));
    }

    #[test]
    fn test_pipeline_operations() {
        let query = JsonQuery::invoices()
            .pipe(JsonPipelineOp::Validate {
                rules: Some(vec!["math_check".to_string()]),
                fail_fast: true,
            })
            .pipe(JsonPipelineOp::Analyze {
                analyzers: Some(vec!["anomaly_detector".to_string()]),
                include_scores: true,
            })
            .pipe(JsonPipelineOp::CrossValidate {
                field_mapping: [("total_amount".to_string(), "contract_value".to_string())]
                    .into_iter()
                    .collect(),
                tolerance: 0.01,
            })
            .build();

        assert_eq!(query.pipeline.as_ref().unwrap().len(), 3);
    }
}

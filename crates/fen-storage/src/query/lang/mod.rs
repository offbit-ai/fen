//! Unified query language for Fen
//!
//! This module provides a SQL-like query language with extensions for:
//! - Vector similarity search (VECTOR_DISTANCE)
//! - Full-text BM25 search (BM25_SCORE)
//! - Text containment (CONTAINS)
//! - Score fusion for hybrid queries
//! - ZIP queries for cross-table joins between invoices and contracts
//!
//! # Query Language Overview
//!
//! The Fen query language is designed for querying invoices and contracts with support for:
//! - Standard SQL-like syntax (SELECT, FROM, WHERE, ORDER BY, LIMIT)
//! - Parameterized queries with `:param` syntax
//! - ZIP joins for cross-table relationship queries
//! - Pipeline operations using `|>` syntax for validation, analysis, and transformation
//!
//! See the full documentation at `docs/QUERY_LANGUAGE.md`.
//!
//! # Basic Query Example
//!
//! ```text
//! SELECT inv.id, inv.invoice_number, inv.total_amount
//! FROM invoices inv
//! WHERE inv.vendor_name = 'Acme Corp'
//!     AND inv.total_amount > 1000
//! ORDER BY inv.invoice_date DESC
//! LIMIT 10
//! ```
//!
//! # Hybrid Search Example
//!
//! Combine vector similarity and BM25 text search for optimal results:
//!
//! ```text
//! SELECT inv.id, inv.invoice_number,
//!     VECTOR_DISTANCE(inv.embedding, :query_vector) AS semantic_score,
//!     BM25_SCORE(inv.extracted_text, :search_terms) AS text_score
//! FROM invoices inv
//! WHERE inv.vendor_name = 'Acme Corp'
//!     AND VECTOR_DISTANCE(inv.embedding, :query_vector) < 0.3
//!     AND CONTAINS(inv.extracted_text, :search_terms)
//! ORDER BY 0.7 * semantic_score + 0.3 * text_score ASC
//! LIMIT 10
//! ```
//!
//! # ZIP Query Example
//!
//! Join invoices and contracts to analyze relationships:
//!
//! ```text
//! SELECT inv.invoice_number, inv.total_amount, con.title, con.total_value
//! FROM invoices inv
//! ZIP contracts con ON inv.vendor_name = con.party_name
//! WHERE inv.total_amount > 1000
//!     AND inv.total_amount < con.total_value
//! ORDER BY inv.total_amount DESC
//! ```
//!
//! ZIP queries support:
//! - `ZIP` or `INNER ZIP` - Only matching pairs
//! - `LEFT ZIP` - All invoices, matching contracts (or NULL)
//! - `CROSS ZIP` - Cartesian product with filter
//!
//! # Usage
//!
//! ## Parsing SQL Queries
//!
//! ```rust,ignore
//! use fen_storage::{parse_query, QueryParams};
//!
//! // Parse a SQL query
//! let query = parse_query(
//!     "SELECT * FROM invoices WHERE vendor_name = :vendor"
//! )?;
//!
//! // Create parameters
//! let params = QueryParams::new()
//!     .with_string("vendor", "Acme Corp");
//!
//! // Execute with QueryExecutor
//! let result = executor.execute(&query, &params).await?;
//! ```
//!
//! ## Using JSON Queries
//!
//! ```rust,ignore
//! use fen_storage::{JsonQuery, JsonCondition, JsonValue};
//!
//! // Build query programmatically
//! let query = JsonQuery::invoices()
//!     .alias("inv")
//!     .select_column("invoice_number")
//!     .filter(JsonCondition::eq("vendor_name", JsonValue::String("Acme".into())))
//!     .order_by_desc("invoice_date")
//!     .limit(10)
//!     .build();
//!
//! // Convert to AST
//! let ast = query.to_ast()?;
//! let params = query.to_params();
//! ```
//!
//! # Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | `VECTOR_DISTANCE(col, vec)` | Cosine distance to query vector |
//! | `BM25_SCORE(col, terms)` | BM25 relevance score |
//! | `CONTAINS(col, terms)` | Full-text containment check |
//! | `COUNT(*)`, `SUM(col)`, etc. | Aggregate functions |
//!
//! # Column Reference
//!
//! ## Invoice Columns
//!
//! `id`, `document_id`, `invoice_number`, `invoice_date`, `due_date`, `po_number`,
//! `contract_id`, `contract_number`, `vendor_name`, `vendor_tax_id`, `bill_to_name`,
//! `currency`, `subtotal`, `tax_amount`, `discount_amount`, `total_amount`,
//! `validation_status`, `confidence_score`, `extracted_text`, `embedding`
//!
//! ## Contract Columns
//!
//! `id`, `document_id`, `contract_number`, `title`, `contract_type`, `effective_date`,
//! `expiration_date`, `execution_date`, `total_value`, `currency`, `party_name`,
//! `validation_status`, `confidence_score`, `extracted_text`, `embedding`
//!
//! # See Also
//!
//! - [`parse_query`] - Parse SQL string to AST
//! - [`QueryParser`] - Low-level parser access
//! - [`FenQuery`] - Parsed query AST
//! - [`JsonQuery`] - JSON query format
//! - [`QueryParams`] - Query parameters

mod ast;
mod error;
pub mod json;
mod lexer;
mod parser;
mod span;

pub use ast::*;
pub use error::{ParseError, ParseErrorKind, QueryError};
pub use json::{JsonCondition, JsonPipelineOp, JsonQuery, JsonQueryBuilder, JsonZipClause};
pub use parser::{parse_query, QueryParser};
pub use span::Span;

//! Unified query language for Fen
//!
//! This module provides a SQL-like query language with extensions for:
//! - Vector similarity search (VECTOR_DISTANCE)
//! - Full-text BM25 search (BM25_SCORE)
//! - Text containment (CONTAINS)
//! - Score fusion for hybrid queries
//!
//! # Example
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

mod ast;
mod error;
mod lexer;
mod parser;
mod span;

pub use ast::*;
pub use error::{ParseError, QueryError};
pub use parser::{parse_query, QueryParser};
pub use span::Span;

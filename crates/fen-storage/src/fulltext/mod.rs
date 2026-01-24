//! Full-text search using Tantivy
//!
//! This module provides BM25-based full-text search capabilities for
//! invoices and contracts using the Tantivy search engine.

mod index;

pub use index::{FullTextIndex, FullTextConfig, SearchResult};

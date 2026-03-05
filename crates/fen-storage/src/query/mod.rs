//! Query engine for the Fen query language (FQL)
//!
//! # Modules
//!
//! - [`lang`] — Parser, AST, and JSON query builder
//! - `executor` — Async query executor (I/O: hot/warm storage, vector/text search)
//! - [`morsel`] — Morsel-driven parallel filter, score, sort, and projection
//! - `zip_executor` — Cross-table ZIP queries with pipeline operations
//! - `engine` — Tiered query routing (hot → warm → cold)
//! - `cache` — LRU query cache with TTL

mod cache;
mod engine;
mod executor;
pub mod lang;
pub mod morsel;
mod zip_executor;

pub use cache::QueryCache;
pub use engine::{QueryEngine, QueryEngineConfig, QueryResult};
pub use executor::{ColumnValue, ExecutionResult, ExecutorConfig, QueryExecutor, ResultRow};
pub use lang::{
    parse_query, FenQuery, JsonCondition, JsonPipelineOp, JsonQuery, JsonQueryBuilder,
    JsonZipClause, ParseError, ParseErrorKind, PipelineOp, QueryError, QueryParams, QueryParser,
    ZipClause, ZipMode,
};
pub use morsel::{MorselConfig, MorselContext, QueryFeatures};
pub use zip_executor::{
    AggregateValue, CrossValidationIssue, CrossValidationIssueType, PipelineResults, ZipExecutor,
    ZipMetadata, ZipPair, ZipResult,
};

mod cache;
mod engine;
mod executor;
pub mod lang;
mod zip_executor;

pub use cache::QueryCache;
pub use engine::{QueryEngine, QueryEngineConfig, QueryResult};
pub use executor::{ExecutionResult, ExecutorConfig, QueryExecutor, ResultRow, ColumnValue};
pub use lang::{
    parse_query, FenQuery, QueryParams, QueryParser, QueryError, ParseError, ParseErrorKind,
    JsonQuery, JsonQueryBuilder, JsonCondition, JsonPipelineOp, JsonZipClause,
    ZipClause, ZipMode,
};
pub use zip_executor::{
    ZipExecutor, ZipResult, ZipPair, ZipMetadata, PipelineResults,
    CrossValidationIssue, CrossValidationIssueType, AggregateValue,
};

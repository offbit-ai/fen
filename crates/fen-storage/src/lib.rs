pub mod config;
pub mod error;
pub mod fulltext;
pub mod hot;
pub mod location;
pub mod query;
pub mod tiered;
pub mod traits;
pub mod warm;

pub use config::{HotStorageBackend, WarmStorageBackend};
#[cfg(feature = "remote-storage")]
pub use config::S3Config;
pub use error::StorageError;
pub use fulltext::{FullTextConfig, FullTextIndex, SearchResult as FullTextSearchResult};
pub use hot::RedbStorage;
pub use location::{DocumentLocationIndex, TierDistribution};
pub use query::{
    parse_query, ColumnValue, ExecutionResult, ExecutorConfig, FenQuery, ParseError, ParseErrorKind,
    QueryCache, QueryEngine, QueryEngineConfig, QueryError, QueryExecutor, QueryParams, QueryParser,
    QueryResult, ResultRow,
    // JSON query format
    JsonQuery, JsonQueryBuilder, JsonCondition, JsonPipelineOp, JsonZipClause,
    // ZIP clause for SQL queries
    ZipClause, ZipMode,
    // ZIP executor for cross-table queries
    ZipExecutor, ZipResult, ZipPair, ZipMetadata, PipelineResults,
    CrossValidationIssue, CrossValidationIssueType, AggregateValue,
};
pub use tiered::{TieredStorage, TieredStorageConfig};
pub use traits::{DocumentStore, InvoiceFilter, QueryMetrics, StorageTier, VectorSearchResult, VectorStore};
pub use warm::LanceStorage;

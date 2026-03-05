pub mod config;
pub mod error;
pub mod fulltext;
pub mod hot;
pub mod location;
pub mod query;
pub mod retention;
pub mod tenant_aware;
pub mod tiered;
pub mod traits;
pub mod warm;

#[cfg(feature = "remote-storage")]
pub use config::S3Config;
pub use config::{GraphBackend, HotStorageBackend, WarmStorageBackend};
pub use error::StorageError;
pub use fulltext::{FullTextConfig, FullTextIndex, SearchResult as FullTextSearchResult};
pub use hot::{AnomalyRecord, AnomalyStats, AnomalyStatus, AnomalyStore, BaselineStore, CacheStats, RedbStorage};
pub use location::{DocumentLocationIndex, TierDistribution};
pub use query::{
    parse_query,
    AggregateValue,
    ColumnValue,
    CrossValidationIssue,
    CrossValidationIssueType,
    ExecutionResult,
    ExecutorConfig,
    FenQuery,
    JsonCondition,
    JsonPipelineOp,
    // JSON query format
    JsonQuery,
    JsonQueryBuilder,
    JsonZipClause,
    ParseError,
    ParseErrorKind,
    PipelineOp,
    PipelineResults,
    QueryCache,
    QueryEngine,
    QueryEngineConfig,
    QueryError,
    QueryExecutor,
    QueryParams,
    QueryParser,
    QueryResult,
    ResultRow,
    // ZIP clause for SQL queries
    ZipClause,
    // ZIP executor for cross-table queries
    ZipExecutor,
    ZipMetadata,
    ZipMode,
    ZipPair,
    ZipResult,
};
pub use tiered::{TieredStorage, TieredStorageConfig};
pub use traits::{
    DocumentStore, InvoiceFilter, QueryMetrics, StorageTier, VectorSearchResult, VectorStore,
};
pub use warm::LanceStorage;

// Multi-tenancy
pub use retention::{
    DocumentMetadata, InMemoryRetentionMetadata, RetentionEnforcer, RetentionJob,
    RetentionMetadataStore, RetentionStats,
};
pub use tenant_aware::{TenantAwareStore, TenantScopedStore, TenantStorageContext};

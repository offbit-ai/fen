//! Kafka topic constants for event routing.
//!
//! All Fen events are published to these topics. Topics are partitioned
//! by tenant ID for scalability and ordering guarantees within a tenant.

/// Topic for document ingestion events.
/// Consumers: Ingestion workers
/// Key: tenant_id:document_type
pub const DOCUMENT_INGESTION: &str = "fen.document.ingestion";

/// Topic for processed document events.
/// Consumers: Validation workers, Baseline updaters
/// Key: document_id
pub const DOCUMENT_PROCESSED: &str = "fen.document.processed";

/// Topic for document processing failure events.
/// Consumers: Dead letter queue handlers, alerting
/// Key: document_id
pub const DOCUMENT_PROCESSING_FAILED: &str = "fen.document.processing-failed";

/// Topic for validation result events.
/// Consumers: Notification service, analytics
/// Key: document_id
pub const VALIDATION_RESULTS: &str = "fen.validation.results";

/// Topic for detected anomaly events.
/// Consumers: Notification service, anomaly store
/// Key: tenant_id
pub const ANOMALY_EVENTS: &str = "fen.anomaly.detected";

/// Topic for baseline update events.
/// Consumers: Cache invalidation, analytics
/// Key: vendor_name
pub const BASELINE_UPDATES: &str = "fen.baseline.updates";

/// Topic for real-time metrics events.
/// Consumers: Metrics aggregator, Prometheus pushgateway
/// Key: metric_name
pub const METRICS: &str = "fen.metrics";

/// Topic for alert events.
/// Consumers: Notification service
/// Key: tenant_id
pub const ALERTS: &str = "fen.alerts";

/// Topic for cluster membership events.
/// Consumers: All cluster nodes
/// Key: node_id
pub const CLUSTER_MEMBERSHIP: &str = "fen.cluster.membership";

/// Topic for shard rebalancing events.
/// Consumers: Data nodes
/// Key: shard_id
pub const SHARD_REBALANCING: &str = "fen.cluster.shard-rebalancing";

/// Dead letter queue for failed event processing.
/// Consumers: DLQ handlers
/// Key: original_topic:original_key
pub const DEAD_LETTER_QUEUE: &str = "fen.dlq";

/// Default number of partitions for topics.
pub const DEFAULT_PARTITIONS: u32 = 32;

/// Default replication factor for topics.
pub const DEFAULT_REPLICATION_FACTOR: u16 = 3;

/// Topic configuration for creating topics programmatically.
#[derive(Debug, Clone)]
pub struct TopicConfig {
    /// Topic name
    pub name: &'static str,
    /// Number of partitions
    pub partitions: u32,
    /// Replication factor
    pub replication_factor: u16,
    /// Retention in milliseconds (default: 7 days)
    pub retention_ms: u64,
    /// Cleanup policy ("delete" or "compact")
    pub cleanup_policy: &'static str,
}

impl TopicConfig {
    /// Create a new topic configuration with defaults.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            partitions: DEFAULT_PARTITIONS,
            replication_factor: DEFAULT_REPLICATION_FACTOR,
            retention_ms: 7 * 24 * 60 * 60 * 1000, // 7 days
            cleanup_policy: "delete",
        }
    }

    /// Create a compacted topic configuration.
    pub const fn compacted(name: &'static str) -> Self {
        Self {
            name,
            partitions: DEFAULT_PARTITIONS,
            replication_factor: DEFAULT_REPLICATION_FACTOR,
            retention_ms: u64::MAX, // Keep forever
            cleanup_policy: "compact",
        }
    }
}

/// All topic configurations for Fen.
pub const ALL_TOPICS: &[TopicConfig] = &[
    TopicConfig::new(DOCUMENT_INGESTION),
    TopicConfig::new(DOCUMENT_PROCESSED),
    TopicConfig::new(DOCUMENT_PROCESSING_FAILED),
    TopicConfig::new(VALIDATION_RESULTS),
    TopicConfig::new(ANOMALY_EVENTS),
    TopicConfig::new(BASELINE_UPDATES),
    TopicConfig::new(METRICS),
    TopicConfig::new(ALERTS),
    TopicConfig::compacted(CLUSTER_MEMBERSHIP),
    TopicConfig::new(SHARD_REBALANCING),
    TopicConfig::new(DEAD_LETTER_QUEUE),
];

//! Test helpers and utilities for setting up test environments

use std::sync::Arc;
use tempfile::TempDir;

use fen_ingestion::IngestionPipeline;
use fen_rules::RuleEngine;
use fen_storage::{
    HotStorageBackend, QueryEngine, QueryEngineConfig, RedbStorage, TieredStorage,
    TieredStorageConfig, WarmStorageBackend,
};

/// Test environment containing all initialized components
pub struct TestEnv {
    /// Tiered storage for hot/warm data
    pub storage: Arc<TieredStorage>,
    /// Rule engine for validation
    pub rule_engine: Arc<RuleEngine>,
    /// Temporary directory (kept alive to prevent cleanup)
    _temp_dir: TempDir,
}

impl TestEnv {
    /// Create a new test environment with in-memory storage
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let storage = Arc::new(
            TieredStorage::in_memory()
                .await
                .expect("Failed to create in-memory storage"),
        );

        let rule_engine = Arc::new(RuleEngine::builtin_only());

        Self {
            storage,
            rule_engine,
            _temp_dir: temp_dir,
        }
    }

    /// Create test environment with file-based storage
    pub async fn with_file_storage() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let hot_path = temp_dir.path().join("hot.redb");
        let warm_path = temp_dir.path().join("warm");

        let config = TieredStorageConfig {
            hot_backend: HotStorageBackend::embedded(hot_path.to_string_lossy().to_string()),
            warm_backend: WarmStorageBackend::embedded(warm_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let storage = Arc::new(
            TieredStorage::new(config)
                .await
                .expect("Failed to create file storage"),
        );

        let rule_engine = Arc::new(RuleEngine::builtin_only());

        Self {
            storage,
            rule_engine,
            _temp_dir: temp_dir,
        }
    }
}

/// Test environment with ingestion pipeline
pub struct IngestionTestEnv {
    /// Ingestion pipeline
    pub pipeline: Arc<IngestionPipeline<RedbStorage>>,
    /// Direct access to storage
    pub storage: Arc<RedbStorage>,
    /// Rule engine for validation
    pub rule_engine: Arc<RuleEngine>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl IngestionTestEnv {
    /// Create a new ingestion test environment
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let storage = Arc::new(
            RedbStorage::in_memory().expect("Failed to create in-memory storage"),
        );

        let pipeline = Arc::new(
            IngestionPipeline::new(storage.clone()).expect("Failed to create pipeline"),
        );

        let rule_engine = Arc::new(RuleEngine::builtin_only());

        Self {
            pipeline,
            storage,
            rule_engine,
            _temp_dir: temp_dir,
        }
    }

    /// Create with file-based storage
    pub async fn with_file_storage() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let db_path = temp_dir.path().join("test.redb");
        let storage = Arc::new(
            RedbStorage::new(&db_path).expect("Failed to create file storage"),
        );

        let pipeline = Arc::new(
            IngestionPipeline::new(storage.clone()).expect("Failed to create pipeline"),
        );

        let rule_engine = Arc::new(RuleEngine::builtin_only());

        Self {
            pipeline,
            storage,
            rule_engine,
            _temp_dir: temp_dir,
        }
    }
}

/// Test environment with query engine
pub struct QueryTestEnv {
    /// Query engine for hybrid queries
    pub query_engine: QueryEngine,
    /// Direct access to hot storage
    pub hot_storage: Arc<RedbStorage>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl QueryTestEnv {
    /// Create a new query test environment
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let warm_path = temp_dir.path().join("warm");

        let hot_storage = Arc::new(
            RedbStorage::in_memory().expect("Failed to create hot storage"),
        );

        let warm_storage = fen_storage::LanceStorage::new(&warm_path)
            .await
            .expect("Failed to create warm storage");

        let query_engine = QueryEngine::new(
            hot_storage.clone(),
            warm_storage,
            QueryEngineConfig::default(),
        );

        Self {
            query_engine,
            hot_storage,
            _temp_dir: temp_dir,
        }
    }
}

/// Initialize tracing for tests (call once at the start of test suite)
pub fn init_test_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fen=debug,test=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_test_writer())
        .try_init();
}

/// Assert that a validation result contains an anomaly with the given type
#[macro_export]
macro_rules! assert_has_anomaly {
    ($result:expr, $anomaly_type:expr) => {
        assert!(
            $result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == $anomaly_type),
            "Expected anomaly type {:?} not found in {:?}",
            $anomaly_type,
            $result.anomalies
        );
    };
}

/// Assert that a validation result does not contain an anomaly with the given type
#[macro_export]
macro_rules! assert_no_anomaly {
    ($result:expr, $anomaly_type:expr) => {
        assert!(
            !$result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == $anomaly_type),
            "Unexpected anomaly type {:?} found in {:?}",
            $anomaly_type,
            $result.anomalies
        );
    };
}

/// Assert that an invoice was stored and can be retrieved
#[macro_export]
macro_rules! assert_invoice_stored {
    ($storage:expr, $invoice:expr) => {
        let retrieved = $storage
            .get_invoice(&$invoice.id)
            .await
            .expect("Failed to get invoice");
        assert!(retrieved.is_some(), "Invoice not found in storage");
        assert_eq!(
            retrieved.unwrap().invoice_number,
            $invoice.invoice_number,
            "Invoice number mismatch"
        );
    };
}

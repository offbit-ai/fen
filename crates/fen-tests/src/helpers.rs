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

/// Test environment with query executor for unified query language tests
pub struct QueryExecutorTestEnv {
    /// Query executor for executing parsed queries
    pub executor: fen_storage::QueryExecutor,
    /// Direct access to hot storage
    pub hot_storage: Arc<RedbStorage>,
    /// Direct access to warm storage
    pub warm_storage: Arc<tokio::sync::RwLock<fen_storage::LanceStorage>>,
    /// Full-text index for BM25 search
    pub fulltext_index: Arc<fen_storage::FullTextIndex>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl QueryExecutorTestEnv {
    /// Create a new query executor test environment
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let warm_path = temp_dir.path().join("warm");
        let fulltext_path = temp_dir.path().join("fulltext");

        let hot_storage = Arc::new(
            RedbStorage::in_memory().expect("Failed to create hot storage"),
        );

        let warm_storage = Arc::new(tokio::sync::RwLock::new(
            fen_storage::LanceStorage::new(&warm_path)
                .await
                .expect("Failed to create warm storage"),
        ));

        let fulltext_index = Arc::new(
            fen_storage::FullTextIndex::new(&fulltext_path, fen_storage::FullTextConfig::default())
                .expect("Failed to create fulltext index"),
        );

        let executor = fen_storage::QueryExecutor::new(
            hot_storage.clone(),
            warm_storage.clone(),
            fulltext_index.clone(),
            fen_storage::ExecutorConfig::default(),
        );

        Self {
            executor,
            hot_storage,
            warm_storage,
            fulltext_index,
            _temp_dir: temp_dir,
        }
    }

    /// Store an invoice with text indexing
    pub async fn store_invoice(&self, invoice: &fen_core::domain::Invoice) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;

        // Store in hot storage
        self.hot_storage.store_invoice(invoice).await?;

        // Index text for full-text search
        self.fulltext_index.index_invoice(invoice).await?;

        // Commit changes to make them visible
        self.fulltext_index.commit().await?;

        Ok(())
    }

    /// Store an invoice with embedding for vector search
    pub async fn store_invoice_with_embedding(
        &self,
        invoice: &fen_core::domain::Invoice,
        embedding: &[f32],
    ) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;

        // Store in hot storage
        self.hot_storage.store_invoice(invoice).await?;

        // Store in warm storage with embedding
        {
            let mut warm = self.warm_storage.write().await;
            warm.store_invoice_with_embedding(invoice, Some(embedding)).await?;
        }

        // Index text for full-text search
        self.fulltext_index.index_invoice(invoice).await?;

        // Commit changes to make them visible
        self.fulltext_index.commit().await?;

        Ok(())
    }
}

/// Test environment for ZIP executor tests
pub struct ZipExecutorTestEnv {
    /// ZIP executor for cross-table queries
    pub zip_executor: fen_storage::ZipExecutor,
    /// Direct access to hot storage
    pub hot_storage: Arc<RedbStorage>,
    /// Direct access to warm storage
    pub warm_storage: Arc<tokio::sync::RwLock<fen_storage::LanceStorage>>,
    /// Full-text index for BM25 search
    pub fulltext_index: Arc<fen_storage::FullTextIndex>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl ZipExecutorTestEnv {
    /// Create a new ZIP executor test environment
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let warm_path = temp_dir.path().join("warm");
        let fulltext_path = temp_dir.path().join("fulltext");

        let hot_storage = Arc::new(
            RedbStorage::in_memory().expect("Failed to create hot storage"),
        );

        let warm_storage = Arc::new(tokio::sync::RwLock::new(
            fen_storage::LanceStorage::new(&warm_path)
                .await
                .expect("Failed to create warm storage"),
        ));

        let fulltext_index = Arc::new(
            fen_storage::FullTextIndex::new(&fulltext_path, fen_storage::FullTextConfig::default())
                .expect("Failed to create fulltext index"),
        );

        let zip_executor = fen_storage::ZipExecutor::new(
            hot_storage.clone(),
            warm_storage.clone(),
            fulltext_index.clone(),
            fen_storage::ExecutorConfig::default(),
        );

        Self {
            zip_executor,
            hot_storage,
            warm_storage,
            fulltext_index,
            _temp_dir: temp_dir,
        }
    }

    /// Store an invoice
    pub async fn store_invoice(&self, invoice: &fen_core::domain::Invoice) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;
        self.hot_storage.store_invoice(invoice).await?;
        self.fulltext_index.index_invoice(invoice).await?;
        self.fulltext_index.commit().await?;
        Ok(())
    }

    /// Store a contract
    pub async fn store_contract(&self, contract: &fen_core::domain::Contract) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;
        self.hot_storage.store_contract(contract).await?;
        Ok(())
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

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

        let storage =
            Arc::new(RedbStorage::in_memory().expect("Failed to create in-memory storage"));

        let pipeline =
            Arc::new(IngestionPipeline::new(storage.clone()).expect("Failed to create pipeline"));

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
        let storage = Arc::new(RedbStorage::new(&db_path).expect("Failed to create file storage"));

        let pipeline =
            Arc::new(IngestionPipeline::new(storage.clone()).expect("Failed to create pipeline"));

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

        let hot_storage = Arc::new(RedbStorage::in_memory().expect("Failed to create hot storage"));

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

        let hot_storage = Arc::new(RedbStorage::in_memory().expect("Failed to create hot storage"));

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
    pub async fn store_invoice(
        &self,
        invoice: &fen_core::domain::Invoice,
    ) -> Result<(), fen_storage::StorageError> {
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
            warm.store_invoice_with_embedding(invoice, Some(embedding))
                .await?;
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

        let hot_storage = Arc::new(RedbStorage::in_memory().expect("Failed to create hot storage"));

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
    pub async fn store_invoice(
        &self,
        invoice: &fen_core::domain::Invoice,
    ) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;
        self.hot_storage.store_invoice(invoice).await?;
        self.fulltext_index.index_invoice(invoice).await?;
        self.fulltext_index.commit().await?;
        Ok(())
    }

    /// Store a contract
    pub async fn store_contract(
        &self,
        contract: &fen_core::domain::Contract,
    ) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;
        self.hot_storage.store_contract(contract).await?;
        Ok(())
    }
}

/// Full integration test environment mirroring AppState wiring.
///
/// Contains all components needed to test the end-to-end flow:
/// ingestion → storage (hot+warm) → fulltext index → validation → anomaly persistence → search.
pub struct IntegrationTestEnv {
    /// Ingestion pipeline (no ML in tests — uses regex/GLiNER only)
    pub pipeline: Arc<IngestionPipeline<RedbStorage>>,
    /// Hot storage (redb)
    pub hot_storage: Arc<RedbStorage>,
    /// Warm storage (LanceDB) for vector search
    pub warm_storage: Arc<tokio::sync::RwLock<fen_storage::LanceStorage>>,
    /// Full-text index
    pub fulltext_index: Arc<fen_storage::FullTextIndex>,
    /// Anomaly store for persisting validation anomalies
    pub anomaly_store: Arc<fen_storage::AnomalyStore>,
    /// Rule engine for validation
    pub rule_engine: Arc<RuleEngine>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl IntegrationTestEnv {
    /// Create a new full integration test environment
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let warm_path = temp_dir.path().join("warm");
        let fulltext_path = temp_dir.path().join("fulltext");

        let hot_storage = Arc::new(RedbStorage::in_memory().expect("Failed to create hot storage"));
        let anomaly_store = Arc::new(
            fen_storage::AnomalyStore::new(hot_storage.db().clone())
                .expect("Failed to create anomaly store"),
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

        let pipeline = Arc::new(
            IngestionPipeline::new(hot_storage.clone()).expect("Failed to create pipeline"),
        );

        let rule_engine = Arc::new(RuleEngine::builtin_only());

        Self {
            pipeline,
            hot_storage,
            warm_storage,
            fulltext_index,
            anomaly_store,
            rule_engine,
            _temp_dir: temp_dir,
        }
    }

    /// Ingest text and perform all post-ingestion enrichment
    /// (fulltext index, warm-tier embedding, validation, anomaly persistence).
    /// Uses a deterministic embedding derived from invoice_number when embedding
    /// is not produced by the pipeline (no ML models in tests).
    pub async fn ingest_and_enrich(
        &self,
        text: &str,
    ) -> Result<fen_core::domain::Invoice, fen_ingestion::IngestionError> {
        let result = self.pipeline.ingest_text(text).await?;
        let invoice = result;

        // 1. Fulltext index
        self.fulltext_index
            .index_invoice(&invoice)
            .await
            .expect("Fulltext index failed");
        self.fulltext_index.commit().await.expect("Fulltext commit failed");

        // 2. Warm-tier with deterministic embedding
        let embedding = crate::consistent_embedding(&invoice.invoice_number, 384);
        {
            let mut warm = self.warm_storage.write().await;
            warm.store_invoice_with_embedding(&invoice, Some(&embedding))
                .await
                .expect("Warm-tier write failed");
        }

        // 3. Validate + persist anomalies
        if let Ok(validation_result) = self.rule_engine.validate_invoice(&invoice).await {
            for anomaly in &validation_result.anomalies {
                let record = fen_storage::AnomalyRecord::new(
                    anomaly.document_id.clone(),
                    &invoice.vendor.name,
                    anomaly.anomaly_type.clone(),
                    anomaly.severity,
                    &anomaly.description,
                )
                .with_invoice_id(invoice.id)
                .with_confidence(anomaly.confidence)
                .with_invoice_date(invoice.invoice_date);
                let _ = self.anomaly_store.store_anomaly(&record).await;
            }
        }

        Ok(invoice)
    }

    /// Store a pre-built invoice with full enrichment (fulltext + warm + validate + anomalies)
    pub async fn store_and_enrich(
        &self,
        invoice: &fen_core::domain::Invoice,
        embedding: &[f32],
    ) -> Result<(), fen_storage::StorageError> {
        use fen_storage::DocumentStore;

        // Hot storage
        self.hot_storage.store_invoice(invoice).await?;

        // Fulltext
        self.fulltext_index.index_invoice(invoice).await?;
        self.fulltext_index.commit().await?;

        // Warm tier
        {
            let mut warm = self.warm_storage.write().await;
            warm.store_invoice_with_embedding(invoice, Some(embedding))
                .await?;
        }

        // Validate + persist anomalies
        if let Ok(validation_result) = self.rule_engine.validate_invoice(invoice).await {
            for anomaly in &validation_result.anomalies {
                let record = fen_storage::AnomalyRecord::new(
                    anomaly.document_id.clone(),
                    &invoice.vendor.name,
                    anomaly.anomaly_type.clone(),
                    anomaly.severity,
                    &anomaly.description,
                )
                .with_invoice_id(invoice.id)
                .with_confidence(anomaly.confidence)
                .with_invoice_date(invoice.invoice_date);
                let _ = self.anomaly_store.store_anomaly(&record).await;
            }
        }

        Ok(())
    }
}

/// Test environment with ingestion pipeline wired to knowledge graph.
///
/// Exercises the full data ingestion path: text → parse → storage → graph.
pub struct GraphIngestionTestEnv {
    /// Ingestion pipeline (with graph feature enabled)
    pub pipeline: Arc<IngestionPipeline<RedbStorage>>,
    /// Direct access to storage
    pub storage: Arc<RedbStorage>,
    /// Knowledge graph store
    pub graph: Arc<fen_graph::KyuGraphStore>,
    /// Rule engine for validation
    pub rule_engine: Arc<RuleEngine>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl GraphIngestionTestEnv {
    /// Create a new graph-enabled ingestion test environment
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let storage =
            Arc::new(RedbStorage::in_memory().expect("Failed to create in-memory storage"));

        let graph = Arc::new(
            fen_graph::KyuGraphStore::in_memory().expect("Failed to create in-memory graph"),
        );

        let pipeline = Arc::new(
            IngestionPipeline::new(storage.clone())
                .expect("Failed to create pipeline")
                .with_graph(graph.clone() as Arc<dyn fen_graph::GraphStore>),
        );

        let rule_engine = Arc::new(RuleEngine::builtin_only());

        Self {
            pipeline,
            storage,
            graph,
            rule_engine,
            _temp_dir: temp_dir,
        }
    }
}

/// Resolve the ONNX models directory.
/// Checks `FEN_TEST_MODELS_DIR` env var, falls back to `{repo_root}/models`.
pub fn models_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("FEN_TEST_MODELS_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("../../models")
    }
}

/// Resolve the test documents directory.
/// Checks `FEN_TEST_DOCUMENTS_DIR` env var, falls back to fen-ml fixtures.
pub fn documents_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("FEN_TEST_DOCUMENTS_DIR") {
        std::path::PathBuf::from(dir)
    } else {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("../fen-ml/tests/fixtures/documents")
    }
}

/// Panics if required models are missing. Returns the models directory.
pub fn require_models() -> std::path::PathBuf {
    let dir = models_dir();
    assert!(
        dir.exists(),
        "Models directory not found at {}. Set FEN_TEST_MODELS_DIR.",
        dir.display()
    );
    for file in &[
        "ocr_detection.onnx",
        "ocr_recognition.onnx",
        "layout_model.onnx",
        "layout_tokenizer.json",
        "table_detection.onnx",
        "table_structure.onnx",
        "embedding_model.onnx",
        "tokenizer.json",
    ] {
        assert!(
            dir.join(file).exists(),
            "Required model file {} not found in {}.",
            file,
            dir.display()
        );
    }
    dir
}

/// Load a test document image by filename.
pub fn load_test_document(filename: &str) -> image::DynamicImage {
    let path = documents_dir().join(filename);
    assert!(path.exists(), "Test document not found: {}", path.display());
    image::ImageReader::open(&path)
        .unwrap_or_else(|e| panic!("Failed to open image {}: {}", path.display(), e))
        .with_guessed_format()
        .unwrap_or_else(|e| panic!("Failed to guess format for {}: {}", path.display(), e))
        .decode()
        .unwrap_or_else(|e| panic!("Failed to decode image {}: {}", path.display(), e))
}

/// Load a test PDF as raw bytes.
pub fn load_test_pdf(filename: &str) -> Vec<u8> {
    let path = documents_dir().join(filename);
    assert!(path.exists(), "Test PDF not found: {}", path.display());
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read PDF {}: {}", path.display(), e))
}

/// Create a small synthetic test image (white background with a black rectangle).
pub fn create_synthetic_image(width: u32, height: u32) -> image::DynamicImage {
    use image::{Rgb, RgbImage};
    let mut img = RgbImage::new(width, height);
    for pixel in img.pixels_mut() {
        *pixel = Rgb([255, 255, 255]);
    }
    for x in 50..200.min(width) {
        for y in 50..80.min(height) {
            img.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }
    image::DynamicImage::ImageRgb8(img)
}

/// Load ONNX models exactly once and share across all tests.
///
/// Models total ~795 MB (layout 478M, table 220M, embedding 86M, OCR 11M).
/// Loading per-test would OOM with parallel execution. This singleton ensures
/// a single copy in memory regardless of test parallelism.
static SHARED_ML_PIPELINE: std::sync::OnceLock<Arc<fen_ml::DocumentIntelligence>> =
    std::sync::OnceLock::new();

/// Get or initialize the shared ML pipeline (thread-safe, loaded once).
pub fn shared_ml_pipeline() -> Arc<fen_ml::DocumentIntelligence> {
    SHARED_ML_PIPELINE
        .get_or_init(|| {
            let models = require_models();
            let config = fen_ml::DocumentIntelligenceConfig {
                ocr: fen_ml::OcrConfig {
                    detection_model_path: Some("ocr_detection.onnx".to_string()),
                    recognition_model_path: Some("ocr_recognition.onnx".to_string()),
                    ..Default::default()
                },
                layout: fen_ml::LayoutModelConfig {
                    model_path: Some("layout_model.onnx".to_string()),
                    ..Default::default()
                },
                table: fen_ml::TableExtractorConfig {
                    detection_model_path: Some("table_detection.onnx".to_string()),
                    structure_model_path: Some("table_structure.onnx".to_string()),
                    ..Default::default()
                },
                embedding: fen_ml::EmbeddingModelConfig {
                    model_path: Some("embedding_model.onnx".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            };
            Arc::new(
                fen_ml::DocumentIntelligence::with_models(config, &models)
                    .expect("Failed to create ML pipeline with models"),
            )
        })
        .clone()
}

/// Test environment with full ML pipeline (LayoutLMv3, TATR, OCR, Embeddings).
///
/// Exercises the ML-enhanced ingestion path: image/PDF → OCR → Layout → Tables → Embeddings → Invoice → Storage → Graph.
/// Uses a shared singleton for ONNX models to avoid OOM from parallel test loading.
pub struct MlPipelineTestEnv {
    /// Document intelligence pipeline (shared singleton)
    pub ml: Arc<fen_ml::DocumentIntelligence>,
    /// Ingestion pipeline wired with ML
    pub pipeline: Arc<IngestionPipeline<RedbStorage>>,
    /// Direct access to storage
    pub storage: Arc<RedbStorage>,
    /// Knowledge graph store
    pub graph: Arc<fen_graph::KyuGraphStore>,
    /// Temporary directory
    _temp_dir: TempDir,
}

impl MlPipelineTestEnv {
    /// Create a new ML pipeline test environment.
    /// Models are loaded once and shared across all test instances.
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let ml = shared_ml_pipeline();

        let storage =
            Arc::new(RedbStorage::in_memory().expect("Failed to create in-memory storage"));

        let graph = Arc::new(
            fen_graph::KyuGraphStore::in_memory().expect("Failed to create in-memory graph"),
        );

        let pipeline = Arc::new(
            IngestionPipeline::with_ml_and_graph(
                storage.clone(),
                ml.clone(),
                graph.clone() as Arc<dyn fen_graph::GraphStore>,
            )
            .expect("Failed to create ML ingestion pipeline"),
        );

        Self {
            ml,
            pipeline,
            storage,
            graph,
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

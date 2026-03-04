use std::path::Path;
use std::sync::Arc;

use fen_events::LocalEventBus;
use fen_graph::RyuGraphStore;
use fen_ingestion::IngestionPipeline;
use fen_ml::{
    DocumentIntelligence, DocumentIntelligenceConfig, EmbeddingModelConfig, LayoutModelConfig,
    OcrConfig, TableExtractorConfig,
};
use fen_notify::NotificationHub;
use fen_rules::RuleEngine;
use fen_storage::{
    AnomalyStore, DocumentLocationIndex, ExecutorConfig, FullTextConfig, FullTextIndex,
    LanceStorage, QueryEngine, QueryEngineConfig, QueryExecutor, RedbStorage, TieredStorage,
};
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::db::{DbConfig, DbPools, Repositories};
use crate::middleware::AuthConfig;
use crate::routes::auth::OidcConfig;

/// Shared application state
pub struct AppState {
    pub storage: Arc<RedbStorage>,
    pub ingestion: Arc<IngestionPipeline<RedbStorage>>,
    pub rule_engine: Arc<RuleEngine>,
    pub anomaly_store: Arc<AnomalyStore>,
    // Optional advanced storage features
    pub fulltext_index: Option<Arc<FullTextIndex>>,
    pub warm_storage: Option<Arc<RwLock<LanceStorage>>>,
    pub query_engine: Option<Arc<QueryEngine>>,
    pub query_executor: Option<Arc<QueryExecutor>>,
    pub location_index: Option<Arc<DocumentLocationIndex>>,
    pub tiered_storage: Option<Arc<TieredStorage>>,
    // Notification and event infrastructure
    pub notification_hub: Option<Arc<NotificationHub>>,
    /// Event bus for internal event routing (used by workers)
    #[allow(dead_code)]
    pub event_bus: Option<Arc<LocalEventBus>>,
    // ML document intelligence pipeline
    /// Optional ML pipeline (requires ONNX model files)
    pub document_intelligence: Option<Arc<DocumentIntelligence>>,
    // Authentication configuration
    /// JWT authentication config
    pub auth_config: AuthConfig,
    /// OIDC provider config (optional, for SSO)
    pub oidc_config: Option<OidcConfig>,
    // Database layer (optional - for user/tenant management and observability)
    /// Database pools (PostgreSQL + TimescaleDB)
    #[allow(dead_code)]
    pub db_pools: Option<DbPools>,
    /// Database repositories (used by admin routes)
    #[allow(dead_code)]
    pub repositories: Option<Repositories>,
}

impl AppState {
    /// Create new application state from config
    pub async fn new(config: &AppConfig) -> anyhow::Result<Self> {
        // Ensure data directory exists
        if let Some(parent) = Path::new(&config.database_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Initialize storage
        let storage = Arc::new(RedbStorage::new(&config.database_path)?);
        tracing::info!(path = %config.database_path, "Storage initialized");

        // Initialize anomaly store (shares same redb database)
        let anomaly_store = Arc::new(AnomalyStore::new(storage.db().clone())?);
        tracing::info!("Anomaly store initialized");

        // Initialize rule engine
        let rule_engine = Arc::new(RuleEngine::new(config.rules_path.as_deref()).await?);
        tracing::info!("Rule engine initialized");

        // Initialize ML document intelligence (optional) — must come before IngestionPipeline
        let document_intelligence = if config.ml.enabled {
            match &config.ml.models_dir {
                Some(models_dir) if models_dir.exists() => {
                    let ml_config = DocumentIntelligenceConfig {
                        ocr: OcrConfig {
                            detection_model_path: Some("ocr_detection.onnx".to_string()),
                            recognition_model_path: Some("ocr_recognition.onnx".to_string()),
                            vocabulary_path: Some("ocr_dicts/en_dict.txt".to_string()),
                            gpu_enabled: config.ml.gpu_enabled,
                            ..Default::default()
                        },
                        layout: LayoutModelConfig {
                            model_path: Some("layout_model.onnx".to_string()),
                            gpu_enabled: config.ml.gpu_enabled,
                            ..Default::default()
                        },
                        table: TableExtractorConfig {
                            detection_model_path: Some("table_detection.onnx".to_string()),
                            structure_model_path: Some("table_structure.onnx".to_string()),
                            gpu_enabled: config.ml.gpu_enabled,
                            ..Default::default()
                        },
                        embedding: EmbeddingModelConfig {
                            model_path: Some("embedding_model.onnx".to_string()),
                            gpu_enabled: config.ml.gpu_enabled,
                            ..Default::default()
                        },
                        ..Default::default()
                    };

                    match DocumentIntelligence::with_models(ml_config, models_dir) {
                        Ok(di) => {
                            tracing::info!(
                                models_dir = %models_dir.display(),
                                "ML document intelligence initialized with models"
                            );
                            Some(Arc::new(di))
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to load ML models, using rule-based fallback");
                            match DocumentIntelligence::new(DocumentIntelligenceConfig::default()) {
                                Ok(di) => Some(Arc::new(di)),
                                Err(e) => {
                                    tracing::error!(error = %e, "Failed to create rule-based fallback");
                                    None
                                }
                            }
                        }
                    }
                }
                _ => {
                    tracing::info!("ML_MODELS_DIR not set or does not exist, using rule-based document intelligence");
                    match DocumentIntelligence::new(DocumentIntelligenceConfig::default()) {
                        Ok(di) => Some(Arc::new(di)),
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to create rule-based document intelligence");
                            None
                        }
                    }
                }
            }
        } else {
            tracing::info!("ML document intelligence disabled (ML_ENABLED=false)");
            None
        };

        // Initialize knowledge graph (optional)
        let graph_store = if let Some(ref graph_path) = config.graph_storage_path {
            std::fs::create_dir_all(graph_path)?;
            match RyuGraphStore::new(graph_path) {
                Ok(store) => {
                    tracing::info!(path = %graph_path.display(), "Knowledge graph initialized");
                    Some(Arc::new(store))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to initialize knowledge graph");
                    None
                }
            }
        } else {
            tracing::info!("Knowledge graph not configured (GRAPH_STORAGE_PATH not set)");
            None
        };

        // Initialize ingestion pipeline — wire ML and/or graph if available
        let ingestion = match (&document_intelligence, &graph_store) {
            (Some(di), Some(graph)) => {
                Arc::new(IngestionPipeline::with_ml_and_graph(
                    storage.clone(),
                    di.clone(),
                    graph.clone(),
                )?)
            }
            (Some(di), None) => {
                Arc::new(IngestionPipeline::with_ml(storage.clone(), di.clone())?)
            }
            _ => Arc::new(IngestionPipeline::new(storage.clone())?),
        };
        tracing::info!(
            ml_enabled = document_intelligence.is_some(),
            graph_enabled = graph_store.is_some(),
            "Ingestion pipeline initialized"
        );

        // Initialize optional fulltext index
        let fulltext_index = match FullTextIndex::in_memory(FullTextConfig::default()) {
            Ok(index) => {
                tracing::info!("Full-text index initialized (in-memory)");
                Some(Arc::new(index))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize full-text index, search will be unavailable");
                None
            }
        };

        // Location index for tier tracking
        let location_index = Arc::new(DocumentLocationIndex::new());
        tracing::info!("Document location index initialized");

        // Initialize warm-tier storage (LanceDB)
        std::fs::create_dir_all(&config.warm_storage_path)?;
        let warm_storage = match LanceStorage::new(&config.warm_storage_path).await {
            Ok(lance) => {
                tracing::info!(path = %config.warm_storage_path, "Warm storage (LanceDB) initialized");
                Some(Arc::new(RwLock::new(lance)))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize warm storage, vector search unavailable");
                None
            }
        };

        // Initialize QueryEngine (requires a separate LanceStorage instance)
        let query_engine = match LanceStorage::new(&config.warm_storage_path).await {
            Ok(lance_for_engine) => {
                let engine = QueryEngine::new(
                    storage.clone(),
                    lance_for_engine,
                    QueryEngineConfig::default(),
                );
                tracing::info!("Query engine initialized");
                Some(Arc::new(engine))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to initialize query engine");
                None
            }
        };

        // Initialize QueryExecutor (requires warm storage + fulltext)
        let query_executor = match (&warm_storage, &fulltext_index) {
            (Some(warm), Some(fti)) => {
                let executor = QueryExecutor::new(
                    storage.clone(),
                    warm.clone(),
                    fti.clone(),
                    ExecutorConfig::default(),
                );
                tracing::info!("Query executor initialized");
                Some(Arc::new(executor))
            }
            _ => {
                tracing::info!("Query executor not available (requires warm storage + fulltext index)");
                None
            }
        };

        // Initialize notification hub for real-time notifications
        let notification_hub = Arc::new(NotificationHub::new());
        tracing::info!("Notification hub initialized");

        // Initialize local event bus for in-process events
        let event_bus = Arc::new(LocalEventBus::new());
        tracing::info!("Local event bus initialized");

        // Load auth configuration from environment
        let auth_config = AuthConfig {
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "development-secret-change-in-production".to_string()),
            require_auth: std::env::var("REQUIRE_AUTH")
                .map(|v| v.to_lowercase() != "false")
                .unwrap_or(true),
        };

        // Load OIDC configuration from environment (optional)
        let oidc_config = std::env::var("OIDC_ISSUER_URL").ok().map(|issuer_url| {
            OidcConfig {
                issuer_url,
                client_id: std::env::var("OIDC_CLIENT_ID").unwrap_or_else(|_| "fen-api".to_string()),
                client_secret: std::env::var("OIDC_CLIENT_SECRET").ok(),
                redirect_uri: std::env::var("OIDC_REDIRECT_URI")
                    .unwrap_or_else(|_| "http://localhost:3000/auth/callback".to_string()),
                scopes: std::env::var("OIDC_SCOPES")
                    .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_else(|_| vec!["openid".to_string(), "profile".to_string(), "email".to_string()]),
            }
        });

        if oidc_config.is_some() {
            tracing::info!("OIDC authentication configured");
        } else {
            tracing::info!("OIDC not configured, using local JWT auth only");
        }

        // Initialize database connections (optional)
        let (db_pools, repositories) = if std::env::var("DATABASE_URL").is_ok() {
            let db_config = DbConfig::from_env();
            match DbPools::new(&db_config).await {
                Ok(pools) => {
                    let repos = pools.repositories();
                    tracing::info!("Database pools initialized");
                    (Some(pools), Some(repos))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to connect to databases, admin features will be limited");
                    (None, None)
                }
            }
        } else {
            tracing::info!("DATABASE_URL not set, running without PostgreSQL/TimescaleDB");
            (None, None)
        };

        Ok(Self {
            storage,
            ingestion,
            rule_engine,
            anomaly_store,
            document_intelligence,
            fulltext_index,
            warm_storage,
            query_engine,
            query_executor,
            location_index: Some(location_index),
            tiered_storage: None, // Full tiered storage requires explicit configuration
            notification_hub: Some(notification_hub),
            event_bus: Some(event_bus),
            auth_config,
            oidc_config,
            db_pools,
            repositories,
        })
    }
}

use std::path::Path;
use std::sync::Arc;

use fen_ingestion::IngestionPipeline;
use fen_rules::RuleEngine;
use fen_storage::{
    DocumentLocationIndex, FullTextConfig, FullTextIndex, QueryEngine, QueryExecutor, RedbStorage,
    TieredStorage,
};

use crate::config::AppConfig;

/// Shared application state
pub struct AppState {
    pub storage: Arc<RedbStorage>,
    pub ingestion: Arc<IngestionPipeline<RedbStorage>>,
    pub rule_engine: Arc<RuleEngine>,
    // Optional advanced storage features
    pub fulltext_index: Option<Arc<FullTextIndex>>,
    pub query_engine: Option<Arc<QueryEngine>>,
    pub query_executor: Option<Arc<QueryExecutor>>,
    pub location_index: Option<Arc<DocumentLocationIndex>>,
    pub tiered_storage: Option<Arc<TieredStorage>>,
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

        // Initialize ingestion pipeline
        let ingestion = Arc::new(IngestionPipeline::new(storage.clone())?);
        tracing::info!("Ingestion pipeline initialized");

        // Initialize rule engine
        let rule_engine = Arc::new(RuleEngine::new(config.rules_path.as_deref()).await?);
        tracing::info!("Rule engine initialized");

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

        // Query engine requires warm storage (LanceDB), so not available in basic mode
        // The QueryEngine will be initialized when TieredStorage is configured
        let query_engine: Option<Arc<QueryEngine>> = None;
        tracing::info!("Query engine not available (requires tiered storage setup)");

        Ok(Self {
            storage,
            ingestion,
            rule_engine,
            fulltext_index,
            query_engine,
            query_executor: None, // Requires warm storage setup
            location_index: Some(location_index),
            tiered_storage: None, // Requires explicit configuration
        })
    }
}

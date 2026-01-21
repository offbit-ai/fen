use std::path::Path;
use std::sync::Arc;

use fen_ingestion::IngestionPipeline;
use fen_rules::RuleEngine;
use fen_storage::RedbStorage;

use crate::config::AppConfig;

/// Shared application state
pub struct AppState {
    pub storage: Arc<RedbStorage>,
    pub ingestion: Arc<IngestionPipeline<RedbStorage>>,
    pub rule_engine: Arc<RuleEngine>,
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
        let rule_engine = Arc::new(
            RuleEngine::new(config.rules_path.as_deref()).await?,
        );
        tracing::info!("Rule engine initialized");

        Ok(Self {
            storage,
            ingestion,
            rule_engine,
        })
    }
}

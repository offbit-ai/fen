//! Fen Ingestion Worker - Kafka consumer for document processing.
//!
//! This worker:
//! 1. Consumes documents from `fen.document.ingestion` topic
//! 2. Extracts text from PDFs (invoices and contracts)
//! 3. Parses using ML models when available, with GLiNER/regex fallback
//! 4. Generates embeddings
//! 5. Stores documents in the appropriate shard
//! 6. Publishes to `fen.document.processed` topic (bincode serialized)
//!
//! # Usage
//!
//! ```bash
//! fen-ingestion-worker --kafka-brokers localhost:9092 --consumer-group fen-ingestion
//! ```

use clap::Parser;
use fen_core::domain::{Contract, Invoice};
use fen_events::{
    topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent,
};
use fen_ingestion::IngestionPipeline;
use fen_storage::config::{HotStorageBackend, WarmStorageBackend};
use fen_storage::tiered::{TieredStorage, TieredStorageConfig};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

#[cfg(feature = "kafka")]
use fen_events::{KafkaConfig, KafkaConsumer, KafkaProducer};

const DEAD_LETTER_TOPIC: &str = "fen.document.dlq";

/// Fen Ingestion Worker
#[derive(Parser, Debug)]
#[command(name = "fen-ingestion-worker")]
#[command(about = "Kafka consumer for document ingestion and processing")]
struct Args {
    /// Kafka bootstrap servers
    #[arg(
        long,
        env = "KAFKA_BOOTSTRAP_SERVERS",
        default_value = "localhost:9092"
    )]
    kafka_brokers: String,

    /// Consumer group ID
    #[arg(long, env = "CONSUMER_GROUP", default_value = "fen-ingestion")]
    consumer_group: String,

    /// Coordinator address for shard routing
    #[arg(
        long,
        env = "COORDINATOR_ADDR",
        default_value = "http://localhost:9002"
    )]
    coordinator_addr: String,

    /// Storage path for local storage
    #[arg(long, env = "STORAGE_PATH", default_value = "/data/fen")]
    storage_path: String,

    /// Number of concurrent processing tasks
    #[arg(long, env = "CONCURRENCY", default_value = "4")]
    concurrency: usize,

    /// Use local event bus instead of Kafka (for development)
    #[arg(long, env = "USE_LOCAL_BUS")]
    use_local_bus: bool,

    /// Health check port
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,

    /// Maximum retries for failed documents
    #[arg(long, env = "MAX_RETRIES", default_value = "3")]
    max_retries: u32,
}

/// Event payload for document ingestion (JSON from upstream)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentIngestedEvent {
    document_id: String,
    tenant_id: String,
    filename: String,
    mime_type: String,
    size_bytes: u64,
    source_hash: String,
    /// Document type hint: "invoice", "contract", or "auto"
    #[serde(default = "default_document_type")]
    document_type: String,
    /// Base64-encoded document bytes (for small documents) or storage URL
    data: Option<String>,
    storage_url: Option<String>,
}

fn default_document_type() -> String {
    "auto".to_string()
}

/// Unified processed event for the Kafka wire format.
///
/// Carries `Option<Invoice>` + `Option<Contract>` so the validation worker can
/// handle both document types. Serialized with bincode for compact Kafka payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentProcessedEvent {
    document_type: String,
    invoice: Option<Invoice>,
    contract: Option<Contract>,
}

/// Event payload for failed processing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentProcessingFailedEvent {
    document_id: String,
    tenant_id: String,
    error_code: String,
    error_message: String,
    retry_count: u32,
    failed_at: String,
}

/// Worker state for health checks
struct WorkerState {
    messages_processed: std::sync::atomic::AtomicU64,
    messages_failed: std::sync::atomic::AtomicU64,
    last_message_time: tokio::sync::RwLock<Option<Instant>>,
    healthy: std::sync::atomic::AtomicBool,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            messages_failed: std::sync::atomic::AtomicU64::new(0),
            last_message_time: tokio::sync::RwLock::new(None),
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn increment_processed(&self) {
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn increment_failed(&self) {
        self.messages_failed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    async fn update_last_message_time(&self) {
        let mut guard = self.last_message_time.write().await;
        *guard = Some(Instant::now());
    }
}

#[derive(Debug)]
enum ProcessingError {
    Deserialization(String),
    Pipeline(String),
    MissingData(String),
}

impl ProcessingError {
    fn code(&self) -> &'static str {
        match self {
            ProcessingError::Deserialization(_) => "DESERIALIZATION_ERROR",
            ProcessingError::Pipeline(_) => "PIPELINE_ERROR",
            ProcessingError::MissingData(_) => "MISSING_DATA_ERROR",
        }
    }
}

impl std::fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingError::Deserialization(e) => write!(f, "Deserialization error: {}", e),
            ProcessingError::Pipeline(e) => write!(f, "Pipeline error: {}", e),
            ProcessingError::MissingData(e) => write!(f, "Missing data: {}", e),
        }
    }
}

impl std::error::Error for ProcessingError {}

/// Determine document type from event payload or filename heuristics.
fn detect_document_type(ingested: &DocumentIngestedEvent) -> &str {
    match ingested.document_type.as_str() {
        "invoice" | "contract" => ingested.document_type.as_str(),
        _ => {
            let lower = ingested.filename.to_lowercase();
            if lower.contains("contract") || lower.contains("agreement") || lower.contains("nda") {
                "contract"
            } else {
                "invoice"
            }
        }
    }
}

/// Process a single document ingestion event — handles both invoices and contracts.
async fn process_document(
    event: &RawEvent,
    pipeline: &IngestionPipeline<TieredStorage>,
) -> Result<(DocumentIngestedEvent, DocumentProcessedEvent, u64), ProcessingError> {
    let start = Instant::now();

    let ingested: DocumentIngestedEvent = serde_json::from_slice(&event.payload)
        .map_err(|e| ProcessingError::Deserialization(e.to_string()))?;

    info!(
        document_id = %ingested.document_id,
        tenant_id = %ingested.tenant_id,
        filename = %ingested.filename,
        size_bytes = ingested.size_bytes,
        "Processing document"
    );

    // Get document bytes
    let document_bytes = if let Some(data) = &ingested.data {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| ProcessingError::Deserialization(format!("Base64 decode error: {}", e)))?
    } else if let Some(storage_url) = &ingested.storage_url {
        fetch_from_storage(storage_url).await?
    } else {
        return Err(ProcessingError::MissingData(
            "No document data or storage URL provided".to_string(),
        ));
    };

    let doc_type = detect_document_type(&ingested);

    let processed_event = match doc_type {
        "contract" => {
            let result = pipeline
                .ingest_contract_pdf(&document_bytes, &ingested.filename)
                .await
                .map_err(|e| ProcessingError::Pipeline(e.to_string()))?;

            info!(
                document_id = %ingested.document_id,
                contract_id = %result.contract.id,
                "Contract processed successfully"
            );

            DocumentProcessedEvent {
                document_type: "contract".to_string(),
                invoice: None,
                contract: Some(result.contract),
            }
        }
        _ => {
            let result = pipeline
                .ingest_pdf(&document_bytes, &ingested.filename)
                .await
                .map_err(|e| ProcessingError::Pipeline(e.to_string()))?;

            info!(
                document_id = %ingested.document_id,
                invoice_id = %result.invoice.id,
                invoice_number = %result.invoice.invoice_number,
                confidence = %result.invoice.confidence_score,
                "Invoice processed successfully"
            );

            DocumentProcessedEvent {
                document_type: "invoice".to_string(),
                invoice: Some(result.invoice),
                contract: None,
            }
        }
    };

    let processing_time_ms = start.elapsed().as_millis() as u64;

    Ok((ingested, processed_event, processing_time_ms))
}

/// Fetch document bytes from a storage URL.
async fn fetch_from_storage(url: &str) -> Result<Vec<u8>, ProcessingError> {
    if let Some(path) = url.strip_prefix("file://") {
        return tokio::fs::read(path)
            .await
            .map_err(|e| ProcessingError::MissingData(format!("Failed to read {}: {}", path, e)));
    }

    Err(ProcessingError::MissingData(format!(
        "Storage URL scheme not yet supported: {}. Use file:// for local, or provide inline data.",
        url
    )))
}

/// Create a DocumentProcessingFailed event
fn create_failed_event(
    document_id: &str,
    tenant_id: &str,
    error: &ProcessingError,
    retry_count: u32,
) -> DocumentProcessingFailedEvent {
    DocumentProcessingFailedEvent {
        document_id: document_id.to_string(),
        tenant_id: tenant_id.to_string(),
        error_code: error.code().to_string(),
        error_message: error.to_string(),
        retry_count,
        failed_at: chrono::Utc::now().to_rfc3339(),
    }
}

async fn run_worker(
    consumer: Arc<dyn EventConsumer>,
    producer: Arc<dyn EventProducer>,
    pipeline: Arc<IngestionPipeline<TieredStorage>>,
    state: Arc<WorkerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    consumer.subscribe(&[topics::DOCUMENT_INGESTION]).await?;

    info!("Worker subscribed to {}", topics::DOCUMENT_INGESTION);

    loop {
        let events = match consumer.poll(1000).await {
            Ok(events) => events,
            Err(e) => {
                warn!(error = %e, "Error polling for messages");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        for event in events {
            let key = event.key.as_deref().unwrap_or_default();
            let key_str = String::from_utf8_lossy(key).to_string();

            let (document_id, tenant_id) =
                match serde_json::from_slice::<DocumentIngestedEvent>(&event.payload) {
                    Ok(ingested) => (ingested.document_id.clone(), ingested.tenant_id.clone()),
                    Err(_) => (key_str.clone(), "unknown".to_string()),
                };

            match process_document(&event, &pipeline).await {
                Ok((_ingested, processed_event, _processing_time_ms)) => {
                    // Serialize with bincode for compact Kafka wire format
                    let payload = bincode::serialize(&processed_event)
                        .expect("Failed to serialize processed event");

                    if let Err(e) = producer
                        .publish(topics::DOCUMENT_PROCESSED, key, &payload)
                        .await
                    {
                        error!(error = %e, document_id = %document_id, "Failed to publish processed event");
                    }

                    state.increment_processed();
                    state.update_last_message_time().await;
                }
                Err(e) => {
                    error!(error = %e, document_id = %document_id, "Failed to process document");

                    let failed_event = create_failed_event(&document_id, &tenant_id, &e, 0);
                    let payload = serde_json::to_vec(&failed_event)
                        .expect("Failed to serialize failed event");

                    if let Err(dlq_err) = producer.publish(DEAD_LETTER_TOPIC, key, &payload).await {
                        error!(error = %dlq_err, document_id = %document_id, "Failed to publish to DLQ");
                    }

                    state.increment_failed();
                }
            }
        }

        if let Err(e) = consumer.commit_all().await {
            warn!(error = %e, "Failed to commit offsets");
        }
    }
}

async fn health_server(port: u16, state: Arc<WorkerState>) {
    use axum::{routing::get, Json, Router};
    use std::net::SocketAddr;

    let state_clone = state.clone();
    let app = Router::new()
        .route(
            "/health",
            get(move || {
                let state = state_clone.clone();
                async move {
                    let healthy = state.healthy.load(std::sync::atomic::Ordering::Relaxed);
                    let processed = state
                        .messages_processed
                        .load(std::sync::atomic::Ordering::Relaxed);
                    let failed = state
                        .messages_failed
                        .load(std::sync::atomic::Ordering::Relaxed);

                    Json(serde_json::json!({
                        "healthy": healthy,
                        "messages_processed": processed,
                        "messages_failed": failed,
                    }))
                }
            }),
        )
        .route(
            "/ready",
            get(|| async { Json(serde_json::json!({"ready": true})) }),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(addr = %addr, "Starting health server");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fen_ingestion=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!(
        kafka_brokers = %args.kafka_brokers,
        consumer_group = %args.consumer_group,
        storage_path = %args.storage_path,
        concurrency = args.concurrency,
        "Starting Fen Ingestion Worker"
    );

    let storage_config = TieredStorageConfig {
        hot_backend: HotStorageBackend::Embedded {
            path: format!("{}/hot.redb", args.storage_path),
        },
        warm_backend: WarmStorageBackend::Embedded {
            path: format!("{}/warm", args.storage_path),
        },
        ..Default::default()
    };

    let storage = Arc::new(
        TieredStorage::new(storage_config)
            .await
            .map_err(|e| format!("Failed to create storage: {}", e))?,
    );

    let pipeline = Arc::new(
        IngestionPipeline::new(storage).map_err(|e| format!("Failed to create pipeline: {}", e))?,
    );

    let state = Arc::new(WorkerState::new());

    let health_state = state.clone();
    tokio::spawn(async move {
        health_server(args.health_port, health_state).await;
    });

    let (consumer, producer): (Arc<dyn EventConsumer>, Arc<dyn EventProducer>) = if args
        .use_local_bus
    {
        let bus = Arc::new(LocalEventBus::new());
        let consumer = Arc::new(LocalEventConsumer::new(bus.clone()));
        (consumer, bus)
    } else {
        #[cfg(feature = "kafka")]
        {
            let config = KafkaConfig {
                bootstrap_servers: args.kafka_brokers.clone(),
                group_id: Some(args.consumer_group.clone()),
                ..Default::default()
            };

            let consumer = Arc::new(KafkaConsumer::new(config.clone())?);
            let producer = Arc::new(KafkaProducer::new(config)?);
            (consumer, producer)
        }
        #[cfg(not(feature = "kafka"))]
        {
            error!("Kafka feature not enabled. Use --use-local-bus or enable the 'kafka' feature.");
            std::process::exit(1);
        }
    };

    run_worker(consumer, producer, pipeline, state).await?;

    Ok(())
}

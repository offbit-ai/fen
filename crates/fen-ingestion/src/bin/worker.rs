//! Fen Ingestion Worker - Kafka consumer for document processing.
//!
//! This worker:
//! 1. Consumes documents from `fen.document.ingestion` topic
//! 2. Extracts text from PDFs
//! 3. Parses invoices/contracts using ML models
//! 4. Generates embeddings
//! 5. Stores documents in the appropriate shard
//! 6. Publishes to `fen.document.processed` topic
//!
//! # Usage
//!
//! ```bash
//! fen-ingestion-worker --kafka-brokers localhost:9092 --consumer-group fen-ingestion
//! ```

use clap::Parser;
use fen_events::{topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[cfg(feature = "kafka")]
use fen_events::{KafkaConfig, KafkaConsumer, KafkaProducer};

/// Fen Ingestion Worker
#[derive(Parser, Debug)]
#[command(name = "fen-ingestion-worker")]
#[command(about = "Kafka consumer for document ingestion and processing")]
struct Args {
    /// Kafka bootstrap servers
    #[arg(long, env = "KAFKA_BOOTSTRAP_SERVERS", default_value = "localhost:9092")]
    kafka_brokers: String,

    /// Consumer group ID
    #[arg(long, env = "CONSUMER_GROUP", default_value = "fen-ingestion")]
    consumer_group: String,

    /// Coordinator address for shard routing
    #[arg(long, env = "COORDINATOR_ADDR", default_value = "http://localhost:9002")]
    coordinator_addr: String,

    /// Number of concurrent processing tasks
    #[arg(long, env = "CONCURRENCY", default_value = "4")]
    concurrency: usize,

    /// Use local event bus instead of Kafka (for development)
    #[arg(long, env = "USE_LOCAL_BUS")]
    use_local_bus: bool,

    /// Health check port
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,
}

/// Worker state for health checks
struct WorkerState {
    messages_processed: std::sync::atomic::AtomicU64,
    last_message_time: tokio::sync::RwLock<Option<std::time::Instant>>,
    healthy: std::sync::atomic::AtomicBool,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            last_message_time: tokio::sync::RwLock::new(None),
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn increment_processed(&self) {
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    async fn update_last_message_time(&self) {
        let mut guard = self.last_message_time.write().await;
        *guard = Some(std::time::Instant::now());
    }
}

async fn process_document(event: &RawEvent) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let key = event.key.as_deref().unwrap_or_default();

    // Parse the ingestion event
    info!(
        topic = %event.topic,
        key_len = key.len(),
        payload_len = event.payload.len(),
        "Processing document"
    );

    // TODO: Implement actual document processing:
    // 1. Deserialize DocumentIngested event
    // 2. Fetch document data from storage or decode from payload
    // 3. Extract text from PDF using pdfium
    // 4. Parse invoice/contract using ML models
    // 5. Generate embeddings
    // 6. Store in shard via gRPC
    // 7. Create DocumentProcessed event

    // For now, create a stub processed event
    let processed_event = serde_json::json!({
        "document_id": String::from_utf8_lossy(key),
        "status": "processed",
        "processing_time_ms": 100,
        "confidence_score": 0.95,
    });

    Ok(serde_json::to_vec(&processed_event)?)
}

async fn run_worker(
    consumer: Arc<dyn EventConsumer>,
    producer: Arc<dyn EventProducer>,
    state: Arc<WorkerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Subscribe to ingestion topic
    consumer.subscribe(&[topics::DOCUMENT_INGESTION]).await?;

    info!("Worker subscribed to {}", topics::DOCUMENT_INGESTION);

    loop {
        // Poll for messages
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
            match process_document(&event).await {
                Ok(processed_payload) => {
                    // Publish to processed topic
                    if let Err(e) = producer
                        .publish(topics::DOCUMENT_PROCESSED, key, &processed_payload)
                        .await
                    {
                        error!(error = %e, "Failed to publish processed event");
                    }

                    state.increment_processed();
                    state.update_last_message_time().await;
                }
                Err(e) => {
                    error!(error = %e, "Failed to process document");
                    // TODO: Publish to dead letter queue
                }
            }
        }

        // Commit offsets
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

                    Json(serde_json::json!({
                        "healthy": healthy,
                        "messages_processed": processed,
                    }))
                }
            }),
        )
        .route("/ready", get(|| async { Json(serde_json::json!({"ready": true})) }));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!(addr = %addr, "Starting health server");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
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
        concurrency = args.concurrency,
        "Starting Fen Ingestion Worker"
    );

    // Create worker state
    let state = Arc::new(WorkerState::new());

    // Start health server
    let health_state = state.clone();
    tokio::spawn(async move {
        health_server(args.health_port, health_state).await;
    });

    // Create consumer and producer
    let (consumer, producer): (Arc<dyn EventConsumer>, Arc<dyn EventProducer>) = if args.use_local_bus {
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

    // Run worker
    run_worker(consumer, producer, state).await?;

    Ok(())
}

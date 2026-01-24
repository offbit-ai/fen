//! Fen Validation Worker - Kafka consumer for rule-based validation.
//!
//! This worker:
//! 1. Consumes documents from `fen.document.processed` topic
//! 2. Runs validation rules against the document
//! 3. Computes statistical anomaly scores
//! 4. Publishes anomalies to `fen.anomaly.detected` topic
//!
//! # Usage
//!
//! ```bash
//! fen-validation-worker --kafka-brokers localhost:9092 --consumer-group fen-validation
//! ```

use clap::Parser;
use fen_events::{topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[cfg(feature = "kafka")]
use fen_events::{KafkaConfig, KafkaConsumer, KafkaProducer};

/// Fen Validation Worker
#[derive(Parser, Debug)]
#[command(name = "fen-validation-worker")]
#[command(about = "Kafka consumer for document validation and anomaly detection")]
struct Args {
    /// Kafka bootstrap servers
    #[arg(long, env = "KAFKA_BOOTSTRAP_SERVERS", default_value = "localhost:9092")]
    kafka_brokers: String,

    /// Consumer group ID
    #[arg(long, env = "CONSUMER_GROUP", default_value = "fen-validation")]
    consumer_group: String,

    /// Rules directory path
    #[arg(long, env = "RULES_PATH", default_value = "/app/rules")]
    rules_path: String,

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
    anomalies_detected: std::sync::atomic::AtomicU64,
    healthy: std::sync::atomic::AtomicBool,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            anomalies_detected: std::sync::atomic::AtomicU64::new(0),
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn increment_processed(&self) {
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn increment_anomalies(&self) {
        self.anomalies_detected
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

async fn validate_document(
    event: &RawEvent,
) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
    let key = event.key.as_deref().unwrap_or_default();

    info!(
        topic = %event.topic,
        key_len = key.len(),
        "Validating document"
    );

    // TODO: Implement actual validation:
    // 1. Deserialize DocumentProcessed event
    // 2. Load document from storage
    // 3. Run validation rules via RuleEngine
    // 4. Compute statistical anomaly scores
    // 5. If anomalies found, create AnomalyDetected events

    // Stub: randomly detect "anomalies" for demonstration
    let has_anomaly = rand::random::<u8>() % 10 == 0; // 10% chance

    if has_anomaly {
        let anomaly_event = serde_json::json!({
            "anomaly_id": uuid::Uuid::new_v4().to_string(),
            "document_id": String::from_utf8_lossy(key),
            "anomaly_type": "price_deviation",
            "severity": "medium",
            "description": "Price deviates significantly from vendor baseline",
            "z_score": 2.5,
            "detected_at": chrono::Utc::now().to_rfc3339(),
        });
        Ok(Some(serde_json::to_vec(&anomaly_event)?))
    } else {
        Ok(None)
    }
}

async fn run_worker(
    consumer: Arc<dyn EventConsumer>,
    producer: Arc<dyn EventProducer>,
    state: Arc<WorkerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Subscribe to processed topic
    consumer.subscribe(&[topics::DOCUMENT_PROCESSED]).await?;

    info!("Worker subscribed to {}", topics::DOCUMENT_PROCESSED);

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
            match validate_document(&event).await {
                Ok(Some(anomaly_payload)) => {
                    // Publish anomaly
                    if let Err(e) = producer
                        .publish(topics::ANOMALY_EVENTS, key, &anomaly_payload)
                        .await
                    {
                        error!(error = %e, "Failed to publish anomaly event");
                    }
                    state.increment_anomalies();
                }
                Ok(None) => {
                    // No anomaly detected
                }
                Err(e) => {
                    error!(error = %e, "Failed to validate document");
                }
            }
            state.increment_processed();
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
                    let anomalies = state
                        .anomalies_detected
                        .load(std::sync::atomic::Ordering::Relaxed);

                    Json(serde_json::json!({
                        "healthy": healthy,
                        "messages_processed": processed,
                        "anomalies_detected": anomalies,
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("fen_rules=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!(
        kafka_brokers = %args.kafka_brokers,
        consumer_group = %args.consumer_group,
        rules_path = %args.rules_path,
        "Starting Fen Validation Worker"
    );

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

    run_worker(consumer, producer, state).await?;

    Ok(())
}

mod rand {
    pub fn random<T: Default>() -> T {
        // Simple stub - in production use proper random
        T::default()
    }
}

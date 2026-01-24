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
use fen_core::domain::{Anomaly, AnomalyType, Invoice, Severity, VendorBaseline};
use fen_events::{
    topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent,
};
use fen_rules::RuleEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[cfg(feature = "kafka")]
use fen_events::{KafkaConfig, KafkaConsumer, KafkaProducer};

/// Fen Validation Worker
#[derive(Parser, Debug)]
#[command(name = "fen-validation-worker")]
#[command(about = "Kafka consumer for document validation and anomaly detection")]
struct Args {
    /// Kafka bootstrap servers
    #[arg(
        long,
        env = "KAFKA_BOOTSTRAP_SERVERS",
        default_value = "localhost:9092"
    )]
    kafka_brokers: String,

    /// Consumer group ID
    #[arg(long, env = "CONSUMER_GROUP", default_value = "fen-validation")]
    consumer_group: String,

    /// Rules directory path (for custom JDM rules)
    #[arg(long, env = "RULES_PATH")]
    rules_path: Option<String>,

    /// Enable statistical anomaly detection
    #[arg(long, env = "ENABLE_STATISTICAL", default_value = "true")]
    enable_statistical: bool,

    /// Use local event bus instead of Kafka (for development)
    #[arg(long, env = "USE_LOCAL_BUS")]
    use_local_bus: bool,

    /// Health check port
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,
}

/// Event payload for processed document (bincode serialized)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentProcessedEvent {
    document_id: String,
    tenant_id: String,
    invoice_id: String,
    document_type: String,
    confidence_score: f64,
    processing_time_ms: u64,
    extracted_fields: Vec<String>,
    vendor_name: String,
    invoice_number: String,
    total_amount: String,
    /// The full Invoice for validation
    invoice: Invoice,
}

/// Event payload for detected anomaly
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnomalyDetectedEvent {
    anomaly_id: String,
    document_id: String,
    tenant_id: String,
    invoice_id: String,
    vendor_name: String,
    anomaly_type: String,
    severity: String,
    description: String,
    field_path: Option<String>,
    expected_value: Option<String>,
    actual_value: Option<String>,
    statistical_score: Option<StatisticalScoreEvent>,
    detected_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatisticalScoreEvent {
    z_score: f64,
    percentile: f64,
    trend: String,
    is_outlier: bool,
    baseline_mean: f64,
    baseline_stddev: f64,
    sample_count: u64,
}

/// Event for validation completion summary
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidationCompletedEvent {
    document_id: String,
    tenant_id: String,
    is_valid: bool,
    anomaly_count: u32,
    high_severity_count: u32,
    validation_time_ms: u64,
    completed_at: String,
}

/// Worker state for health checks and baseline caching
struct WorkerState {
    messages_processed: std::sync::atomic::AtomicU64,
    anomalies_detected: std::sync::atomic::AtomicU64,
    healthy: std::sync::atomic::AtomicBool,
    /// Cache of vendor baselines: vendor_name -> baselines
    baseline_cache: RwLock<HashMap<String, Vec<VendorBaseline>>>,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            anomalies_detected: std::sync::atomic::AtomicU64::new(0),
            healthy: std::sync::atomic::AtomicBool::new(true),
            baseline_cache: RwLock::new(HashMap::new()),
        }
    }

    fn increment_processed(&self) {
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn increment_anomalies(&self, count: u64) {
        self.anomalies_detected
            .fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    }

    async fn get_baselines(&self, vendor_name: &str) -> Vec<VendorBaseline> {
        let cache = self.baseline_cache.read().await;
        cache.get(vendor_name).cloned().unwrap_or_default()
    }

    async fn update_baselines(&self, vendor_name: String, baselines: Vec<VendorBaseline>) {
        let mut cache = self.baseline_cache.write().await;
        cache.insert(vendor_name, baselines);
    }
}

/// Convert domain Anomaly to event format
fn anomaly_to_event(anomaly: &Anomaly, processed: &DocumentProcessedEvent) -> AnomalyDetectedEvent {
    let severity_str = match anomaly.severity {
        Severity::Low => "Low",
        Severity::Medium => "Medium",
        Severity::High => "High",
        Severity::Critical => "Critical",
    };

    let anomaly_type_str = match anomaly.anomaly_type {
        AnomalyType::MathMismatch => "MathMismatch",
        AnomalyType::MissingField => "MissingField",
        AnomalyType::InvalidFormat => "InvalidFormat",
        AnomalyType::OutOfRange => "OutOfRange",
        AnomalyType::PotentialDuplicate => "PotentialDuplicate",
        AnomalyType::DateInconsistency => "DateInconsistency",
        AnomalyType::ContractViolation => "ContractViolation",
        AnomalyType::ValidationFailure => "ValidationFailure",
        AnomalyType::StatisticalOutlier => "StatisticalOutlier",
    };

    let statistical_score = anomaly
        .statistical_score
        .as_ref()
        .map(|s| StatisticalScoreEvent {
            z_score: s.z_score,
            percentile: s.percentile,
            trend: format!("{:?}", s.trend),
            is_outlier: s.is_outlier,
            baseline_mean: s.baseline_mean,
            baseline_stddev: s.baseline_stddev,
            sample_count: s.sample_count,
        });

    AnomalyDetectedEvent {
        anomaly_id: uuid::Uuid::new_v4().to_string(),
        document_id: processed.document_id.clone(),
        tenant_id: processed.tenant_id.clone(),
        invoice_id: processed.invoice_id.clone(),
        vendor_name: processed.vendor_name.clone(),
        anomaly_type: anomaly_type_str.to_string(),
        severity: severity_str.to_string(),
        description: anomaly.description.clone(),
        field_path: anomaly.field_path.clone(),
        expected_value: anomaly.expected_value.clone(),
        actual_value: anomaly.actual_value.clone(),
        statistical_score,
        detected_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Validate a document and return anomaly events
async fn validate_document(
    event: &RawEvent,
    engine: &RuleEngine,
    state: &WorkerState,
) -> Result<
    (DocumentProcessedEvent, Vec<AnomalyDetectedEvent>, u64),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let start = Instant::now();

    // Deserialize the processed event (bincode format)
    let processed: DocumentProcessedEvent = bincode::deserialize(&event.payload)?;

    info!(
        document_id = %processed.document_id,
        tenant_id = %processed.tenant_id,
        invoice_id = %processed.invoice_id,
        vendor_name = %processed.vendor_name,
        "Validating document"
    );

    // Get baselines for statistical analysis
    let baselines = state.get_baselines(&processed.vendor_name).await;

    // Run validation using the actual invoice from the event
    let result = engine
        .validate_invoice_with_baselines(&processed.invoice, &baselines)
        .await?;

    let validation_time_ms = start.elapsed().as_millis() as u64;

    // Convert anomalies to events
    let anomaly_events: Vec<AnomalyDetectedEvent> = result
        .anomalies
        .iter()
        .map(|a| anomaly_to_event(a, &processed))
        .collect();

    debug!(
        document_id = %processed.document_id,
        is_valid = result.is_valid,
        anomaly_count = anomaly_events.len(),
        validation_time_ms = validation_time_ms,
        "Validation completed"
    );

    Ok((processed, anomaly_events, validation_time_ms))
}

async fn run_worker(
    consumer: Arc<dyn EventConsumer>,
    producer: Arc<dyn EventProducer>,
    engine: Arc<RuleEngine>,
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

            match validate_document(&event, &engine, &state).await {
                Ok((processed, anomaly_events, validation_time_ms)) => {
                    // Publish each anomaly as a separate event
                    for anomaly in &anomaly_events {
                        let payload = serde_json::to_vec(anomaly)?;
                        if let Err(e) = producer
                            .publish(topics::ANOMALY_EVENTS, key, &payload)
                            .await
                        {
                            error!(
                                error = %e,
                                anomaly_id = %anomaly.anomaly_id,
                                "Failed to publish anomaly event"
                            );
                        }
                    }

                    // Publish validation completion summary
                    let high_severity_count = anomaly_events
                        .iter()
                        .filter(|a| a.severity == "High")
                        .count() as u32;

                    let completion = ValidationCompletedEvent {
                        document_id: processed.document_id.clone(),
                        tenant_id: processed.tenant_id.clone(),
                        is_valid: anomaly_events.is_empty(),
                        anomaly_count: anomaly_events.len() as u32,
                        high_severity_count,
                        validation_time_ms,
                        completed_at: chrono::Utc::now().to_rfc3339(),
                    };

                    let payload = serde_json::to_vec(&completion)?;
                    if let Err(e) = producer
                        .publish(topics::VALIDATION_RESULTS, key, &payload)
                        .await
                    {
                        error!(
                            error = %e,
                            document_id = %processed.document_id,
                            "Failed to publish validation completion"
                        );
                    }

                    state.increment_anomalies(anomaly_events.len() as u64);
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
                .add_directive("fen_rules=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!(
        kafka_brokers = %args.kafka_brokers,
        consumer_group = %args.consumer_group,
        rules_path = ?args.rules_path,
        enable_statistical = args.enable_statistical,
        "Starting Fen Validation Worker"
    );

    // Create rule engine
    let rules_path = args.rules_path.as_ref().map(Path::new);
    let mut engine = RuleEngine::new(rules_path).await?;

    // Enable statistical analysis if requested
    if args.enable_statistical {
        engine = engine.with_statistical_analysis();
        info!("Statistical anomaly detection enabled");
    }

    let engine = Arc::new(engine);
    let state = Arc::new(WorkerState::new());

    // Start health server
    let health_state = state.clone();
    tokio::spawn(async move {
        health_server(args.health_port, health_state).await;
    });

    // Create consumer and producer
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

    run_worker(consumer, producer, engine, state).await?;

    Ok(())
}

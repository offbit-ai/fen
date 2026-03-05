//! Fen Validation Worker - Kafka consumer for rule-based validation.
//!
//! This worker:
//! 1. Consumes documents from `fen.document.processed` topic (bincode)
//! 2. Runs structural + GoRules + statistical validation
//! 3. Persists detected anomalies to the anomaly store
//! 4. Publishes anomalies to `fen.anomaly.detected` topic
//! 5. Publishes validation results summary to `fen.validation.results`
//! 6. Subscribes to baseline updates for statistical analysis
//! 7. Broadcasts notifications via notification hub
//!
//! # Usage
//!
//! ```bash
//! fen-validation-worker --kafka-brokers localhost:9092 --consumer-group fen-validation
//! ```

use clap::Parser;
use fen_core::domain::{Anomaly, AnomalyType, Contract, Invoice, Severity, VendorBaseline};
use fen_events::{
    topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent,
};
use fen_notify::NotificationHub;
use fen_rules::RuleEngine;
use fen_storage::{AnomalyRecord, AnomalyStore, RedbStorage};
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

    /// Storage path for anomaly persistence (redb database)
    #[arg(long, env = "STORAGE_PATH", default_value = "data/fen.redb")]
    storage_path: String,
}

/// Unified processed event — matches the ingestion worker's output format.
/// Deserialized from bincode on the Kafka wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentProcessedEvent {
    document_type: String,
    invoice: Option<Invoice>,
    contract: Option<Contract>,
}

/// Event payload for detected anomaly (JSON for downstream consumers)
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

/// Worker state for health checks, baseline caching, and anomaly persistence.
struct WorkerState {
    messages_processed: std::sync::atomic::AtomicU64,
    anomalies_detected: std::sync::atomic::AtomicU64,
    healthy: std::sync::atomic::AtomicBool,
    /// Cache of vendor baselines: vendor_name -> baselines
    baseline_cache: RwLock<HashMap<String, Vec<VendorBaseline>>>,
    /// Anomaly store for persistence
    anomaly_store: AnomalyStore,
    /// Notification hub for broadcasting alerts
    notification_hub: NotificationHub,
}

impl WorkerState {
    fn new(anomaly_store: AnomalyStore) -> Self {
        Self {
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            anomalies_detected: std::sync::atomic::AtomicU64::new(0),
            healthy: std::sync::atomic::AtomicBool::new(true),
            baseline_cache: RwLock::new(HashMap::new()),
            anomaly_store,
            notification_hub: NotificationHub::new(),
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
fn anomaly_to_event(anomaly: &Anomaly, invoice: &Invoice) -> AnomalyDetectedEvent {
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
        document_id: invoice.document_id.to_string(),
        tenant_id: invoice.tenant_id.to_string(),
        invoice_id: invoice.id.to_string(),
        vendor_name: invoice.vendor.name.clone(),
        anomaly_type: anomaly_type_str.to_string(),
        severity: severity_str.to_string(),
        description: anomaly.description.clone(),
        field_path: anomaly.field_path.clone(),
        expected_value: anomaly.expected_value.clone(),
        actual_value: anomaly.actual_value.clone(),
        statistical_score,
        detected_at: anomaly
            .detected_at
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
    }
}

/// Convert domain Anomaly to AnomalyRecord for persistence.
fn anomaly_to_record(anomaly: &Anomaly, invoice: &Invoice) -> AnomalyRecord {
    let mut record = AnomalyRecord::new(
        anomaly.document_id.clone(),
        &invoice.vendor.name,
        anomaly.anomaly_type.clone(),
        anomaly.severity,
        &anomaly.description,
    )
    .with_invoice_id(invoice.id)
    .with_confidence(anomaly.confidence)
    .with_invoice_date(invoice.invoice_date);

    if let Some(ref field) = anomaly.field_path {
        record = record.with_field(field);
    }

    if let (Some(ref expected), Some(ref actual)) = (&anomaly.expected_value, &anomaly.actual_value)
    {
        record = record.with_values(expected, actual);
    }

    if let Some(ref score) = anomaly.statistical_score {
        record = record.with_statistical_score(score);
    }

    record
}

/// Persist anomalies to the anomaly store. Non-fatal — logs warnings on individual failures.
async fn persist_anomalies(
    anomaly_store: &AnomalyStore,
    anomalies: &[Anomaly],
    invoice: &Invoice,
) {
    for anomaly in anomalies {
        let record = anomaly_to_record(anomaly, invoice);
        if let Err(e) = anomaly_store.store_anomaly(&record).await {
            warn!(
                anomaly_type = %anomaly.anomaly_type,
                invoice_id = %invoice.id,
                error = %e,
                "Failed to persist anomaly"
            );
        }
    }
}

/// Map domain anomaly severities to notification severity.
fn map_anomaly_severity(anomalies: &[Anomaly]) -> fen_notify::Severity {
    if anomalies
        .iter()
        .any(|a| a.severity == Severity::Critical)
    {
        fen_notify::Severity::Critical
    } else if anomalies
        .iter()
        .any(|a| a.severity == Severity::High)
    {
        fen_notify::Severity::Error
    } else if anomalies
        .iter()
        .any(|a| a.severity == Severity::Medium)
    {
        fen_notify::Severity::Warning
    } else {
        fen_notify::Severity::Info
    }
}

/// Validate an invoice and return anomaly events
async fn validate_invoice(
    invoice: &Invoice,
    engine: &RuleEngine,
    state: &WorkerState,
) -> Result<
    (Vec<AnomalyDetectedEvent>, bool, u64),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let start = Instant::now();

    // Get baselines for statistical analysis
    let baselines = state.get_baselines(&invoice.vendor.name).await;

    // Run full validation: structural + statistical + GoRules
    let result = engine
        .validate_invoice_with_baselines(invoice, &baselines)
        .await?;

    let validation_time_ms = start.elapsed().as_millis() as u64;

    // Convert anomalies to events
    let anomaly_events: Vec<AnomalyDetectedEvent> = result
        .anomalies
        .iter()
        .map(|a| anomaly_to_event(a, invoice))
        .collect();

    // Persist anomalies to store
    if !result.anomalies.is_empty() {
        persist_anomalies(&state.anomaly_store, &result.anomalies, invoice).await;
    }

    // Broadcast notifications for anomalies
    if !result.anomalies.is_empty() {
        let severity = map_anomaly_severity(&result.anomalies);
        let anomaly_count = result.anomalies.len();

        let payload = state.notification_hub.anomaly_notification(
            invoice.tenant_id.clone(),
            &invoice.document_id.to_string(),
            &format!("{} anomalies detected", anomaly_count),
            severity,
            &format!(
                "Invoice {} from vendor '{}' has {} anomalies requiring review",
                invoice.invoice_number, invoice.vendor.name, anomaly_count,
            ),
        );

        if let Err(e) = state.notification_hub.broadcast(payload).await {
            warn!(error = %e, "Failed to broadcast anomaly notification");
        }
    }

    debug!(
        invoice_id = %invoice.id,
        is_valid = result.is_valid,
        anomaly_count = anomaly_events.len(),
        validation_time_ms,
        "Validation completed"
    );

    Ok((anomaly_events, result.is_valid, validation_time_ms))
}

/// Process a baseline update event to refresh the vendor baseline cache.
async fn process_baseline_update(event: &RawEvent, state: &WorkerState) {
    #[derive(Deserialize)]
    struct BaselineUpdate {
        vendor_name: String,
        baselines: Vec<VendorBaseline>,
    }

    match serde_json::from_slice::<BaselineUpdate>(&event.payload) {
        Ok(update) => {
            info!(
                vendor = %update.vendor_name,
                baseline_count = update.baselines.len(),
                "Updating vendor baselines"
            );
            state
                .update_baselines(update.vendor_name, update.baselines)
                .await;
        }
        Err(e) => {
            warn!(error = %e, "Failed to deserialize baseline update");
        }
    }
}

async fn run_worker(
    consumer: Arc<dyn EventConsumer>,
    producer: Arc<dyn EventProducer>,
    engine: Arc<RuleEngine>,
    state: Arc<WorkerState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Subscribe to processed documents and baseline updates
    consumer
        .subscribe(&[topics::DOCUMENT_PROCESSED, topics::BASELINE_UPDATES])
        .await?;

    info!(
        "Worker subscribed to {} and {}",
        topics::DOCUMENT_PROCESSED,
        topics::BASELINE_UPDATES
    );

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
            let topic = &event.topic;

            // Route by topic
            if topic == topics::BASELINE_UPDATES {
                process_baseline_update(&event, &state).await;
                continue;
            }

            // Deserialize the processed event (bincode format from ingestion worker)
            let processed: DocumentProcessedEvent = match bincode::deserialize(&event.payload) {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "Failed to deserialize DOCUMENT_PROCESSED event");
                    continue;
                }
            };

            match processed.document_type.as_str() {
                "invoice" => {
                    let Some(ref invoice) = processed.invoice else {
                        warn!("Invoice event missing invoice payload");
                        continue;
                    };

                    info!(
                        invoice_id = %invoice.id,
                        vendor = %invoice.vendor.name,
                        "Validating invoice"
                    );

                    match validate_invoice(invoice, &engine, &state).await {
                        Ok((anomaly_events, is_valid, validation_time_ms)) => {
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

                            // Count High + Critical for severity summary
                            let high_severity_count = anomaly_events
                                .iter()
                                .filter(|a| a.severity == "High" || a.severity == "Critical")
                                .count() as u32;

                            let completion = ValidationCompletedEvent {
                                document_id: invoice.document_id.to_string(),
                                tenant_id: invoice.tenant_id.to_string(),
                                is_valid,
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
                                    invoice_id = %invoice.id,
                                    "Failed to publish validation completion"
                                );
                            }

                            state.increment_anomalies(anomaly_events.len() as u64);
                        }
                        Err(e) => {
                            error!(
                                invoice_id = %invoice.id,
                                error = %e,
                                "Failed to validate invoice"
                            );
                        }
                    }
                }
                "contract" => {
                    if let Some(ref contract) = processed.contract {
                        debug!(
                            contract_id = %contract.id,
                            "Contract processed event received — no invoice validation needed"
                        );
                        // Contract events pass through for downstream consumers;
                        // contract-specific validation can be added here in the future.
                    }
                }
                other => {
                    warn!(document_type = other, "Unknown document type, skipping");
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
        storage_path = %args.storage_path,
        "Starting Fen Validation Worker"
    );

    // Initialize storage for anomaly persistence
    if let Some(parent) = Path::new(&args.storage_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let storage = RedbStorage::new(&args.storage_path)?;
    let anomaly_store = AnomalyStore::new(storage.db().clone())?;
    info!(path = %args.storage_path, "Anomaly store initialized");

    // Create rule engine
    let rules_path = args.rules_path.as_ref().map(Path::new);
    let mut engine = RuleEngine::new(rules_path).await?;

    // Enable statistical analysis if requested
    if args.enable_statistical {
        engine = engine.with_statistical_analysis();
        info!("Statistical anomaly detection enabled");
    }

    let engine = Arc::new(engine);
    let state = Arc::new(WorkerState::new(anomaly_store));

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

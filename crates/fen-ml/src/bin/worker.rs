//! Fen Baseline Worker - Kafka consumer for vendor baseline updates.
//!
//! This worker:
//! 1. Consumes documents from `fen.document.processed` topic
//! 2. Extracts statistical features (prices, quantities, etc.)
//! 3. Updates vendor baselines with new data points
//! 4. Publishes baseline updates to `fen.baseline.updates` topic
//!
//! # Usage
//!
//! ```bash
//! fen-baseline-worker --kafka-brokers localhost:9092 --consumer-group fen-baseline
//! ```

use clap::Parser;
use fen_core::domain::Invoice;
use fen_events::{
    topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[cfg(feature = "kafka")]
use fen_events::{KafkaConfig, KafkaConsumer, KafkaProducer};

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
    /// The full Invoice - baseline worker extracts metrics from it
    invoice: Invoice,
}

/// Event payload for baseline update
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaselineComputedEvent {
    baseline_id: String,
    vendor_name: String,
    tenant_id: String,
    metric_name: String,
    mean: f64,
    stddev: f64,
    min_value: f64,
    max_value: f64,
    sample_count: u64,
    updated_at: String,
}

/// Fen Baseline Worker
#[derive(Parser, Debug)]
#[command(name = "fen-baseline-worker")]
#[command(about = "Kafka consumer for vendor baseline computation")]
struct Args {
    /// Kafka bootstrap servers
    #[arg(
        long,
        env = "KAFKA_BOOTSTRAP_SERVERS",
        default_value = "localhost:9092"
    )]
    kafka_brokers: String,

    /// Consumer group ID
    #[arg(long, env = "CONSUMER_GROUP", default_value = "fen-baseline")]
    consumer_group: String,

    /// Minimum samples before publishing baseline
    #[arg(long, env = "MIN_SAMPLES", default_value = "10")]
    min_samples: usize,

    /// Use local event bus instead of Kafka
    #[arg(long, env = "USE_LOCAL_BUS")]
    use_local_bus: bool,

    /// Health check port
    #[arg(long, env = "HEALTH_PORT", default_value = "8080")]
    health_port: u16,
}

/// Running statistics for a vendor metric using Welford's online algorithm
#[derive(Debug, Clone)]
struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64, // For Welford's online variance algorithm
    min_value: f64,
    max_value: f64,
}

impl Default for RunningStats {
    fn default() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min_value: f64::MAX,
            max_value: f64::MIN,
        }
    }
}

impl RunningStats {
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;

        // Track min/max
        if value < self.min_value {
            self.min_value = value;
        }
        if value > self.max_value {
            self.max_value = value;
        }
    }

    fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }
}

/// Vendor statistics key combining vendor name and tenant
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct VendorKey {
    tenant_id: String,
    vendor_name: String,
}

/// Worker state
struct WorkerState {
    messages_processed: std::sync::atomic::AtomicU64,
    baselines_updated: std::sync::atomic::AtomicU64,
    /// (tenant_id, vendor_name) -> metric_name -> RunningStats
    vendor_stats: RwLock<HashMap<VendorKey, HashMap<String, RunningStats>>>,
    healthy: std::sync::atomic::AtomicBool,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            messages_processed: std::sync::atomic::AtomicU64::new(0),
            baselines_updated: std::sync::atomic::AtomicU64::new(0),
            vendor_stats: RwLock::new(HashMap::new()),
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn increment_processed(&self) {
        self.messages_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn increment_baselines(&self, count: u64) {
        self.baselines_updated
            .fetch_add(count, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Extract numeric metrics from the invoice
fn extract_metrics(invoice: &Invoice) -> Vec<(String, f64)> {
    use rust_decimal::prelude::ToPrimitive;
    let mut metrics = Vec::new();

    // Extract total_amount
    if let Some(value) = invoice.total_amount.to_f64() {
        metrics.push(("total_amount".to_string(), value));
    }

    // Extract subtotal
    if let Some(value) = invoice.subtotal.to_f64() {
        metrics.push(("subtotal".to_string(), value));
    }

    // Extract tax_amount
    if let Some(value) = invoice.tax_amount.to_f64() {
        metrics.push(("tax_amount".to_string(), value));
    }

    // Extract line_item_count
    metrics.push((
        "line_item_count".to_string(),
        invoice.line_items.len() as f64,
    ));

    // Extract confidence_score
    metrics.push((
        "confidence_score".to_string(),
        invoice.confidence_score as f64,
    ));

    metrics
}

/// Process a document event and update baseline statistics
/// Returns baseline events to publish when enough samples are collected
async fn process_for_baseline(
    event: &RawEvent,
    state: &WorkerState,
    min_samples: usize,
) -> Vec<Vec<u8>> {
    // 1. Deserialize DocumentProcessed event (bincode format)
    let processed: DocumentProcessedEvent = match bincode::deserialize(&event.payload) {
        Ok(p) => p,
        Err(e) => {
            error!(error = %e, "Failed to deserialize DocumentProcessedEvent");
            return Vec::new();
        }
    };

    // Skip if vendor name is empty
    if processed.vendor_name.is_empty() {
        debug!(
            document_id = %processed.document_id,
            "Skipping document with empty vendor name"
        );
        return Vec::new();
    }

    debug!(
        document_id = %processed.document_id,
        tenant_id = %processed.tenant_id,
        vendor_name = %processed.vendor_name,
        "Processing document for baseline"
    );

    // 2. Extract numeric fields from the invoice
    let metrics = extract_metrics(&processed.invoice);

    if metrics.is_empty() {
        debug!(
            document_id = %processed.document_id,
            "No numeric metrics extracted from document"
        );
        return Vec::new();
    }

    let vendor_key = VendorKey {
        tenant_id: processed.tenant_id.clone(),
        vendor_name: processed.vendor_name.clone(),
    };

    let mut baseline_events = Vec::new();

    // 3. Update running statistics for each metric
    let mut stats = state.vendor_stats.write().await;
    let vendor_metrics = stats.entry(vendor_key.clone()).or_default();

    for (metric_name, value) in metrics {
        let metric_stats = vendor_metrics.entry(metric_name.clone()).or_default();
        metric_stats.update(value);

        // 4. If enough samples, compute and return baseline
        // Publish baseline update every 10 samples after minimum threshold
        if metric_stats.count as usize >= min_samples && metric_stats.count % 10 == 0 {
            let baseline_event = BaselineComputedEvent {
                baseline_id: uuid::Uuid::new_v4().to_string(),
                vendor_name: vendor_key.vendor_name.clone(),
                tenant_id: vendor_key.tenant_id.clone(),
                metric_name: metric_name.clone(),
                mean: metric_stats.mean,
                stddev: metric_stats.stddev(),
                min_value: metric_stats.min_value,
                max_value: metric_stats.max_value,
                sample_count: metric_stats.count,
                updated_at: chrono::Utc::now().to_rfc3339(),
            };

            info!(
                vendor_name = %baseline_event.vendor_name,
                metric_name = %baseline_event.metric_name,
                mean = baseline_event.mean,
                stddev = baseline_event.stddev,
                sample_count = baseline_event.sample_count,
                "Publishing baseline update"
            );

            if let Ok(payload) = serde_json::to_vec(&baseline_event) {
                baseline_events.push(payload);
            }
        }
    }

    baseline_events
}

async fn run_worker(
    consumer: Arc<dyn EventConsumer>,
    producer: Arc<dyn EventProducer>,
    state: Arc<WorkerState>,
    min_samples: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            let baseline_payloads = process_for_baseline(&event, &state, min_samples).await;

            for baseline_payload in &baseline_payloads {
                if let Err(e) = producer
                    .publish(topics::BASELINE_UPDATES, key, baseline_payload)
                    .await
                {
                    warn!(error = %e, "Failed to publish baseline update");
                }
            }

            if !baseline_payloads.is_empty() {
                state.increment_baselines(baseline_payloads.len() as u64);
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
                    let baselines = state
                        .baselines_updated
                        .load(std::sync::atomic::Ordering::Relaxed);

                    Json(serde_json::json!({
                        "healthy": healthy,
                        "messages_processed": processed,
                        "baselines_updated": baselines,
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
                .add_directive("fen_ml=debug".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    info!(
        kafka_brokers = %args.kafka_brokers,
        consumer_group = %args.consumer_group,
        min_samples = args.min_samples,
        "Starting Fen Baseline Worker"
    );

    let state = Arc::new(WorkerState::new());

    let health_state = state.clone();
    tokio::spawn(async move {
        health_server(args.health_port, health_state).await;
    });

    let (consumer, producer): (Arc<dyn EventConsumer>, Arc<dyn EventProducer>) =
        if args.use_local_bus {
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
                tracing::error!(
                    "Kafka feature not enabled. Use --use-local-bus or enable the 'kafka' feature."
                );
                std::process::exit(1);
            }
        };

    run_worker(consumer, producer, state, args.min_samples).await?;

    Ok(())
}

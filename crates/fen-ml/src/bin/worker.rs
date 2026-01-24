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
use fen_events::{topics, EventConsumer, EventProducer, LocalEventBus, LocalEventConsumer, RawEvent};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[cfg(feature = "kafka")]
use fen_events::{KafkaConfig, KafkaConsumer, KafkaProducer};

/// Fen Baseline Worker
#[derive(Parser, Debug)]
#[command(name = "fen-baseline-worker")]
#[command(about = "Kafka consumer for vendor baseline computation")]
struct Args {
    /// Kafka bootstrap servers
    #[arg(long, env = "KAFKA_BOOTSTRAP_SERVERS", default_value = "localhost:9092")]
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

/// Running statistics for a vendor metric
#[derive(Debug, Clone, Default)]
struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64, // For Welford's online variance algorithm
}

impl RunningStats {
    fn update(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
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

/// Worker state
struct WorkerState {
    messages_processed: std::sync::atomic::AtomicU64,
    baselines_updated: std::sync::atomic::AtomicU64,
    // vendor_name -> metric_name -> RunningStats
    vendor_stats: RwLock<HashMap<String, HashMap<String, RunningStats>>>,
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

    fn increment_baselines(&self) {
        self.baselines_updated
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

async fn process_for_baseline(
    event: &RawEvent,
    state: &WorkerState,
    min_samples: usize,
) -> Option<Vec<u8>> {
    // TODO: Implement actual baseline extraction:
    // 1. Deserialize DocumentProcessed event
    // 2. Extract vendor name and numeric fields
    // 3. Update running statistics
    // 4. If enough samples, compute and return baseline

    let key = event.key.as_deref().unwrap_or_default();

    // Stub implementation
    let vendor_name = format!("vendor_{}", key.first().unwrap_or(&0) % 10);
    let metric_name = "total_amount".to_string();
    let value = 100.0 + (event.payload.len() as f64 % 50.0); // Fake value

    let mut stats = state.vendor_stats.write().await;
    let vendor_metrics = stats.entry(vendor_name.clone()).or_default();
    let metric_stats = vendor_metrics.entry(metric_name.clone()).or_default();
    metric_stats.update(value);

    if metric_stats.count as usize >= min_samples && metric_stats.count % 10 == 0 {
        // Publish baseline update every 10 samples after minimum
        let baseline_event = serde_json::json!({
            "baseline_id": uuid::Uuid::new_v4().to_string(),
            "vendor_name": vendor_name,
            "metric_name": metric_name,
            "mean": metric_stats.mean,
            "stddev": metric_stats.stddev(),
            "sample_count": metric_stats.count,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });

        return Some(serde_json::to_vec(&baseline_event).ok()?);
    }

    None
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
            if let Some(baseline_payload) = process_for_baseline(&event, &state, min_samples).await
            {
                if let Err(e) = producer
                    .publish(topics::BASELINE_UPDATES, key, &baseline_payload)
                    .await
                {
                    warn!(error = %e, "Failed to publish baseline update");
                }
                state.increment_baselines();
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
            tracing::error!("Kafka feature not enabled. Use --use-local-bus or enable the 'kafka' feature.");
            std::process::exit(1);
        }
    };

    run_worker(consumer, producer, state, args.min_samples).await?;

    Ok(())
}

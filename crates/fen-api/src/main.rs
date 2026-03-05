use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod config;
mod conversions;
pub mod db;
mod error;
mod gates;
mod middleware;
mod routes;
mod state;
mod workers;

use crate::config::AppConfig;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fen_api=debug,fen_ingestion=debug,fen_rules=debug,fen_storage=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Fen API server");

    // Load configuration
    let config = AppConfig::from_env();

    tracing::info!(
        bind_address = %config.bind_address,
        database_path = %config.database_path,
        "Configuration loaded"
    );

    // Initialize application state
    let state = Arc::new(AppState::new(&config).await?);

    tracing::info!("Application state initialized");

    // Spawn background anomaly detection worker on the local event bus
    if let Some(ref event_bus) = state.event_bus {
        let _worker_handle = workers::spawn_anomaly_worker(
            event_bus.clone(),
            state.rule_engine.clone(),
            state.anomaly_store.clone(),
            state.notification_hub.clone(),
        );
        tracing::info!("Anomaly detection worker spawned");
    }

    // Build router
    let app = app::build_router(state, &config);

    // Start server
    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    tracing::info!("Listening on {}", config.bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}

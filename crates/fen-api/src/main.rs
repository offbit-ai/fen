use std::sync::Arc;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use fen_api::app;
use fen_api::config::AppConfig;
use fen_api::state::AppState;
use fen_api::workers;

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

    // Start server with graceful shutdown
    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    tracing::info!("Listening on {}", config.bind_address);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down"),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }
}

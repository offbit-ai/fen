use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::routes::{documents, health, ingest, validate};
use crate::state::AppState;

/// Build the application router
pub fn build_router(state: Arc<AppState>, config: &AppConfig) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health::health_check))
        // Documents - combined GET and POST on same path
        .route("/documents", get(documents::list_documents).post(ingest::ingest_document))
        .route("/documents/:id", get(documents::get_document).delete(documents::delete_document))
        // Validation
        .route("/validate", post(validate::validate_documents))
        // Stats
        .route("/stats", get(documents::get_stats))
        // Add middleware
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(config.max_upload_size))
        .layer(cors)
        .with_state(state)
}

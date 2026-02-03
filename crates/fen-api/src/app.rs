use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::middleware::RateLimitLayer;
use crate::routes::{documents, events, health, ingest, notifications, rules, search, storage, validate};
use crate::state::AppState;

/// Build the application router
pub fn build_router(state: Arc<AppState>, config: &AppConfig) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create rate limiter from config
    let rate_limit = RateLimitLayer::new(config.rate_limit_rps, config.rate_limit_burst);

    Router::new()
        // Health check (no rate limiting)
        .route("/health", get(health::health_check))
        // Documents - combined GET and POST on same path
        .route(
            "/documents",
            get(documents::list_documents).post(ingest::ingest_document),
        )
        .route(
            "/documents/:id",
            get(documents::get_document).delete(documents::delete_document),
        )
        // Validation
        .route("/validate", post(validate::validate_documents))
        // Stats
        .route("/stats", get(documents::get_stats))
        // Search endpoints
        .route("/search/text", get(search::text_search))
        .route("/search/semantic", get(search::semantic_search))
        .route("/search/query", post(search::execute_query))
        // Storage management endpoints
        .route("/storage/tiers", get(storage::tier_distribution))
        .route("/storage/cache/stats", get(storage::cache_stats))
        .route("/storage/cache/clear", post(storage::clear_cache))
        .route("/storage/migrate", post(storage::trigger_migration))
        // Rules management endpoints
        .route("/rules/status", get(rules::rule_status))
        .route("/rules/config", get(rules::rule_config))
        // Notification endpoints
        .route("/notifications/providers", get(notifications::list_providers))
        .route("/notifications/health", get(notifications::provider_health))
        .route("/notifications/send", post(notifications::send_notification))
        .route(
            "/notifications/preferences/:tenant_id",
            get(notifications::get_preferences).put(notifications::set_preferences),
        )
        // Event streaming endpoints
        .route("/events/topics", get(events::list_topics))
        .route("/events/stream", get(events::event_stream))
        .route("/events/metrics", get(events::metrics_stream))
        // Add middleware (order matters - rate limit first, then trace, then body limit)
        .layer(TraceLayer::new_for_http())
        .layer(rate_limit)
        .layer(RequestBodyLimitLayer::new(config.max_upload_size))
        .layer(cors)
        .with_state(state)
}

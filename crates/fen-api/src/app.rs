use std::sync::Arc;

use axum::{
    routing::{get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::middleware::RateLimitLayer;
use crate::routes::{admin, auth, documents, events, health, ingest, notifications, rules, search, storage, validate};
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
        // Authentication endpoints (public)
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/token", post(auth::token))
        .route("/auth/userinfo", get(auth::userinfo))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/providers", get(auth::list_providers))
        // Admin endpoints - Tenant management (SystemAdmin only)
        .route("/admin/tenants", get(admin::list_tenants).post(admin::create_tenant))
        .route(
            "/admin/tenants/:tenant_id",
            get(admin::get_tenant).put(admin::update_tenant),
        )
        .route("/admin/tenants/:tenant_id/suspend", post(admin::suspend_tenant))
        .route("/admin/tenants/:tenant_id/activate", post(admin::activate_tenant))
        // Admin endpoints - User management (TenantAdmin or SystemAdmin)
        .route("/admin/users", get(admin::list_users).post(admin::create_user))
        .route(
            "/admin/users/:user_id",
            get(admin::get_user).put(admin::update_user).delete(admin::delete_user),
        )
        .route("/admin/users/:user_id/roles", put(admin::assign_roles))
        .route("/admin/users/:user_id/suspend", post(admin::suspend_user))
        .route("/admin/users/:user_id/activate", post(admin::activate_user))
        // Admin dashboard
        .route("/admin/dashboard", get(admin::dashboard))
        // Add middleware (order matters - rate limit first, then trace, then body limit)
        .layer(TraceLayer::new_for_http())
        .layer(rate_limit)
        .layer(RequestBodyLimitLayer::new(config.max_upload_size))
        .layer(cors)
        .with_state(state)
}

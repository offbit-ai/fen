use std::sync::Arc;

use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::middleware::{auth_middleware, metrics::metrics_middleware, RateLimitLayer};
use crate::routes::{admin, anomalies, auth, documents, events, graph, health, ingest, notifications, rules, search, storage, validate};
use crate::state::AppState;

/// Build the application router
pub fn build_router(state: Arc<AppState>, config: &AppConfig) -> Router {
    let cors = build_cors_layer(config);

    // Create rate limiter from config
    let rate_limit = RateLimitLayer::new(config.rate_limit_rps, config.rate_limit_burst);

    // Auth config for the middleware
    let auth_config = Arc::new(state.auth_config.clone());

    // Initialize Prometheus metrics recorder
    let prom_handle = crate::middleware::metrics::init_metrics();

    // Public routes — no authentication required
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route(
            "/metrics",
            get(crate::middleware::metrics::metrics_handler)
                .with_state(prom_handle),
        )
        // Authentication endpoints (must be public for login flow)
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/token", post(auth::token))
        .route("/auth/providers", get(auth::list_providers));

    // Protected routes — require valid JWT
    let protected_routes = Router::new()
        // Documents
        .route(
            "/documents",
            get(documents::list_documents).post(ingest::ingest_document),
        )
        .route(
            "/documents/:id",
            get(documents::get_document).delete(documents::delete_document),
        )
        // Ingest
        .route("/ingest", post(ingest::ingest_document))
        .route("/ingest/contract", post(ingest::ingest_contract))
        // Per-document actions
        .route("/documents/:id/validate", post(documents::validate_document))
        .route("/documents/:id/approve", post(documents::approve_document))
        .route("/documents/:id/reject", post(documents::reject_document))
        // Validation (bulk)
        .route("/validate", post(validate::validate_documents))
        // Stats
        .route("/stats", get(documents::get_stats))
        // Anomaly endpoints
        .route("/anomalies", get(anomalies::list_anomalies))
        .route("/anomalies/stats", get(anomalies::get_anomaly_stats))
        .route("/anomalies/bulk/resolve", post(anomalies::bulk_resolve))
        .route("/anomalies/bulk/dismiss", post(anomalies::bulk_dismiss))
        .route("/anomalies/:id", get(anomalies::get_anomaly))
        .route("/anomalies/:id/resolve", post(anomalies::resolve_anomaly))
        .route("/anomalies/:id/dismiss", post(anomalies::dismiss_anomaly))
        .route("/anomalies/:id/investigate", post(anomalies::investigate_anomaly))
        // Search endpoints
        .route("/search/text", get(search::text_search))
        .route("/search/semantic", get(search::semantic_search))
        .route("/search/query", post(search::execute_query))
        // Unified query engine
        .route("/query", post(search::execute_query))
        .route("/query/zip", post(search::execute_zip_query))
        // Storage management
        .route("/storage/tiers", get(storage::tier_distribution))
        .route("/storage/cache/stats", get(storage::cache_stats))
        .route("/storage/cache/clear", post(storage::clear_cache))
        .route("/storage/migrate", post(storage::trigger_migration))
        // Rules management
        .route("/rules", get(rules::list_rules))
        .route("/rules/status", get(rules::rule_status))
        .route("/rules/config", get(rules::rule_config))
        .route("/rules/:id", get(rules::get_rule))
        .route("/rules/:id/test", post(rules::test_rule))
        // Notifications
        .route("/notifications/providers", get(notifications::list_providers))
        .route("/notifications/health", get(notifications::provider_health))
        .route("/notifications/send", post(notifications::send_notification))
        .route(
            "/notifications/preferences/:tenant_id",
            get(notifications::get_preferences).put(notifications::set_preferences),
        )
        // Knowledge graph
        .route("/graph/query", post(graph::query_cypher))
        .route("/graph/vendors/:name/network", get(graph::vendor_network))
        .route("/graph/invoices/:id/contracts", get(graph::invoice_contracts))
        .route("/graph/rdf/load", post(graph::rdf_load))
        .route("/graph/rdf/inspect", post(graph::rdf_inspect))
        // Event streaming
        .route("/events/topics", get(events::list_topics))
        .route("/events/stream", get(events::event_stream))
        .route("/events/metrics", get(events::metrics_stream))
        // Auth (protected — requires existing auth)
        .route("/auth/userinfo", get(auth::userinfo))
        .route("/auth/logout", post(auth::logout))
        // Admin endpoints
        .route("/admin/tenants", get(admin::list_tenants).post(admin::create_tenant))
        .route(
            "/admin/tenants/:tenant_id",
            get(admin::get_tenant).put(admin::update_tenant),
        )
        .route("/admin/tenants/:tenant_id/suspend", post(admin::suspend_tenant))
        .route("/admin/tenants/:tenant_id/activate", post(admin::activate_tenant))
        .route("/admin/users", get(admin::list_users).post(admin::create_user))
        .route(
            "/admin/users/:user_id",
            get(admin::get_user).put(admin::update_user).delete(admin::delete_user),
        )
        .route("/admin/users/:user_id/roles", put(admin::assign_roles))
        .route("/admin/users/:user_id/suspend", post(admin::suspend_user))
        .route("/admin/users/:user_id/activate", post(admin::activate_user))
        .route("/admin/dashboard", get(admin::dashboard))
        // Apply auth middleware to all protected routes
        .layer(middleware::from_fn_with_state(auth_config, auth_middleware));

    // Merge public and protected route groups
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn_with_state(state.clone(), metrics_middleware))
        .layer(TraceLayer::new_for_http())
        .layer(rate_limit)
        .layer(RequestBodyLimitLayer::new(config.max_upload_size))
        .layer(cors)
        .with_state(state)
}

/// Build CORS layer from configuration.
fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    use axum::http::{HeaderName, Method};
    use tower_http::cors::AllowOrigin;

    let methods = vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::OPTIONS,
    ];

    let headers = vec![
        HeaderName::from_static("authorization"),
        HeaderName::from_static("content-type"),
        HeaderName::from_static("accept"),
    ];

    let origin = if config.cors_origins.is_empty()
        || config.cors_origins.iter().any(|o| o == "*")
    {
        AllowOrigin::any()
    } else {
        let origins: Vec<_> = config
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        AllowOrigin::list(origins)
    };

    CorsLayer::new()
        .allow_origin(origin)
        .allow_methods(methods)
        .allow_headers(headers)
}

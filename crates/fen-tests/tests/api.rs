//! API route integration tests.
//!
//! Tests the HTTP layer: auth enforcement, input validation, and happy-path
//! document ingestion via `tower::ServiceExt::oneshot`.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use fen_api::app::build_router;
use fen_api::config::AppConfig;
use fen_api::middleware::AuthConfig;
use fen_api::state::AppState;
use fen_events::LocalEventBus;
use fen_rules::RuleEngine;
use fen_storage::{AnomalyStore, RedbStorage};

/// Build a minimal `AppState` suitable for testing (no ML, no graph, no DB).
async fn test_state(require_auth: bool) -> (Arc<AppState>, AppConfig) {
    let storage = Arc::new(RedbStorage::in_memory().expect("redb in-memory"));
    let anomaly_store =
        Arc::new(AnomalyStore::new(storage.db().clone()).expect("anomaly store"));
    let rule_engine = Arc::new(
        RuleEngine::new(None::<&std::path::Path>)
            .await
            .expect("rule engine"),
    );
    let ingestion = Arc::new(
        fen_ingestion::IngestionPipeline::new(storage.clone()).expect("ingestion"),
    );
    let event_bus = Arc::new(LocalEventBus::new());

    let config = AppConfig {
        bind_address: "127.0.0.1:0".to_string(),
        database_path: "unused".to_string(),
        rules_path: None,
        max_upload_size: 10 * 1024 * 1024,
        rate_limit_rps: 1000,
        rate_limit_burst: 2000,
        statistical: Default::default(),
        ml: Default::default(),
        warm_storage_path: "unused".to_string(),
        graph_storage_path: None,
        cors_origins: vec![],
    };

    let state = Arc::new(AppState {
        storage,
        ingestion,
        rule_engine,
        anomaly_store,
        fulltext_index: None,
        warm_storage: None,
        query_engine: None,
        query_executor: None,
        zip_executor: None,
        location_index: None,
        tiered_storage: None,
        notification_hub: None,
        event_bus: Some(event_bus),
        document_intelligence: None,
        graph_store: None,
        auth_config: AuthConfig {
            jwt_secret: "test-secret-key".to_string(),
            require_auth,
        },
        oidc_config: None,
        db_pools: None,
        repositories: None,
    });

    (state, config)
}

fn make_jwt(secret: &str, tenant_id: &str, roles: &[&str]) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::json;

    let now = chrono::Utc::now().timestamp() as usize;
    let claims = json!({
        "sub": "test-user",
        "tenant_id": tenant_id,
        "roles": roles,
        "iat": now,
        "exp": now + 3600,
    });

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

// ── Auth enforcement ────────────────────────────────────────────────

#[tokio::test]
async fn protected_route_returns_401_without_token() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/documents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_returns_401_with_invalid_token() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/documents")
                .header("Authorization", "Bearer invalid.jwt.token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn protected_route_accepts_valid_token() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let tenant_id = uuid::Uuid::new_v4().to_string();
    let token = make_jwt("test-secret-key", &tenant_id, &["analyst"]);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/documents")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // GET /documents with valid auth should return 200 (empty list)
    assert_eq!(response.status(), StatusCode::OK);
}

// ── Public routes ───────────────────────────────────────────────────

#[tokio::test]
async fn health_endpoint_is_public() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_endpoint_is_public() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// ── Input validation ────────────────────────────────────────────────

#[tokio::test]
async fn ingest_rejects_non_pdf_extension() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let tenant_id = uuid::Uuid::new_v4().to_string();
    let token = make_jwt("test-secret-key", &tenant_id, &["analyst"]);

    // Build a multipart body with a .txt file
    let boundary = "----TestBoundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\nContent-Type: text/plain\r\n\r\nhello world\r\n--{boundary}--\r\n"
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ingest")
                .header("Authorization", format!("Bearer {}", token))
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn ingest_rejects_invalid_pdf_magic_bytes() {
    let (state, config) = test_state(true).await;
    let app = build_router(state, &config);

    let tenant_id = uuid::Uuid::new_v4().to_string();
    let token = make_jwt("test-secret-key", &tenant_id, &["analyst"]);

    // Build a multipart body with .pdf extension but wrong magic bytes
    let boundary = "----TestBoundary";
    let fake_content = "NOT-A-PDF file content here";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"fake.pdf\"\r\nContent-Type: application/pdf\r\n\r\n{fake_content}\r\n--{boundary}--\r\n"
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ingest")
                .header("Authorization", format!("Bearer {}", token))
                .header(
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

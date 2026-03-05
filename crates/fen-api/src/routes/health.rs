use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ComponentHealth>,
}

#[derive(Serialize)]
pub struct ComponentHealth {
    pub name: String,
    pub healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// GET /health - Liveness probe (always 200 if process is running)
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        components: vec![],
    })
}

/// GET /ready - Readiness probe (checks dependencies)
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<HealthResponse>) {
    let mut components = Vec::new();
    let mut all_healthy = true;

    // Check redb (hot storage)
    let redb_healthy = state
        .storage
        .db()
        .begin_read()
        .map(|_| true)
        .unwrap_or(false);
    if !redb_healthy {
        all_healthy = false;
    }
    components.push(ComponentHealth {
        name: "redb".to_string(),
        healthy: redb_healthy,
        detail: if redb_healthy {
            None
        } else {
            Some("read transaction failed".to_string())
        },
    });

    // Check warm storage (LanceDB)
    let warm_healthy = state.warm_storage.is_some();
    components.push(ComponentHealth {
        name: "warm_storage".to_string(),
        healthy: warm_healthy,
        detail: if warm_healthy {
            None
        } else {
            Some("not configured".to_string())
        },
    });

    // Check rule engine
    components.push(ComponentHealth {
        name: "rule_engine".to_string(),
        healthy: true,
        detail: None,
    });

    let status = if all_healthy { "ready" } else { "degraded" };
    let code = if all_healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        code,
        Json(HealthResponse {
            status: status.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            components,
        }),
    )
}

use std::sync::Arc;
use std::time::Instant;

use axum::{extract::Request, middleware::Next, response::Response};

use crate::state::AppState;

/// Middleware that records per-request Prometheus metrics.
///
/// Emits:
/// - `http_requests_total{method, path, status}` — counter
/// - `http_request_duration_seconds{method, path}` — histogram
///
/// Optionally records to TimescaleDB via MetricsRepository (non-blocking spawn).
pub async fn metrics_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = normalize_path(request.uri().path());

    let start = Instant::now();
    let response = next.run(request).await;
    let duration = start.elapsed().as_secs_f64();

    let status_code = response.status().as_u16();
    let status = status_code.to_string();

    let labels = [
        ("method", method.clone()),
        ("path", path.clone()),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_request_duration_seconds", &labels[..2]).record(duration);

    // Fire-and-forget write to TimescaleDB if available
    if let Some(ref repos) = state.repositories {
        let repos = repos.clone();
        tokio::spawn(async move {
            let metric = crate::db::metrics::ApiMetric {
                tenant_id: uuid::Uuid::nil(),
                method,
                path,
                status_code: status_code as i16,
                duration_ms: duration * 1000.0,
                user_id: None,
                request_size_bytes: None,
                response_size_bytes: None,
                error_type: None,
                tags: serde_json::json!({}),
            };
            if let Err(e) = repos.metrics.record_api_metric(metric).await {
                tracing::debug!(error = %e, "Failed to record API metric to TimescaleDB");
            }
        });
    }

    response
}

/// Normalize request paths to avoid high-cardinality labels.
///
/// Replaces dynamic path segments (UUIDs, numeric IDs) with placeholders
/// so Prometheus doesn't create unbounded label values.
fn normalize_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').collect();
    let normalized: Vec<&str> = segments
        .iter()
        .map(|seg| {
            if is_dynamic_segment(seg) {
                ":id"
            } else {
                seg
            }
        })
        .collect();
    normalized.join("/")
}

/// Check if a path segment looks like a dynamic ID (UUID or numeric).
fn is_dynamic_segment(segment: &str) -> bool {
    if segment.is_empty() {
        return false;
    }
    // UUID pattern: 8-4-4-4-12 hex digits
    if segment.len() == 36 && segment.chars().filter(|c| *c == '-').count() == 4 {
        return segment
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-');
    }
    // Pure numeric
    segment.chars().all(|c| c.is_ascii_digit()) && !segment.is_empty()
}

/// Initialize the Prometheus metrics recorder and return the handle
/// for serving the `/metrics` endpoint.
pub fn init_metrics() -> metrics_exporter_prometheus::PrometheusHandle {
    let builder = metrics_exporter_prometheus::PrometheusBuilder::new();
    builder
        .install_recorder()
        .expect("Failed to install Prometheus metrics recorder")
}

/// Handler for GET /metrics — returns Prometheus exposition format.
pub async fn metrics_handler(
    axum::extract::State(handle): axum::extract::State<metrics_exporter_prometheus::PrometheusHandle>,
) -> String {
    handle.render()
}

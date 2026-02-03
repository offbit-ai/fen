//! Metrics repository for TimescaleDB.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use fen_core::domain::TenantId;

/// API metric record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMetric {
    pub tenant_id: Uuid,
    pub method: String,
    pub path: String,
    pub status_code: i16,
    pub duration_ms: f64,
    pub user_id: Option<Uuid>,
    pub request_size_bytes: Option<i32>,
    pub response_size_bytes: Option<i32>,
    pub error_type: Option<String>,
    pub tags: serde_json::Value,
}

/// Document event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEvent {
    pub tenant_id: Uuid,
    pub document_id: Uuid,
    pub event_type: String,
    pub stage: Option<String>,
    pub duration_ms: Option<f64>,
    pub success: bool,
    pub document_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
}

/// Anomaly event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub tenant_id: Uuid,
    pub anomaly_id: Uuid,
    pub document_id: Uuid,
    pub anomaly_type: String,
    pub severity: String,
    pub confidence: Option<f64>,
    pub detection_method: Option<String>,
    pub z_score: Option<f64>,
    pub percentile: Option<f64>,
    pub status: String,
    pub description: Option<String>,
    pub metadata: serde_json::Value,
}

/// System metric record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetric {
    pub node_id: String,
    pub metric_name: String,
    pub metric_value: f64,
    pub labels: serde_json::Value,
}

/// Tenant metrics summary
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TenantMetricsSummary {
    pub total_requests: Option<i64>,
    pub avg_latency_ms: Option<f64>,
    pub error_rate: Option<f64>,
    pub documents_processed: Option<i64>,
    pub anomalies_detected: Option<i64>,
    pub anomalies_open: Option<i64>,
}

/// Hourly API metrics aggregate
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ApiMetricsHourly {
    pub bucket: DateTime<Utc>,
    pub tenant_id: Uuid,
    pub path: String,
    pub request_count: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
    pub error_count: i64,
}

/// Repository for metrics operations
#[derive(Clone)]
pub struct MetricsRepository {
    pool: PgPool,
}

impl MetricsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Insert Methods
    // =========================================================================

    /// Record an API metric
    pub async fn record_api_metric(&self, metric: ApiMetric) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO api_metrics (
                time, tenant_id, method, path, status_code, duration_ms,
                user_id, request_size_bytes, response_size_bytes, error_type, tags
            )
            VALUES (NOW(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(metric.tenant_id)
        .bind(&metric.method)
        .bind(&metric.path)
        .bind(metric.status_code)
        .bind(metric.duration_ms)
        .bind(metric.user_id)
        .bind(metric.request_size_bytes)
        .bind(metric.response_size_bytes)
        .bind(&metric.error_type)
        .bind(&metric.tags)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a document event
    pub async fn record_document_event(&self, event: DocumentEvent) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO document_events (
                time, tenant_id, document_id, event_type, stage, duration_ms,
                success, document_type, file_size_bytes, error_message, metadata
            )
            VALUES (NOW(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(event.tenant_id)
        .bind(event.document_id)
        .bind(&event.event_type)
        .bind(&event.stage)
        .bind(event.duration_ms)
        .bind(event.success)
        .bind(&event.document_type)
        .bind(event.file_size_bytes)
        .bind(&event.error_message)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record an anomaly event
    pub async fn record_anomaly_event(&self, event: AnomalyEvent) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO anomaly_events (
                time, tenant_id, anomaly_id, document_id, anomaly_type, severity,
                confidence, detection_method, z_score, percentile, status, description, metadata
            )
            VALUES (NOW(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
        )
        .bind(event.tenant_id)
        .bind(event.anomaly_id)
        .bind(event.document_id)
        .bind(&event.anomaly_type)
        .bind(&event.severity)
        .bind(event.confidence)
        .bind(&event.detection_method)
        .bind(event.z_score)
        .bind(event.percentile)
        .bind(&event.status)
        .bind(&event.description)
        .bind(&event.metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record a system metric
    pub async fn record_system_metric(&self, metric: SystemMetric) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO system_metrics (time, node_id, metric_name, metric_value, labels)
            VALUES (NOW(), $1, $2, $3, $4)
            "#,
        )
        .bind(&metric.node_id)
        .bind(&metric.metric_name)
        .bind(metric.metric_value)
        .bind(&metric.labels)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // =========================================================================
    // Query Methods
    // =========================================================================

    /// Get tenant metrics summary (uses the helper function)
    pub async fn get_tenant_summary(
        &self,
        tenant_id: &TenantId,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<TenantMetricsSummary, sqlx::Error> {
        let summary = sqlx::query_as::<_, TenantMetricsSummary>(
            "SELECT * FROM get_tenant_metrics_summary($1, $2, $3)",
        )
        .bind(tenant_id.0)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(&self.pool)
        .await?;

        Ok(summary)
    }

    /// Get hourly API metrics for a tenant
    pub async fn get_api_metrics_hourly(
        &self,
        tenant_id: &TenantId,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        path_filter: Option<&str>,
    ) -> Result<Vec<ApiMetricsHourly>, sqlx::Error> {
        let rows = if let Some(path) = path_filter {
            sqlx::query_as::<_, ApiMetricsHourly>(
                r#"
                SELECT * FROM api_metrics_hourly
                WHERE tenant_id = $1 AND bucket BETWEEN $2 AND $3 AND path = $4
                ORDER BY bucket DESC
                "#,
            )
            .bind(tenant_id.0)
            .bind(start_time)
            .bind(end_time)
            .bind(path)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, ApiMetricsHourly>(
                r#"
                SELECT * FROM api_metrics_hourly
                WHERE tenant_id = $1 AND bucket BETWEEN $2 AND $3
                ORDER BY bucket DESC
                "#,
            )
            .bind(tenant_id.0)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    /// Get recent document events
    pub async fn get_recent_document_events(
        &self,
        tenant_id: &TenantId,
        limit: i64,
    ) -> Result<Vec<DocumentEventRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, DocumentEventRow>(
            r#"
            SELECT time, document_id, event_type, stage, duration_ms, success,
                   document_type, file_size_bytes, error_message
            FROM document_events
            WHERE tenant_id = $1
            ORDER BY time DESC
            LIMIT $2
            "#,
        )
        .bind(tenant_id.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get recent anomaly events
    pub async fn get_recent_anomaly_events(
        &self,
        tenant_id: &TenantId,
        limit: i64,
        severity_filter: Option<&str>,
    ) -> Result<Vec<AnomalyEventRow>, sqlx::Error> {
        let rows = if let Some(severity) = severity_filter {
            sqlx::query_as::<_, AnomalyEventRow>(
                r#"
                SELECT time, anomaly_id, document_id, anomaly_type, severity,
                       confidence, status, description
                FROM anomaly_events
                WHERE tenant_id = $1 AND severity = $2
                ORDER BY time DESC
                LIMIT $3
                "#,
            )
            .bind(tenant_id.0)
            .bind(severity)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AnomalyEventRow>(
                r#"
                SELECT time, anomaly_id, document_id, anomaly_type, severity,
                       confidence, status, description
                FROM anomaly_events
                WHERE tenant_id = $1
                ORDER BY time DESC
                LIMIT $2
                "#,
            )
            .bind(tenant_id.0)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    /// Get error rate over time
    pub async fn get_error_rate_timeseries(
        &self,
        tenant_id: &TenantId,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval: &str,
    ) -> Result<Vec<ErrorRatePoint>, sqlx::Error> {
        let query = format!(
            r#"
            SELECT
                time_bucket('{}', time) AS bucket,
                COUNT(*) AS total,
                SUM(CASE WHEN status_code >= 400 THEN 1 ELSE 0 END) AS errors
            FROM api_metrics
            WHERE tenant_id = $1 AND time BETWEEN $2 AND $3
            GROUP BY bucket
            ORDER BY bucket
            "#,
            interval
        );

        let rows = sqlx::query_as::<_, ErrorRatePoint>(&query)
            .bind(tenant_id.0)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }
}

// Additional row types for queries

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct DocumentEventRow {
    pub time: DateTime<Utc>,
    pub document_id: Uuid,
    pub event_type: String,
    pub stage: Option<String>,
    pub duration_ms: Option<f64>,
    pub success: bool,
    pub document_type: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct AnomalyEventRow {
    pub time: DateTime<Utc>,
    pub anomaly_id: Uuid,
    pub document_id: Uuid,
    pub anomaly_type: String,
    pub severity: String,
    pub confidence: Option<f64>,
    pub status: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ErrorRatePoint {
    pub bucket: DateTime<Utc>,
    pub total: i64,
    pub errors: i64,
}

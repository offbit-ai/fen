use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fen_core::domain::AnomalyId;
use fen_storage::{AnomalyRecord, AnomalyStatus};

use crate::error::ApiError;
use crate::state::AppState;

// --- Request / Response types ---

#[derive(Deserialize)]
pub struct ListAnomaliesParams {
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub severity: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub anomaly_type: Option<String>,
    pub document_id: Option<String>,
    #[allow(dead_code)]
    pub from_date: Option<String>,
    #[allow(dead_code)]
    pub to_date: Option<String>,
    #[allow(dead_code)]
    pub sort_by: Option<String>,
    #[allow(dead_code)]
    pub sort_order: Option<String>,
}

#[derive(Serialize)]
pub struct AnomalyResponse {
    pub id: String,
    pub document_id: String,
    pub anomaly_type: String,
    pub severity: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_value: Option<String>,
    pub confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistical_score: Option<StatisticalScoreResponse>,
    pub detected_at: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct StatisticalScoreResponse {
    pub z_score: f64,
    pub percentile: f64,
    pub baseline_mean: f64,
    pub baseline_stddev: f64,
}

#[derive(Serialize)]
pub struct AnomalyListResponse {
    pub items: Vec<AnomalyResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Deserialize)]
pub struct ResolveRequest {
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct DismissRequest {
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkResolveRequest {
    pub ids: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkDismissRequest {
    pub ids: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct BulkActionResponse {
    pub resolved: Option<usize>,
    pub dismissed: Option<usize>,
}

#[derive(Serialize)]
pub struct AnomalyStatsResponse {
    pub total: usize,
    pub by_severity: HashMap<String, usize>,
    pub by_status: HashMap<String, usize>,
    pub by_type: HashMap<String, usize>,
}

// --- Helpers ---

fn anomaly_type_to_string(at: &fen_core::domain::AnomalyType) -> String {
    match at {
        fen_core::domain::AnomalyType::MathMismatch => "math_mismatch".to_string(),
        fen_core::domain::AnomalyType::MissingField => "missing_field".to_string(),
        fen_core::domain::AnomalyType::InvalidFormat => "invalid_format".to_string(),
        fen_core::domain::AnomalyType::OutOfRange => "out_of_range".to_string(),
        fen_core::domain::AnomalyType::PotentialDuplicate => "potential_duplicate".to_string(),
        fen_core::domain::AnomalyType::DateInconsistency => "date_inconsistency".to_string(),
        fen_core::domain::AnomalyType::ContractViolation => "contract_violation".to_string(),
        fen_core::domain::AnomalyType::ValidationFailure => "validation_failure".to_string(),
        fen_core::domain::AnomalyType::StatisticalOutlier => "statistical_outlier".to_string(),
    }
}

fn severity_to_string(s: &fen_core::domain::Severity) -> String {
    match s {
        fen_core::domain::Severity::Low => "low".to_string(),
        fen_core::domain::Severity::Medium => "medium".to_string(),
        fen_core::domain::Severity::High => "high".to_string(),
        fen_core::domain::Severity::Critical => "critical".to_string(),
    }
}

fn record_to_response(r: &AnomalyRecord) -> AnomalyResponse {
    let statistical_score = if r.z_score.is_some() || r.percentile.is_some() {
        Some(StatisticalScoreResponse {
            z_score: r.z_score.unwrap_or(0.0),
            percentile: r.percentile.unwrap_or(0.0),
            baseline_mean: r.baseline_mean.unwrap_or(0.0),
            baseline_stddev: r.baseline_stddev.unwrap_or(0.0),
        })
    } else {
        None
    };

    AnomalyResponse {
        id: r.id.to_string(),
        document_id: r.document_id.to_string(),
        anomaly_type: anomaly_type_to_string(&r.anomaly_type),
        severity: severity_to_string(&r.severity),
        description: r.description.clone(),
        field_path: r.field_path.clone(),
        expected_value: r.expected_value.clone(),
        actual_value: r.actual_value.clone(),
        confidence: r.confidence,
        statistical_score,
        detected_at: r.detected_at.to_rfc3339(),
        status: r.status.to_string(),
        resolved_by: r.resolved_by.clone(),
        resolved_at: r.resolved_at.map(|dt| dt.to_rfc3339()),
        created_at: r.detected_at.to_rfc3339(),
    }
}

fn parse_anomaly_id(id_str: &str) -> Result<AnomalyId, ApiError> {
    let uuid = Uuid::parse_str(id_str)
        .map_err(|_| ApiError::BadRequest(format!("Invalid anomaly ID: {}", id_str)))?;
    Ok(AnomalyId(uuid))
}

fn parse_status_filter(s: &str) -> Option<AnomalyStatus> {
    match s {
        "open" => Some(AnomalyStatus::Open),
        "investigating" => Some(AnomalyStatus::Investigating),
        "resolved" => Some(AnomalyStatus::Resolved),
        "dismissed" => Some(AnomalyStatus::Dismissed),
        _ => None,
    }
}

fn parse_severity_filter(s: &str) -> Option<fen_core::domain::Severity> {
    match s {
        "low" => Some(fen_core::domain::Severity::Low),
        "medium" => Some(fen_core::domain::Severity::Medium),
        "high" => Some(fen_core::domain::Severity::High),
        "critical" => Some(fen_core::domain::Severity::Critical),
        _ => None,
    }
}

fn parse_type_filter(s: &str) -> Option<fen_core::domain::AnomalyType> {
    match s {
        "math_mismatch" => Some(fen_core::domain::AnomalyType::MathMismatch),
        "missing_field" => Some(fen_core::domain::AnomalyType::MissingField),
        "invalid_format" => Some(fen_core::domain::AnomalyType::InvalidFormat),
        "out_of_range" => Some(fen_core::domain::AnomalyType::OutOfRange),
        "potential_duplicate" => Some(fen_core::domain::AnomalyType::PotentialDuplicate),
        "date_inconsistency" => Some(fen_core::domain::AnomalyType::DateInconsistency),
        "contract_violation" => Some(fen_core::domain::AnomalyType::ContractViolation),
        "validation_failure" => Some(fen_core::domain::AnomalyType::ValidationFailure),
        "statistical_outlier" => Some(fen_core::domain::AnomalyType::StatisticalOutlier),
        _ => None,
    }
}

// --- Handlers ---

/// GET /anomalies - List anomalies with pagination and filters
pub async fn list_anomalies(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListAnomaliesParams>,
) -> Result<Json<AnomalyListResponse>, ApiError> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    // Get all anomalies (we filter in memory for now — redb doesn't have complex queries)
    let all = state.anomaly_store.list_all(10000, 0).await?;
    let _total_unfiltered = all.len();

    // Apply filters
    let filtered: Vec<&AnomalyRecord> = all
        .iter()
        .filter(|r| {
            if let Some(ref sev) = params.severity {
                if let Some(filter_sev) = parse_severity_filter(sev) {
                    if r.severity != filter_sev {
                        return false;
                    }
                }
            }
            if let Some(ref status) = params.status {
                if let Some(filter_status) = parse_status_filter(status) {
                    if r.status != filter_status {
                        return false;
                    }
                }
            }
            if let Some(ref at) = params.anomaly_type {
                if let Some(filter_type) = parse_type_filter(at) {
                    if r.anomaly_type != filter_type {
                        return false;
                    }
                }
            }
            if let Some(ref doc_id) = params.document_id {
                if r.document_id.to_string() != *doc_id {
                    return false;
                }
            }
            true
        })
        .collect();

    let total = filtered.len();
    let items: Vec<AnomalyResponse> = filtered
        .into_iter()
        .skip(offset)
        .take(page_size)
        .map(record_to_response)
        .collect();

    Ok(Json(AnomalyListResponse {
        items,
        total,
        limit: page_size,
        offset,
    }))
}

/// GET /anomalies/stats - Get anomaly statistics
pub async fn get_anomaly_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AnomalyStatsResponse>, ApiError> {
    let stats = state.anomaly_store.get_stats().await?;

    let mut by_severity = HashMap::new();
    by_severity.insert("low".to_string(), stats.by_severity_low);
    by_severity.insert("medium".to_string(), stats.by_severity_medium);
    by_severity.insert("high".to_string(), stats.by_severity_high);
    by_severity.insert("critical".to_string(), stats.by_severity_critical);

    let mut by_status = HashMap::new();
    by_status.insert("open".to_string(), stats.by_status_open);
    by_status.insert("investigating".to_string(), stats.by_status_investigating);
    by_status.insert("resolved".to_string(), stats.by_status_resolved);
    by_status.insert("dismissed".to_string(), stats.by_status_dismissed);

    // Convert AnomalyType debug names to snake_case for frontend
    let by_type: HashMap<String, usize> = stats
        .by_type
        .into_iter()
        .map(|(k, v)| {
            let snake = match k.as_str() {
                "MathMismatch" => "math_mismatch",
                "MissingField" => "missing_field",
                "InvalidFormat" => "invalid_format",
                "OutOfRange" => "out_of_range",
                "PotentialDuplicate" => "potential_duplicate",
                "DateInconsistency" => "date_inconsistency",
                "ContractViolation" => "contract_violation",
                "ValidationFailure" => "validation_failure",
                "StatisticalOutlier" => "statistical_outlier",
                other => other,
            };
            (snake.to_string(), v)
        })
        .collect();

    Ok(Json(AnomalyStatsResponse {
        total: stats.total,
        by_severity,
        by_status,
        by_type,
    }))
}

/// GET /anomalies/:id - Get a single anomaly
pub async fn get_anomaly(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AnomalyResponse>, ApiError> {
    let anomaly_id = parse_anomaly_id(&id)?;

    let record = state
        .anomaly_store
        .get_anomaly(&anomaly_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Anomaly not found: {}", id)))?;

    Ok(Json(record_to_response(&record)))
}

/// POST /anomalies/:id/resolve - Resolve an anomaly
pub async fn resolve_anomaly(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ResolveRequest>,
) -> Result<Json<AnomalyResponse>, ApiError> {
    let anomaly_id = parse_anomaly_id(&id)?;

    let mut record = state
        .anomaly_store
        .get_anomaly(&anomaly_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Anomaly not found: {}", id)))?;

    record.status = AnomalyStatus::Resolved;
    record.resolved_at = Some(chrono::Utc::now());
    record.resolution_notes = body.notes;
    record.resolved_by = Some("system".to_string());

    state.anomaly_store.update_anomaly(&record).await?;

    Ok(Json(record_to_response(&record)))
}

/// POST /anomalies/:id/dismiss - Dismiss an anomaly
pub async fn dismiss_anomaly(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DismissRequest>,
) -> Result<Json<AnomalyResponse>, ApiError> {
    let anomaly_id = parse_anomaly_id(&id)?;

    let mut record = state
        .anomaly_store
        .get_anomaly(&anomaly_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Anomaly not found: {}", id)))?;

    record.status = AnomalyStatus::Dismissed;
    record.resolved_at = Some(chrono::Utc::now());
    record.resolution_notes = body.reason;
    record.resolved_by = Some("system".to_string());

    state.anomaly_store.update_anomaly(&record).await?;

    Ok(Json(record_to_response(&record)))
}

/// POST /anomalies/:id/investigate - Mark anomaly as investigating
pub async fn investigate_anomaly(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<AnomalyResponse>, ApiError> {
    let anomaly_id = parse_anomaly_id(&id)?;

    let mut record = state
        .anomaly_store
        .get_anomaly(&anomaly_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Anomaly not found: {}", id)))?;

    record.status = AnomalyStatus::Investigating;

    state.anomaly_store.update_anomaly(&record).await?;

    Ok(Json(record_to_response(&record)))
}

/// POST /anomalies/bulk/resolve - Bulk resolve anomalies
pub async fn bulk_resolve(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkResolveRequest>,
) -> Result<Json<BulkActionResponse>, ApiError> {
    let mut resolved = 0;

    for id_str in &body.ids {
        if let Ok(anomaly_id) = parse_anomaly_id(id_str) {
            if let Ok(Some(mut record)) = state.anomaly_store.get_anomaly(&anomaly_id).await {
                record.status = AnomalyStatus::Resolved;
                record.resolved_at = Some(chrono::Utc::now());
                record.resolution_notes = body.notes.clone();
                record.resolved_by = Some("system".to_string());

                if state.anomaly_store.update_anomaly(&record).await.is_ok() {
                    resolved += 1;
                }
            }
        }
    }

    Ok(Json(BulkActionResponse {
        resolved: Some(resolved),
        dismissed: None,
    }))
}

/// POST /anomalies/bulk/dismiss - Bulk dismiss anomalies
pub async fn bulk_dismiss(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BulkDismissRequest>,
) -> Result<Json<BulkActionResponse>, ApiError> {
    let mut dismissed = 0;

    for id_str in &body.ids {
        if let Ok(anomaly_id) = parse_anomaly_id(id_str) {
            if let Ok(Some(mut record)) = state.anomaly_store.get_anomaly(&anomaly_id).await {
                record.status = AnomalyStatus::Dismissed;
                record.resolved_at = Some(chrono::Utc::now());
                record.resolution_notes = body.reason.clone();
                record.resolved_by = Some("system".to_string());

                if state.anomaly_store.update_anomaly(&record).await.is_ok() {
                    dismissed += 1;
                }
            }
        }
    }

    Ok(Json(BulkActionResponse {
        resolved: None,
        dismissed: Some(dismissed),
    }))
}

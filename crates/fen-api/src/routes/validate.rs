use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fen_core::domain::InvoiceId;
use fen_storage::DocumentStore;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ValidateRequest {
    pub document_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct ValidateResponse {
    pub results: Vec<DocumentValidation>,
    pub total_validated: usize,
    pub total_passed: usize,
    pub total_failed: usize,
}

#[derive(Serialize)]
pub struct DocumentValidation {
    pub document_id: String,
    pub is_valid: bool,
    pub anomaly_count: usize,
    pub has_critical: bool,
    pub validation_time_ms: u64,
    pub anomalies: Vec<AnomalyDto>,
}

#[derive(Serialize)]
pub struct AnomalyDto {
    pub anomaly_type: String,
    pub severity: String,
    pub description: String,
    pub field_path: Option<String>,
    pub expected_value: Option<String>,
    pub actual_value: Option<String>,
    pub confidence: f32,
}

/// POST /validate - Validate documents for anomalies
pub async fn validate_documents(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, ApiError> {
    if request.document_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one document ID is required".to_string(),
        ));
    }

    if request.document_ids.len() > 100 {
        return Err(ApiError::BadRequest(
            "Maximum 100 documents per request".to_string(),
        ));
    }

    let mut results = Vec::with_capacity(request.document_ids.len());
    let mut total_passed = 0;
    let mut total_failed = 0;

    for id_str in &request.document_ids {
        let uuid = Uuid::parse_str(id_str)
            .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id_str)))?;

        let invoice_id = InvoiceId(uuid);

        // Get the invoice
        let invoice = state
            .storage
            .get_invoice(&invoice_id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Document not found: {}", id_str)))?;

        // Run validation through rule engine
        let validation_result = state.rule_engine.validate_invoice(&invoice).await?;

        if validation_result.is_valid {
            total_passed += 1;
        } else {
            total_failed += 1;
        }

        // Persist anomalies to anomaly store
        if !validation_result.anomalies.is_empty() {
            crate::conversions::persist_anomalies(
                &state.anomaly_store,
                &validation_result.anomalies,
                &invoice,
            ).await;
        }

        results.push(DocumentValidation {
            document_id: id_str.clone(),
            is_valid: validation_result.is_valid,
            anomaly_count: validation_result.anomalies.len(),
            has_critical: validation_result.has_critical(),
            validation_time_ms: validation_result.validation_time_ms,
            anomalies: validation_result
                .anomalies
                .into_iter()
                .map(|a| AnomalyDto {
                    anomaly_type: a.anomaly_type.to_string(),
                    severity: a.severity.to_string(),
                    description: a.description,
                    field_path: a.field_path,
                    expected_value: a.expected_value,
                    actual_value: a.actual_value,
                    confidence: a.confidence,
                })
                .collect(),
        });
    }

    Ok(Json(ValidateResponse {
        total_validated: results.len(),
        total_passed,
        total_failed,
        results,
    }))
}

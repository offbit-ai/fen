use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fen_core::domain::{Contract, ContractId, InvoiceId};
use fen_storage::DocumentStore;

use crate::error::ApiError;
use crate::routes::ingest::{
    contract_type_to_string, invoice_to_full_response, validation_status_to_string,
    InvoiceFullResponse,
};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    // Frontend pagination params (page-based)
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    // Type filter: "invoice" or "contract"
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
    // Additional filters (recognized but not yet fully implemented)
    #[allow(dead_code)]
    pub status: Option<String>,
    #[allow(dead_code)]
    pub vendor: Option<String>,
    #[allow(dead_code)]
    pub sort_by: Option<String>,
    #[allow(dead_code)]
    pub sort_order: Option<String>,
}

impl ListParams {
    fn resolve_limit_offset(&self) -> (usize, usize) {
        if let (Some(page), Some(page_size)) = (self.page, self.page_size) {
            let page = page.max(1);
            let limit = page_size.min(1000);
            let offset = (page - 1) * limit;
            (limit, offset)
        } else {
            let limit = self.limit.unwrap_or(100).min(1000);
            let offset = self.offset.unwrap_or(0);
            (limit, offset)
        }
    }
}

#[derive(Serialize)]
pub struct InvoiceListResponse {
    pub items: Vec<InvoiceFullResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Serialize)]
pub struct ContractResponse {
    pub id: String,
    pub document_id: String,
    pub tenant_id: String,
    pub contract_number: Option<String>,
    pub title: String,
    pub contract_type: String,
    pub parties: Vec<crate::routes::ingest::PartyResponse>,
    pub effective_date: String,
    pub expiration_date: Option<String>,
    pub execution_date: Option<String>,
    pub total_value: Option<String>,
    pub currency: Option<String>,
    pub validation_status: String,
    pub confidence_score: f32,
    pub vendor_name: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn contract_to_response(c: &Contract) -> ContractResponse {
    let now = chrono::Utc::now().to_rfc3339();
    let vendor_name = c
        .parties
        .first()
        .map(|p| p.name.clone())
        .unwrap_or_default();

    let status = compute_contract_status(c);

    ContractResponse {
        id: c.id.to_string(),
        document_id: c.document_id.to_string(),
        tenant_id: c.tenant_id.to_string(),
        contract_number: c.contract_number.clone(),
        title: c.title.clone(),
        contract_type: contract_type_to_string(&c.contract_type),
        parties: c
            .parties
            .iter()
            .map(|p| crate::routes::ingest::PartyResponse {
                id: p.id.to_string(),
                name: p.name.clone(),
                tax_id: p.tax_id.clone(),
                address: p.address.as_ref().map(|a| crate::routes::ingest::AddressResponse {
                    street: a.street.clone(),
                    city: a.city.clone(),
                    state: a.state.clone(),
                    postal_code: a.postal_code.clone(),
                    country: a.country.clone(),
                }),
                contact: p.contact.as_ref().map(|ct| crate::routes::ingest::ContactResponse {
                    name: ct.name.clone(),
                    email: ct.email.clone(),
                    phone: ct.phone.clone(),
                }),
            })
            .collect(),
        effective_date: c.effective_date.to_string(),
        expiration_date: c.expiration_date.map(|d| d.to_string()),
        execution_date: c.execution_date.map(|d| d.to_string()),
        total_value: c.total_value.map(|v| v.to_string()),
        currency: c.currency.as_ref().map(|cur| cur.to_string()),
        validation_status: validation_status_to_string(&c.validation_status),
        confidence_score: c.confidence_score,
        vendor_name,
        start_date: c.effective_date.to_string(),
        end_date: c.expiration_date.map(|d| d.to_string()),
        status,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn compute_contract_status(c: &Contract) -> String {
    let today = chrono::Utc::now().date_naive();
    if let Some(exp) = c.expiration_date {
        if exp < today {
            return "expired".to_string();
        }
        let days_until = (exp - today).num_days();
        if days_until <= 30 {
            return "expiring_soon".to_string();
        }
    }
    if c.effective_date > today {
        return "draft".to_string();
    }
    "active".to_string()
}

#[derive(Serialize)]
pub struct ContractListResponse {
    pub items: Vec<ContractResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ListResponse {
    Invoices(InvoiceListResponse),
    Contracts(ContractListResponse),
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_invoices: usize,
    pub total_contracts: usize,
    pub total_value: f64,
    pub total_anomalies: usize,
    pub open_anomalies: usize,
    pub anomaly_rate: f64,
}

/// GET /documents - List documents (supports ?type=invoice|contract, page/page_size)
pub async fn list_documents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<ListResponse>, ApiError> {
    let (limit, offset) = params.resolve_limit_offset();
    let doc_type = params.doc_type.as_deref().unwrap_or("invoice");

    match doc_type {
        "contract" => {
            let contracts = state.storage.list_contracts(limit, offset).await?;
            let total = state.storage.count_contracts().await?;

            let items: Vec<ContractResponse> = contracts.iter().map(contract_to_response).collect();

            Ok(Json(ListResponse::Contracts(ContractListResponse {
                items,
                total,
                limit,
                offset,
            })))
        }
        _ => {
            let invoices = state.storage.list_invoices(limit, offset).await?;
            let total = state.storage.count_invoices().await?;

            let items: Vec<InvoiceFullResponse> =
                invoices.iter().map(invoice_to_full_response).collect();

            Ok(Json(ListResponse::Invoices(InvoiceListResponse {
                items,
                total,
                limit,
                offset,
            })))
        }
    }
}

/// GET /documents/:id - Get a specific document (invoice or contract)
pub async fn get_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    // Try invoice first
    let invoice_id = InvoiceId(uuid);
    if let Some(invoice) = state.storage.get_invoice(&invoice_id).await? {
        let response = invoice_to_full_response(&invoice);
        return Ok(Json(
            serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))?,
        ));
    }

    // Try contract
    let contract_id = ContractId(uuid);
    if let Some(contract) = state.storage.get_contract(&contract_id).await? {
        let response = contract_to_response(&contract);
        return Ok(Json(
            serde_json::to_value(response).map_err(|e| ApiError::Internal(e.to_string()))?,
        ));
    }

    Err(ApiError::NotFound(format!("Document not found: {}", id)))
}

/// DELETE /documents/:id - Delete a document (invoice or contract)
pub async fn delete_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    let invoice_id = InvoiceId(uuid);
    if state.storage.delete_invoice(&invoice_id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }

    let contract_id = ContractId(uuid);
    if state.storage.delete_contract(&contract_id).await? {
        return Ok(StatusCode::NO_CONTENT);
    }

    Err(ApiError::NotFound(format!("Document not found: {}", id)))
}

/// GET /stats - Get storage statistics with dashboard metrics
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatsResponse>, ApiError> {
    let total_invoices = state.storage.count_invoices().await?;
    let total_contracts = state.storage.count_contracts().await?;

    // Compute total invoice value
    let invoices = state.storage.list_invoices(10000, 0).await?;
    let total_value: f64 = invoices
        .iter()
        .filter_map(|inv| inv.total_amount.to_string().parse::<f64>().ok())
        .sum();

    // Get anomaly stats
    let anomaly_stats = state.anomaly_store.get_stats().await?;
    let total_docs = total_invoices + total_contracts;
    let anomaly_rate = if total_docs > 0 {
        (anomaly_stats.by_status_open as f64 / total_docs as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(StatsResponse {
        total_invoices,
        total_contracts,
        total_value,
        total_anomalies: anomaly_stats.total,
        open_anomalies: anomaly_stats.by_status_open,
        anomaly_rate,
    }))
}

// --- Per-document actions ---

#[derive(Deserialize)]
pub struct ApproveRequest {
    #[allow(dead_code)]
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct RejectRequest {
    #[allow(dead_code)]
    pub reason: Option<String>,
}

/// POST /documents/:id/validate - Validate a single document
pub async fn validate_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    let invoice_id = InvoiceId(uuid);
    let invoice = state
        .storage
        .get_invoice(&invoice_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Document not found: {}", id)))?;

    let result = state.rule_engine.validate_invoice(&invoice).await?;

    Ok(Json(serde_json::json!({
        "valid": result.is_valid,
        "errors": result.anomalies.iter().map(|a| a.description.clone()).collect::<Vec<_>>(),
        "anomaly_count": result.anomalies.len(),
        "validation_time_ms": result.validation_time_ms,
    })))
}

/// POST /documents/:id/approve - Approve a document
pub async fn approve_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(_body): Json<ApproveRequest>,
) -> Result<Json<InvoiceFullResponse>, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    let invoice_id = InvoiceId(uuid);
    let mut invoice = state
        .storage
        .get_invoice(&invoice_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Document not found: {}", id)))?;

    invoice.validation_status = fen_core::ValidationStatus::Passed;
    state.storage.store_invoice(&invoice).await?;

    Ok(Json(invoice_to_full_response(&invoice)))
}

/// POST /documents/:id/reject - Reject a document
pub async fn reject_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(_body): Json<RejectRequest>,
) -> Result<Json<InvoiceFullResponse>, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    let invoice_id = InvoiceId(uuid);
    let mut invoice = state
        .storage
        .get_invoice(&invoice_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Document not found: {}", id)))?;

    invoice.validation_status = fen_core::ValidationStatus::Failed;
    state.storage.store_invoice(&invoice).await?;

    Ok(Json(invoice_to_full_response(&invoice)))
}

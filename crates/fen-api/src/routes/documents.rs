use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use fen_core::domain::{Invoice, InvoiceId};
use fen_storage::DocumentStore;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Serialize)]
pub struct InvoiceResponse {
    pub id: String,
    pub document_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: Option<String>,
    pub total_amount: String,
    pub subtotal: String,
    pub tax_amount: String,
    pub currency: String,
    pub vendor_name: String,
    pub validation_status: String,
    pub confidence_score: f32,
    pub line_items_count: usize,
}

impl From<Invoice> for InvoiceResponse {
    fn from(inv: Invoice) -> Self {
        Self {
            id: inv.id.to_string(),
            document_id: inv.document_id.to_string(),
            invoice_number: inv.invoice_number,
            invoice_date: inv.invoice_date.to_string(),
            due_date: inv.due_date.map(|d| d.to_string()),
            total_amount: inv.total_amount.to_string(),
            subtotal: inv.subtotal.to_string(),
            tax_amount: inv.tax_amount.to_string(),
            currency: inv.currency.to_string(),
            vendor_name: inv.vendor.name,
            validation_status: inv.validation_status.to_string(),
            confidence_score: inv.confidence_score,
            line_items_count: inv.line_items.len(),
        }
    }
}

#[derive(Serialize)]
pub struct ListResponse {
    pub items: Vec<InvoiceResponse>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_invoices: usize,
    pub total_contracts: usize,
}

/// GET /documents - List all documents
pub async fn list_documents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListParams>,
) -> Result<Json<ListResponse>, ApiError> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let offset = params.offset.unwrap_or(0);

    let invoices = state.storage.list_invoices(limit, offset).await?;
    let total = state.storage.count_invoices().await?;

    let items: Vec<InvoiceResponse> = invoices.into_iter().map(Into::into).collect();

    Ok(Json(ListResponse {
        items,
        total,
        limit,
        offset,
    }))
}

/// GET /documents/:id - Get a specific document
pub async fn get_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<InvoiceResponse>, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    let invoice_id = InvoiceId(uuid);

    let invoice = state
        .storage
        .get_invoice(&invoice_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Document not found: {}", id)))?;

    Ok(Json(invoice.into()))
}

/// DELETE /documents/:id - Delete a document
pub async fn delete_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", id)))?;

    let invoice_id = InvoiceId(uuid);

    let deleted = state.storage.delete_invoice(&invoice_id).await?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Document not found: {}", id)))
    }
}

/// GET /stats - Get storage statistics
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatsResponse>, ApiError> {
    let total_invoices = state.storage.count_invoices().await?;
    let total_contracts = state.storage.count_contracts().await?;

    Ok(Json(StatsResponse {
        total_invoices,
        total_contracts,
    }))
}

use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use fen_core::domain::Invoice;

use crate::error::ApiError;
use crate::state::AppState;

/// Full invoice response matching frontend Invoice type
#[derive(Serialize)]
pub struct InvoiceFullResponse {
    pub id: String,
    pub document_id: String,
    pub tenant_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub due_date: Option<String>,
    pub po_number: Option<String>,
    pub contract_id: Option<String>,
    pub contract_number: Option<String>,
    pub vendor: PartyResponse,
    pub bill_to: PartyResponse,
    pub currency: String,
    pub line_items: Vec<LineItemResponse>,
    pub subtotal: String,
    pub tax_amount: String,
    pub discount_amount: String,
    pub total_amount: String,
    pub validation_status: String,
    pub confidence_score: f32,
    pub extracted_text: String,
    pub vendor_name: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct PartyResponse {
    pub id: String,
    pub name: String,
    pub tax_id: Option<String>,
    pub address: Option<AddressResponse>,
    pub contact: Option<ContactResponse>,
}

#[derive(Serialize)]
pub struct AddressResponse {
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Serialize)]
pub struct ContactResponse {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Serialize)]
pub struct LineItemResponse {
    pub line_number: u32,
    pub description: String,
    pub quantity: String,
    pub unit: Option<String>,
    pub unit_price: String,
    pub tax_rate: Option<String>,
    pub discount: Option<String>,
    pub total: String,
    pub item_code: Option<String>,
}

pub fn invoice_to_full_response(inv: &Invoice) -> InvoiceFullResponse {
    let now = chrono::Utc::now().to_rfc3339();
    InvoiceFullResponse {
        id: inv.id.to_string(),
        document_id: inv.document_id.to_string(),
        tenant_id: inv.tenant_id.to_string(),
        invoice_number: inv.invoice_number.clone(),
        invoice_date: inv.invoice_date.to_string(),
        due_date: inv.due_date.map(|d| d.to_string()),
        po_number: inv.po_number.clone(),
        contract_id: inv.contract_id.as_ref().map(|id| id.to_string()),
        contract_number: inv.contract_number.clone(),
        vendor: party_to_response(&inv.vendor),
        bill_to: party_to_response(&inv.bill_to),
        currency: inv.currency.to_string(),
        line_items: inv
            .line_items
            .iter()
            .map(|li| LineItemResponse {
                line_number: li.line_number,
                description: li.description.clone(),
                quantity: li.quantity.to_string(),
                unit: li.unit.clone(),
                unit_price: li.unit_price.to_string(),
                tax_rate: li.tax_rate.map(|r| r.to_string()),
                discount: li.discount.map(|d| d.to_string()),
                total: li.total.to_string(),
                item_code: li.item_code.clone(),
            })
            .collect(),
        subtotal: inv.subtotal.to_string(),
        tax_amount: inv.tax_amount.to_string(),
        discount_amount: inv.discount_amount.to_string(),
        total_amount: inv.total_amount.to_string(),
        validation_status: validation_status_to_string(&inv.validation_status),
        confidence_score: inv.confidence_score,
        extracted_text: inv.extracted_text.clone(),
        vendor_name: inv.vendor.name.clone(),
        status: map_validation_to_ui_status(&inv.validation_status),
        created_at: now.clone(),
        updated_at: now,
    }
}

fn party_to_response(party: &fen_core::domain::Party) -> PartyResponse {
    PartyResponse {
        id: party.id.to_string(),
        name: party.name.clone(),
        tax_id: party.tax_id.clone(),
        address: party.address.as_ref().map(|a| AddressResponse {
            street: a.street.clone(),
            city: a.city.clone(),
            state: a.state.clone(),
            postal_code: a.postal_code.clone(),
            country: a.country.clone(),
        }),
        contact: party.contact.as_ref().map(|c| ContactResponse {
            name: c.name.clone(),
            email: c.email.clone(),
            phone: c.phone.clone(),
        }),
    }
}

pub fn validation_status_to_string(vs: &fen_core::ValidationStatus) -> String {
    match vs {
        fen_core::ValidationStatus::Pending => "pending".to_string(),
        fen_core::ValidationStatus::Passed => "passed".to_string(),
        fen_core::ValidationStatus::PassedWithWarnings => "passed_with_warnings".to_string(),
        fen_core::ValidationStatus::Failed => "failed".to_string(),
    }
}

fn map_validation_to_ui_status(vs: &fen_core::ValidationStatus) -> String {
    match vs {
        fen_core::ValidationStatus::Pending => "pending".to_string(),
        fen_core::ValidationStatus::Passed => "validated".to_string(),
        fen_core::ValidationStatus::PassedWithWarnings => "flagged".to_string(),
        fen_core::ValidationStatus::Failed => "rejected".to_string(),
    }
}

pub fn contract_type_to_string(ct: &fen_core::domain::ContractType) -> String {
    match ct {
        fen_core::domain::ContractType::ServiceAgreement => "service_agreement".to_string(),
        fen_core::domain::ContractType::PurchaseOrder => "purchase_order".to_string(),
        fen_core::domain::ContractType::MasterServiceAgreement => "master_service_agreement".to_string(),
        fen_core::domain::ContractType::StatementOfWork => "statement_of_work".to_string(),
        fen_core::domain::ContractType::Amendment => "amendment".to_string(),
        fen_core::domain::ContractType::Other => "other".to_string(),
    }
}

/// POST /documents and POST /ingest - Upload and ingest a PDF document
pub async fn ingest_document(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    // Extract file from multipart form
    let mut file_data: Option<(String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let filename = field.file_name().unwrap_or("unknown.pdf").to_string();

            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to read file: {}", e)))?;

            file_data = Some((filename, data.to_vec()));
            break;
        }
    }

    let (filename, data) = file_data.ok_or_else(|| {
        ApiError::BadRequest("No file provided. Use form field 'file'.".to_string())
    })?;

    // Validate file type
    if !filename.to_lowercase().ends_with(".pdf") {
        return Err(ApiError::BadRequest(
            "Only PDF files are supported".to_string(),
        ));
    }

    // Check file size and magic bytes
    if data.is_empty() {
        return Err(ApiError::BadRequest("Empty file".to_string()));
    }
    if !data.starts_with(b"%PDF-") {
        return Err(ApiError::BadRequest(
            "Invalid PDF: file does not start with %PDF- header".to_string(),
        ));
    }

    tracing::info!(filename = %filename, size = data.len(), "Processing uploaded file");

    // Process through ingestion pipeline
    let result = state.ingestion.ingest_pdf(&data, &filename).await?;
    let invoice = result.invoice;
    let embedding = result.embedding;

    // Post-ingestion enrichment (all non-fatal)

    // 1. Fulltext index
    if let Some(ref index) = state.fulltext_index {
        if let Err(e) = index.index_invoice(&invoice).await {
            tracing::warn!(error = %e, "Failed to index invoice in fulltext");
        } else if let Err(e) = index.commit().await {
            tracing::warn!(error = %e, "Failed to commit fulltext index");
        }
    }

    // 2. Warm-tier embedding write
    if let (Some(ref warm), Some(ref emb)) = (&state.warm_storage, &embedding) {
        let mut warm = warm.write().await;
        if let Err(e) = warm.store_invoice_with_embedding(&invoice, Some(emb)).await {
            tracing::warn!(error = %e, "Failed to store invoice in warm tier");
        }
    }

    // 3. Publish DOCUMENT_PROCESSED event for async anomaly detection + notifications
    if let Some(ref event_bus) = state.event_bus {
        crate::workers::publish_document_processed(event_bus, &invoice).await;
    } else {
        // Fallback: synchronous anomaly detection when event bus is unavailable
        match state.rule_engine.validate_invoice(&invoice).await {
            Ok(validation_result) => {
                if !validation_result.anomalies.is_empty() {
                    tracing::info!(
                        invoice_id = %invoice.id,
                        anomaly_count = validation_result.anomalies.len(),
                        "Auto-validation detected anomalies"
                    );
                    crate::conversions::persist_anomalies(
                        &state.anomaly_store,
                        &validation_result.anomalies,
                        &invoice,
                    ).await;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Auto-validation failed");
            }
        }
    }

    let response = invoice_to_full_response(&invoice);

    Ok((StatusCode::CREATED, Json(response)))
}

/// POST /ingest/contract - Upload and ingest a contract PDF
pub async fn ingest_contract(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut file_data: Option<(String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            let filename = field.file_name().unwrap_or("unknown.pdf").to_string();

            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to read file: {}", e)))?;

            file_data = Some((filename, data.to_vec()));
            break;
        }
    }

    let (filename, data) = file_data.ok_or_else(|| {
        ApiError::BadRequest("No file provided. Use form field 'file'.".to_string())
    })?;

    if !filename.to_lowercase().ends_with(".pdf") {
        return Err(ApiError::BadRequest(
            "Only PDF files are supported".to_string(),
        ));
    }

    if data.is_empty() {
        return Err(ApiError::BadRequest("Empty file".to_string()));
    }
    if !data.starts_with(b"%PDF-") {
        return Err(ApiError::BadRequest(
            "Invalid PDF: file does not start with %PDF- header".to_string(),
        ));
    }

    tracing::info!(filename = %filename, size = data.len(), "Processing uploaded contract");

    let result = state.ingestion.ingest_contract_pdf(&data, &filename).await?;
    let contract = result.contract;
    let embedding = result.embedding;

    // Post-ingestion enrichment (all non-fatal)

    // 1. Fulltext index
    if let Some(ref index) = state.fulltext_index {
        if let Err(e) = index.index_contract(&contract).await {
            tracing::warn!(error = %e, "Failed to index contract in fulltext");
        } else if let Err(e) = index.commit().await {
            tracing::warn!(error = %e, "Failed to commit fulltext index");
        }
    }

    // 2. Log embedding availability (LanceDB has no contracts table yet)
    if embedding.is_some() {
        tracing::debug!(
            contract_id = %contract.id,
            "Contract embedding available but warm-tier contracts table not yet implemented"
        );
    }

    // 3. Publish DOCUMENT_PROCESSED event for downstream consumers
    if let Some(ref event_bus) = state.event_bus {
        crate::workers::publish_contract_processed(event_bus, &contract).await;
    }

    let response = crate::routes::documents::contract_to_response(&contract);

    Ok((StatusCode::CREATED, Json(response)))
}

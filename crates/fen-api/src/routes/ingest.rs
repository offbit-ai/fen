use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
pub struct IngestResponse {
    pub document_id: String,
    pub invoice_id: String,
    pub invoice_number: String,
    pub invoice_date: String,
    pub total_amount: String,
    pub vendor: String,
    pub status: String,
    pub confidence_score: f32,
}

/// POST /documents - Upload and ingest a PDF document
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

    // Check file size (basic validation)
    if data.is_empty() {
        return Err(ApiError::BadRequest("Empty file".to_string()));
    }

    tracing::info!(filename = %filename, size = data.len(), "Processing uploaded file");

    // Process through ingestion pipeline
    let invoice = state.ingestion.ingest_pdf(&data, &filename).await?;

    Ok((
        StatusCode::CREATED,
        Json(IngestResponse {
            document_id: invoice.document_id.to_string(),
            invoice_id: invoice.id.to_string(),
            invoice_number: invoice.invoice_number.clone(),
            invoice_date: invoice.invoice_date.to_string(),
            total_amount: invoice.total_amount.to_string(),
            vendor: invoice.vendor.name.clone(),
            status: "ingested".to_string(),
            confidence_score: invoice.confidence_score,
        }),
    ))
}

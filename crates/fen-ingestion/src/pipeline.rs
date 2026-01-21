use std::sync::Arc;

use fen_core::domain::{DocumentId, Invoice};
use fen_storage::DocumentStore;

use crate::error::IngestionError;
use crate::pdf::{ExtractedPdf, InvoiceParser, PdfExtractor};

/// Document ingestion pipeline
pub struct IngestionPipeline<S: DocumentStore> {
    invoice_parser: InvoiceParser,
    storage: Arc<S>,
}

impl<S: DocumentStore + Send + Sync + 'static> IngestionPipeline<S> {
    /// Create a new ingestion pipeline with the given storage
    pub fn new(storage: Arc<S>) -> Result<Self, IngestionError> {
        Ok(Self {
            invoice_parser: InvoiceParser::new(),
            storage,
        })
    }

    /// Ingest a PDF document from bytes
    pub async fn ingest_pdf(&self, bytes: &[u8], filename: &str) -> Result<Invoice, IngestionError> {
        // Generate document ID
        let document_id = DocumentId::new();

        tracing::info!(
            document_id = %document_id,
            filename = %filename,
            size = bytes.len(),
            "Starting PDF ingestion"
        );

        // Extract PDF in a blocking task since Pdfium isn't Send
        let bytes_owned = bytes.to_vec();
        let extracted: ExtractedPdf = tokio::task::spawn_blocking(move || {
            let pdf_extractor = PdfExtractor::new()?;
            pdf_extractor.extract_from_bytes(&bytes_owned)
        })
        .await
        .map_err(|e| IngestionError::Internal(format!("Task join error: {}", e)))??;

        tracing::info!(
            document_id = %document_id,
            page_count = %extracted.page_count,
            text_length = %extracted.text.len(),
            "PDF text extracted"
        );

        // Parse invoice from text
        let invoice = self.invoice_parser.parse(&extracted.text, document_id)?;

        tracing::info!(
            document_id = %document_id,
            invoice_id = %invoice.id,
            invoice_number = %invoice.invoice_number,
            total_amount = %invoice.total_amount,
            "Invoice parsed"
        );

        // Store in storage
        self.storage.store_invoice(&invoice).await?;

        tracing::info!(
            invoice_id = %invoice.id,
            "Invoice stored successfully"
        );

        Ok(invoice)
    }

    /// Ingest from raw text (for testing or non-PDF sources)
    pub async fn ingest_text(&self, text: &str) -> Result<Invoice, IngestionError> {
        let document_id = DocumentId::new();

        tracing::info!(
            document_id = %document_id,
            text_length = text.len(),
            "Starting text ingestion"
        );

        // Parse invoice from text
        let invoice = self.invoice_parser.parse(text, document_id)?;

        // Store
        self.storage.store_invoice(&invoice).await?;

        Ok(invoice)
    }

    /// Get the underlying storage
    pub fn storage(&self) -> &Arc<S> {
        &self.storage
    }
}

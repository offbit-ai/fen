use std::sync::Arc;

use fen_core::domain::{DocumentId, Invoice};
use fen_ml::DocumentIntelligence;
use fen_storage::DocumentStore;
use image::DynamicImage;

use crate::error::IngestionError;
use crate::pdf::{ExtractedPdf, InvoiceParser, MlInvoiceParser, PdfExtractor};

/// Configuration for the ingestion pipeline
#[derive(Clone)]
pub struct IngestionConfig {
    /// Whether to use ML-enhanced parsing
    pub use_ml: bool,
    /// Minimum text length to skip OCR (if text extraction succeeds)
    pub min_text_for_skip_ocr: usize,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            use_ml: false,
            min_text_for_skip_ocr: 100,
        }
    }
}

/// Document ingestion pipeline with optional ML enhancement
pub struct IngestionPipeline<S: DocumentStore> {
    config: IngestionConfig,
    invoice_parser: InvoiceParser,
    ml_parser: MlInvoiceParser,
    ml_pipeline: Option<Arc<DocumentIntelligence>>,
    storage: Arc<S>,
}

impl<S: DocumentStore + Send + Sync + 'static> IngestionPipeline<S> {
    /// Create a new ingestion pipeline with the given storage (no ML)
    pub fn new(storage: Arc<S>) -> Result<Self, IngestionError> {
        Self::with_config(storage, IngestionConfig::default(), None)
    }

    /// Create a new ingestion pipeline with ML capabilities
    pub fn with_ml(
        storage: Arc<S>,
        ml_pipeline: Arc<DocumentIntelligence>,
    ) -> Result<Self, IngestionError> {
        let config = IngestionConfig {
            use_ml: true,
            ..Default::default()
        };
        Self::with_config(storage, config, Some(ml_pipeline))
    }

    /// Create a new ingestion pipeline with custom configuration
    pub fn with_config(
        storage: Arc<S>,
        config: IngestionConfig,
        ml_pipeline: Option<Arc<DocumentIntelligence>>,
    ) -> Result<Self, IngestionError> {
        Ok(Self {
            config,
            invoice_parser: InvoiceParser::new(),
            ml_parser: MlInvoiceParser::new(),
            ml_pipeline,
            storage,
        })
    }

    /// Ingest a PDF document from bytes
    pub async fn ingest_pdf(&self, bytes: &[u8], filename: &str) -> Result<Invoice, IngestionError> {
        let document_id = DocumentId::new();

        tracing::info!(
            document_id = %document_id,
            filename = %filename,
            size = bytes.len(),
            use_ml = %self.config.use_ml,
            "Starting PDF ingestion"
        );

        // Extract PDF text and optionally render pages
        let bytes_owned = bytes.to_vec();
        let use_ml = self.config.use_ml && self.ml_pipeline.is_some();
        let min_text = self.config.min_text_for_skip_ocr;

        let (extracted, rendered_pages): (ExtractedPdf, Option<Vec<DynamicImage>>) =
            tokio::task::spawn_blocking(move || {
                let pdf_extractor = PdfExtractor::new()?;
                let extracted = pdf_extractor.extract_from_bytes(&bytes_owned)?;

                // Only render pages if ML is enabled and text extraction was poor
                let rendered = if use_ml && (!extracted.has_text || extracted.text.len() < min_text) {
                    let pages = pdf_extractor.render_pages_from_bytes(&bytes_owned)?;
                    Some(pages.into_iter().map(|p| p.image).collect())
                } else if use_ml {
                    // Still render first page for layout analysis even with good text
                    let pages = pdf_extractor.render_pages_from_bytes(&bytes_owned)?;
                    Some(pages.into_iter().take(1).map(|p| p.image).collect())
                } else {
                    None
                };

                Ok::<_, IngestionError>((extracted, rendered))
            })
            .await
            .map_err(|e| IngestionError::Internal(format!("Task join error: {}", e)))??;

        tracing::info!(
            document_id = %document_id,
            page_count = %extracted.page_count,
            text_length = %extracted.text.len(),
            has_text = %extracted.has_text,
            has_rendered_pages = %rendered_pages.is_some(),
            "PDF extracted"
        );

        // Parse invoice - use ML if available, otherwise regex
        let invoice = if let (Some(ml), Some(pages)) = (&self.ml_pipeline, rendered_pages) {
            self.parse_with_ml(ml, &pages, &extracted, document_id).await?
        } else {
            self.invoice_parser.parse(&extracted.text, document_id)?
        };

        tracing::info!(
            document_id = %document_id,
            invoice_id = %invoice.id,
            invoice_number = %invoice.invoice_number,
            total_amount = %invoice.total_amount,
            confidence = %invoice.confidence_score,
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

    /// Parse invoice using ML pipeline
    async fn parse_with_ml(
        &self,
        ml: &DocumentIntelligence,
        pages: &[DynamicImage],
        extracted: &ExtractedPdf,
        document_id: DocumentId,
    ) -> Result<Invoice, IngestionError> {
        // Process first page with ML (or all pages for multi-page documents)
        let image = pages.first().ok_or_else(|| {
            IngestionError::MlProcessing("No rendered pages available".to_string())
        })?;

        // Run ML document intelligence pipeline
        let processed = ml
            .process_image(image)
            .await
            .map_err(|e: fen_ml::error::MlError| IngestionError::MlProcessing(e.to_string()))?;

        // If ML didn't extract text (e.g., no OCR model), use PDF-extracted text
        let processed = if processed.text.is_empty() && !extracted.text.is_empty() {
            fen_ml::ProcessedDocument {
                text: extracted.text.clone(),
                ..processed
            }
        } else {
            processed
        };

        // Parse invoice from ML results
        self.ml_parser.parse(&processed, document_id)
    }

    /// Ingest from raw text (for testing or non-PDF sources)
    pub async fn ingest_text(&self, text: &str) -> Result<Invoice, IngestionError> {
        let document_id = DocumentId::new();

        tracing::info!(
            document_id = %document_id,
            text_length = text.len(),
            "Starting text ingestion"
        );

        // Parse invoice from text (regex-based for raw text)
        let invoice = self.invoice_parser.parse(text, document_id)?;

        // Store
        self.storage.store_invoice(&invoice).await?;

        Ok(invoice)
    }

    /// Get the underlying storage
    pub fn storage(&self) -> &Arc<S> {
        &self.storage
    }

    /// Check if ML is enabled
    pub fn has_ml(&self) -> bool {
        self.config.use_ml && self.ml_pipeline.is_some()
    }
}

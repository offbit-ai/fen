use std::sync::Arc;

use fen_core::domain::{Contract, DocumentId, Invoice};
#[cfg(feature = "graph")]
use fen_graph::GraphStore;
use fen_ml::DocumentIntelligence;
use fen_storage::DocumentStore;
use image::DynamicImage;

use crate::contract::{ContractParser, GlinerContractParser, MlContractParser};
use crate::error::IngestionError;
use crate::pdf::{ExtractedPdf, GlinerInvoiceParser, InvoiceParser, MlInvoiceParser, PdfExtractor};

/// Configuration for the ingestion pipeline
#[derive(Clone)]
pub struct IngestionConfig {
    /// Whether to use ML-enhanced parsing
    pub use_ml: bool,
    /// Enable GLiNER text-based extraction
    pub use_gliner: bool,
    /// Skip page rendering when GLiNER + text is available
    pub gliner_skip_rendering: bool,
    /// Minimum text length to skip OCR (if text extraction succeeds)
    pub min_text_for_skip_ocr: usize,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            use_ml: false,
            use_gliner: true,
            gliner_skip_rendering: true,
            min_text_for_skip_ocr: 100,
        }
    }
}

/// Document ingestion pipeline with optional ML enhancement
pub struct IngestionPipeline<S: DocumentStore> {
    config: IngestionConfig,
    invoice_parser: InvoiceParser,
    ml_parser: MlInvoiceParser,
    gliner_invoice_parser: GlinerInvoiceParser,
    contract_parser: ContractParser,
    ml_contract_parser: MlContractParser,
    gliner_contract_parser: GlinerContractParser,
    ml_pipeline: Option<Arc<DocumentIntelligence>>,
    #[cfg(feature = "graph")]
    graph: Option<Arc<dyn GraphStore>>,
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

    /// Create a new ingestion pipeline with ML and knowledge graph
    #[cfg(feature = "graph")]
    pub fn with_ml_and_graph(
        storage: Arc<S>,
        ml_pipeline: Arc<DocumentIntelligence>,
        graph: Arc<dyn GraphStore>,
    ) -> Result<Self, IngestionError> {
        let config = IngestionConfig {
            use_ml: true,
            ..Default::default()
        };
        Ok(Self {
            config,
            invoice_parser: InvoiceParser::new(),
            ml_parser: MlInvoiceParser::new(),
            gliner_invoice_parser: GlinerInvoiceParser::new(),
            contract_parser: ContractParser::new(),
            ml_contract_parser: MlContractParser::new(),
            gliner_contract_parser: GlinerContractParser::new(),
            ml_pipeline: Some(ml_pipeline),
            graph: Some(graph),
            storage,
        })
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
            gliner_invoice_parser: GlinerInvoiceParser::new(),
            contract_parser: ContractParser::new(),
            ml_contract_parser: MlContractParser::new(),
            gliner_contract_parser: GlinerContractParser::new(),
            ml_pipeline,
            #[cfg(feature = "graph")]
            graph: None,
            storage,
        })
    }

    /// Set the graph store (can be called after construction)
    #[cfg(feature = "graph")]
    pub fn with_graph(mut self, graph: Arc<dyn GraphStore>) -> Self {
        self.graph = Some(graph);
        self
    }

    /// Ingest a PDF document from bytes
    pub async fn ingest_pdf(
        &self,
        bytes: &[u8],
        filename: &str,
    ) -> Result<Invoice, IngestionError> {
        let document_id = DocumentId::new();
        let has_gliner = self.has_gliner();

        tracing::info!(
            document_id = %document_id,
            filename = %filename,
            size = bytes.len(),
            use_ml = %self.config.use_ml,
            use_gliner = %has_gliner,
            "Starting PDF ingestion"
        );

        // Extract PDF text and optionally render pages
        let bytes_owned = bytes.to_vec();
        let use_ml = self.config.use_ml && self.ml_pipeline.is_some();
        let min_text = self.config.min_text_for_skip_ocr;
        let skip_rendering = has_gliner && self.config.gliner_skip_rendering;

        let (extracted, rendered_pages): (ExtractedPdf, Option<Vec<DynamicImage>>) =
            tokio::task::spawn_blocking(move || {
                let pdf_extractor = PdfExtractor::new()?;
                let extracted = pdf_extractor.extract_from_bytes(&bytes_owned)?;

                // Skip rendering when GLiNER can handle text-based PDFs
                let needs_rendering = if skip_rendering
                    && extracted.has_text
                    && extracted.text.len() >= min_text
                {
                    false // GLiNER works on text — skip expensive rendering
                } else if use_ml && (!extracted.has_text || extracted.text.len() < min_text) {
                    true // Scanned PDF: must render for OCR
                } else if use_ml {
                    true // Still render first page for layout analysis
                } else {
                    false
                };

                let rendered = if needs_rendering {
                    let pages = pdf_extractor.render_pages_from_bytes(&bytes_owned)?;
                    if extracted.has_text && extracted.text.len() >= min_text {
                        // Good text: only render first page for layout
                        Some(pages.into_iter().take(1).map(|p| p.image).collect())
                    } else {
                        Some(pages.into_iter().map(|p| p.image).collect())
                    }
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

        // Parse invoice — tiered approach:
        // 1. GLiNER text extraction (fastest, no rendering needed)
        // 2. ML LayoutLMv3 path (for scanned docs or GLiNER unavailable)
        // 3. Regex fallback (last resort)
        let (invoice, embedding) = if has_gliner
            && extracted.has_text
            && extracted.text.len() >= min_text
        {
            self.parse_invoice_with_gliner(&extracted.text, document_id)?
        } else if let (Some(ml), Some(pages)) = (&self.ml_pipeline, &rendered_pages) {
            self.parse_with_ml(ml, pages, &extracted, document_id)
                .await?
        } else {
            (
                self.invoice_parser.parse(&extracted.text, document_id)?,
                None,
            )
        };

        tracing::info!(
            document_id = %document_id,
            invoice_id = %invoice.id,
            invoice_number = %invoice.invoice_number,
            total_amount = %invoice.total_amount,
            confidence = %invoice.confidence_score,
            has_embedding = %embedding.is_some(),
            "Invoice parsed"
        );

        // Write to knowledge graph (non-fatal)
        #[cfg(feature = "graph")]
        if let Some(graph) = &self.graph {
            if let Err(e) = graph.write_invoice(&invoice).await {
                tracing::warn!(
                    error = %e,
                    invoice_id = %invoice.id,
                    "Failed to write invoice to knowledge graph"
                );
            }
        }

        // Store in storage (with embedding for warm-tier indexing)
        self.storage
            .store_invoice_with_embedding(&invoice, embedding.as_deref())
            .await?;

        tracing::info!(
            invoice_id = %invoice.id,
            "Invoice stored successfully"
        );

        Ok(invoice)
    }

    /// Parse invoice using GLiNER tiered text extraction
    fn parse_invoice_with_gliner(
        &self,
        text: &str,
        document_id: DocumentId,
    ) -> Result<(Invoice, Option<Vec<f32>>), IngestionError> {
        let ml = self.ml_pipeline.as_ref().ok_or_else(|| {
            IngestionError::Internal("GLiNER requires ML pipeline".to_string())
        })?;

        let (result, confidence) = ml
            .extract_entities_from_text(
                text,
                crate::pdf::gliner_parser::INVOICE_LABELS,
                crate::pdf::gliner_parser::INVOICE_EXPECTED,
            )
            .map_err(|e| IngestionError::MlProcessing(e.to_string()))?;

        tracing::info!(
            tier = %result.tier,
            escalated = %result.escalated,
            entities = result.entities.len(),
            overall_confidence = %confidence.overall,
            completeness = %confidence.completeness,
            elapsed_ms = result.processing_time_ms,
            "GLiNER invoice extraction"
        );

        let invoice = self.gliner_invoice_parser.build(
            &result,
            document_id,
            text,
            confidence.overall,
        )?;

        Ok((invoice, None)) // GLiNER doesn't produce embeddings
    }

    /// Parse invoice using ML pipeline, returning the invoice and its document embedding.
    async fn parse_with_ml(
        &self,
        ml: &DocumentIntelligence,
        pages: &[DynamicImage],
        extracted: &ExtractedPdf,
        document_id: DocumentId,
    ) -> Result<(Invoice, Option<Vec<f32>>), IngestionError> {
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

        // Extract document-level embedding before parsing consumes the data
        let embedding = if processed.embeddings.document.is_empty() {
            None
        } else {
            Some(processed.embeddings.document.clone())
        };

        // Parse invoice from ML results
        let invoice = self.ml_parser.parse(&processed, document_id)?;
        Ok((invoice, embedding))
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

    /// Ingest a contract PDF document from bytes
    pub async fn ingest_contract_pdf(
        &self,
        bytes: &[u8],
        filename: &str,
    ) -> Result<Contract, IngestionError> {
        let document_id = DocumentId::new();
        let has_gliner = self.has_gliner();

        tracing::info!(
            document_id = %document_id,
            filename = %filename,
            size = bytes.len(),
            use_ml = %self.config.use_ml,
            use_gliner = %has_gliner,
            "Starting contract PDF ingestion"
        );

        // Extract PDF text and optionally render pages
        let bytes_owned = bytes.to_vec();
        let use_ml = self.config.use_ml && self.ml_pipeline.is_some();
        let min_text = self.config.min_text_for_skip_ocr;
        let skip_rendering = has_gliner && self.config.gliner_skip_rendering;

        let (extracted, rendered_pages): (ExtractedPdf, Option<Vec<DynamicImage>>) =
            tokio::task::spawn_blocking(move || {
                let pdf_extractor = PdfExtractor::new()?;
                let extracted = pdf_extractor.extract_from_bytes(&bytes_owned)?;

                let needs_rendering = if skip_rendering
                    && extracted.has_text
                    && extracted.text.len() >= min_text
                {
                    false
                } else if use_ml && (!extracted.has_text || extracted.text.len() < min_text) {
                    true
                } else if use_ml {
                    true
                } else {
                    false
                };

                let rendered = if needs_rendering {
                    let pages = pdf_extractor.render_pages_from_bytes(&bytes_owned)?;
                    if extracted.has_text && extracted.text.len() >= min_text {
                        Some(pages.into_iter().take(1).map(|p| p.image).collect())
                    } else {
                        Some(pages.into_iter().map(|p| p.image).collect())
                    }
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
            "Contract PDF extracted"
        );

        // Parse contract — tiered approach (same as invoice)
        let contract = if has_gliner
            && extracted.has_text
            && extracted.text.len() >= min_text
        {
            self.parse_contract_with_gliner(&extracted.text, document_id)?
        } else if let (Some(ml), Some(pages)) = (&self.ml_pipeline, &rendered_pages) {
            self.parse_contract_with_ml(ml, pages, &extracted, document_id)
                .await?
        } else {
            self.contract_parser.parse(&extracted.text, document_id)?
        };

        tracing::info!(
            document_id = %document_id,
            contract_id = %contract.id,
            contract_number = ?contract.contract_number,
            contract_type = ?contract.contract_type,
            confidence = %contract.confidence_score,
            "Contract parsed"
        );

        // Write to knowledge graph (non-fatal)
        #[cfg(feature = "graph")]
        if let Some(graph) = &self.graph {
            if let Err(e) = graph.write_contract(&contract).await {
                tracing::warn!(
                    error = %e,
                    contract_id = %contract.id,
                    "Failed to write contract to knowledge graph"
                );
            }
        }

        // Store in storage
        self.storage.store_contract(&contract).await?;

        tracing::info!(
            contract_id = %contract.id,
            "Contract stored successfully"
        );

        Ok(contract)
    }

    /// Parse contract using GLiNER tiered text extraction
    fn parse_contract_with_gliner(
        &self,
        text: &str,
        document_id: DocumentId,
    ) -> Result<Contract, IngestionError> {
        let ml = self.ml_pipeline.as_ref().ok_or_else(|| {
            IngestionError::Internal("GLiNER requires ML pipeline".to_string())
        })?;

        let (result, confidence) = ml
            .extract_entities_from_text(
                text,
                crate::contract::gliner_parser::CONTRACT_LABELS,
                crate::contract::gliner_parser::CONTRACT_EXPECTED,
            )
            .map_err(|e| IngestionError::MlProcessing(e.to_string()))?;

        tracing::info!(
            tier = %result.tier,
            escalated = %result.escalated,
            entities = result.entities.len(),
            overall_confidence = %confidence.overall,
            completeness = %confidence.completeness,
            elapsed_ms = result.processing_time_ms,
            "GLiNER contract extraction"
        );

        self.gliner_contract_parser.build(
            &result,
            document_id,
            text,
            confidence.overall,
        )
    }

    /// Parse contract using ML pipeline
    async fn parse_contract_with_ml(
        &self,
        ml: &DocumentIntelligence,
        pages: &[DynamicImage],
        extracted: &ExtractedPdf,
        document_id: DocumentId,
    ) -> Result<Contract, IngestionError> {
        let image = pages.first().ok_or_else(|| {
            IngestionError::MlProcessing("No rendered pages available".to_string())
        })?;

        // Run ML document intelligence pipeline (same generic pipeline)
        let processed = ml
            .process_image(image)
            .await
            .map_err(|e: fen_ml::error::MlError| IngestionError::MlProcessing(e.to_string()))?;

        // If ML didn't extract text, use PDF-extracted text
        let processed = if processed.text.is_empty() && !extracted.text.is_empty() {
            fen_ml::ProcessedDocument {
                text: extracted.text.clone(),
                ..processed
            }
        } else {
            processed
        };

        // Parse contract from ML results
        self.ml_contract_parser.parse(&processed, document_id)
    }

    /// Ingest contract from raw text (for testing or non-PDF sources)
    pub async fn ingest_contract_text(&self, text: &str) -> Result<Contract, IngestionError> {
        let document_id = DocumentId::new();

        tracing::info!(
            document_id = %document_id,
            text_length = text.len(),
            "Starting contract text ingestion"
        );

        let contract = self.contract_parser.parse(text, document_id)?;
        self.storage.store_contract(&contract).await?;

        Ok(contract)
    }

    /// Get the underlying storage
    pub fn storage(&self) -> &Arc<S> {
        &self.storage
    }

    /// Check if ML is enabled
    pub fn has_ml(&self) -> bool {
        self.config.use_ml && self.ml_pipeline.is_some()
    }

    /// Check if GLiNER text extraction is enabled and available
    pub fn has_gliner(&self) -> bool {
        self.config.use_gliner
            && self
                .ml_pipeline
                .as_ref()
                .is_some_and(|ml| ml.gliner.has_model())
    }

    /// Check if knowledge graph is enabled
    #[cfg(feature = "graph")]
    pub fn has_graph(&self) -> bool {
        self.graph.is_some()
    }
}

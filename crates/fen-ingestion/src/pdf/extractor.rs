use std::path::Path;

use pdfium_render::prelude::*;

use crate::error::IngestionError;

/// Extracted text and metadata from a PDF
#[derive(Debug, Clone)]
pub struct ExtractedPdf {
    pub text: String,
    pub page_count: usize,
    pub metadata: PdfMetadataInfo,
}

/// PDF metadata
#[derive(Debug, Clone, Default)]
pub struct PdfMetadataInfo {
    pub title: Option<String>,
    pub author: Option<String>,
    pub creation_date: Option<String>,
}

/// PDF text extractor using pdfium-render
pub struct PdfExtractor {
    pdfium: Pdfium,
}

impl PdfExtractor {
    /// Create a new PDF extractor
    pub fn new() -> Result<Self, IngestionError> {
        // Try to bind to system pdfium or use bundled
        let pdfium = Pdfium::default();
        Ok(Self { pdfium })
    }

    /// Extract text from a PDF file
    pub fn extract_from_file(&self, path: impl AsRef<Path>) -> Result<ExtractedPdf, IngestionError> {
        let document = self
            .pdfium
            .load_pdf_from_file(path.as_ref(), None)
            .map_err(|e| IngestionError::PdfLoad(e.to_string()))?;

        self.extract_from_document(&document)
    }

    /// Extract text from PDF bytes
    pub fn extract_from_bytes(&self, bytes: &[u8]) -> Result<ExtractedPdf, IngestionError> {
        let document = self
            .pdfium
            .load_pdf_from_byte_slice(bytes, None)
            .map_err(|e| IngestionError::PdfLoad(e.to_string()))?;

        self.extract_from_document(&document)
    }

    fn extract_from_document(&self, document: &PdfDocument) -> Result<ExtractedPdf, IngestionError> {
        let mut full_text = String::new();
        let page_count = document.pages().len() as usize;

        // Extract text from each page
        for (i, page) in document.pages().iter().enumerate() {
            match page.text() {
                Ok(page_text) => {
                    // Get all text from the page - returns String directly
                    let text = page_text.all();
                    full_text.push_str(&text);
                    full_text.push('\n');
                }
                Err(e) => {
                    tracing::warn!(page = i, error = %e, "Failed to get text object from page");
                }
            }
        }

        // Extract metadata using the tag-based API
        let meta = document.metadata();
        let metadata = PdfMetadataInfo {
            title: meta.get(PdfDocumentMetadataTagType::Title).map(|t| t.value().to_string()),
            author: meta.get(PdfDocumentMetadataTagType::Author).map(|t| t.value().to_string()),
            creation_date: meta.get(PdfDocumentMetadataTagType::CreationDate).map(|t| t.value().to_string()),
        };

        Ok(ExtractedPdf {
            text: full_text.trim().to_string(),
            page_count,
            metadata,
        })
    }
}

impl Default for PdfExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Pdfium")
    }
}

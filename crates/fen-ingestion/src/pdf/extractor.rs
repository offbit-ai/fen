use std::path::Path;

use image::{DynamicImage, RgbaImage};
use pdfium_render::prelude::*;

use crate::error::IngestionError;

/// Extracted text and metadata from a PDF
#[derive(Debug, Clone)]
pub struct ExtractedPdf {
    pub text: String,
    pub page_count: usize,
    pub metadata: PdfMetadataInfo,
    /// Whether text extraction produced meaningful content
    pub has_text: bool,
}

/// PDF page rendered as image for ML processing
pub struct RenderedPage {
    pub image: DynamicImage,
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
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

        let trimmed_text = full_text.trim().to_string();
        let has_text = !trimmed_text.is_empty() && trimmed_text.len() > 50;

        Ok(ExtractedPdf {
            text: trimmed_text,
            page_count,
            metadata,
            has_text,
        })
    }

    /// Render PDF pages as images for ML processing
    pub fn render_pages_from_bytes(&self, bytes: &[u8]) -> Result<Vec<RenderedPage>, IngestionError> {
        let document = self
            .pdfium
            .load_pdf_from_byte_slice(bytes, None)
            .map_err(|e| IngestionError::PdfLoad(e.to_string()))?;

        self.render_pages(&document)
    }

    fn render_pages(&self, document: &PdfDocument) -> Result<Vec<RenderedPage>, IngestionError> {
        let mut pages = Vec::new();
        let render_config = PdfRenderConfig::new()
            .set_target_width(2048)
            .set_maximum_height(2048)
            .rotate_if_landscape(PdfPageRenderRotation::None, false);

        for (i, page) in document.pages().iter().enumerate() {
            let width = page.width().value as u32;
            let height = page.height().value as u32;

            // Render page to bitmap
            let bitmap = page
                .render_with_config(&render_config)
                .map_err(|e| IngestionError::PdfRender(format!("Page {}: {}", i, e)))?;

            // Convert to DynamicImage
            let image = bitmap_to_image(&bitmap)?;

            pages.push(RenderedPage {
                image,
                page_index: i,
                width,
                height,
            });
        }

        Ok(pages)
    }
}

/// Convert pdfium bitmap to DynamicImage
fn bitmap_to_image(bitmap: &PdfBitmap) -> Result<DynamicImage, IngestionError> {
    let width = bitmap.width() as u32;
    let height = bitmap.height() as u32;

    // Get raw RGBA bytes from the bitmap
    let buffer = bitmap.as_raw_bytes();

    // Create image from raw bytes
    let img = RgbaImage::from_raw(width, height, buffer.to_vec())
        .ok_or_else(|| IngestionError::PdfRender("Failed to create image from bitmap".to_string()))?;

    Ok(DynamicImage::ImageRgba8(img))
}

impl Default for PdfExtractor {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Pdfium")
    }
}

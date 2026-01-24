//! Fen ML - Machine Learning components for document intelligence
//!
//! This crate provides ML-based document processing capabilities:
//! - OCR (Optical Character Recognition) for scanned documents
//! - Layout understanding using LayoutLMv3-style models
//! - Table extraction using TATR-style detection
//! - Document embeddings for semantic search

pub mod embedding;
pub mod error;
pub mod layout;
pub mod ocr;
pub mod table;

pub use embedding::{DocumentEmbeddings, EmbeddingModel, EmbeddingModelConfig};
pub use error::MlError;
pub use layout::{
    LayoutLabel, LayoutModel, LayoutModelConfig, LayoutRegion, LayoutResult, NamedEntity,
};
pub use ocr::{BoundingBox, OcrConfig, OcrEngine, OcrProvider, OcrResult, TextRegion};
pub use table::{ExtractedTable, TableCell, TableExtractor, TableExtractorConfig};

use std::path::Path;

/// Configuration for the complete document intelligence pipeline
#[derive(Debug, Clone, Default)]
pub struct DocumentIntelligenceConfig {
    pub ocr: OcrConfig,
    pub layout: LayoutModelConfig,
    pub table: TableExtractorConfig,
    pub embedding: EmbeddingModelConfig,
}

/// Complete document intelligence pipeline
pub struct DocumentIntelligence {
    pub ocr: OcrEngine,
    pub layout: LayoutModel,
    pub table: TableExtractor,
    pub embedding: EmbeddingModel,
}

impl DocumentIntelligence {
    /// Create pipeline without ML models (rule-based fallback)
    pub fn new(config: DocumentIntelligenceConfig) -> Result<Self, MlError> {
        Ok(Self {
            ocr: OcrEngine::new(config.ocr)?,
            layout: LayoutModel::new(config.layout)?,
            table: TableExtractor::new(config.table)?,
            embedding: EmbeddingModel::new(config.embedding)?,
        })
    }

    /// Create pipeline with ONNX models
    pub fn with_models(
        config: DocumentIntelligenceConfig,
        models_dir: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let models_dir = models_dir.as_ref();

        let ocr = if let (Some(det), Some(rec)) = (
            &config.ocr.detection_model_path,
            &config.ocr.recognition_model_path,
        ) {
            OcrEngine::with_models(
                config.ocr.clone(),
                models_dir.join(det),
                models_dir.join(rec),
            )?
        } else {
            OcrEngine::new(config.ocr)?
        };

        let layout = if let Some(model_path) = &config.layout.model_path {
            let tokenizer_path = models_dir.join("layout_tokenizer.json");
            LayoutModel::with_model(
                config.layout.clone(),
                models_dir.join(model_path),
                tokenizer_path,
            )?
        } else {
            LayoutModel::new(config.layout)?
        };

        let table = if let (Some(det), Some(struct_)) = (
            &config.table.detection_model_path,
            &config.table.structure_model_path,
        ) {
            TableExtractor::with_models(
                config.table.clone(),
                models_dir.join(det),
                models_dir.join(struct_),
            )?
        } else {
            TableExtractor::new(config.table)?
        };

        let embedding = if let Some(model_path) = &config.embedding.model_path {
            let tokenizer_path = models_dir.join("tokenizer.json");
            EmbeddingModel::with_model(
                config.embedding.clone(),
                models_dir.join(model_path),
                tokenizer_path,
            )?
        } else {
            EmbeddingModel::new(config.embedding)?
        };

        Ok(Self {
            ocr,
            layout,
            table,
            embedding,
        })
    }

    /// Process a document image
    pub async fn process_image(
        &self,
        image: &image::DynamicImage,
    ) -> Result<ProcessedDocument, MlError> {
        // Step 1: OCR
        let ocr_result = self.ocr.process_image(image).await?;

        // Step 2: Layout analysis
        let layout_result = self.layout.analyze(image, &ocr_result)?;

        // Step 3: Table extraction
        let table_result = self.table.extract(image, &ocr_result)?;

        // Step 4: Generate embeddings
        let sections: Vec<_> = layout_result
            .regions
            .iter()
            .enumerate()
            .map(|(i, r)| {
                (
                    format!("section_{}", i),
                    format!("{:?}", r.label),
                    r.text.clone(),
                )
            })
            .collect();

        let entities: Vec<_> = layout_result
            .entities
            .iter()
            .map(|e| (format!("{:?}", e.entity_type), e.value.clone()))
            .collect();

        let embeddings =
            self.embedding
                .embed_document(&layout_result.text, &sections, &entities)?;

        let text = layout_result.text.clone();
        Ok(ProcessedDocument {
            text,
            ocr_result,
            layout_result,
            tables: table_result.tables,
            embeddings,
        })
    }
}

/// Result of processing a document through the intelligence pipeline
#[derive(Debug, Clone)]
pub struct ProcessedDocument {
    /// Full text content
    pub text: String,

    /// OCR result with text regions
    pub ocr_result: OcrResult,

    /// Layout analysis result
    pub layout_result: LayoutResult,

    /// Extracted tables
    pub tables: Vec<ExtractedTable>,

    /// Document embeddings
    pub embeddings: DocumentEmbeddings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_intelligence_creation() {
        let config = DocumentIntelligenceConfig::default();
        let di = DocumentIntelligence::new(config).unwrap();

        assert!(!di.ocr.has_models());
        assert!(!di.layout.has_model());
        assert!(!di.table.has_models());
        assert!(!di.embedding.has_model());
    }
}

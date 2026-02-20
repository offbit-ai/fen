//! Fen ML - Machine Learning components for document intelligence
//!
//! This crate provides ML-based document processing capabilities:
//! - OCR (Optical Character Recognition) for scanned documents
//! - Layout understanding using LayoutLMv3-style models
//! - Table extraction using TATR-style detection
//! - Document embeddings for semantic search

pub mod embedding;
pub mod error;
pub mod gliner;
pub mod layout;
pub mod ocr;
pub mod table;

pub use embedding::{DocumentEmbeddings, EmbeddingModel, EmbeddingModelConfig};
pub use error::MlError;
pub use gliner::{
    ExtractionConfidence, GlinerConfig, GlinerExtractor, GlinerExtractionResult, GlinerModelConfig,
};
pub use layout::{
    LayoutLabel, LayoutModel, LayoutModelConfig, LayoutRegion, LayoutResult, NamedEntity,
};
pub use ocr::{
    BoundingBox, OcrConfig, OcrEngine, OcrProvider, OcrResult, PreprocessingConfig, TextRegion,
};
pub use table::{ExtractedTable, TableCell, TableExtractor, TableExtractorConfig};

use std::path::Path;

/// Load a PaddleOCR character dictionary from a file.
/// Format: one character per line. Index 0 = blank/CTC, 1..N = chars, N+1 = EOS.
fn load_vocabulary_from_file(path: &Path) -> Result<Vec<char>, MlError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| MlError::Configuration(format!("Failed to read vocabulary {}: {}", path.display(), e)))?;
    Ok(content.lines().filter_map(|line| line.chars().next()).collect())
}

/// Configuration for the complete document intelligence pipeline
#[derive(Debug, Clone, Default)]
pub struct DocumentIntelligenceConfig {
    pub ocr: OcrConfig,
    pub layout: LayoutModelConfig,
    pub table: TableExtractorConfig,
    pub embedding: EmbeddingModelConfig,
    pub gliner: GlinerConfig,
}

/// Complete document intelligence pipeline
pub struct DocumentIntelligence {
    pub ocr: OcrEngine,
    pub layout: LayoutModel,
    pub table: TableExtractor,
    pub embedding: EmbeddingModel,
    pub gliner: GlinerExtractor,
}

impl DocumentIntelligence {
    /// Create pipeline without ML models (rule-based fallback)
    pub fn new(config: DocumentIntelligenceConfig) -> Result<Self, MlError> {
        Ok(Self {
            ocr: OcrEngine::new(config.ocr)?,
            layout: LayoutModel::new(config.layout)?,
            table: TableExtractor::new(config.table)?,
            embedding: EmbeddingModel::new(config.embedding)?,
            gliner: GlinerExtractor::new(config.gliner)?,
        })
    }

    /// Create pipeline with ONNX models
    pub fn with_models(
        config: DocumentIntelligenceConfig,
        models_dir: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let models_dir = models_dir.as_ref();

        let ocr = if !config.ocr.language_model_paths.is_empty() {
            // Multi-language mode: load per-language recognition models
            let det_path = models_dir.join(
                config.ocr.detection_model_path.as_deref().unwrap_or("ocr_detection.onnx"),
            );

            let mut lang_models = Vec::new();
            for (lang, paths) in &config.ocr.language_model_paths {
                let model_path = models_dir.join(&paths.model_path);
                let dict_path = models_dir.join(&paths.dict_path);
                let vocabulary = load_vocabulary_from_file(&dict_path)?;
                lang_models.push((*lang, model_path, vocabulary));
            }

            OcrEngine::with_multilang_models(config.ocr.clone(), det_path, lang_models)?
        } else if let (Some(det), Some(rec)) = (
            &config.ocr.detection_model_path,
            &config.ocr.recognition_model_path,
        ) {
            // Single-model mode (backward compatible)
            let mut ocr_config = config.ocr.clone();

            // Load vocabulary from file if configured
            if let Some(vocab_path) = &ocr_config.vocabulary_path {
                let full_path = models_dir.join(vocab_path);
                if full_path.exists() {
                    let vocab = load_vocabulary_from_file(&full_path)?;
                    tracing::info!(
                        vocabulary_path = %full_path.display(),
                        vocabulary_size = vocab.len(),
                        "Loaded OCR vocabulary from file"
                    );
                    ocr_config.decoder_config.vocabulary = Some(vocab);
                }
            }

            OcrEngine::with_models(
                ocr_config,
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

        let gliner = GlinerExtractor::with_models(config.gliner, models_dir)?;

        Ok(Self {
            ocr,
            layout,
            table,
            embedding,
            gliner,
        })
    }

    /// Extract entities from text using GLiNER (no image rendering needed)
    pub fn extract_entities_from_text(
        &self,
        text: &str,
        labels: &[&str],
        expected_labels: &[&str],
    ) -> Result<(GlinerExtractionResult, ExtractionConfidence), MlError> {
        self.gliner.extract(text, labels, expected_labels)
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
        assert!(!di.gliner.has_model());
    }
}

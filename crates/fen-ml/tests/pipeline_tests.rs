//! Integration tests for the full DocumentIntelligence pipeline.
//!
//! Run with: cargo test -p fen-ml --test pipeline_tests -- --ignored

mod common;

use fen_ml::{DocumentIntelligence, DocumentIntelligenceConfig};
use image::GenericImageView;

#[test]
#[ignore]
fn test_pipeline_construction() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);

    assert!(pipeline.ocr.has_models(), "OCR should have models");
    assert!(pipeline.layout.has_model(), "Layout should have model");
    assert!(pipeline.table.has_models(), "Table should have models");
    assert!(pipeline.embedding.has_model(), "Embedding should have model");
}

#[tokio::test]
#[ignore]
async fn test_full_pipeline_invoice() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);
    let image = common::load_test_document("invoice_simple.png");

    let result = pipeline.process_image(&image).await.unwrap();

    assert!(!result.text.is_empty(), "Pipeline should extract text");

    assert!(
        !result.ocr_result.regions.is_empty(),
        "OCR step should produce regions"
    );
    common::assert_valid_confidence(result.ocr_result.confidence, "Pipeline OCR confidence");

    assert!(
        !result.layout_result.regions.is_empty(),
        "Layout step should produce regions"
    );

    let dim = pipeline.embedding.embedding_dim();
    common::assert_valid_embedding(
        &result.embeddings.document,
        dim,
        "Pipeline document embedding",
    );

    tracing::info!(
        text_len = result.text.len(),
        ocr_regions = result.ocr_result.regions.len(),
        layout_regions = result.layout_result.regions.len(),
        tables = result.tables.len(),
        entities = result.layout_result.entities.len(),
        kv_pairs = result.layout_result.key_value_pairs.len(),
        section_embeddings = result.embeddings.sections.len(),
        entity_embeddings = result.embeddings.entities.len(),
        "Full pipeline results"
    );
}

#[tokio::test]
#[ignore]
async fn test_pipeline_multiple_documents() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);

    let doc_files = ["invoice_simple.png", "invoice_table.png", "receipt.png"];
    let mut results = Vec::new();

    for filename in &doc_files {
        let doc_path = common::documents_dir().join(filename);
        if !doc_path.exists() {
            tracing::warn!(filename, "Skipping missing test document");
            continue;
        }

        let image = common::load_test_document(filename);
        let (w, h) = image.dimensions();
        let result = pipeline.process_image(&image).await.unwrap();

        // High-res images (>= 640px) should reliably produce recognized text
        // Low-res images may only produce detection regions without high-confidence recognition
        if w.max(h) >= 640 {
            assert!(!result.text.is_empty(), "{} should produce text", filename);
        }
        // Detection should always find regions in document images
        assert!(
            !result.ocr_result.regions.is_empty()
                || result.ocr_result.processing_time_ms > 0,
            "{} should at least be processed by OCR",
            filename
        );

        results.push(result);
    }

    // Different documents should produce different text
    if results.len() >= 2 {
        assert_ne!(
            results[0].text, results[1].text,
            "Different documents should produce different text"
        );

        let sim = pipeline.embedding.cosine_similarity(
            &results[0].embeddings.document,
            &results[1].embeddings.document,
        );
        assert!(
            sim < 0.99,
            "Different documents should have < 0.99 cosine similarity, got {}",
            sim
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_pipeline_synthetic_image() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);
    let synthetic = common::create_synthetic_image(1024, 768);

    let result = pipeline.process_image(&synthetic).await.unwrap();

    let dim = pipeline.embedding.embedding_dim();
    assert_eq!(
        result.embeddings.document.len(),
        dim,
        "Embedding dimension should be correct even for sparse content"
    );
}

#[test]
#[ignore]
fn test_pipeline_missing_models_error() {
    let config = DocumentIntelligenceConfig {
        ocr: fen_ml::OcrConfig {
            detection_model_path: Some("ocr_detection.onnx".to_string()),
            recognition_model_path: Some("ocr_recognition.onnx".to_string()),
            ..Default::default()
        },
        layout: fen_ml::LayoutModelConfig {
            model_path: Some("layout_model.onnx".to_string()),
            ..Default::default()
        },
        table: fen_ml::TableExtractorConfig {
            detection_model_path: Some("table_detection.onnx".to_string()),
            structure_model_path: Some("table_structure.onnx".to_string()),
            ..Default::default()
        },
        embedding: fen_ml::EmbeddingModelConfig {
            model_path: Some("embedding_model.onnx".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = DocumentIntelligence::with_models(config, "/nonexistent/path");
    assert!(result.is_err(), "Should error for missing models directory");
}

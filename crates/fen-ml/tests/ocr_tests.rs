//! Integration tests for OcrEngine with real ONNX models.
//!
//! Run with: cargo test -p fen-ml --test ocr_tests -- --ignored

mod common;

use fen_ml::OcrProvider;

#[test]
#[ignore]
fn test_ocr_model_loading() {
    common::init_test_tracing();
    let models = common::require_models();
    let engine = common::build_ocr_engine(&models);
    assert!(
        engine.has_models(),
        "OCR engine should report models loaded"
    );
}

#[tokio::test]
#[ignore]
async fn test_ocr_invoice_produces_text() {
    common::init_test_tracing();
    let models = common::require_models();
    let engine = common::build_ocr_engine(&models);
    let image = common::load_test_document("invoice_simple.png");

    let result = engine.process_image(&image).await.unwrap();

    assert!(
        !result.text.is_empty(),
        "OCR should extract text from invoice"
    );
    assert!(!result.regions.is_empty(), "OCR should detect text regions");
    assert!(
        result.processing_time_ms > 0,
        "Processing time should be recorded"
    );
    common::assert_valid_confidence(result.confidence, "OCR overall");

    for (i, region) in result.regions.iter().enumerate() {
        assert!(
            !region.text.is_empty(),
            "Region {} text should not be empty",
            i
        );
        common::assert_valid_confidence(region.confidence, &format!("Region {}", i));
        common::assert_valid_bbox(&region.bbox, &format!("Region {} bbox", i));
    }

    // Regions should be in reading order
    for i in 1..result.regions.len() {
        assert!(
            result.regions[i].order >= result.regions[i - 1].order,
            "Regions should be in reading order"
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_ocr_process_bytes() {
    common::init_test_tracing();
    let models = common::require_models();
    let engine = common::build_ocr_engine(&models);

    let path = common::documents_dir().join("invoice_simple.png");
    let bytes = std::fs::read(&path).expect("Failed to read test image bytes");

    let result = engine.process_bytes(&bytes).await.unwrap();
    assert!(!result.text.is_empty(), "process_bytes should produce text");
}

#[tokio::test]
#[ignore]
async fn test_ocr_blank_image_returns_empty() {
    common::init_test_tracing();
    let models = common::require_models();
    let engine = common::build_ocr_engine(&models);
    let blank = common::create_synthetic_image(800, 600);

    let result = engine.process_image(&blank).await.unwrap();

    assert!(
        result.regions.len() < 5,
        "Blank image should have very few detected regions, got {}",
        result.regions.len()
    );
}

#[tokio::test]
#[ignore]
async fn test_ocr_large_image_handling() {
    common::init_test_tracing();
    let models = common::require_models();
    let engine = common::build_ocr_engine(&models);

    let large = common::create_synthetic_image(4000, 3000);

    // Should not error -- engine resizes internally
    let result = engine.process_image(&large).await.unwrap();
    assert!(result.processing_time_ms > 0);
}

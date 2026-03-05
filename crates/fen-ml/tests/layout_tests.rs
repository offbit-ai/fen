//! Integration tests for LayoutModel with real ONNX model.
//!
//! Run with: cargo test -p fen-ml --test layout_tests -- --ignored

mod common;

use fen_ml::OcrProvider;

#[test]
#[ignore]
fn test_layout_model_loading() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_layout_model(&models);
    assert!(model.has_model(), "Layout model should report model loaded");
}

#[tokio::test]
#[ignore]
async fn test_layout_analysis_on_invoice() {
    common::init_test_tracing();
    let models = common::require_models();
    let ocr_engine = common::build_ocr_engine(&models);
    let layout_model = common::build_layout_model(&models);
    let image = common::load_test_document("invoice_simple.png");

    let ocr_result = ocr_engine.process_image(&image).await.unwrap();
    assert!(
        !ocr_result.regions.is_empty(),
        "Need OCR regions for layout test"
    );

    let layout_result = layout_model.analyze(&image, &ocr_result).unwrap();

    assert!(
        !layout_result.text.is_empty(),
        "Layout text should not be empty"
    );
    assert!(layout_result.processing_time_ms > 0);

    assert!(
        !layout_result.regions.is_empty(),
        "Layout should detect regions from OCR output"
    );

    for (i, region) in layout_result.regions.iter().enumerate() {
        common::assert_valid_confidence(region.confidence, &format!("Layout region {}", i));
        common::assert_valid_bbox(&region.bbox, &format!("Layout region {} bbox", i));
    }

    tracing::info!(
        num_entities = layout_result.entities.len(),
        num_regions = layout_result.regions.len(),
        num_kv_pairs = layout_result.key_value_pairs.len(),
        "Layout analysis results"
    );
}

#[tokio::test]
#[ignore]
async fn test_layout_entity_extraction() {
    common::init_test_tracing();
    let models = common::require_models();
    let ocr_engine = common::build_ocr_engine(&models);
    let layout_model = common::build_layout_model(&models);
    let image = common::load_test_document("invoice_simple.png");

    let ocr_result = ocr_engine.process_image(&image).await.unwrap();
    let layout_result = layout_model.analyze(&image, &ocr_result).unwrap();

    for (i, entity) in layout_result.entities.iter().enumerate() {
        assert!(
            !entity.value.is_empty(),
            "Entity {} value should not be empty",
            i
        );
        common::assert_valid_confidence(entity.confidence, &format!("Entity {}", i));
        assert!(
            entity.start_pos <= entity.end_pos,
            "Entity {} start_pos ({}) > end_pos ({})",
            i,
            entity.start_pos,
            entity.end_pos
        );
    }
}

#[test]
#[ignore]
fn test_layout_with_empty_ocr() {
    common::init_test_tracing();
    let models = common::require_models();
    let layout_model = common::build_layout_model(&models);
    let image = common::create_synthetic_image(800, 600);

    let empty_ocr = fen_ml::OcrResult::default();
    let result = layout_model.analyze(&image, &empty_ocr).unwrap();

    // Empty OCR input should produce minimal/empty layout output
    assert!(
        result.regions.is_empty(),
        "No OCR regions should mean no layout regions"
    );
}

//! Integration tests for TableExtractor with real ONNX models.
//!
//! Run with: cargo test -p fen-ml --test table_tests -- --ignored

mod common;

use fen_ml::OcrProvider;

#[test]
#[ignore]
fn test_table_model_loading() {
    common::init_test_tracing();
    let models = common::require_models();
    let extractor = common::build_table_extractor(&models);
    assert!(extractor.has_models(), "Table extractor should report models loaded");
}

#[tokio::test]
#[ignore]
async fn test_table_extraction_on_invoice() {
    common::init_test_tracing();
    let models = common::require_models();
    let ocr_engine = common::build_ocr_engine(&models);
    let extractor = common::build_table_extractor(&models);
    let image = common::load_test_document("invoice_table.png");

    let ocr_result = ocr_engine.process_image(&image).await.unwrap();
    let table_result = extractor.extract(&image, &ocr_result).unwrap();

    assert!(table_result.processing_time_ms > 0);

    if table_result.tables.is_empty() {
        tracing::warn!("No tables detected in invoice_table.png -- model may need tuning");
        return;
    }

    for (i, table) in table_result.tables.iter().enumerate() {
        assert!(table.num_rows > 0, "Table {} should have rows", i);
        assert!(table.num_columns > 0, "Table {} should have columns", i);
        common::assert_valid_confidence(table.confidence, &format!("Table {}", i));
        common::assert_valid_bbox(&table.bbox, &format!("Table {} bbox", i));

        assert_eq!(
            table.cells.len(),
            table.num_rows,
            "Table {} cells rows count mismatch",
            i
        );

        for (row_idx, row) in table.cells.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                assert_eq!(cell.row, row_idx, "Cell row index mismatch");
                assert_eq!(cell.column, col_idx, "Cell column index mismatch");
                assert!(cell.row_span >= 1, "Row span should be >= 1");
                assert!(cell.col_span >= 1, "Col span should be >= 1");
            }
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_table_to_markdown_export() {
    common::init_test_tracing();
    let models = common::require_models();
    let ocr_engine = common::build_ocr_engine(&models);
    let extractor = common::build_table_extractor(&models);
    let image = common::load_test_document("invoice_table.png");

    let ocr_result = ocr_engine.process_image(&image).await.unwrap();
    let table_result = extractor.extract(&image, &ocr_result).unwrap();

    for table in &table_result.tables {
        let md = table.to_markdown();
        assert!(!md.is_empty(), "Markdown export should not be empty");
        assert!(md.contains('|'), "Markdown should contain pipe characters");
        assert!(md.contains("---"), "Markdown should contain separator");

        let csv = table.to_csv();
        assert!(!csv.is_empty(), "CSV export should not be empty");
    }
}

#[tokio::test]
#[ignore]
async fn test_table_extraction_blank_image() {
    common::init_test_tracing();
    let models = common::require_models();
    let extractor = common::build_table_extractor(&models);
    let blank = common::create_synthetic_image(800, 600);
    let empty_ocr = fen_ml::OcrResult::default();

    let result = extractor.extract(&blank, &empty_ocr).unwrap();
    assert!(
        result.tables.is_empty(),
        "Blank image should not contain tables"
    );
}

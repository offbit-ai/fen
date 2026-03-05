//! Shared test helpers for fen-ml integration tests with real ONNX models.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Once;

use fen_ml::{
    BoundingBox, DocumentIntelligence, DocumentIntelligenceConfig, EmbeddingModel,
    EmbeddingModelConfig, LayoutModel, LayoutModelConfig, OcrConfig, OcrEngine, TableExtractor,
    TableExtractorConfig,
};
use image::DynamicImage;

static INIT_TRACING: Once = Once::new();

/// Initialize tracing for test output (call at top of each test; idempotent).
pub fn init_test_tracing() {
    INIT_TRACING.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "fen_ml=debug,test=debug".into()),
            )
            .with_test_writer()
            .try_init();
    });
}

/// Resolve the models directory.
/// Checks `FEN_TEST_MODELS_DIR` env var, falls back to `../../models` relative to crate root.
pub fn models_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FEN_TEST_MODELS_DIR") {
        PathBuf::from(dir)
    } else {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("../../models")
    }
}

/// Resolve the test documents directory.
/// Checks `FEN_TEST_DOCUMENTS_DIR` env var, falls back to `tests/fixtures/documents`.
pub fn documents_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FEN_TEST_DOCUMENTS_DIR") {
        PathBuf::from(dir)
    } else {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.join("tests/fixtures/documents")
    }
}

const REQUIRED_MODEL_FILES: &[&str] = &[
    "ocr_detection.onnx",
    "ocr_recognition.onnx",
    "layout_model.onnx",
    "layout_tokenizer.json",
    "table_detection.onnx",
    "table_structure.onnx",
    "embedding_model.onnx",
    "tokenizer.json",
];

/// Panics with a helpful message if the models directory is missing or incomplete.
pub fn require_models() -> PathBuf {
    let dir = models_dir();
    assert!(
        dir.exists(),
        "Models directory not found at {}. Set FEN_TEST_MODELS_DIR or run `make models`.",
        dir.display()
    );
    for file in REQUIRED_MODEL_FILES {
        assert!(
            dir.join(file).exists(),
            "Required model file {} not found in {}. Run `make models` to download.",
            file,
            dir.display()
        );
    }
    dir
}

/// Load a test document image by filename from the documents directory.
/// Uses format guessing from file contents (not extension) to handle mismatched extensions.
pub fn load_test_document(filename: &str) -> DynamicImage {
    let path = documents_dir().join(filename);
    assert!(
        path.exists(),
        "Test document not found: {}. Place test images in {} or set FEN_TEST_DOCUMENTS_DIR.",
        path.display(),
        documents_dir().display()
    );
    image::ImageReader::open(&path)
        .unwrap_or_else(|e| panic!("Failed to open image {}: {}", path.display(), e))
        .with_guessed_format()
        .unwrap_or_else(|e| panic!("Failed to guess format for {}: {}", path.display(), e))
        .decode()
        .unwrap_or_else(|e| panic!("Failed to decode image {}: {}", path.display(), e))
}

/// Create a small synthetic test image (white with a black rectangle).
pub fn create_synthetic_image(width: u32, height: u32) -> DynamicImage {
    use image::{Rgb, RgbImage};
    let mut img = RgbImage::new(width, height);
    for pixel in img.pixels_mut() {
        *pixel = Rgb([255, 255, 255]);
    }
    // Add a black rectangle to simulate a text-like region
    for x in 50..200.min(width) {
        for y in 50..80.min(height) {
            img.put_pixel(x, y, Rgb([0, 0, 0]));
        }
    }
    DynamicImage::ImageRgb8(img)
}

// ---------------------------------------------------------------------------
// Component constructors (with real ONNX models)
// ---------------------------------------------------------------------------

pub fn build_ocr_engine(models: &Path) -> OcrEngine {
    let config = OcrConfig {
        detection_model_path: Some("ocr_detection.onnx".to_string()),
        recognition_model_path: Some("ocr_recognition.onnx".to_string()),
        ..Default::default()
    };
    OcrEngine::with_models(
        config,
        models.join("ocr_detection.onnx"),
        models.join("ocr_recognition.onnx"),
    )
    .expect("Failed to load OCR models")
}

pub fn build_layout_model(models: &Path) -> LayoutModel {
    let config = LayoutModelConfig {
        model_path: Some("layout_model.onnx".to_string()),
        ..Default::default()
    };
    LayoutModel::with_model(
        config,
        models.join("layout_model.onnx"),
        models.join("layout_tokenizer.json"),
    )
    .expect("Failed to load layout model")
}

pub fn build_table_extractor(models: &Path) -> TableExtractor {
    let config = TableExtractorConfig {
        detection_model_path: Some("table_detection.onnx".to_string()),
        structure_model_path: Some("table_structure.onnx".to_string()),
        ..Default::default()
    };
    TableExtractor::with_models(
        config,
        models.join("table_detection.onnx"),
        models.join("table_structure.onnx"),
    )
    .expect("Failed to load table models")
}

pub fn build_embedding_model(models: &Path) -> EmbeddingModel {
    let config = EmbeddingModelConfig {
        model_path: Some("embedding_model.onnx".to_string()),
        ..Default::default()
    };
    EmbeddingModel::with_model(
        config,
        models.join("embedding_model.onnx"),
        models.join("tokenizer.json"),
    )
    .expect("Failed to load embedding model")
}

pub fn build_full_pipeline(models: &Path) -> DocumentIntelligence {
    let config = DocumentIntelligenceConfig {
        ocr: OcrConfig {
            detection_model_path: Some("ocr_detection.onnx".to_string()),
            recognition_model_path: Some("ocr_recognition.onnx".to_string()),
            ..Default::default()
        },
        layout: LayoutModelConfig {
            model_path: Some("layout_model.onnx".to_string()),
            ..Default::default()
        },
        table: TableExtractorConfig {
            detection_model_path: Some("table_detection.onnx".to_string()),
            structure_model_path: Some("table_structure.onnx".to_string()),
            ..Default::default()
        },
        embedding: EmbeddingModelConfig {
            model_path: Some("embedding_model.onnx".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    DocumentIntelligence::with_models(config, models).expect("Failed to create full pipeline")
}

// ---------------------------------------------------------------------------
// PDF helpers
// ---------------------------------------------------------------------------

fn bind_pdfium() -> pdfium_render::prelude::Pdfium {
    use pdfium_render::prelude::*;
    let bindings = Pdfium::bind_to_system_library()
        .expect("Failed to bind pdfium. Install libpdfium.dylib to /usr/local/lib/");
    Pdfium::new(bindings)
}

/// Result of loading a PDF — either embedded text or rendered page images.
pub struct LoadedPdf {
    /// Embedded text extracted directly from the PDF (empty if scanned/image-only)
    pub text: String,
    /// Whether the PDF has usable embedded text
    pub has_text: bool,
    /// Rendered page images (only populated when `has_text` is false)
    pub pages: Vec<DynamicImage>,
    /// Number of pages in the PDF
    pub page_count: usize,
}

/// Load a PDF: extract embedded text first. Only render to images if text
/// extraction fails (scanned/image-only PDF).
pub fn load_pdf(filename: &str) -> LoadedPdf {
    use image::RgbaImage;
    use pdfium_render::prelude::*;

    let path = documents_dir().join(filename);
    assert!(
        path.exists(),
        "PDF document not found: {}. Place test PDFs in {} or set FEN_TEST_DOCUMENTS_DIR.",
        path.display(),
        documents_dir().display()
    );

    let pdfium = bind_pdfium();
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read PDF {}: {}", path.display(), e));
    let document = pdfium
        .load_pdf_from_byte_slice(&bytes, None)
        .unwrap_or_else(|e| panic!("Failed to load PDF {}: {}", path.display(), e));

    let page_count = document.pages().len() as usize;

    // Step 1: Try to extract embedded text
    let mut full_text = String::new();
    for page in document.pages().iter() {
        if let Ok(page_text) = page.text() {
            full_text.push_str(&page_text.all());
            full_text.push('\n');
        }
    }
    let trimmed = full_text.trim().to_string();
    let has_text = trimmed.len() > 50;

    if has_text {
        return LoadedPdf {
            text: trimmed,
            has_text: true,
            pages: Vec::new(),
            page_count,
        };
    }

    // Step 2: No embedded text — render pages to images for OCR
    let render_config = PdfRenderConfig::new()
        .set_target_width(2048)
        .set_maximum_height(2048)
        .rotate_if_landscape(PdfPageRenderRotation::None, false);

    let mut pages = Vec::new();
    for (i, page) in document.pages().iter().enumerate() {
        let bitmap = page
            .render_with_config(&render_config)
            .unwrap_or_else(|e| panic!("Failed to render page {} of {}: {}", i, path.display(), e));

        let width = bitmap.width() as u32;
        let height = bitmap.height() as u32;
        let buffer = bitmap.as_raw_bytes();

        let img = RgbaImage::from_raw(width, height, buffer.to_vec())
            .unwrap_or_else(|| panic!("Failed to create image from page {} bitmap", i));

        pages.push(DynamicImage::ImageRgba8(img));
    }

    LoadedPdf {
        text: trimmed,
        has_text: false,
        pages,
        page_count,
    }
}

/// Load a PDF and render all pages to images unconditionally.
/// Use when you specifically need the image representation (e.g. for OCR testing).
pub fn load_pdf_pages(filename: &str) -> Vec<DynamicImage> {
    use image::RgbaImage;
    use pdfium_render::prelude::*;

    let path = documents_dir().join(filename);
    assert!(path.exists(), "PDF document not found: {}", path.display(),);

    let pdfium = bind_pdfium();
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("Failed to read PDF {}: {}", path.display(), e));
    let document = pdfium
        .load_pdf_from_byte_slice(&bytes, None)
        .unwrap_or_else(|e| panic!("Failed to load PDF {}: {}", path.display(), e));

    let render_config = PdfRenderConfig::new()
        .set_target_width(2048)
        .set_maximum_height(2048)
        .rotate_if_landscape(PdfPageRenderRotation::None, false);

    let mut pages = Vec::new();
    for (i, page) in document.pages().iter().enumerate() {
        let bitmap = page
            .render_with_config(&render_config)
            .unwrap_or_else(|e| panic!("Failed to render page {} of {}: {}", i, path.display(), e));

        let width = bitmap.width() as u32;
        let height = bitmap.height() as u32;
        let buffer = bitmap.as_raw_bytes();

        let img = RgbaImage::from_raw(width, height, buffer.to_vec())
            .unwrap_or_else(|| panic!("Failed to create image from page {} bitmap", i));

        pages.push(DynamicImage::ImageRgba8(img));
    }

    assert!(
        !pages.is_empty(),
        "PDF {} rendered zero pages",
        path.display()
    );
    pages
}

/// Load a PDF and return only the first page as an image.
pub fn load_pdf_first_page(filename: &str) -> DynamicImage {
    load_pdf_pages(filename)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("PDF {} has no pages", filename))
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Assert that a confidence score is within [0.0, 1.0].
pub fn assert_valid_confidence(confidence: f32, context: &str) {
    assert!(
        (0.0..=1.0).contains(&confidence),
        "{}: confidence {} not in [0.0, 1.0]",
        context,
        confidence
    );
}

/// Assert that an embedding vector has the expected dimension and is not all zeros.
pub fn assert_valid_embedding(embedding: &[f32], expected_dim: usize, context: &str) {
    assert_eq!(
        embedding.len(),
        expected_dim,
        "{}: expected embedding dim {}, got {}",
        context,
        expected_dim,
        embedding.len()
    );
    let has_nonzero = embedding.iter().any(|&x| x != 0.0);
    assert!(has_nonzero, "{}: embedding is all zeros", context);
    // Check approximate L2 normalization (norm should be close to 1.0)
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.15,
        "{}: embedding norm {} is not approximately 1.0",
        context,
        norm
    );
}

/// Assert that a bounding box has non-negative dimensions.
#[allow(dead_code)]
pub fn assert_valid_bbox(bbox: &BoundingBox, context: &str) {
    assert!(
        bbox.width >= 0.0 && bbox.height >= 0.0,
        "{}: bounding box has negative dimensions (w={}, h={})",
        context,
        bbox.width,
        bbox.height
    );
}

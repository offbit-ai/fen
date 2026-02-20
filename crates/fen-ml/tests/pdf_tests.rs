//! Integration tests for the ML pipeline with PDF documents.
//!
//! Uses text extraction first (for text-based PDFs), falling back to
//! image rendering + OCR only for scanned/image-only PDFs.
//!
//! Requires: pdfium native library (libpdfium.dylib) installed to /usr/local/lib/
//!
//! Run with: cargo test -p fen-ml --test pdf_tests -- --ignored

mod common;

use fen_ml::OcrProvider;

#[test]
#[ignore]
fn test_pdf_text_extraction() {
    common::init_test_tracing();

    let pdf = common::load_pdf("invoice_hf.pdf");
    assert!(pdf.has_text, "Text-based PDF should have extractable text");
    assert!(
        pdf.text.len() > 50,
        "Should extract meaningful text, got {} chars",
        pdf.text.len()
    );
    assert!(
        pdf.pages.is_empty(),
        "Text-based PDF should not render images"
    );

    tracing::info!(
        text_len = pdf.text.len(),
        page_count = pdf.page_count,
        "Extracted text from PDF"
    );
}

#[test]
#[ignore]
fn test_pdf_rendering_produces_images() {
    common::init_test_tracing();

    // load_pdf_pages always renders, for when we need images explicitly
    let pages = common::load_pdf_pages("invoice_hf.pdf");
    assert!(!pages.is_empty(), "PDF should produce at least one page");

    for (i, page) in pages.iter().enumerate() {
        let (w, h) = (page.width(), page.height());
        assert!(w > 0 && h > 0, "Page {} should have positive dimensions", i);
        tracing::info!(page = i, width = w, height = h, "Rendered PDF page");
    }
}

#[tokio::test]
#[ignore]
async fn test_pdf_ocr_on_rendered_invoice() {
    common::init_test_tracing();
    let models = common::require_models();
    let ocr_engine = common::build_ocr_engine(&models);
    let image = common::load_pdf_first_page("invoice_hf.pdf");

    let result = ocr_engine.process_image(&image).await.unwrap();

    assert!(!result.text.is_empty(), "OCR should extract text from rendered PDF");
    assert!(
        !result.regions.is_empty(),
        "OCR should detect regions in rendered PDF"
    );
    common::assert_valid_confidence(result.confidence, "PDF OCR confidence");

    tracing::info!(
        text_len = result.text.len(),
        regions = result.regions.len(),
        "PDF OCR results"
    );
}

#[tokio::test]
#[ignore]
async fn test_pdf_pipeline_text_and_embedding() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);

    // Text-based PDF: extract text, embed it directly
    let pdf = common::load_pdf("invoice_aws.pdf");
    assert!(pdf.has_text, "invoice_aws.pdf should have embedded text");

    let embedding = pipeline.embedding.embed(&pdf.text).unwrap();
    let dim = pipeline.embedding.embedding_dim();
    common::assert_valid_embedding(&embedding, dim, "PDF text embedding");

    // Scanned PDF (if any) would go through OCR instead
    tracing::info!(
        text_len = pdf.text.len(),
        embedding_dim = dim,
        "PDF text + embedding pipeline"
    );
}

#[tokio::test]
#[ignore]
async fn test_pdf_varied_invoices() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);

    let pdf_files = [
        "invoice_hf.pdf",
        "invoice_aws.pdf",
        "invoice_flipkart.pdf",
        "invoice_oyo.pdf",
        "invoice_netpresse.pdf",
        "invoice_scribbles.pdf",
        "invoice_femstac_a.pdf",
        "invoice_excid3.pdf",
    ];

    let mut embeddings: Vec<(String, Vec<f32>)> = Vec::new();

    for filename in &pdf_files {
        let doc_path = common::documents_dir().join(filename);
        if !doc_path.exists() {
            tracing::warn!(filename, "Skipping missing PDF document");
            continue;
        }

        let pdf = common::load_pdf(filename);

        let text = if pdf.has_text {
            // Text-based PDF: use extracted text directly
            tracing::info!(filename, text_len = pdf.text.len(), "Using embedded text");
            pdf.text
        } else {
            // Scanned PDF: render + OCR
            let image = common::load_pdf_first_page(filename);
            let result = pipeline.process_image(&image).await.unwrap();
            tracing::info!(
                filename,
                text_len = result.text.len(),
                regions = result.ocr_result.regions.len(),
                "Used OCR for scanned PDF"
            );
            result.text
        };

        assert!(
            !text.is_empty(),
            "{} should produce text (via extraction or OCR)",
            filename
        );

        let embedding = pipeline.embedding.embed(&text).unwrap();
        embeddings.push((filename.to_string(), embedding));
    }

    assert!(
        embeddings.len() >= 2,
        "Should successfully process at least 2 PDF invoices"
    );

    // Different PDFs should produce different embeddings
    let sim = pipeline.embedding.cosine_similarity(
        &embeddings[0].1,
        &embeddings[1].1,
    );
    assert!(
        sim < 0.99,
        "Different PDFs ({} vs {}) should have < 0.99 cosine similarity, got {}",
        embeddings[0].0,
        embeddings[1].0,
        sim
    );
}

#[tokio::test]
#[ignore]
async fn test_pdf_multi_page() {
    common::init_test_tracing();

    let doc_path = common::documents_dir().join("form_w9.pdf");
    if !doc_path.exists() {
        panic!("form_w9.pdf not found in test fixtures");
    }

    let pdf = common::load_pdf("form_w9.pdf");
    tracing::info!(
        page_count = pdf.page_count,
        has_text = pdf.has_text,
        text_len = pdf.text.len(),
        "Loaded multi-page PDF"
    );

    if pdf.has_text {
        assert!(
            pdf.text.len() > 100,
            "Multi-page PDF should have substantial text"
        );
    } else {
        // Scanned multi-page: OCR each page
        let models = common::require_models();
        let ocr_engine = common::build_ocr_engine(&models);
        let pages = common::load_pdf_pages("form_w9.pdf");

        let mut total_text = String::new();
        for (i, page) in pages.iter().enumerate() {
            let result = ocr_engine.process_image(page).await.unwrap();
            tracing::info!(
                page = i,
                text_len = result.text.len(),
                regions = result.regions.len(),
                "OCR result for page"
            );
            total_text.push_str(&result.text);
            total_text.push('\n');
        }

        assert!(
            !total_text.trim().is_empty(),
            "Multi-page PDF should produce text across all pages"
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_pdf_receipt_and_statement() {
    common::init_test_tracing();
    let models = common::require_models();
    let pipeline = common::build_full_pipeline(&models);

    let pdf_files = ["receipt_excid3.pdf", "statement_excid3.pdf"];

    for filename in &pdf_files {
        let doc_path = common::documents_dir().join(filename);
        if !doc_path.exists() {
            tracing::warn!(filename, "Skipping missing PDF");
            continue;
        }

        let pdf = common::load_pdf(filename);
        let text = if pdf.has_text {
            pdf.text
        } else {
            let image = common::load_pdf_first_page(filename);
            let result = pipeline.process_image(&image).await.unwrap();
            result.text
        };

        let embedding = pipeline.embedding.embed(&text).unwrap();
        let dim = pipeline.embedding.embedding_dim();
        assert_eq!(
            embedding.len(),
            dim,
            "{} should produce embedding with correct dimension",
            filename
        );

        tracing::info!(filename, text_len = text.len(), "Processed PDF");
    }
}

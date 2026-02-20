//! Integration tests for EmbeddingModel with real ONNX model.
//!
//! Run with: cargo test -p fen-ml --test embedding_tests -- --ignored

mod common;

#[test]
#[ignore]
fn test_embedding_model_loading() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_embedding_model(&models);
    assert!(model.has_model(), "Embedding model should report model loaded");
    assert_eq!(model.embedding_dim(), 384, "MiniLM-L6-v2 embedding dim should be 384");
}

#[test]
#[ignore]
fn test_embed_single_text() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_embedding_model(&models);

    let embedding = model.embed("This is a test invoice for Acme Corp").unwrap();
    common::assert_valid_embedding(&embedding, model.embedding_dim(), "Single text embedding");
}

#[test]
#[ignore]
fn test_embed_batch() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_embedding_model(&models);

    let texts = [
        "Invoice number INV-2024-001",
        "Payment due date February 14 2024",
        "Total amount $1,134.00",
    ];
    let embeddings = model.embed_batch(&texts).unwrap();

    assert_eq!(embeddings.len(), 3, "Batch should return one embedding per text");
    for (i, emb) in embeddings.iter().enumerate() {
        common::assert_valid_embedding(emb, model.embedding_dim(), &format!("Batch embedding {}", i));
    }
}

#[test]
#[ignore]
fn test_embedding_semantic_similarity() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_embedding_model(&models);

    let emb_invoice1 = model.embed("Invoice for office supplies").unwrap();
    let emb_invoice2 = model.embed("Bill for office materials and stationery").unwrap();
    let emb_unrelated = model.embed("The weather forecast predicts rain tomorrow").unwrap();

    let sim_related = model.cosine_similarity(&emb_invoice1, &emb_invoice2);
    let sim_unrelated = model.cosine_similarity(&emb_invoice1, &emb_unrelated);

    assert!(
        sim_related > sim_unrelated,
        "Similar texts ({:.4}) should have higher similarity than dissimilar ({:.4})",
        sim_related,
        sim_unrelated
    );

    assert!((-1.0..=1.0).contains(&sim_related));
    assert!((-1.0..=1.0).contains(&sim_unrelated));
}

#[test]
#[ignore]
fn test_embedding_determinism_and_caching() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_embedding_model(&models);

    let text = "Invoice #12345 from Acme Corp";
    let emb1 = model.embed(text).unwrap();
    let emb2 = model.embed(text).unwrap(); // should hit cache

    assert_eq!(emb1.len(), emb2.len());
    for (a, b) in emb1.iter().zip(emb2.iter()) {
        assert!(
            (a - b).abs() < 1e-6,
            "Repeated embeddings should be identical"
        );
    }
}

#[test]
#[ignore]
fn test_embed_document() {
    common::init_test_tracing();
    let models = common::require_models();
    let model = common::build_embedding_model(&models);
    let dim = model.embedding_dim();

    let full_text = "Invoice INV-2024-001 from Acme Supplies for Widget Corp. Total: $1,134.00";
    let sections = vec![
        ("s0".to_string(), "Header".to_string(), "Invoice INV-2024-001".to_string()),
        ("s1".to_string(), "Body".to_string(), "Widget Corp order details".to_string()),
    ];
    let entities = vec![
        ("Amount".to_string(), "$1,134.00".to_string()),
        ("Date".to_string(), "2024-01-15".to_string()),
    ];

    let doc_embeddings = model.embed_document(full_text, &sections, &entities).unwrap();

    common::assert_valid_embedding(&doc_embeddings.document, dim, "Document embedding");
    assert_eq!(doc_embeddings.sections.len(), 2);
    assert_eq!(doc_embeddings.entities.len(), 2);

    for (i, sec) in doc_embeddings.sections.iter().enumerate() {
        common::assert_valid_embedding(&sec.embedding, dim, &format!("Section {} embedding", i));
        assert!(!sec.section_id.is_empty());
        assert!(!sec.text.is_empty());
    }

    for (i, ent) in doc_embeddings.entities.iter().enumerate() {
        common::assert_valid_embedding(&ent.embedding, dim, &format!("Entity {} embedding", i));
        assert!(!ent.value.is_empty());
    }
}

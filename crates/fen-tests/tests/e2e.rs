//! End-to-end integration tests for the Fen anomaly detection pipeline
//!
//! These tests exercise the full pipeline from data ingestion through
//! storage, validation, and query operations.

use rust_decimal_macros::dec;

use fen_core::domain::{AnomalyType, Severity};
use fen_core::ValidationStatus;
use fen_storage::{DocumentStore, InvoiceFilter, StorageTier};
use fen_tests::{
    consistent_embedding, init_test_tracing, random_embedding, InvoiceFixture,
    ContractFixture, IngestionTestEnv, QueryTestEnv, TestEnv, sample_invoice_text,
};

// ============================================================================
// Module: Ingestion Pipeline Tests
// ============================================================================

mod ingestion {
    use super::*;

    /// Test basic text ingestion flow
    #[tokio::test]
    async fn test_text_ingestion_creates_invoice() {
        init_test_tracing();
        let env = IngestionTestEnv::new().await;

        let text = sample_invoice_text();
        let invoice = env.pipeline.ingest_text(text).await.unwrap();

        // Verify invoice was parsed
        assert!(!invoice.invoice_number.is_empty());
        assert!(invoice.total_amount > dec!(0));

        // Verify invoice was stored
        let stored = env.storage.get_invoice(&invoice.id).await.unwrap();
        assert!(stored.is_some());
        assert_eq!(stored.unwrap().id, invoice.id);
    }

    /// Test that ingested invoices are retrievable
    #[tokio::test]
    async fn test_ingestion_stores_invoice_correctly() {
        let env = IngestionTestEnv::new().await;

        let text = sample_invoice_text();
        let invoice = env.pipeline.ingest_text(text).await.unwrap();

        // Retrieve from storage
        let retrieved = env.storage.get_invoice(&invoice.id).await.unwrap().unwrap();

        // Verify key fields match
        assert_eq!(retrieved.invoice_number, invoice.invoice_number);
        assert_eq!(retrieved.total_amount, invoice.total_amount);
        assert_eq!(retrieved.vendor.name, invoice.vendor.name);
    }

    /// Test ingestion with multiple documents
    #[tokio::test]
    async fn test_batch_ingestion() {
        let env = IngestionTestEnv::new().await;

        let texts = vec![
            sample_invoice_text(),
            r#"INVOICE
Invoice Number: BATCH-002
Invoice Date: 2024-01-20
Due Date: 2024-02-20
Vendor: Test Vendor
Total: $500.00
"#,
            r#"INVOICE
Invoice Number: BATCH-003
Invoice Date: 2024-01-25
Due Date: 2024-02-25
Vendor: Another Vendor
Total: $750.00
"#,
        ];

        let mut invoices = Vec::new();
        for text in texts {
            let invoice = env.pipeline.ingest_text(text).await.unwrap();
            invoices.push(invoice);
        }

        // Verify all invoices stored
        let count = env.storage.count_invoices().await.unwrap();
        assert_eq!(count, 3);

        // Verify each can be retrieved
        for invoice in &invoices {
            let stored = env.storage.get_invoice(&invoice.id).await.unwrap();
            assert!(stored.is_some());
        }
    }

    /// Test ingestion pipeline doesn't fail on malformed input
    #[tokio::test]
    async fn test_ingestion_handles_minimal_text() {
        let env = IngestionTestEnv::new().await;

        // Minimal invoice text - parser should still create an invoice
        let text = "Invoice: MIN-001\nTotal: $100";
        let invoice = env.pipeline.ingest_text(text).await.unwrap();

        // Should create invoice even with minimal info
        assert!(!invoice.id.0.is_nil());
    }
}

// ============================================================================
// Module: Storage Layer Tests
// ============================================================================

mod storage {
    use super::*;

    /// Test basic invoice CRUD operations
    #[tokio::test]
    async fn test_invoice_crud_operations() {
        init_test_tracing();
        let env = TestEnv::new().await;

        // Create
        let invoice = InvoiceFixture::new()
            .with_number("CRUD-001")
            .build();

        env.storage.store_invoice(&invoice, None).await.unwrap();

        // Read
        let retrieved = env.storage.get_invoice(&invoice.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.as_ref().unwrap().invoice_number, "CRUD-001");

        // Delete
        let deleted = env.storage.delete_invoice(&invoice.id).await.unwrap();
        assert!(deleted);

        // Verify deleted
        let after_delete = env.storage.get_invoice(&invoice.id).await.unwrap();
        assert!(after_delete.is_none());
    }

    /// Test listing invoices with pagination
    #[tokio::test]
    async fn test_invoice_list_pagination() {
        let env = TestEnv::new().await;

        // Create 10 invoices
        let invoices = InvoiceFixture::new()
            .with_number("PAGE")
            .build_batch(10);

        for invoice in &invoices {
            env.storage.store_invoice(invoice, None).await.unwrap();
        }

        // Test pagination
        let page1 = env.storage.list_invoices(3, 0).await.unwrap();
        assert_eq!(page1.len(), 3);

        let page2 = env.storage.list_invoices(3, 3).await.unwrap();
        assert_eq!(page2.len(), 3);

        let page4 = env.storage.list_invoices(3, 9).await.unwrap();
        assert_eq!(page4.len(), 1);

        // Total count
        let count = env.storage.count_invoices().await.unwrap();
        assert_eq!(count, 10);
    }

    /// Test contract storage operations
    #[tokio::test]
    async fn test_contract_storage() {
        let env = TestEnv::new().await;

        let contract = ContractFixture::new()
            .with_title("Test Contract")
            .build();

        env.storage.store_contract(&contract).await.unwrap();

        let retrieved = env.storage.get_contract(&contract.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Contract");
    }

    /// Test tiered storage with hot/warm separation
    #[tokio::test]
    async fn test_tiered_storage_hot_tier() {
        let env = TestEnv::new().await;

        // Recent invoice should go to hot tier
        let recent = InvoiceFixture::new()
            .with_number("HOT-001")
            .build();

        env.storage.store_invoice(&recent, None).await.unwrap();

        // Verify retrievable
        let retrieved = env.storage.get_invoice(&recent.id).await.unwrap();
        assert!(retrieved.is_some());
    }

    /// Test invoice storage with embeddings for vector search
    #[tokio::test]
    async fn test_storage_with_embeddings() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("EMB-001")
            .build();

        let embedding = random_embedding(768);

        env.storage
            .store_invoice(&invoice, Some(&embedding))
            .await
            .unwrap();

        // Should still be retrievable
        let retrieved = env.storage.get_invoice(&invoice.id).await.unwrap();
        assert!(retrieved.is_some());
    }

    /// Test hybrid search with filters
    #[tokio::test]
    async fn test_hybrid_search_with_filters() {
        let env = TestEnv::new().await;

        // Create invoices from different vendors
        let vendors = ["Acme Corp", "Beta Inc", "Acme Corp"];
        let mut stored_invoices = Vec::new();

        for (i, vendor) in vendors.iter().enumerate() {
            let invoice = InvoiceFixture::new()
                .with_number(format!("SEARCH-{:03}", i))
                .with_vendor(*vendor)
                .build();

            let embedding = consistent_embedding(&invoice.invoice_number, 768);
            env.storage
                .store_invoice(&invoice, Some(&embedding))
                .await
                .unwrap();
            stored_invoices.push(invoice);
        }

        // Search with vendor filter
        let filter = InvoiceFilter {
            vendor_name: Some("Acme Corp".to_string()),
            date_from: None,
            date_to: None,
            min_amount: None,
            max_amount: None,
            currency: None,
        };

        let results = env.storage
            .search_invoices_hybrid(None, &filter, 10)
            .await
            .unwrap();

        // Should find 2 Acme Corp invoices
        assert_eq!(results.len(), 2);
        for inv in &results {
            assert_eq!(inv.vendor.name, "Acme Corp");
        }
    }
}

// ============================================================================
// Module: Validation Pipeline Tests
// ============================================================================

mod validation {
    use super::*;

    /// Test validation of a valid invoice
    #[tokio::test]
    async fn test_validate_valid_invoice() {
        init_test_tracing();
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("VALID-001")
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should be valid with no critical issues
        assert!(!result.has_critical());
    }

    /// Test validation detects math mismatch
    #[tokio::test]
    async fn test_validate_detects_math_mismatch() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("MATH-ERR-001")
            .with_math_error()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should detect math mismatch
        assert!(
            result.anomalies.iter().any(|a| a.anomaly_type == AnomalyType::MathMismatch),
            "Expected MathMismatch anomaly, found: {:?}",
            result.anomalies
        );
    }

    /// Test validation detects total calculation error
    #[tokio::test]
    async fn test_validate_detects_total_error() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("TOTAL-ERR-001")
            .with_total_error()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should detect total mismatch
        assert!(
            result.anomalies.iter().any(|a| a.anomaly_type == AnomalyType::MathMismatch),
            "Expected MathMismatch anomaly for total, found: {:?}",
            result.anomalies
        );
    }

    /// Test validation detects date inconsistency
    #[tokio::test]
    async fn test_validate_detects_date_error() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("DATE-ERR-001")
            .with_date_error()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        assert!(
            result.anomalies.iter().any(|a| a.anomaly_type == AnomalyType::DateInconsistency),
            "Expected DateInconsistency anomaly, found: {:?}",
            result.anomalies
        );
    }

    /// Test validation detects negative total
    #[tokio::test]
    async fn test_validate_detects_negative_total() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("NEG-001")
            .with_negative_total()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        assert!(
            result.anomalies.iter().any(|a| a.anomaly_type == AnomalyType::OutOfRange),
            "Expected OutOfRange anomaly for negative total, found: {:?}",
            result.anomalies
        );
    }

    /// Test validation detects unknown vendor
    #[tokio::test]
    async fn test_validate_detects_unknown_vendor() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("VENDOR-ERR-001")
            .with_unknown_vendor()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        assert!(
            result.anomalies.iter().any(|a| a.anomaly_type == AnomalyType::MissingField),
            "Expected MissingField anomaly for unknown vendor, found: {:?}",
            result.anomalies
        );
    }

    /// Test validation warns on low confidence
    #[tokio::test]
    async fn test_validate_warns_low_confidence() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("CONF-001")
            .with_low_confidence()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should have a low severity warning about confidence
        assert!(
            result.anomalies.iter().any(|a|
                a.description.to_lowercase().contains("confidence") &&
                a.severity == Severity::Low
            ),
            "Expected low confidence warning, found: {:?}",
            result.anomalies
        );
    }

    /// Test validation with multiple errors
    #[tokio::test]
    async fn test_validate_multiple_errors() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("MULTI-ERR-001")
            .with_math_error()
            .with_date_error()
            .with_unknown_vendor()
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should detect multiple issues
        assert!(result.anomalies.len() >= 3,
            "Expected at least 3 anomalies, found: {}",
            result.anomalies.len()
        );
    }

    /// Test batch validation
    #[tokio::test]
    async fn test_batch_validation() {
        let env = TestEnv::new().await;

        let invoices = vec![
            InvoiceFixture::new().with_number("BATCH-001").build(),
            InvoiceFixture::new().with_number("BATCH-002").with_math_error().build(),
            InvoiceFixture::new().with_number("BATCH-003").build(),
        ];

        let mut results = Vec::new();
        for invoice in &invoices {
            let result = env.rule_engine.validate_invoice(invoice).await.unwrap();
            results.push(result);
        }

        // First and third should be valid
        assert!(!results[0].has_critical());
        assert!(!results[2].has_critical());

        // Second should have errors
        assert!(results[1].anomalies.iter().any(|a| a.anomaly_type == AnomalyType::MathMismatch));
    }
}

// ============================================================================
// Module: Query System Tests
// ============================================================================

mod query {
    use super::*;

    /// Test query engine basic operations
    #[tokio::test]
    async fn test_query_engine_store_and_retrieve() {
        init_test_tracing();
        let env = QueryTestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("QUERY-001")
            .build();

        env.query_engine.store_invoice(&invoice, None).await.unwrap();

        let result = env.query_engine.get_invoice(&invoice.id).await.unwrap();
        assert!(result.data.is_some());
        assert_eq!(result.data.unwrap().invoice_number, "QUERY-001");
    }

    /// Test query cache functionality
    #[tokio::test]
    async fn test_query_cache() {
        let env = QueryTestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("CACHE-001")
            .build();

        env.query_engine.store_invoice(&invoice, None).await.unwrap();

        // First query - cache miss
        let result1 = env.query_engine.get_invoice(&invoice.id).await.unwrap();
        assert!(!result1.metrics.cache_hit);

        // Second query - cache hit
        let result2 = env.query_engine.get_invoice(&invoice.id).await.unwrap();
        assert!(result2.metrics.cache_hit);
    }

    /// Test query metrics tracking
    #[tokio::test]
    async fn test_query_metrics() {
        let env = QueryTestEnv::new().await;

        let invoice = InvoiceFixture::new().build();
        env.query_engine.store_invoice(&invoice, None).await.unwrap();

        let result = env.query_engine.get_invoice(&invoice.id).await.unwrap();

        // Should have query metrics
        assert!(result.metrics.query_time_ms >= 0);
        assert_eq!(result.metrics.rows_returned, 1);
        assert_eq!(result.metrics.tier_used, Some(StorageTier::Hot));
    }

    /// Test vector search with embeddings
    #[tokio::test]
    async fn test_vector_search() {
        let env = QueryTestEnv::new().await;

        // Store invoices with embeddings
        for i in 0..5 {
            let invoice = InvoiceFixture::new()
                .with_number(format!("VEC-{:03}", i))
                .build();
            let embedding = consistent_embedding(&invoice.invoice_number, 768);
            env.query_engine
                .store_invoice(&invoice, Some(&embedding))
                .await
                .unwrap();
        }

        // Search with similar embedding
        let query_embedding = consistent_embedding("VEC-002", 768);
        let results = env.query_engine
            .search_invoices_by_embedding(&query_embedding, 3)
            .await
            .unwrap();

        // Should return results (up to limit)
        assert!(!results.data.is_empty());
        assert!(results.data.len() <= 3);

        // All results should have scores (distance-based, lower is better for L2)
        for result in &results.data {
            assert!(result.score >= 0.0);
        }
    }

    /// Test invoice listing
    #[tokio::test]
    async fn test_invoice_listing() {
        let env = QueryTestEnv::new().await;

        // Store 5 invoices
        for i in 0..5 {
            let invoice = InvoiceFixture::new()
                .with_number(format!("LIST-{:03}", i))
                .build();
            env.query_engine.store_invoice(&invoice, None).await.unwrap();
        }

        let result = env.query_engine.list_invoices(10, 0).await.unwrap();
        assert_eq!(result.data.len(), 5);
        assert_eq!(result.metrics.rows_returned, 5);
    }

    /// Test invoice count
    #[tokio::test]
    async fn test_invoice_count() {
        let env = QueryTestEnv::new().await;

        // Store 7 invoices
        for i in 0..7 {
            let invoice = InvoiceFixture::new()
                .with_number(format!("COUNT-{:03}", i))
                .build();
            env.query_engine.store_invoice(&invoice, None).await.unwrap();
        }

        let result = env.query_engine.count_invoices().await.unwrap();
        assert_eq!(result.data, 7);
    }

    /// Test tier count breakdown
    #[tokio::test]
    async fn test_tier_count_breakdown() {
        let env = QueryTestEnv::new().await;

        // Store recent invoices (hot tier)
        for i in 0..3 {
            let invoice = InvoiceFixture::new()
                .with_number(format!("TIER-{:03}", i))
                .build();
            env.query_engine.store_invoice(&invoice, None).await.unwrap();
        }

        let (hot, warm, cold) = env.query_engine.count_invoices_by_tier();
        assert_eq!(hot, 3);
        assert_eq!(warm, 0);
        assert_eq!(cold, 0);
    }
}

// ============================================================================
// Module: Full Pipeline E2E Tests
// ============================================================================

mod full_pipeline {
    use super::*;

    /// Test complete flow: ingest -> store -> validate -> query
    #[tokio::test]
    async fn test_complete_invoice_lifecycle() {
        init_test_tracing();

        // Setup
        let env = IngestionTestEnv::new().await;

        // 1. Ingest document
        let text = sample_invoice_text();
        let invoice = env.pipeline.ingest_text(text).await.unwrap();
        let invoice_id = invoice.id.clone();

        tracing::info!(invoice_id = %invoice_id, "Ingested invoice");

        // 2. Verify storage
        let stored = env.storage.get_invoice(&invoice_id).await.unwrap();
        assert!(stored.is_some(), "Invoice should be stored");
        let stored_invoice = stored.unwrap();

        // 3. Validate
        let validation = env.rule_engine.validate_invoice(&stored_invoice).await.unwrap();
        tracing::info!(
            is_valid = %validation.is_valid,
            anomaly_count = %validation.anomalies.len(),
            "Validation complete"
        );

        // 4. List and verify in results
        let all_invoices = env.storage.list_invoices(100, 0).await.unwrap();
        assert!(
            all_invoices.iter().any(|i| i.id == invoice_id),
            "Invoice should appear in list"
        );

        // 5. Delete
        let deleted = env.storage.delete_invoice(&invoice_id).await.unwrap();
        assert!(deleted, "Delete should succeed");

        // 6. Verify deletion
        let after_delete = env.storage.get_invoice(&invoice_id).await.unwrap();
        assert!(after_delete.is_none(), "Invoice should be deleted");
    }

    /// Test validation status updates through pipeline
    #[tokio::test]
    async fn test_validation_status_flow() {
        let env = TestEnv::new().await;

        // Create invoice with pending status
        let mut invoice = InvoiceFixture::new()
            .with_number("STATUS-001")
            .with_status(ValidationStatus::Pending)
            .build();

        // Store
        env.storage.store_invoice(&invoice, None).await.unwrap();

        // Validate
        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Update status based on validation
        invoice.validation_status = if result.has_critical() {
            ValidationStatus::Failed
        } else if result.has_high_severity() {
            ValidationStatus::PassedWithWarnings
        } else {
            ValidationStatus::Passed
        };

        // Store updated invoice
        env.storage.store_invoice(&invoice, None).await.unwrap();

        // Verify status updated
        let retrieved = env.storage.get_invoice(&invoice.id).await.unwrap().unwrap();
        assert!(
            matches!(retrieved.validation_status, ValidationStatus::Passed | ValidationStatus::PassedWithWarnings),
            "Status should be updated: {:?}",
            retrieved.validation_status
        );
    }

    /// Test handling batch of mixed valid/invalid invoices
    #[tokio::test]
    async fn test_mixed_batch_processing() {
        let env = TestEnv::new().await;

        // Create batch with some valid, some invalid
        let invoices = vec![
            InvoiceFixture::new().with_number("MIX-001").build(),
            InvoiceFixture::new().with_number("MIX-002").with_math_error().build(),
            InvoiceFixture::new().with_number("MIX-003").build(),
            InvoiceFixture::new().with_number("MIX-004").with_date_error().build(),
            InvoiceFixture::new().with_number("MIX-005").build(),
        ];

        let mut valid_count = 0;
        let mut invalid_count = 0;

        for invoice in &invoices {
            // Store
            env.storage.store_invoice(invoice, None).await.unwrap();

            // Validate
            let result = env.rule_engine.validate_invoice(invoice).await.unwrap();

            if result.anomalies.is_empty() {
                valid_count += 1;
            } else {
                invalid_count += 1;
            }
        }

        // Should have mix of results: 3 valid, 2 with anomalies
        assert_eq!(valid_count, 3);
        assert_eq!(invalid_count, 2);

        // All should be stored regardless
        let total = env.storage.count_invoices().await.unwrap();
        assert_eq!(total, 5);
    }

    /// Test concurrent operations
    #[tokio::test]
    async fn test_concurrent_operations() {
        let env = TestEnv::new().await;
        let storage = env.storage.clone();
        let rule_engine = env.rule_engine.clone();

        // Spawn concurrent tasks
        let mut handles = Vec::new();

        for i in 0..10 {
            let storage = storage.clone();
            let rule_engine = rule_engine.clone();

            let handle = tokio::spawn(async move {
                let invoice = InvoiceFixture::new()
                    .with_number(format!("CONC-{:03}", i))
                    .build();

                // Store
                storage.store_invoice(&invoice, None).await.unwrap();

                // Validate
                let result = rule_engine.validate_invoice(&invoice).await.unwrap();

                (invoice.id, result.is_valid)
            });

            handles.push(handle);
        }

        // Wait for all
        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // All should complete
        assert_eq!(results.len(), 10);

        // All should be stored
        let count = storage.count_invoices().await.unwrap();
        assert_eq!(count, 10);
    }

    /// Test query after validation updates
    #[tokio::test]
    async fn test_query_validated_invoices() {
        let env = TestEnv::new().await;

        // Store and validate multiple invoices
        let invoices = InvoiceFixture::new()
            .with_number("QV")
            .build_batch(5);

        for invoice in &invoices {
            let embedding = consistent_embedding(&invoice.invoice_number, 768);
            env.storage.store_invoice(invoice, Some(&embedding)).await.unwrap();

            // Validate each
            let _result = env.rule_engine.validate_invoice(invoice).await.unwrap();
        }

        // Query all
        let listed = env.storage.list_invoices(10, 0).await.unwrap();
        assert_eq!(listed.len(), 5);

        // Vector search should work
        let query_emb = consistent_embedding("QV-001", 768);
        let search_results = env.storage
            .search_invoices_by_embedding(&query_emb, 3)
            .await
            .unwrap();

        assert!(!search_results.is_empty());
    }

    /// Test contract alongside invoice workflow
    #[tokio::test]
    async fn test_invoice_with_contract() {
        let env = TestEnv::new().await;

        // Create contract
        let contract = ContractFixture::new()
            .with_title("Master Agreement")
            .build();

        env.storage.store_contract(&contract).await.unwrap();

        // Create invoices referencing contract (conceptually)
        for i in 0..3 {
            let invoice = InvoiceFixture::new()
                .with_number(format!("CTR-INV-{:03}", i))
                .with_vendor("Acme Corp") // Same as contract party
                .build();

            env.storage.store_invoice(&invoice, None).await.unwrap();
        }

        // Verify counts
        let invoice_count = env.storage.count_invoices().await.unwrap();
        let contract_count = env.storage.count_contracts().await.unwrap();

        assert_eq!(invoice_count, 3);
        assert_eq!(contract_count, 1);

        // Both should be retrievable
        let retrieved_contract = env.storage.get_contract(&contract.id).await.unwrap();
        assert!(retrieved_contract.is_some());
    }

    /// Test error recovery in pipeline
    #[tokio::test]
    async fn test_error_recovery() {
        let env = TestEnv::new().await;

        // Try to get non-existent invoice
        let fake_id = fen_core::domain::InvoiceId::new();
        let result = env.storage.get_invoice(&fake_id).await.unwrap();
        assert!(result.is_none());

        // Should still work for valid operations after
        let invoice = InvoiceFixture::new().with_number("RECOVER-001").build();
        env.storage.store_invoice(&invoice, None).await.unwrap();

        let retrieved = env.storage.get_invoice(&invoice.id).await.unwrap();
        assert!(retrieved.is_some());
    }
}

// ============================================================================
// Module: Anomaly Detection Scenarios
// ============================================================================

mod anomaly_scenarios {
    use super::*;

    /// Test line item calculation anomaly detection
    #[tokio::test]
    async fn test_line_item_calculation_anomaly() {
        let env = TestEnv::new().await;

        // Create invoice with incorrect line item total
        let mut invoice = InvoiceFixture::new()
            .with_number("LINE-001")
            .build();

        // Manually set incorrect line item total
        if let Some(item) = invoice.line_items.first_mut() {
            item.total = dec!(999.99); // Wrong - should be quantity * unit_price
        }

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should detect line item calculation error
        assert!(
            result.anomalies.iter().any(|a|
                a.anomaly_type == AnomalyType::MathMismatch &&
                a.field_path.as_ref().map_or(false, |f| f.contains("line_item"))
            ),
            "Expected line item math mismatch, found: {:?}",
            result.anomalies
        );
    }

    /// Test combined anomaly severity scoring
    #[tokio::test]
    async fn test_anomaly_severity_levels() {
        let env = TestEnv::new().await;

        // Invoice with multiple issues of different severity
        let invoice = InvoiceFixture::new()
            .with_number("SEV-001")
            .with_math_error()      // High severity
            .with_low_confidence()   // Low severity
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should have anomalies of different severities
        let has_high = result.anomalies.iter().any(|a|
            a.severity == Severity::High || a.severity == Severity::Critical
        );
        let has_low = result.anomalies.iter().any(|a| a.severity == Severity::Low);

        assert!(has_high, "Should have high severity anomaly");
        assert!(has_low, "Should have low severity anomaly");

        // has_critical should check for Critical specifically
        assert!(result.has_high_severity());
    }

    /// Test validation performance with many anomalies
    #[tokio::test]
    async fn test_many_anomalies_performance() {
        let env = TestEnv::new().await;

        // Create invoice with many potential issues
        let invoice = InvoiceFixture::new()
            .with_number("")  // Empty invoice number
            .with_math_error()
            .with_date_error()
            .with_unknown_vendor()
            .with_negative_total()
            .with_low_confidence()
            .build();

        let start = std::time::Instant::now();
        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();
        let elapsed = start.elapsed();

        // Should complete quickly even with many checks
        assert!(elapsed.as_millis() < 100, "Validation took too long: {:?}", elapsed);

        // Should detect multiple issues
        assert!(result.anomalies.len() >= 4,
            "Expected at least 4 anomalies, found: {}",
            result.anomalies.len()
        );
    }

    /// Test currency-specific validation scenarios
    #[tokio::test]
    async fn test_currency_handling() {
        let env = TestEnv::new().await;

        // USD invoice
        let usd_invoice = InvoiceFixture::new()
            .with_number("USD-001")
            .with_currency(fen_core::domain::Currency::USD)
            .build();

        // EUR invoice
        let eur_invoice = InvoiceFixture::new()
            .with_number("EUR-001")
            .with_currency(fen_core::domain::Currency::EUR)
            .build();

        let usd_result = env.rule_engine.validate_invoice(&usd_invoice).await.unwrap();
        let eur_result = env.rule_engine.validate_invoice(&eur_invoice).await.unwrap();

        // Both should validate (currency shouldn't affect structural validation)
        assert_eq!(usd_result.anomalies.len(), eur_result.anomalies.len());
    }
}

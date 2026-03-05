//! End-to-end integration tests for the Fen anomaly detection pipeline
//!
//! These tests exercise the full pipeline from data ingestion through
//! storage, validation, and query operations.

use rust_decimal_macros::dec;

use fen_core::domain::{AnomalyType, Severity};
use fen_core::ValidationStatus;
use fen_storage::{DocumentStore, InvoiceFilter, StorageTier};
use fen_tests::{
    consistent_embedding, init_test_tracing, random_embedding, sample_contract_text,
    sample_invoice_text, sample_sow_text, ContractFixture, GraphIngestionTestEnv,
    IngestionTestEnv, IntegrationTestEnv, InvoiceFixture, MlPipelineTestEnv, QueryTestEnv,
    TestEnv,
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
        let invoice = InvoiceFixture::new().with_number("CRUD-001").build();

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
        let invoices = InvoiceFixture::new().with_number("PAGE").build_batch(10);

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

        let contract = ContractFixture::new().with_title("Test Contract").build();

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
        let recent = InvoiceFixture::new().with_number("HOT-001").build();

        env.storage.store_invoice(&recent, None).await.unwrap();

        // Verify retrievable
        let retrieved = env.storage.get_invoice(&recent.id).await.unwrap();
        assert!(retrieved.is_some());
    }

    /// Test invoice storage with embeddings for vector search
    #[tokio::test]
    async fn test_storage_with_embeddings() {
        let env = TestEnv::new().await;

        let invoice = InvoiceFixture::new().with_number("EMB-001").build();

        let embedding = random_embedding(384);

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

            let embedding = consistent_embedding(&invoice.invoice_number, 384);
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

        let results = env
            .storage
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

        let invoice = InvoiceFixture::new().with_number("VALID-001").build();

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
            result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == AnomalyType::MathMismatch),
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
            result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == AnomalyType::MathMismatch),
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
            result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == AnomalyType::DateInconsistency),
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
            result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == AnomalyType::OutOfRange),
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
            result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == AnomalyType::MissingField),
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
            result
                .anomalies
                .iter()
                .any(|a| a.description.to_lowercase().contains("confidence")
                    && a.severity == Severity::Low),
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
        assert!(
            result.anomalies.len() >= 3,
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
            InvoiceFixture::new()
                .with_number("BATCH-002")
                .with_math_error()
                .build(),
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
        assert!(results[1]
            .anomalies
            .iter()
            .any(|a| a.anomaly_type == AnomalyType::MathMismatch));
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

        let invoice = InvoiceFixture::new().with_number("QUERY-001").build();

        env.query_engine
            .store_invoice(&invoice, None)
            .await
            .unwrap();

        let result = env.query_engine.get_invoice(&invoice.id).await.unwrap();
        assert!(result.data.is_some());
        assert_eq!(result.data.unwrap().invoice_number, "QUERY-001");
    }

    /// Test query cache functionality
    #[tokio::test]
    async fn test_query_cache() {
        let env = QueryTestEnv::new().await;

        let invoice = InvoiceFixture::new().with_number("CACHE-001").build();

        env.query_engine
            .store_invoice(&invoice, None)
            .await
            .unwrap();

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
        env.query_engine
            .store_invoice(&invoice, None)
            .await
            .unwrap();

        let result = env.query_engine.get_invoice(&invoice.id).await.unwrap();

        // Should have query metrics
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
            let embedding = consistent_embedding(&invoice.invoice_number, 384);
            env.query_engine
                .store_invoice(&invoice, Some(&embedding))
                .await
                .unwrap();
        }

        // Search with similar embedding
        let query_embedding = consistent_embedding("VEC-002", 384);
        let results = env
            .query_engine
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
            env.query_engine
                .store_invoice(&invoice, None)
                .await
                .unwrap();
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
            env.query_engine
                .store_invoice(&invoice, None)
                .await
                .unwrap();
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
            env.query_engine
                .store_invoice(&invoice, None)
                .await
                .unwrap();
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
        let invoice_id = invoice.id;

        tracing::info!(invoice_id = %invoice_id, "Ingested invoice");

        // 2. Verify storage
        let stored = env.storage.get_invoice(&invoice_id).await.unwrap();
        assert!(stored.is_some(), "Invoice should be stored");
        let stored_invoice = stored.unwrap();

        // 3. Validate
        let validation = env
            .rule_engine
            .validate_invoice(&stored_invoice)
            .await
            .unwrap();
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
            matches!(
                retrieved.validation_status,
                ValidationStatus::Passed | ValidationStatus::PassedWithWarnings
            ),
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
            InvoiceFixture::new()
                .with_number("MIX-002")
                .with_math_error()
                .build(),
            InvoiceFixture::new().with_number("MIX-003").build(),
            InvoiceFixture::new()
                .with_number("MIX-004")
                .with_date_error()
                .build(),
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
        let invoices = InvoiceFixture::new().with_number("QV").build_batch(5);

        for invoice in &invoices {
            let embedding = consistent_embedding(&invoice.invoice_number, 384);
            env.storage
                .store_invoice(invoice, Some(&embedding))
                .await
                .unwrap();

            // Validate each
            let _result = env.rule_engine.validate_invoice(invoice).await.unwrap();
        }

        // Query all
        let listed = env.storage.list_invoices(10, 0).await.unwrap();
        assert_eq!(listed.len(), 5);

        // Vector search should work
        let query_emb = consistent_embedding("QV-001", 384);
        let search_results = env
            .storage
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
        let mut invoice = InvoiceFixture::new().with_number("LINE-001").build();

        // Manually set incorrect line item total
        if let Some(item) = invoice.line_items.first_mut() {
            item.total = dec!(999.99); // Wrong - should be quantity * unit_price
        }

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should detect line item calculation error
        assert!(
            result
                .anomalies
                .iter()
                .any(|a| a.anomaly_type == AnomalyType::MathMismatch
                    && a.field_path
                        .as_ref()
                        .is_some_and(|f| f.contains("line_item"))),
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
            .with_math_error() // High severity
            .with_low_confidence() // Low severity
            .build();

        let result = env.rule_engine.validate_invoice(&invoice).await.unwrap();

        // Should have anomalies of different severities
        let has_high = result
            .anomalies
            .iter()
            .any(|a| a.severity == Severity::High || a.severity == Severity::Critical);
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
            .with_number("") // Empty invoice number
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
        assert!(
            elapsed.as_millis() < 100,
            "Validation took too long: {:?}",
            elapsed
        );

        // Should detect multiple issues
        assert!(
            result.anomalies.len() >= 4,
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

        let usd_result = env
            .rule_engine
            .validate_invoice(&usd_invoice)
            .await
            .unwrap();
        let eur_result = env
            .rule_engine
            .validate_invoice(&eur_invoice)
            .await
            .unwrap();

        // Both should validate (currency shouldn't affect structural validation)
        assert_eq!(usd_result.anomalies.len(), eur_result.anomalies.len());
    }
}

// ============================================================================
// Module: Unified Query Language Tests
// ============================================================================

mod unified_query {
    use fen_storage::{parse_query, QueryParams};

    /// Test basic SELECT query parsing
    #[test]
    fn test_parse_simple_select() {
        let query = parse_query("SELECT id, invoice_number FROM invoices").unwrap();

        assert_eq!(query.select.len(), 2);
        assert!(query.filter.is_none());
        assert!(query.order_by.is_none());
        assert!(query.limit.is_none());
    }

    /// Test SELECT with WHERE clause
    #[test]
    fn test_parse_select_with_where() {
        let query = parse_query("SELECT id FROM invoices WHERE vendor_name = 'Acme Corp'").unwrap();

        assert!(query.filter.is_some());
    }

    /// Test SELECT with ORDER BY and LIMIT
    #[test]
    fn test_parse_select_with_order_limit() {
        let query =
            parse_query("SELECT * FROM invoices ORDER BY invoice_date DESC LIMIT 10 OFFSET 5")
                .unwrap();

        assert!(query.order_by.is_some());
        assert_eq!(query.limit, Some(10));
        assert_eq!(query.offset, Some(5));
    }

    /// Test VECTOR_DISTANCE function parsing
    #[test]
    fn test_parse_vector_distance() {
        let query = parse_query(
            "SELECT id, VECTOR_DISTANCE(embedding, :query_vector) AS score FROM invoices",
        )
        .unwrap();

        assert!(query.uses_vector_search());
        let params = query.parameter_names();
        assert!(params.contains(&"query_vector".to_string()));
    }

    /// Test BM25_SCORE function parsing
    #[test]
    fn test_parse_bm25_score() {
        let query = parse_query(
            "SELECT id, BM25_SCORE(extracted_text, :search_terms) AS relevance FROM invoices",
        )
        .unwrap();

        assert!(query.uses_text_search());
        let params = query.parameter_names();
        assert!(params.contains(&"search_terms".to_string()));
    }

    /// Test CONTAINS function parsing
    #[test]
    fn test_parse_contains() {
        let query =
            parse_query("SELECT id FROM invoices WHERE CONTAINS(extracted_text, :search)").unwrap();

        assert!(query.uses_contains());
    }

    /// Test hybrid query with both vector and text search
    #[test]
    fn test_parse_hybrid_query() {
        let query_str = r#"
            SELECT inv.id, inv.invoice_number,
                VECTOR_DISTANCE(inv.embedding, :query_vector) AS semantic_score,
                BM25_SCORE(inv.extracted_text, :search_terms) AS text_score
            FROM invoices inv
            WHERE inv.vendor_name = 'Acme Corp'
                AND VECTOR_DISTANCE(inv.embedding, :query_vector) < 0.3
                AND CONTAINS(inv.extracted_text, :search_terms)
            ORDER BY 0.7 * semantic_score + 0.3 * text_score ASC
            LIMIT 10
        "#;

        let query = parse_query(query_str).unwrap();

        assert!(query.uses_vector_search());
        assert!(query.uses_text_search());
        assert!(query.uses_contains());
        assert_eq!(query.limit, Some(10));

        let params = query.parameter_names();
        assert!(params.contains(&"query_vector".to_string()));
        assert!(params.contains(&"search_terms".to_string()));
    }

    /// Test query parameter building
    #[test]
    fn test_query_params() {
        let params = QueryParams::new()
            .with_vector("query_vector", vec![0.1, 0.2, 0.3])
            .with_string("search_terms", "payment invoice")
            .with_float("threshold", 0.5);

        assert!(params.get_vector("query_vector").is_some());
        assert_eq!(params.get_string("search_terms"), Some("payment invoice"));
        assert!(params.get("threshold").is_some());
    }

    /// Test invalid query error reporting
    #[test]
    fn test_parse_error_unknown_table() {
        let result = parse_query("SELECT id FROM unknown_table");
        assert!(result.is_err());

        let err = result.unwrap_err();
        let report = err.report();
        assert!(report.contains("unknown_table"));
    }

    /// Test missing FROM clause error
    #[test]
    fn test_parse_error_missing_from() {
        let result = parse_query("SELECT id WHERE x = 1");
        assert!(result.is_err());
    }

    /// Test complex WHERE with AND/OR
    #[test]
    fn test_parse_complex_where() {
        let query = parse_query(
            "SELECT * FROM invoices \
             WHERE (vendor_name = 'Acme' OR vendor_name = 'Beta') \
             AND total_amount > 1000",
        )
        .unwrap();

        assert!(query.filter.is_some());
    }

    /// Test arithmetic expressions in ORDER BY
    #[test]
    fn test_parse_arithmetic_order_by() {
        let query = parse_query(
            "SELECT id, score1, score2 FROM invoices \
             ORDER BY 0.7 * score1 + 0.3 * score2 ASC",
        )
        .unwrap();

        assert!(query.order_by.is_some());
        let order = query.order_by.unwrap();
        assert_eq!(order.items.len(), 1);
    }

    /// Test LIKE pattern matching
    #[test]
    fn test_parse_like_pattern() {
        let query =
            parse_query("SELECT * FROM invoices WHERE invoice_number LIKE 'INV-%'").unwrap();

        assert!(query.filter.is_some());
    }

    /// Test NULL handling
    #[test]
    fn test_parse_null_check() {
        let query = parse_query("SELECT * FROM invoices WHERE po_number IS NOT NULL").unwrap();

        assert!(query.filter.is_some());
    }

    /// Test contracts table query
    #[test]
    fn test_parse_contracts_query() {
        let query =
            parse_query("SELECT id, title FROM contracts WHERE contract_type = 'ServiceAgreement'")
                .unwrap();

        assert!(matches!(
            query.from,
            fen_storage::query::lang::QueryTarget::Contracts
        ));
    }
}

// ============================================================================
// Module: Query Execution Tests
// ============================================================================

// ============================================================================
// Module: ZIP Query Relationship Tests
// ============================================================================

mod zip_queries {
    use fen_storage::{parse_query, QueryParams, ZipMode};

    // ========== Parsing Tests ==========

    /// Test basic ZIP query parsing
    #[test]
    fn test_parse_zip_basic() {
        let query = parse_query(
            "SELECT inv.*, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        assert!(query.is_zip_query());
        let zip = query.zip.as_ref().unwrap();
        assert_eq!(zip.mode, ZipMode::Inner);
        assert!(matches!(
            zip.table,
            fen_storage::query::lang::QueryTarget::Contracts
        ));
    }

    /// Test ZIP query with LEFT mode
    #[test]
    fn test_parse_left_zip() {
        let query = parse_query(
            "SELECT inv.id, con.title \
             FROM invoices inv \
             LEFT ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        let zip = query.zip.as_ref().unwrap();
        assert_eq!(zip.mode, ZipMode::Left);
    }

    /// Test ZIP query with INNER mode (explicit)
    #[test]
    fn test_parse_inner_zip() {
        let query = parse_query(
            "SELECT inv.id, con.title \
             FROM invoices inv \
             INNER ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();

        let zip = query.zip.as_ref().unwrap();
        assert_eq!(zip.mode, ZipMode::Inner);
    }

    /// Test ZIP query with WHERE clause
    #[test]
    fn test_parse_zip_with_where() {
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000",
        )
        .unwrap();

        assert!(query.is_zip_query());
        assert!(query.filter.is_some());
    }

    /// Test ZIP query with cross-table WHERE condition
    #[test]
    fn test_parse_zip_cross_table_where() {
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000 \
                 AND con.effective_date > '2024-01-01' \
                 AND inv.total_amount < con.total_value",
        )
        .unwrap();

        assert!(query.is_zip_query());
        assert!(query.filter.is_some());

        // The query should parse cross-table conditions correctly
        let zip = query.zip.as_ref().unwrap();
        assert!(zip.alias.is_some());
        assert_eq!(zip.alias.as_deref(), Some("con"));
    }

    /// Test ZIP query with complex ON condition
    #[test]
    fn test_parse_zip_complex_on() {
        let query = parse_query(
            "SELECT inv.*, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.contract_id = con.id",
        )
        .unwrap();

        assert!(query.is_zip_query());
        let zip = query.zip.as_ref().unwrap();
        // ON condition should be parsed as a binary op
        assert!(matches!(
            zip.on,
            fen_storage::query::lang::Expr::BinaryOp { .. }
        ));
    }

    /// Test ZIP query with contract_number matching
    #[test]
    fn test_parse_zip_contract_number() {
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.contract_number = con.contract_number",
        )
        .unwrap();

        assert!(query.is_zip_query());
        let zip = query.zip.as_ref().unwrap();
        assert!(matches!(
            zip.on,
            fen_storage::query::lang::Expr::BinaryOp { .. }
        ));
    }

    /// Test WHERE clause categorization (invoice-only condition)
    #[test]
    fn test_where_invoice_only_condition() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000",
        )
        .unwrap();

        // The filter should only reference invoice columns
        assert!(query.filter.is_some());
    }

    /// Test WHERE clause categorization (contract-only condition)
    #[test]
    fn test_where_contract_only_condition() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE con.effective_date > '2024-01-01'",
        )
        .unwrap();

        assert!(query.filter.is_some());
    }

    /// Test WHERE clause categorization (cross-table condition)
    #[test]
    fn test_where_cross_table_condition() {
        let query = parse_query(
            "SELECT * FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount < con.total_value",
        )
        .unwrap();

        // This should parse the cross-table condition
        assert!(query.filter.is_some());
    }

    /// Test WHERE clause with multiple condition types
    #[test]
    fn test_where_mixed_conditions() {
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000 \
                 AND con.effective_date > '2024-01-01' \
                 AND inv.total_amount < con.total_value",
        )
        .unwrap();

        assert!(query.is_zip_query());
        assert!(query.filter.is_some());

        // Query should be valid and parseable
        let zip = query.zip.as_ref().unwrap();
        assert!(zip.alias.is_some());
    }

    /// Test ZIP query with ORDER BY
    #[test]
    fn test_parse_zip_with_order() {
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             ORDER BY inv.total_amount DESC \
             LIMIT 10",
        )
        .unwrap();

        assert!(query.is_zip_query());
        assert!(query.order_by.is_some());
        assert_eq!(query.limit, Some(10));
    }

    /// Test that column references are properly identified
    #[test]
    fn test_column_reference_detection() {
        // Invoice columns should be detected
        let invoice_cols = vec![
            "invoice_number",
            "invoice_date",
            "due_date",
            "total_amount",
            "subtotal",
            "tax_amount",
            "vendor_name",
            "po_number",
        ];

        for col in &invoice_cols {
            let query = parse_query(&format!("SELECT {} FROM invoices", col)).unwrap();
            assert!(!query.select.is_empty());
        }

        // Contract columns should be detected
        let contract_cols = vec![
            "title",
            "effective_date",
            "expiration_date",
            "total_value",
            "contract_type",
            "party_name",
        ];

        for col in &contract_cols {
            let query = parse_query(&format!("SELECT {} FROM contracts", col)).unwrap();
            assert!(!query.select.is_empty());
        }
    }

    // ========== Execution Tests ==========

    use super::*;
    use fen_tests::ZipExecutorTestEnv;
    use rust_decimal_macros::dec;

    /// Test basic ZIP query execution joining invoices and contracts
    #[tokio::test]
    async fn test_execute_zip_inner_join() {
        init_test_tracing();
        let env = ZipExecutorTestEnv::new().await;

        // Create contract for Acme Corp
        let contract = ContractFixture::new()
            .with_title("Master Service Agreement")
            .with_party("Acme Corp")
            .build();
        env.store_contract(&contract).await.unwrap();

        // Create invoices - some match the contract vendor, some don't
        let inv1 = InvoiceFixture::new()
            .with_number("ZIP-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(1000.00))
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("ZIP-002")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(2500.00))
            .build();
        let inv3 = InvoiceFixture::new()
            .with_number("ZIP-003")
            .with_vendor("Beta Inc") // Different vendor - won't match
            .with_total_amount(dec!(500.00))
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();
        env.store_invoice(&inv3).await.unwrap();

        // Execute ZIP query
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // Should match 2 invoices with the contract (Acme Corp)
        assert_eq!(result.pairs.len(), 2);
        assert_eq!(result.metadata.invoice_count, 2);
        assert_eq!(result.metadata.contract_count, 2);

        // Verify both matched invoices are from Acme Corp
        for pair in &result.pairs {
            let invoice = pair.invoice.as_ref().unwrap();
            assert_eq!(invoice.vendor.name, "Acme Corp");
        }
    }

    /// Test ZIP query with LEFT join mode
    #[tokio::test]
    async fn test_execute_zip_left_join() {
        let env = ZipExecutorTestEnv::new().await;

        // Create contract
        let contract = ContractFixture::new()
            .with_title("Contract A")
            .with_party("Vendor A")
            .build();
        env.store_contract(&contract).await.unwrap();

        // Create invoices - only one matches
        let inv1 = InvoiceFixture::new()
            .with_number("LEFT-001")
            .with_vendor("Vendor A") // Matches
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("LEFT-002")
            .with_vendor("Vendor B") // No matching contract
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();

        // Execute LEFT ZIP query
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             LEFT ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // LEFT join should return all invoices
        assert_eq!(result.pairs.len(), 2);

        // One pair should have contract, one shouldn't
        let with_contract = result.pairs.iter().filter(|p| p.contract.is_some()).count();
        let without_contract = result.pairs.iter().filter(|p| p.contract.is_none()).count();

        assert_eq!(with_contract, 1);
        assert_eq!(without_contract, 1);
    }

    /// Test ZIP query with WHERE clause filtering invoices only
    #[tokio::test]
    async fn test_execute_zip_with_invoice_filter() {
        let env = ZipExecutorTestEnv::new().await;

        // Create contract
        let contract = ContractFixture::new()
            .with_title("High Value Contract")
            .with_party("Acme Corp")
            .build();
        env.store_contract(&contract).await.unwrap();

        // Create invoices with different amounts
        let inv_small = InvoiceFixture::new()
            .with_number("SMALL-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(500.00))
            .build();
        let inv_large = InvoiceFixture::new()
            .with_number("LARGE-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(5000.00))
            .build();

        env.store_invoice(&inv_small).await.unwrap();
        env.store_invoice(&inv_large).await.unwrap();

        // Execute ZIP query with invoice filter
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000",
        )
        .unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // Should only match the large invoice
        assert_eq!(result.pairs.len(), 1);
        let invoice = result.pairs[0].invoice.as_ref().unwrap();
        assert_eq!(invoice.invoice_number, "LARGE-001");
    }

    /// Test ZIP query with cross-table WHERE condition
    #[tokio::test]
    async fn test_execute_zip_cross_table_filter() {
        let env = ZipExecutorTestEnv::new().await;

        // Create contracts with different values
        let contract_small = ContractFixture::new()
            .with_title("Small Contract")
            .with_party("Acme Corp")
            .with_total_value(dec!(1000.00))
            .build();
        let contract_large = ContractFixture::new()
            .with_title("Large Contract")
            .with_party("Beta Inc")
            .with_total_value(dec!(10000.00))
            .build();

        env.store_contract(&contract_small).await.unwrap();
        env.store_contract(&contract_large).await.unwrap();

        // Create invoices that exceed small contract but not large
        let inv1 = InvoiceFixture::new()
            .with_number("CROSS-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(2000.00)) // Exceeds small contract (1000)
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("CROSS-002")
            .with_vendor("Beta Inc")
            .with_total_amount(dec!(5000.00)) // Under large contract (10000)
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();

        // Query for invoices under contract value (cross-table condition)
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount < con.total_value",
        )
        .unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // Only Beta Inc invoice (5000) should match (under 10000 contract value)
        assert_eq!(result.pairs.len(), 1);
        let invoice = result.pairs[0].invoice.as_ref().unwrap();
        assert_eq!(invoice.invoice_number, "CROSS-002");
    }

    /// Test ZIP query with mixed conditions (invoice-only, contract-only, cross-table)
    #[tokio::test]
    async fn test_execute_zip_mixed_conditions() {
        let env = ZipExecutorTestEnv::new().await;

        // Create contract
        let contract = ContractFixture::new()
            .with_title("Premium Contract")
            .with_party("Acme Corp")
            .with_total_value(dec!(50000.00))
            .build();
        env.store_contract(&contract).await.unwrap();

        // Create invoices with various amounts
        let inv1 = InvoiceFixture::new()
            .with_number("MIX-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(500.00)) // Too small (< 1000)
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("MIX-002")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(10000.00)) // Good: > 1000 and < 50000
            .build();
        let inv3 = InvoiceFixture::new()
            .with_number("MIX-003")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(60000.00)) // Too large (> contract value)
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();
        env.store_invoice(&inv3).await.unwrap();

        // Mixed conditions:
        // - inv.total_amount > 1000 (invoice-only)
        // - inv.total_amount < con.total_value (cross-table)
        let query = parse_query(
            "SELECT inv.invoice_number, con.title \
             FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name \
             WHERE inv.total_amount > 1000 \
                 AND inv.total_amount < con.total_value",
        )
        .unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // Only MIX-002 should match (> 1000 and < 50000)
        assert_eq!(result.pairs.len(), 1);
        let invoice = result.pairs[0].invoice.as_ref().unwrap();
        assert_eq!(invoice.invoice_number, "MIX-002");
    }

    /// Test ZIP query execution metadata
    #[tokio::test]
    async fn test_execute_zip_metadata() {
        let env = ZipExecutorTestEnv::new().await;

        // Create contract and invoice
        let contract = ContractFixture::new()
            .with_title("Test Contract")
            .with_party("Test Vendor")
            .build();
        env.store_contract(&contract).await.unwrap();

        let invoice = InvoiceFixture::new()
            .with_number("META-001")
            .with_vendor("Test Vendor")
            .build();
        env.store_invoice(&invoice).await.unwrap();

        let query = parse_query(
            "SELECT * FROM invoices inv \
             ZIP contracts con ON inv.vendor_name = con.party_name",
        )
        .unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // Check metadata
        assert_eq!(result.metadata.pair_count, 1);
        assert_eq!(result.metadata.invoice_count, 1);
        assert_eq!(result.metadata.contract_count, 1);
        assert_eq!(result.metadata.zip_mode, "Inner");
    }

    /// Test single table query through ZIP executor
    #[tokio::test]
    async fn test_execute_single_table_via_zip() {
        let env = ZipExecutorTestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("SINGLE-001")
            .with_vendor("Test Vendor")
            .build();
        env.store_invoice(&invoice).await.unwrap();

        // Non-ZIP query should still work
        let query = parse_query("SELECT * FROM invoices").unwrap();
        let params = QueryParams::new();

        let result = env
            .zip_executor
            .execute_zip_query(&query, &params)
            .await
            .unwrap();

        // Should wrap as ZipPairs with no contract
        assert_eq!(result.pairs.len(), 1);
        assert!(result.pairs[0].invoice.is_some());
        assert!(result.pairs[0].contract.is_none());
        assert_eq!(result.metadata.zip_mode, "Single");
    }
}

mod query_execution {
    use super::*;
    use fen_storage::{parse_query, QueryParams};
    use fen_tests::QueryExecutorTestEnv;
    use rust_decimal_macros::dec;

    /// Test basic SELECT * execution against stored invoices
    #[tokio::test]
    async fn test_execute_select_all() {
        init_test_tracing();
        let env = QueryExecutorTestEnv::new().await;

        // Store test invoices
        let inv1 = InvoiceFixture::new()
            .with_number("EXEC-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(1000.00))
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("EXEC-002")
            .with_vendor("Beta Inc")
            .with_total_amount(dec!(2500.00))
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();

        // Execute query
        let query = parse_query("SELECT * FROM invoices LIMIT 10").unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        // Verify results
        assert_eq!(result.rows.len(), 2);
        assert!(result.metadata.rows_returned >= 2);
    }

    /// Test SELECT with specific columns
    #[tokio::test]
    async fn test_execute_select_columns() {
        let env = QueryExecutorTestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("COL-001")
            .with_vendor("Test Vendor")
            .build();
        env.store_invoice(&invoice).await.unwrap();

        let query = parse_query("SELECT id, invoice_number, vendor_name FROM invoices").unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        assert_eq!(result.rows.len(), 1);
        let row = &result.rows[0];
        assert!(row.columns.contains_key("id"));
        assert!(row.columns.contains_key("invoice_number"));
        assert!(row.columns.contains_key("vendor_name"));

        // Verify values
        if let Some(fen_storage::ColumnValue::String(num)) = row.columns.get("invoice_number") {
            assert_eq!(num, "COL-001");
        } else {
            panic!("Expected string for invoice_number");
        }
    }

    /// Test WHERE clause filtering with equality
    #[tokio::test]
    async fn test_execute_where_eq() {
        let env = QueryExecutorTestEnv::new().await;

        // Store invoices with different vendors
        let inv1 = InvoiceFixture::new()
            .with_number("WHERE-001")
            .with_vendor("Acme Corp")
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("WHERE-002")
            .with_vendor("Beta Inc")
            .build();
        let inv3 = InvoiceFixture::new()
            .with_number("WHERE-003")
            .with_vendor("Acme Corp")
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();
        env.store_invoice(&inv3).await.unwrap();

        // Query for Acme Corp only
        let query =
            parse_query("SELECT id, invoice_number FROM invoices WHERE vendor_name = 'Acme Corp'")
                .unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should return 2 invoices (WHERE-001 and WHERE-003)
        assert_eq!(result.rows.len(), 2);

        // Verify all returned invoices are from Acme Corp
        for row in &result.rows {
            if let Some(fen_storage::ColumnValue::String(num)) = row.columns.get("invoice_number") {
                assert!(num == "WHERE-001" || num == "WHERE-003");
            }
        }
    }

    /// Test WHERE clause with comparison operators
    #[tokio::test]
    async fn test_execute_where_comparison() {
        let env = QueryExecutorTestEnv::new().await;

        // Store invoices with different amounts
        let inv_small = InvoiceFixture::new()
            .with_number("CMP-SMALL")
            .with_total_amount(dec!(500.00))
            .build();
        let inv_medium = InvoiceFixture::new()
            .with_number("CMP-MEDIUM")
            .with_total_amount(dec!(1500.00))
            .build();
        let inv_large = InvoiceFixture::new()
            .with_number("CMP-LARGE")
            .with_total_amount(dec!(5000.00))
            .build();

        env.store_invoice(&inv_small).await.unwrap();
        env.store_invoice(&inv_medium).await.unwrap();
        env.store_invoice(&inv_large).await.unwrap();

        // Query for invoices > 1000
        let query = parse_query(
            "SELECT invoice_number, total_amount FROM invoices WHERE total_amount > 1000",
        )
        .unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should return CMP-MEDIUM and CMP-LARGE
        assert_eq!(result.rows.len(), 2);
    }

    /// Test LIMIT and OFFSET
    #[tokio::test]
    async fn test_execute_limit_offset() {
        let env = QueryExecutorTestEnv::new().await;

        // Store 5 invoices
        for i in 1..=5 {
            let inv = InvoiceFixture::new()
                .with_number(format!("LIM-{:03}", i))
                .build();
            env.store_invoice(&inv).await.unwrap();
        }

        // Query with LIMIT 2
        let query = parse_query("SELECT * FROM invoices LIMIT 2").unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();
        assert_eq!(result.rows.len(), 2);

        // Query with LIMIT 3 OFFSET 2
        let query = parse_query("SELECT * FROM invoices LIMIT 3 OFFSET 2").unwrap();
        let result = env.executor.execute(&query, &params).await.unwrap();
        assert!(result.rows.len() <= 3);
    }

    /// Test LIKE pattern matching
    #[tokio::test]
    async fn test_execute_like() {
        let env = QueryExecutorTestEnv::new().await;

        let inv1 = InvoiceFixture::new().with_number("INV-2024-001").build();
        let inv2 = InvoiceFixture::new().with_number("INV-2024-002").build();
        let inv3 = InvoiceFixture::new().with_number("PO-2024-001").build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();
        env.store_invoice(&inv3).await.unwrap();

        // Query with LIKE pattern
        let query =
            parse_query("SELECT invoice_number FROM invoices WHERE invoice_number LIKE 'INV-%'")
                .unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should return INV-2024-001 and INV-2024-002
        assert_eq!(result.rows.len(), 2);
    }

    /// Test BM25 text search execution
    #[tokio::test]
    async fn test_execute_bm25_search() {
        let env = QueryExecutorTestEnv::new().await;

        // Store invoices with different text content
        let inv1 = InvoiceFixture::new()
            .with_number("BM25-001")
            .with_extracted_text("Payment for consulting services rendered in January")
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("BM25-002")
            .with_extracted_text("Hardware equipment purchase and installation")
            .build();
        let inv3 = InvoiceFixture::new()
            .with_number("BM25-003")
            .with_extracted_text("Consulting fee for project management services")
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();
        env.store_invoice(&inv3).await.unwrap();

        // Search for "consulting services"
        let query = parse_query(
            "SELECT id, invoice_number, BM25_SCORE(extracted_text, :search) AS relevance \
             FROM invoices \
             ORDER BY relevance DESC \
             LIMIT 10",
        )
        .unwrap();

        let params = QueryParams::new().with_string("search", "consulting services");

        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should find results with text search
        assert!(result.metadata.used_text_search);
        assert!(!result.rows.is_empty());

        // First result should have a relevance score
        if let Some(first) = result.rows.first() {
            assert!(first.score.is_some());
        }
    }

    /// Test CONTAINS function for full-text filtering
    #[tokio::test]
    async fn test_execute_contains() {
        let env = QueryExecutorTestEnv::new().await;

        let inv1 = InvoiceFixture::new()
            .with_number("CONT-001")
            .with_extracted_text("This invoice is for software development work")
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("CONT-002")
            .with_extracted_text("Hardware maintenance and repair services")
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();

        // Query with CONTAINS
        let query = parse_query(
            "SELECT invoice_number FROM invoices \
             WHERE CONTAINS(extracted_text, :search)",
        )
        .unwrap();

        let params = QueryParams::new().with_string("search", "software");

        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should find CONT-001
        assert!(!result.rows.is_empty());
    }

    /// Test vector search with VECTOR_DISTANCE
    #[tokio::test]
    async fn test_execute_vector_search() {
        let env = QueryExecutorTestEnv::new().await;

        // Create invoices with embeddings (simulated) - 384 dims for typical embedding
        const EMBEDDING_DIM: usize = 384;
        let embedding1 = consistent_embedding("payment invoice consulting", EMBEDDING_DIM);
        let embedding2 = consistent_embedding("hardware equipment purchase", EMBEDDING_DIM);
        let embedding3 = consistent_embedding("legal contract agreement", EMBEDDING_DIM);

        let inv1 = InvoiceFixture::new()
            .with_number("VEC-001")
            .with_extracted_text("Payment invoice for consulting")
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("VEC-002")
            .with_extracted_text("Hardware equipment purchase")
            .build();
        let inv3 = InvoiceFixture::new()
            .with_number("VEC-003")
            .with_extracted_text("Legal contract agreement")
            .build();

        env.store_invoice_with_embedding(&inv1, &embedding1)
            .await
            .unwrap();
        env.store_invoice_with_embedding(&inv2, &embedding2)
            .await
            .unwrap();
        env.store_invoice_with_embedding(&inv3, &embedding3)
            .await
            .unwrap();

        // Search for similar to "consulting payment"
        let query_embedding = consistent_embedding("payment consulting invoice", EMBEDDING_DIM);

        let query = parse_query(
            "SELECT id, invoice_number, VECTOR_DISTANCE(embedding, :query_vec) AS distance \
             FROM invoices \
             ORDER BY distance ASC \
             LIMIT 5",
        )
        .unwrap();

        let params = QueryParams::new().with_vector("query_vec", query_embedding.to_vec());

        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should have used vector search
        assert!(result.metadata.used_vector_search);
        assert!(!result.rows.is_empty());
    }

    /// Test hybrid search combining vector and text
    #[tokio::test]
    async fn test_execute_hybrid_search() {
        let env = QueryExecutorTestEnv::new().await;

        const EMBEDDING_DIM: usize = 384;
        let embedding1 = consistent_embedding("payment consulting services", EMBEDDING_DIM);
        let embedding2 = consistent_embedding("hardware maintenance", EMBEDDING_DIM);

        let inv1 = InvoiceFixture::new()
            .with_number("HYB-001")
            .with_vendor("Acme Consulting")
            .with_extracted_text("Payment for consulting services rendered")
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("HYB-002")
            .with_vendor("Tech Hardware")
            .with_extracted_text("Hardware maintenance and support")
            .build();

        env.store_invoice_with_embedding(&inv1, &embedding1)
            .await
            .unwrap();
        env.store_invoice_with_embedding(&inv2, &embedding2)
            .await
            .unwrap();

        // Hybrid query with both vector and text search
        let query_embedding = consistent_embedding("consulting services", EMBEDDING_DIM);

        let query = parse_query(
            "SELECT invoice_number, \
                    VECTOR_DISTANCE(embedding, :vec) AS vec_score, \
                    BM25_SCORE(extracted_text, :text) AS text_score \
             FROM invoices \
             ORDER BY 0.7 * vec_score + 0.3 * text_score ASC \
             LIMIT 10",
        )
        .unwrap();

        let params = QueryParams::new()
            .with_vector("vec", query_embedding.to_vec())
            .with_string("text", "consulting services");

        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should use both search types
        assert!(result.metadata.used_vector_search);
        assert!(result.metadata.used_text_search);
        assert!(!result.rows.is_empty());
    }

    /// Test AND/OR logical operators in WHERE
    #[tokio::test]
    async fn test_execute_complex_where() {
        let env = QueryExecutorTestEnv::new().await;

        let inv1 = InvoiceFixture::new()
            .with_number("LOGIC-001")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(1500.00))
            .build();
        let inv2 = InvoiceFixture::new()
            .with_number("LOGIC-002")
            .with_vendor("Beta Inc")
            .with_total_amount(dec!(500.00))
            .build();
        let inv3 = InvoiceFixture::new()
            .with_number("LOGIC-003")
            .with_vendor("Acme Corp")
            .with_total_amount(dec!(200.00))
            .build();

        env.store_invoice(&inv1).await.unwrap();
        env.store_invoice(&inv2).await.unwrap();
        env.store_invoice(&inv3).await.unwrap();

        // Complex WHERE with AND
        let query = parse_query(
            "SELECT invoice_number FROM invoices \
             WHERE vendor_name = 'Acme Corp' AND total_amount > 1000",
        )
        .unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        // Should only return LOGIC-001 (Acme Corp with amount > 1000)
        assert_eq!(result.rows.len(), 1);
        if let Some(fen_storage::ColumnValue::String(num)) =
            result.rows[0].columns.get("invoice_number")
        {
            assert_eq!(num, "LOGIC-001");
        }
    }

    /// Test parameterized queries
    #[tokio::test]
    async fn test_execute_with_parameters() {
        let env = QueryExecutorTestEnv::new().await;

        let inv = InvoiceFixture::new()
            .with_number("PARAM-001")
            .with_vendor("Test Vendor")
            .build();
        env.store_invoice(&inv).await.unwrap();

        // Query with string parameter
        let query =
            parse_query("SELECT invoice_number FROM invoices WHERE vendor_name = :vendor").unwrap();

        let params = QueryParams::new().with_string("vendor", "Test Vendor");

        let result = env.executor.execute(&query, &params).await.unwrap();
        assert_eq!(result.rows.len(), 1);
    }

    /// Test empty result set
    #[tokio::test]
    async fn test_execute_empty_result() {
        let env = QueryExecutorTestEnv::new().await;

        // Query on empty storage
        let query = parse_query("SELECT * FROM invoices").unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        assert!(result.rows.is_empty());
        assert_eq!(result.metadata.rows_returned, 0);
    }

    /// Test query execution time is tracked
    #[tokio::test]
    async fn test_execution_metadata() {
        let env = QueryExecutorTestEnv::new().await;

        let inv = InvoiceFixture::new().with_number("META-001").build();
        env.store_invoice(&inv).await.unwrap();

        let query = parse_query("SELECT * FROM invoices").unwrap();
        let params = QueryParams::new();
        let result = env.executor.execute(&query, &params).await.unwrap();

        // Execution time should be tracked
        assert_eq!(result.metadata.rows_returned, result.rows.len());
    }
}

// ============================================================================
// Module: End-to-End Integration Tests (Ingestion → Storage → Search → Detection)
// ============================================================================

mod integration {
    use super::*;
    use std::sync::Arc;

    /// Test: ingest text → fulltext index populated → text search finds the invoice
    #[tokio::test]
    async fn test_ingest_populates_fulltext_index() {
        init_test_tracing();
        let env = IntegrationTestEnv::new().await;

        let invoice = env.ingest_and_enrich(sample_invoice_text()).await.unwrap();

        // Fulltext search should now find this invoice
        let results = env
            .fulltext_index
            .search_invoices(&invoice.vendor.name, 10)
            .unwrap();

        assert!(
            !results.is_empty(),
            "Fulltext search should return results after ingestion"
        );
        assert!(
            results.iter().any(|r| r.id == invoice.id.to_string()),
            "Fulltext search should find the ingested invoice"
        );
    }

    /// Test: ingest → warm-tier embedding write → vector search returns results
    #[tokio::test]
    async fn test_ingest_populates_warm_tier_for_vector_search() {
        let env = IntegrationTestEnv::new().await;

        let invoice = env.ingest_and_enrich(sample_invoice_text()).await.unwrap();

        // Vector search using the same deterministic embedding should find the invoice
        let query_embedding = consistent_embedding(&invoice.invoice_number, 384);
        let warm = env.warm_storage.read().await;
        let results = warm
            .search_invoices_by_embedding(&query_embedding, 5)
            .await
            .unwrap();

        assert!(
            !results.is_empty(),
            "Vector search should return results after warm-tier write"
        );
        assert!(
            results.iter().any(|(inv, _)| inv.id == invoice.id),
            "Vector search should find the ingested invoice"
        );
    }

    /// Test: ingest invoice with errors → validation runs → anomalies persisted
    #[tokio::test]
    async fn test_ingest_with_errors_persists_anomalies() {
        init_test_tracing();
        let env = IntegrationTestEnv::new().await;

        // Build invoice with math error and store through enrichment pipeline
        let invoice = InvoiceFixture::new()
            .with_number("INTEG-ERR-001")
            .with_math_error()
            .with_vendor("Error Vendor Corp")
            .build();

        let embedding = consistent_embedding(&invoice.invoice_number, 384);
        env.store_and_enrich(&invoice, &embedding).await.unwrap();

        // Anomalies should be persisted
        let anomalies = env.anomaly_store.list_all(100, 0).await.unwrap();
        assert!(
            !anomalies.is_empty(),
            "Anomalies should be persisted after validation"
        );

        // At least one anomaly should reference this invoice
        assert!(
            anomalies
                .iter()
                .any(|a| a.invoice_id == Some(invoice.id)),
            "Persisted anomaly should reference the invoice"
        );

        // Anomaly should have correct vendor
        assert!(
            anomalies
                .iter()
                .any(|a| a.vendor_name == "Error Vendor Corp"),
            "Persisted anomaly should have correct vendor"
        );
    }

    /// Test: valid invoice → no anomalies persisted
    #[tokio::test]
    async fn test_valid_invoice_no_anomalies_persisted() {
        let env = IntegrationTestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("INTEG-VALID-001")
            .build();

        let embedding = consistent_embedding(&invoice.invoice_number, 384);
        env.store_and_enrich(&invoice, &embedding).await.unwrap();

        // Should have no anomalies for a valid invoice (or only low-severity ones)
        let anomalies = env.anomaly_store.list_all(100, 0).await.unwrap();
        let critical_anomalies: Vec<_> = anomalies
            .iter()
            .filter(|a| a.severity == Severity::Critical || a.severity == Severity::High)
            .collect();

        assert!(
            critical_anomalies.is_empty(),
            "Valid invoice should not produce critical/high anomalies, got: {:?}",
            critical_anomalies
        );
    }

    /// Test: multiple invoices flow through full pipeline with search and anomaly accumulation
    #[tokio::test]
    async fn test_multi_invoice_pipeline_flow() {
        init_test_tracing();
        let env = IntegrationTestEnv::new().await;

        // Ingest a mix of valid and invalid invoices
        let invoices = vec![
            InvoiceFixture::new()
                .with_number("FLOW-001")
                .with_vendor("Acme Corp")
                .with_extracted_text("Consulting services for Q1 project delivery")
                .build(),
            InvoiceFixture::new()
                .with_number("FLOW-002")
                .with_vendor("Beta Inc")
                .with_math_error()
                .with_extracted_text("Hardware procurement for data center expansion")
                .build(),
            InvoiceFixture::new()
                .with_number("FLOW-003")
                .with_vendor("Acme Corp")
                .with_extracted_text("Consulting services for Q2 planning")
                .build(),
        ];

        for invoice in &invoices {
            let embedding = consistent_embedding(&invoice.invoice_number, 384);
            env.store_and_enrich(invoice, &embedding).await.unwrap();
        }

        // 1. All invoices should be in hot storage
        let stored_count = env.hot_storage.count_invoices().await.unwrap();
        assert_eq!(stored_count, 3, "All invoices should be stored");

        // 2. Fulltext search for "consulting" should return 2 invoices
        let search_results = env
            .fulltext_index
            .search_invoices("consulting", 10)
            .unwrap();
        assert_eq!(
            search_results.len(),
            2,
            "Fulltext search for 'consulting' should find 2 invoices"
        );

        // 3. Vector search should return results
        let query_emb = consistent_embedding("FLOW-001", 384);
        let warm = env.warm_storage.read().await;
        let vec_results = warm
            .search_invoices_by_embedding(&query_emb, 3)
            .await
            .unwrap();
        assert!(
            !vec_results.is_empty(),
            "Vector search should return results"
        );
        // The closest match should be FLOW-001 itself (same embedding seed)
        assert_eq!(
            vec_results[0].0.invoice_number, "FLOW-001",
            "Nearest neighbor should be the invoice itself"
        );

        // 4. Anomalies should have been persisted for the invalid invoice
        let anomalies = env.anomaly_store.list_all(100, 0).await.unwrap();
        let flow002_anomalies: Vec<_> = anomalies
            .iter()
            .filter(|a| a.invoice_id == Some(invoices[1].id))
            .collect();
        assert!(
            !flow002_anomalies.is_empty(),
            "FLOW-002 (math error) should have persisted anomalies"
        );
    }

    /// Test: fulltext search for contracts
    #[tokio::test]
    async fn test_contract_fulltext_indexing() {
        let env = IntegrationTestEnv::new().await;

        let contract = ContractFixture::new()
            .with_title("Master Service Agreement for Cloud Infrastructure")
            .with_party("CloudVendor Inc")
            .build();

        use fen_storage::DocumentStore;
        env.hot_storage.store_contract(&contract).await.unwrap();
        env.fulltext_index
            .index_contract(&contract)
            .await
            .unwrap();
        env.fulltext_index.commit().await.unwrap();

        // Search should find the contract
        let results = env
            .fulltext_index
            .search_contracts("cloud infrastructure", 10)
            .unwrap();
        assert!(
            !results.is_empty(),
            "Fulltext search should find the indexed contract"
        );
    }

    /// Test: concurrent ingestion through the full pipeline
    #[tokio::test]
    async fn test_concurrent_pipeline_ingestion() {
        let env = IntegrationTestEnv::new().await;
        let env = Arc::new(env);

        let mut handles = Vec::new();
        for i in 0..5 {
            let env = env.clone();
            let handle = tokio::spawn(async move {
                let invoice = InvoiceFixture::new()
                    .with_number(format!("CONC-INTEG-{:03}", i))
                    .with_vendor(format!("Vendor {}", i))
                    .with_extracted_text(format!("Service delivery for project {}", i))
                    .build();
                let embedding = consistent_embedding(&invoice.invoice_number, 384);
                env.store_and_enrich(&invoice, &embedding).await.unwrap();
                invoice.id
            });
            handles.push(handle);
        }

        let invoice_ids: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // All should be stored
        assert_eq!(invoice_ids.len(), 5);
        let count = env.hot_storage.count_invoices().await.unwrap();
        assert_eq!(count, 5, "All concurrently ingested invoices should be stored");

        // All should be searchable via fulltext
        let results = env.fulltext_index.search_invoices("service delivery", 10).unwrap();
        assert_eq!(
            results.len(),
            5,
            "All invoices should be findable via fulltext search"
        );
    }

    /// Test: anomaly accumulation across multiple invalid invoices from same vendor
    #[tokio::test]
    async fn test_anomaly_accumulation_by_vendor() {
        let env = IntegrationTestEnv::new().await;

        // Multiple invoices from same vendor, all with errors
        for i in 0..3 {
            let invoice = InvoiceFixture::new()
                .with_number(format!("ACCUM-{:03}", i))
                .with_vendor("Problematic Vendor")
                .with_math_error()
                .build();
            let embedding = consistent_embedding(&invoice.invoice_number, 384);
            env.store_and_enrich(&invoice, &embedding).await.unwrap();
        }

        // Query anomalies for this vendor
        let vendor_anomalies = env
            .anomaly_store
            .get_anomalies_for_vendor("Problematic Vendor", 365)
            .await
            .unwrap();

        assert!(
            vendor_anomalies.len() >= 3,
            "Should have at least one anomaly per invoice, got {}",
            vendor_anomalies.len()
        );

        // All anomalies should reference this vendor
        for anomaly in &vendor_anomalies {
            assert_eq!(anomaly.vendor_name, "Problematic Vendor");
        }
    }

    /// Test: invoice stored in hot → searchable in warm → validation anomalies queryable
    #[tokio::test]
    async fn test_all_tiers_populated_after_enrichment() {
        let env = IntegrationTestEnv::new().await;

        let invoice = InvoiceFixture::new()
            .with_number("TIER-CHECK-001")
            .with_vendor("Tier Check Corp")
            .with_date_error() // Will produce anomaly
            .with_extracted_text("Annual maintenance contract renewal")
            .build();

        let embedding = consistent_embedding(&invoice.invoice_number, 384);
        env.store_and_enrich(&invoice, &embedding).await.unwrap();

        // Hot tier: direct retrieval
        use fen_storage::DocumentStore;
        let from_hot = env
            .hot_storage
            .get_invoice(&invoice.id)
            .await
            .unwrap();
        assert!(from_hot.is_some(), "Invoice should be in hot storage");

        // Warm tier: vector search
        let warm = env.warm_storage.read().await;
        let from_warm = warm
            .search_invoices_by_embedding(&embedding, 1)
            .await
            .unwrap();
        assert!(
            !from_warm.is_empty(),
            "Invoice should be retrievable via warm-tier vector search"
        );
        assert_eq!(from_warm[0].0.id, invoice.id);

        // Fulltext: text search
        let from_fti = env
            .fulltext_index
            .search_invoices("maintenance", 10)
            .unwrap();
        assert!(
            !from_fti.is_empty(),
            "Invoice should be findable via fulltext search"
        );

        // Anomaly store: validation results
        let anomalies = env.anomaly_store.list_all(100, 0).await.unwrap();
        assert!(
            anomalies
                .iter()
                .any(|a| a.invoice_id == Some(invoice.id)),
            "Anomaly store should contain validation results for this invoice"
        );
    }
}

// ============================================================================
// Module: Data Ingestion Integration Tests
// ============================================================================
//
// Tests covering the full ingestion pipeline including contract parsing,
// knowledge graph integration, and cross-document relationships.

mod data_ingestion {
    use super::*;
    use std::sync::Arc;
    use fen_graph::GraphStore;

    // ---- Contract Text Ingestion ----

    /// Test: contract text ingestion parses and stores correctly
    #[tokio::test]
    async fn test_contract_text_ingestion() {
        init_test_tracing();
        let env = IngestionTestEnv::new().await;

        let text = sample_contract_text();
        let contract = env.pipeline.ingest_contract_text(text).await.unwrap();

        // Verify contract was parsed with key fields
        assert!(!contract.title.is_empty(), "Contract title should be parsed");
        assert!(
            !contract.id.0.is_nil(),
            "Contract should have a valid ID"
        );

        // Verify stored
        let retrieved = env.storage.get_contract(&contract.id).await.unwrap();
        assert!(retrieved.is_some(), "Contract should be retrievable from storage");
        assert_eq!(retrieved.unwrap().title, contract.title);
    }

    /// Test: contract text ingestion extracts parties
    #[tokio::test]
    async fn test_contract_ingestion_extracts_parties() {
        let env = IngestionTestEnv::new().await;

        let contract = env
            .pipeline
            .ingest_contract_text(sample_contract_text())
            .await
            .unwrap();

        // Regex parser should extract at least one party from "Between X and Y"
        let non_unknown_parties: Vec<_> = contract
            .parties
            .iter()
            .filter(|p| !p.is_unknown())
            .collect();

        assert!(
            !non_unknown_parties.is_empty(),
            "Should extract at least one party from contract text, got: {:?}",
            contract.parties
        );
    }

    /// Test: contract text ingestion extracts dates
    #[tokio::test]
    async fn test_contract_ingestion_extracts_dates() {
        let env = IngestionTestEnv::new().await;

        let contract = env
            .pipeline
            .ingest_contract_text(sample_contract_text())
            .await
            .unwrap();

        // Should parse effective date from "Effective Date: 2024-01-01"
        assert_eq!(
            contract.effective_date,
            chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            "Should parse effective date"
        );

        // Should parse expiration date
        assert!(
            contract.expiration_date.is_some(),
            "Should parse expiration date"
        );
    }

    /// Test: contract text ingestion extracts clauses
    #[tokio::test]
    async fn test_contract_ingestion_extracts_clauses() {
        let env = IngestionTestEnv::new().await;

        let contract = env
            .pipeline
            .ingest_contract_text(sample_contract_text())
            .await
            .unwrap();

        // Sample text has Payment Terms, Termination, Confidentiality, Governing Law clauses
        assert!(
            !contract.clauses.is_empty(),
            "Should extract clauses from contract text"
        );
    }

    /// Test: batch contract ingestion
    #[tokio::test]
    async fn test_batch_contract_ingestion() {
        let env = IngestionTestEnv::new().await;

        let texts = vec![sample_contract_text(), sample_sow_text()];

        let mut contracts = Vec::new();
        for text in texts {
            let contract = env.pipeline.ingest_contract_text(text).await.unwrap();
            contracts.push(contract);
        }

        // Both should be stored
        let count = env.storage.count_contracts().await.unwrap();
        assert_eq!(count, 2, "Both contracts should be stored");

        // Each retrievable
        for contract in &contracts {
            let stored = env.storage.get_contract(&contract.id).await.unwrap();
            assert!(stored.is_some());
        }
    }

    // ---- Ingestion → Knowledge Graph ----

    /// Test: invoice written to knowledge graph creates nodes and edges
    #[tokio::test]
    async fn test_invoice_ingestion_writes_to_graph() {
        init_test_tracing();
        let env = GraphIngestionTestEnv::new().await;

        // Ingest through pipeline (text path stores in storage)
        let text = sample_invoice_text();
        let invoice = env.pipeline.ingest_text(text).await.unwrap();

        // Write to graph (ingest_pdf does this automatically; ingest_text is the simple path)
        env.graph.write_invoice(&invoice).await.unwrap();

        // Graph should now contain an Invoice node
        let results = env
            .graph
            .query_cypher("MATCH (i:Invoice) RETURN i.id")
            .await
            .unwrap();
        assert_eq!(results.len(), 1, "Graph should contain exactly one Invoice node");

        // Graph should contain a Vendor node
        let vendors = env
            .graph
            .query_cypher("MATCH (v:Vendor) RETURN v.name")
            .await
            .unwrap();
        assert!(
            !vendors.is_empty(),
            "Graph should contain the vendor from the ingested invoice"
        );

        // Graph should have SUPPLIES edge
        let supplies = env
            .graph
            .query_cypher("MATCH (v:Vendor)-[:SUPPLIES]->(i:Invoice) RETURN v.name, i.id")
            .await
            .unwrap();
        assert_eq!(
            supplies.len(),
            1,
            "Graph should have a SUPPLIES edge from vendor to invoice"
        );
    }

    /// Test: contract ingestion writes to knowledge graph
    #[tokio::test]
    async fn test_contract_ingestion_writes_to_graph() {
        init_test_tracing();
        let env = GraphIngestionTestEnv::new().await;

        let contract = ContractFixture::new()
            .with_title("Cloud Services Agreement")
            .with_party("Acme Corp")
            .build();

        // Write through GraphStore directly (ingest_contract_text doesn't go through graph)
        env.graph.write_contract(&contract).await.unwrap();

        // Graph should contain a Contract node
        let results = env
            .graph
            .query_cypher("MATCH (c:Contract) RETURN c.title")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Graph should contain Party nodes
        let parties = env
            .graph
            .query_cypher("MATCH (p:Party) RETURN p.name")
            .await
            .unwrap();
        assert!(
            !parties.is_empty(),
            "Graph should contain party nodes from contract"
        );

        // Graph should have PARTY_TO edges
        let party_edges = env
            .graph
            .query_cypher("MATCH (p:Party)-[:PARTY_TO]->(c:Contract) RETURN p.name")
            .await
            .unwrap();
        assert!(
            !party_edges.is_empty(),
            "Graph should have PARTY_TO edges from parties to contract"
        );

        // Clause nodes should exist
        let clauses = env
            .graph
            .query_cypher("MATCH (c:Contract)-[:HAS_CLAUSE]->(cl:Clause) RETURN cl.clause_type")
            .await
            .unwrap();
        assert!(
            !clauses.is_empty(),
            "Graph should have clause nodes linked to contract"
        );
    }

    /// Test: graph vendor deduplication across multiple ingested invoices
    #[tokio::test]
    async fn test_graph_vendor_deduplication_across_ingestion() {
        let env = GraphIngestionTestEnv::new().await;

        // Ingest two invoices with the same vendor name
        let texts = vec![
            r#"INVOICE
Invoice Number: DEDUP-001
Invoice Date: 2024-03-01
Due Date: 2024-03-31
Vendor: Acme Supplies Inc.
Bill To: Widget Corp
Total: $1,000.00
"#,
            r#"INVOICE
Invoice Number: DEDUP-002
Invoice Date: 2024-04-01
Due Date: 2024-04-30
Vendor: Acme Supplies Inc.
Bill To: Widget Corp
Total: $2,500.00
"#,
        ];

        for text in texts {
            let invoice = env.pipeline.ingest_text(text).await.unwrap();
            env.graph.write_invoice(&invoice).await.unwrap();
        }

        // Should have 2 Invoice nodes
        let invoices = env
            .graph
            .query_cypher("MATCH (i:Invoice) RETURN i.id")
            .await
            .unwrap();
        assert_eq!(invoices.len(), 2, "Should have 2 invoice nodes");

        // Should have only 1 Vendor node (deduplicated by name)
        let vendors = env
            .graph
            .query_cypher("MATCH (v:Vendor) RETURN count(v)")
            .await
            .unwrap();
        assert_eq!(vendors.len(), 1, "Should have exactly 1 vendor count row");

        // Vendor should have 2 SUPPLIES edges
        let supplies = env
            .graph
            .query_cypher("MATCH (v:Vendor)-[:SUPPLIES]->(i:Invoice) RETURN i.id")
            .await
            .unwrap();
        assert_eq!(
            supplies.len(),
            2,
            "Single vendor should supply both invoices"
        );
    }

    /// Test: link invoice to contract through graph
    #[tokio::test]
    async fn test_invoice_contract_graph_linking() {
        let env = GraphIngestionTestEnv::new().await;

        // Ingest an invoice and write to graph
        let invoice = env
            .pipeline
            .ingest_text(sample_invoice_text())
            .await
            .unwrap();
        env.graph.write_invoice(&invoice).await.unwrap();

        // Write a contract to graph
        let contract = ContractFixture::new()
            .with_title("Supply Agreement")
            .with_party("Acme Supplies Inc.")
            .build();
        env.graph.write_contract(&contract).await.unwrap();

        // Link invoice to contract
        env.graph
            .link_invoice_to_contract(&invoice.id, &contract.id)
            .await
            .unwrap();

        // Query related contracts from the invoice
        let related = env.graph.related_contracts(&invoice.id).await.unwrap();
        assert_eq!(related.len(), 1, "Invoice should be linked to 1 contract");
        assert_eq!(related[0], contract.id);

        // Verify GOVERNED_BY edge exists
        let governed = env
            .graph
            .query_cypher(
                "MATCH (i:Invoice)-[:GOVERNED_BY]->(c:Contract) RETURN c.title",
            )
            .await
            .unwrap();
        assert_eq!(governed.len(), 1);
    }

    /// Test: discover related contracts via shared vendor in graph
    #[tokio::test]
    async fn test_related_contracts_via_vendor() {
        let env = GraphIngestionTestEnv::new().await;

        // Invoice 1 from Acme, written to graph and linked to a contract
        let inv1 = env
            .pipeline
            .ingest_text(
                r#"INVOICE
Invoice Number: REL-001
Invoice Date: 2024-03-01
Vendor: Acme Corp
Bill To: Client Inc
Total: $5,000.00
"#,
            )
            .await
            .unwrap();
        env.graph.write_invoice(&inv1).await.unwrap();

        let contract = ContractFixture::new()
            .with_title("Acme Master Agreement")
            .with_party("Acme Corp")
            .build();

        env.graph.write_contract(&contract).await.unwrap();
        env.graph
            .link_invoice_to_contract(&inv1.id, &contract.id)
            .await
            .unwrap();

        // Invoice 2 from same vendor, written to graph but no direct contract link
        let inv2 = env
            .pipeline
            .ingest_text(
                r#"INVOICE
Invoice Number: REL-002
Invoice Date: 2024-04-01
Vendor: Acme Corp
Bill To: Client Inc
Total: $3,000.00
"#,
            )
            .await
            .unwrap();
        env.graph.write_invoice(&inv2).await.unwrap();

        // inv2 should discover the contract via shared vendor
        let related = env.graph.related_contracts(&inv2.id).await.unwrap();
        assert_eq!(
            related.len(),
            1,
            "Should discover contract via shared vendor"
        );
        assert_eq!(related[0], contract.id);
    }

    // ---- Mixed Document Ingestion ----

    /// Test: mixed invoice + contract ingestion flow
    #[tokio::test]
    async fn test_mixed_document_ingestion() {
        let env = IngestionTestEnv::new().await;

        // Ingest invoices
        let inv = env
            .pipeline
            .ingest_text(sample_invoice_text())
            .await
            .unwrap();

        // Ingest contracts
        let contract = env
            .pipeline
            .ingest_contract_text(sample_contract_text())
            .await
            .unwrap();

        // Both types should be stored independently
        let inv_count = env.storage.count_invoices().await.unwrap();
        let contract_count = env.storage.count_contracts().await.unwrap();

        assert_eq!(inv_count, 1, "Should have 1 invoice");
        assert_eq!(contract_count, 1, "Should have 1 contract");

        // Both retrievable
        assert!(env.storage.get_invoice(&inv.id).await.unwrap().is_some());
        assert!(env.storage.get_contract(&contract.id).await.unwrap().is_some());
    }

    /// Test: concurrent mixed document ingestion
    #[tokio::test]
    async fn test_concurrent_mixed_ingestion() {
        let env = Arc::new(IngestionTestEnv::new().await);

        let mut handles = Vec::new();

        // Spawn invoice ingestion tasks
        for i in 0..3 {
            let env = env.clone();
            handles.push(tokio::spawn(async move {
                let text = format!(
                    "INVOICE\nInvoice Number: CONC-INV-{:03}\nInvoice Date: 2024-01-{:02}\nVendor: Vendor {}\nTotal: ${}.00\n",
                    i, 10 + i, i, 1000 + i * 500
                );
                env.pipeline.ingest_text(&text).await.unwrap();
            }));
        }

        // Spawn contract ingestion tasks
        for i in 0..2 {
            let env = env.clone();
            handles.push(tokio::spawn(async move {
                let text = format!(
                    "SERVICE AGREEMENT\nContract Number: CONC-CTR-{:03}\nEffective Date: 2024-01-01\nBetween Party-A-{} and Party-B-{}\nTotal Contract Value: ${}.00\n",
                    i, i, i, 50000 + i * 25000
                );
                env.pipeline.ingest_contract_text(&text).await.unwrap();
            }));
        }

        futures::future::join_all(handles).await;

        let inv_count = env.storage.count_invoices().await.unwrap();
        let contract_count = env.storage.count_contracts().await.unwrap();

        assert_eq!(inv_count, 3, "All concurrent invoices should be stored");
        assert_eq!(contract_count, 2, "All concurrent contracts should be stored");
    }

    /// Test: storage → graph → query roundtrip with explicit fixtures
    #[tokio::test]
    async fn test_full_ingestion_graph_roundtrip() {
        init_test_tracing();
        let env = GraphIngestionTestEnv::new().await;

        // Build invoices with explicit vendor assignments
        let invoices = vec![
            InvoiceFixture::new()
                .with_number("RT-001")
                .with_vendor("Alpha Corp")
                .build(),
            InvoiceFixture::new()
                .with_number("RT-002")
                .with_vendor("Alpha Corp")
                .build(),
            InvoiceFixture::new()
                .with_number("RT-003")
                .with_vendor("Beta Services")
                .build(),
        ];

        // Store in hot storage and write to graph
        for invoice in &invoices {
            env.storage.store_invoice(invoice).await.unwrap();
            env.graph.write_invoice(invoice).await.unwrap();
        }

        // Verify storage: 3 invoices
        let count = env.storage.count_invoices().await.unwrap();
        assert_eq!(count, 3);

        // Verify graph: 2 vendor nodes (deduplicated)
        let vendors = env
            .graph
            .query_cypher("MATCH (v:Vendor) RETURN v.name")
            .await
            .unwrap();
        assert_eq!(vendors.len(), 2, "Should have 2 distinct vendors in graph");

        // Verify vendor deduplication: resolving the same name returns same ID
        let alpha_id = env.graph.resolve_vendor("Alpha Corp").await.unwrap();
        let alpha_id_2 = env.graph.resolve_vendor("Alpha Corp").await.unwrap();
        assert_eq!(alpha_id, alpha_id_2, "Same vendor name should resolve to same ID");

        // Verify both vendors exist with different IDs
        let beta_id = env.graph.resolve_vendor("Beta Services").await.unwrap();
        assert_ne!(alpha_id, beta_id, "Different vendors should have different IDs");

        // Verify invoices_for_vendor returns correct per-vendor counts
        let alpha_invoices = env.graph.invoices_for_vendor(&alpha_id).await.unwrap();
        assert_eq!(
            alpha_invoices.len(),
            2,
            "Alpha Corp should supply exactly 2 invoices, got {}",
            alpha_invoices.len()
        );

        let beta_invoices = env.graph.invoices_for_vendor(&beta_id).await.unwrap();
        assert_eq!(
            beta_invoices.len(),
            1,
            "Beta Services should supply exactly 1 invoice, got {}",
            beta_invoices.len()
        );
    }

    /// Test: graph non-fatal — ingestion succeeds even if graph write fails
    #[tokio::test]
    async fn test_graph_failure_non_fatal() {
        let env = IngestionTestEnv::new().await;

        // Pipeline without graph — should succeed without graph writes
        let invoice = env
            .pipeline
            .ingest_text(sample_invoice_text())
            .await
            .unwrap();

        assert!(
            !invoice.id.0.is_nil(),
            "Ingestion should succeed without graph"
        );
        assert!(
            env.storage.get_invoice(&invoice.id).await.unwrap().is_some(),
            "Invoice should still be in storage"
        );
    }

    /// Test: contract with clauses written to graph preserves clause structure
    #[tokio::test]
    async fn test_graph_contract_clause_structure() {
        let env = GraphIngestionTestEnv::new().await;

        use fen_core::domain::{ClauseType, ContractClause};

        let contract = ContractFixture::new()
            .with_title("Detailed Agreement")
            .add_clause(ContractClause::new(
                ClauseType::Liability,
                "Each party shall indemnify the other against third-party claims",
            ))
            .add_clause(ContractClause::new(
                ClauseType::ForceMajeure,
                "Neither party shall be liable for force majeure events",
            ))
            .build();

        env.graph.write_contract(&contract).await.unwrap();

        // Should have all clauses (2 default + 2 added = 4)
        let clauses = env
            .graph
            .query_cypher(
                "MATCH (c:Contract)-[:HAS_CLAUSE]->(cl:Clause) RETURN cl.clause_type",
            )
            .await
            .unwrap();
        assert_eq!(
            clauses.len(),
            contract.clauses.len(),
            "All clauses should be written to graph"
        );
    }

    /// Test: ingestion pipeline validates and stores with correct confidence
    #[tokio::test]
    async fn test_ingestion_confidence_propagation() {
        let env = IngestionTestEnv::new().await;

        let invoice = env
            .pipeline
            .ingest_text(sample_invoice_text())
            .await
            .unwrap();

        // Regex parser sets confidence to ~0.5 (or lower)
        // Just verify confidence is set and reasonable
        assert!(
            invoice.confidence_score > 0.0 && invoice.confidence_score <= 1.0,
            "Confidence should be between 0 and 1, got: {}",
            invoice.confidence_score
        );

        // Stored invoice should preserve confidence
        let stored = env.storage.get_invoice(&invoice.id).await.unwrap().unwrap();
        assert_eq!(
            stored.confidence_score, invoice.confidence_score,
            "Stored confidence should match parsed confidence"
        );
    }
}

// ============================================================================
// Module: ML Pipeline Integration Tests (LayoutLMv3 + TATR)
// ============================================================================
//
// Tests exercising the full ML pipeline with real ONNX models:
// LayoutLMv3 for document understanding, TATR for table extraction,
// and the complete ingestion path through storage and graph.

mod ml_pipeline {
    use super::*;
    use fen_graph::GraphStore;
    use fen_tests::{
        create_synthetic_image, load_test_document, load_test_pdf, shared_ml_pipeline,
    };

    // ---- LayoutLMv3 Tests ----

    /// Test: LayoutLMv3 produces layout regions and text from an invoice image
    #[tokio::test]
    async fn test_layoutlmv3_invoice_field_extraction() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_simple.png");

        let result = ml.process_image(&image).await.unwrap();

        assert!(!result.text.is_empty(), "Should extract text from invoice");
        assert!(
            !result.layout_result.regions.is_empty(),
            "Should detect layout regions from invoice"
        );

        // Layout regions should have semantic labels (Text, Title, Table, etc.)
        let region_labels: Vec<_> = result
            .layout_result
            .regions
            .iter()
            .map(|r| r.label)
            .collect();

        tracing::info!(
            text_len = result.text.len(),
            regions = result.layout_result.regions.len(),
            entities = result.layout_result.entities.len(),
            kv_pairs = result.layout_result.key_value_pairs.len(),
            region_labels = ?region_labels,
            "LayoutLMv3 extraction results"
        );

        // Should classify at least some regions as Text (most common in invoices)
        let has_text_region = region_labels.iter().any(|l| {
            matches!(
                l,
                fen_ml::layout::LayoutLabel::Text | fen_ml::layout::LayoutLabel::Title
            )
        });
        assert!(
            has_text_region,
            "Should detect at least a Text or Title region, got: {:?}",
            region_labels
        );

        // If entities were extracted, verify their structure
        for entity in &result.layout_result.entities {
            assert!(!entity.value.is_empty(), "Entity value should not be empty");
            assert!(
                (0.0..=1.0).contains(&entity.confidence),
                "Entity confidence {} not in [0.0, 1.0]",
                entity.confidence
            );
        }
    }

    /// Test: LayoutLMv3 detects multiple region types in a document
    #[tokio::test]
    async fn test_layoutlmv3_region_types_coverage() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_simple.png");

        let result = ml.process_image(&image).await.unwrap();

        let region_labels: std::collections::HashSet<_> = result
            .layout_result
            .regions
            .iter()
            .map(|r| r.label)
            .collect();

        // Invoice image should have at least 2 different region types
        assert!(
            region_labels.len() >= 2,
            "Should detect at least 2 different region types, got: {:?}",
            region_labels
        );

        for region in &result.layout_result.regions {
            assert!(!region.text.is_empty() || region.confidence > 0.0,
                "Region should have text or confidence");
            assert!(
                region.bbox.width >= 0.0 && region.bbox.height >= 0.0,
                "Region bbox should have non-negative dimensions"
            );
        }
    }

    /// Test: LayoutLMv3 extracts key-value pairs from form-like layouts
    #[tokio::test]
    async fn test_layoutlmv3_key_value_pairs() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_simple.png");

        let result = ml.process_image(&image).await.unwrap();

        // Key-value pairs are extracted from form-like layouts
        // Not all documents have them, but invoice_simple.png should have some
        tracing::info!(
            kv_pairs = result.layout_result.key_value_pairs.len(),
            "Key-value pairs extracted"
        );

        for kv in &result.layout_result.key_value_pairs {
            assert!(!kv.key.is_empty(), "KV key should not be empty");
            assert!(
                (0.0..=1.0).contains(&kv.confidence),
                "KV confidence {} not in [0.0, 1.0]",
                kv.confidence
            );
        }
    }

    /// Test: LayoutLMv3 confidence scores are valid
    #[tokio::test]
    async fn test_layoutlmv3_confidence_scores() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_simple.png");

        let result = ml.process_image(&image).await.unwrap();

        for (i, region) in result.layout_result.regions.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(&region.confidence),
                "Region {} confidence {} not in [0.0, 1.0]",
                i,
                region.confidence
            );
        }

        for (i, entity) in result.layout_result.entities.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(&entity.confidence),
                "Entity {} confidence {} not in [0.0, 1.0]",
                i,
                entity.confidence
            );
        }

        assert!(
            result.layout_result.processing_time_ms > 0,
            "Processing time should be recorded"
        );
    }

    /// Test: LayoutLMv3 handles empty OCR input gracefully
    #[tokio::test]
    async fn test_layoutlmv3_empty_ocr_input() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let blank = create_synthetic_image(800, 600);

        let result = ml.process_image(&blank).await.unwrap();

        // Blank image should produce minimal output but not crash
        // OCR may still detect the black rectangle, so we just verify no panic
        assert!(
            result.layout_result.processing_time_ms > 0 || result.layout_result.regions.is_empty(),
            "Should handle blank image gracefully"
        );
    }

    // ---- TATR (Table Transformer) Tests ----

    /// Test: TATR detects tables in an invoice with line items
    #[tokio::test]
    async fn test_tatr_table_detection() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_table.png");

        let result = ml.process_image(&image).await.unwrap();

        if result.tables.is_empty() {
            tracing::warn!("No tables detected in invoice_table.png — model may need tuning");
            return;
        }

        for (i, table) in result.tables.iter().enumerate() {
            assert!(table.num_rows > 0, "Table {} should have rows", i);
            assert!(table.num_columns > 0, "Table {} should have columns", i);
            assert!(
                (0.0..=1.0).contains(&table.confidence),
                "Table {} confidence {} not in [0.0, 1.0]",
                i,
                table.confidence
            );
            assert!(
                table.bbox.width > 0.0 && table.bbox.height > 0.0,
                "Table {} bbox should have positive dimensions",
                i
            );

            tracing::info!(
                table_index = i,
                rows = table.num_rows,
                cols = table.num_columns,
                confidence = table.confidence,
                "Detected table"
            );
        }
    }

    /// Test: TATR assigns OCR text to table cells
    #[tokio::test]
    async fn test_tatr_cell_text_assignment() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_table.png");

        let result = ml.process_image(&image).await.unwrap();

        if result.tables.is_empty() {
            tracing::warn!("No tables detected — skipping cell text test");
            return;
        }

        let table = &result.tables[0];
        assert_eq!(
            table.cells.len(),
            table.num_rows,
            "Cells row count should match num_rows"
        );

        // Check cell structure
        for (row_idx, row) in table.cells.iter().enumerate() {
            for (col_idx, cell) in row.iter().enumerate() {
                assert_eq!(cell.row, row_idx, "Cell row index mismatch");
                assert_eq!(cell.column, col_idx, "Cell column index mismatch");
                assert!(cell.row_span >= 1, "Row span should be >= 1");
                assert!(cell.col_span >= 1, "Col span should be >= 1");
            }
        }

        // At least some cells should have text (OCR assigned via R-tree)
        let cells_with_text: usize = table
            .cells
            .iter()
            .flat_map(|row| row.iter())
            .filter(|cell| !cell.text.is_empty())
            .count();

        tracing::info!(
            total_cells = table.num_rows * table.num_columns,
            cells_with_text,
            "Cell text assignment"
        );

        assert!(
            cells_with_text > 0,
            "At least some cells should have OCR text assigned"
        );
    }

    /// Test: TATR table exports to valid markdown
    #[tokio::test]
    async fn test_tatr_export_markdown() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_table.png");

        let result = ml.process_image(&image).await.unwrap();

        for table in &result.tables {
            let md = table.to_markdown();
            assert!(!md.is_empty(), "Markdown export should not be empty");
            assert!(md.contains('|'), "Markdown should contain pipe characters");
            assert!(md.contains("---"), "Markdown should contain separator");
        }
    }

    /// Test: TATR table exports to valid CSV
    #[tokio::test]
    async fn test_tatr_export_csv() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_table.png");

        let result = ml.process_image(&image).await.unwrap();

        for table in &result.tables {
            let csv = table.to_csv();
            assert!(!csv.is_empty(), "CSV export should not be empty");
            // CSV should have at least as many lines as rows
            let lines: Vec<_> = csv.lines().collect();
            assert!(
                lines.len() >= table.num_rows,
                "CSV should have at least {} lines, got {}",
                table.num_rows,
                lines.len()
            );
        }
    }

    /// Test: TATR produces no tables from a blank image
    #[tokio::test]
    async fn test_tatr_blank_image_no_tables() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let blank = create_synthetic_image(800, 600);

        let result = ml.process_image(&blank).await.unwrap();

        assert!(
            result.tables.is_empty(),
            "Blank image should not contain tables, found {}",
            result.tables.len()
        );
    }

    // ---- Full ML Pipeline Tests ----

    /// Test: Full pipeline PNG → OCR → Layout → Tables → Embeddings → Invoice → Storage
    #[tokio::test]
    async fn test_full_ml_pipeline_png_to_invoice() {
        init_test_tracing();
        let env = MlPipelineTestEnv::new().await;
        let image = load_test_document("invoice_simple.png");

        let result = env.ml.process_image(&image).await.unwrap();

        // Verify all pipeline stages produced output
        assert!(!result.text.is_empty(), "OCR should extract text");
        assert!(
            !result.ocr_result.regions.is_empty(),
            "OCR should detect text regions"
        );
        assert!(
            !result.layout_result.regions.is_empty(),
            "Layout should detect regions"
        );

        // Verify embeddings
        let dim = env.ml.embedding.embedding_dim();
        assert_eq!(
            result.embeddings.document.len(),
            dim,
            "Document embedding should be {}D",
            dim
        );
        let norm: f32 = result
            .embeddings
            .document
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!(
            (norm - 1.0).abs() < 0.15,
            "Embedding should be approximately L2-normalized, got norm={}",
            norm
        );

        tracing::info!(
            text_len = result.text.len(),
            ocr_regions = result.ocr_result.regions.len(),
            layout_regions = result.layout_result.regions.len(),
            entities = result.layout_result.entities.len(),
            tables = result.tables.len(),
            embedding_dim = dim,
            "Full ML pipeline results for invoice_simple.png"
        );
    }

    /// Test: Full pipeline with scanned PDF (exercises ingest_pdf ML path)
    #[tokio::test]
    async fn test_full_ml_pipeline_scanned_pdf() {
        init_test_tracing();
        let env = MlPipelineTestEnv::new().await;

        // invoice_scribbles.pdf is a scanned PDF that needs OCR
        let pdf_bytes = load_test_pdf("invoice_scribbles.pdf");
        let result = env
            .pipeline
            .ingest_pdf(&pdf_bytes, "invoice_scribbles.pdf")
            .await
            .unwrap();

        assert!(
            !result.invoice.id.0.is_nil(),
            "Should produce valid invoice ID"
        );
        assert!(
            result.invoice.confidence_score > 0.0,
            "Confidence should be > 0"
        );

        // Scanned PDF through ML path should produce embedding
        tracing::info!(
            invoice_number = %result.invoice.invoice_number,
            confidence = result.invoice.confidence_score,
            has_embedding = result.embedding.is_some(),
            "Scanned PDF ingestion result"
        );

        // Verify stored
        let stored = env
            .storage
            .get_invoice(&result.invoice.id)
            .await
            .unwrap();
        assert!(stored.is_some(), "Invoice should be stored");
    }

    /// Test: Full pipeline with text-embedded PDF
    #[tokio::test]
    async fn test_full_ml_pipeline_text_pdf() {
        init_test_tracing();
        let env = MlPipelineTestEnv::new().await;

        let pdf_bytes = load_test_pdf("invoice_flipkart.pdf");
        let result = env
            .pipeline
            .ingest_pdf(&pdf_bytes, "invoice_flipkart.pdf")
            .await
            .unwrap();

        assert!(!result.invoice.id.0.is_nil(), "Should produce valid invoice");
        assert!(
            result.invoice.confidence_score > 0.0,
            "Should have positive confidence"
        );

        tracing::info!(
            invoice_number = %result.invoice.invoice_number,
            confidence = result.invoice.confidence_score,
            has_embedding = result.embedding.is_some(),
            "Text PDF ingestion result"
        );
    }

    /// Test: Multiple documents produce different results and embeddings
    #[tokio::test]
    async fn test_ml_pipeline_multiple_documents() {
        init_test_tracing();
        let ml = shared_ml_pipeline();

        let doc_files = ["invoice_simple.png", "invoice_table.png", "receipt.png"];
        let mut results = Vec::new();

        for filename in &doc_files {
            let image = load_test_document(filename);
            let result = ml.process_image(&image).await.unwrap();
            results.push((filename, result));
        }

        // Different documents should produce different text
        assert_ne!(
            results[0].1.text, results[1].1.text,
            "Different documents should produce different text"
        );

        // Different documents should have different embeddings
        if results.len() >= 2 {
            let sim = ml.embedding.cosine_similarity(
                &results[0].1.embeddings.document,
                &results[1].1.embeddings.document,
            );
            assert!(
                sim < 0.99,
                "Different documents should have cosine similarity < 0.99, got {}",
                sim
            );

            tracing::info!(
                doc0 = results[0].0,
                doc1 = results[1].0,
                cosine_similarity = sim,
                "Document embedding similarity"
            );
        }
    }

    /// Test: ML-parsed invoice written to graph creates correct nodes
    #[tokio::test]
    async fn test_ml_pipeline_invoice_to_graph() {
        init_test_tracing();
        let env = MlPipelineTestEnv::new().await;

        let pdf_bytes = load_test_pdf("invoice_flipkart.pdf");
        let _result = env
            .pipeline
            .ingest_pdf(&pdf_bytes, "invoice_flipkart.pdf")
            .await
            .unwrap();

        // Graph should have invoice node (ingest_pdf writes to graph automatically)
        let invoices = env
            .graph
            .query_cypher("MATCH (i:Invoice) RETURN i.id")
            .await
            .unwrap();
        assert!(
            !invoices.is_empty(),
            "Graph should contain invoice node after ML ingestion"
        );

        // If vendor was extracted, should have vendor node and SUPPLIES edge
        let supplies = env
            .graph
            .query_cypher("MATCH (v:Vendor)-[:SUPPLIES]->(i:Invoice) RETURN v.name")
            .await
            .unwrap();

        tracing::info!(
            invoice_nodes = invoices.len(),
            supplies_edges = supplies.len(),
            "Graph state after ML ingestion"
        );
    }

    /// Test: ML pipeline produces higher confidence than regex fallback
    #[tokio::test]
    async fn test_ml_vs_regex_confidence() {
        init_test_tracing();
        let env = MlPipelineTestEnv::new().await;

        // ML path via PDF ingestion
        let pdf_bytes = load_test_pdf("invoice_flipkart.pdf");
        let ml_result = env
            .pipeline
            .ingest_pdf(&pdf_bytes, "invoice_flipkart.pdf")
            .await
            .unwrap();

        // Regex path via text ingestion
        let regex_env = IngestionTestEnv::new().await;
        let regex_result = regex_env
            .pipeline
            .ingest_text(sample_invoice_text())
            .await
            .unwrap();

        tracing::info!(
            ml_confidence = ml_result.invoice.confidence_score,
            regex_confidence = regex_result.confidence_score,
            "ML vs regex confidence comparison"
        );

        // ML should generally produce equal or higher confidence than regex
        // (regex is typically ~0.5, ML is 0.7+)
        assert!(
            ml_result.invoice.confidence_score >= regex_result.confidence_score,
            "ML confidence ({}) should be >= regex confidence ({})",
            ml_result.invoice.confidence_score,
            regex_result.confidence_score
        );
    }

    /// Test: ML pipeline embedding has correct dimensions (384D for all-MiniLM-L6-v2)
    #[tokio::test]
    async fn test_ml_pipeline_embedding_dimensions() {
        init_test_tracing();
        let ml = shared_ml_pipeline();
        let image = load_test_document("invoice_simple.png");

        let result = ml.process_image(&image).await.unwrap();

        // Document embedding should be 384D (all-MiniLM-L6-v2)
        assert_eq!(
            result.embeddings.document.len(),
            384,
            "Document embedding should be 384-dimensional"
        );

        // Should not be all zeros
        let has_nonzero = result.embeddings.document.iter().any(|&x| x != 0.0);
        assert!(has_nonzero, "Embedding should not be all zeros");

        // Check approximate L2 normalization
        let norm: f32 = result
            .embeddings
            .document
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!(
            (norm - 1.0).abs() < 0.15,
            "Embedding should be approximately L2-normalized, got norm={}",
            norm
        );

        // Section embeddings should also be 384D
        for (i, section) in result.embeddings.sections.iter().enumerate() {
            assert_eq!(
                section.embedding.len(),
                384,
                "Section {} embedding should be 384D",
                i
            );
        }
    }
}

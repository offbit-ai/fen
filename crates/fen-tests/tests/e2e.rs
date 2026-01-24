//! End-to-end integration tests for the Fen anomaly detection pipeline
//!
//! These tests exercise the full pipeline from data ingestion through
//! storage, validation, and query operations.

use rust_decimal_macros::dec;

use fen_core::domain::{AnomalyType, Severity};
use fen_core::ValidationStatus;
use fen_storage::{DocumentStore, InvoiceFilter, StorageTier};
use fen_tests::{
    consistent_embedding, init_test_tracing, random_embedding, sample_invoice_text,
    ContractFixture, IngestionTestEnv, InvoiceFixture, QueryTestEnv, TestEnv,
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
        let invoice_id = invoice.id.clone();

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
            let embedding = consistent_embedding(&invoice.invoice_number, 768);
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
        let query_emb = consistent_embedding("QV-001", 768);
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
                        .map_or(false, |f| f.contains("line_item"))),
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
    use super::*;
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
        assert!(result.metadata.execution_time_ms >= 0);
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
    use fen_storage::{parse_query, DocumentStore, QueryParams};
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
                .with_number(&format!("LIM-{:03}", i))
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
        const EMBEDDING_DIM: usize = 768;
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

        const EMBEDDING_DIM: usize = 768;
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
        assert!(result.metadata.execution_time_ms >= 0);
        assert_eq!(result.metadata.rows_returned, result.rows.len());
    }
}

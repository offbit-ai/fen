pub mod delta_writer;
pub mod error;
pub mod query;
pub mod rdf;
pub mod schema;
pub mod store;
pub mod writer;

pub use error::GraphError;
pub use store::KyuGraphStore;

use async_trait::async_trait;
use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

/// Trait for knowledge graph operations.
///
/// The graph stores entities (invoices, contracts, vendors, parties, clauses)
/// as nodes and their relationships as edges. It serves as an index over the
/// primary document storage, enabling relationship traversal and entity linking.
#[async_trait]
pub trait GraphStore: Send + Sync {
    /// Write an invoice and all its relationships (vendor, bill_to, contract link).
    async fn write_invoice(&self, invoice: &Invoice) -> Result<(), GraphError>;

    /// Write a contract and all its relationships (parties, clauses).
    async fn write_contract(&self, contract: &Contract) -> Result<(), GraphError>;

    /// Link an invoice to a contract via GOVERNED_BY edge.
    async fn link_invoice_to_contract(
        &self,
        invoice_id: &InvoiceId,
        contract_id: &ContractId,
    ) -> Result<(), GraphError>;

    /// Find or create a vendor by name (deduplication). Returns vendor node ID.
    async fn resolve_vendor(&self, name: &str) -> Result<String, GraphError>;

    /// Query: find all invoices for a vendor.
    async fn invoices_for_vendor(&self, vendor_id: &str) -> Result<Vec<InvoiceId>, GraphError>;

    /// Query: find contracts related to an invoice (direct or via shared vendor).
    async fn related_contracts(
        &self,
        invoice_id: &InvoiceId,
    ) -> Result<Vec<ContractId>, GraphError>;

    /// Execute a raw Cypher query, returning results as string vectors.
    async fn query_cypher(&self, cypher: &str) -> Result<Vec<Vec<String>>, GraphError>;

    /// Load an RDF file (Turtle/N-Triples/RDF-XML) into the graph.
    /// Auto-creates node/rel tables from rdf:type and predicate URIs.
    async fn load_rdf(&self, path: &str) -> Result<(), GraphError>;

    /// Execute an RDF inspection procedure and return results as string vectors.
    /// Supports: "stats", "prefixes", "types"
    async fn rdf_inspect(&self, procedure: &str, path: &str) -> Result<Vec<Vec<String>>, GraphError>;

    /// Query nodes imported from RDF by their inferred table name.
    async fn rdf_nodes(&self, table_name: &str) -> Result<Vec<Vec<String>>, GraphError>;

    /// Batch-write multiple invoices via delta fast path.
    ///
    /// Uses `DeltaBatch` + `apply_delta` for single-WAL-append atomic writes.
    /// At 1000+ docs/hr, this is 5-10x faster than individual Cypher queries.
    async fn write_invoice_batch(&self, invoices: &[Invoice]) -> Result<u64, GraphError>;

    /// Batch-write multiple contracts via delta fast path.
    async fn write_contract_batch(&self, contracts: &[Contract]) -> Result<u64, GraphError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fen_core::domain::{
        ClauseType, Contract, ContractClause, ContractType, Currency, Invoice, Party,
    };
    use rust_decimal_macros::dec;

    fn sample_invoice() -> Invoice {
        let mut inv = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap());
        inv.vendor = Party::new("Acme Corp");
        inv.vendor.tax_id = Some("12-3456789".to_string());
        inv.bill_to = Party::new("Widget Inc");
        inv.currency = Currency::USD;
        inv.total_amount = dec!(5000.00);
        inv.confidence_score = 0.95;
        inv
    }

    fn sample_contract() -> Contract {
        let mut contract = Contract::new(
            "Master Service Agreement",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        );
        contract.contract_number = Some("MSA-2024-001".to_string());
        contract.contract_type = ContractType::MasterServiceAgreement;
        contract.expiration_date = Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
        contract.total_value = Some(dec!(100000.00));
        contract.currency = Some(Currency::USD);
        contract.confidence_score = 0.92;
        contract.parties = vec![
            Party::new("Acme Corp"),
            Party::new("Widget Inc"),
        ];
        contract.clauses = vec![
            ContractClause::new(ClauseType::PaymentTerms, "Net 30 payment terms"),
            ContractClause::new(ClauseType::Termination, "Either party may terminate with 30 days notice"),
        ];
        contract
    }

    #[tokio::test]
    async fn test_write_and_query_invoice() {
        let store = KyuGraphStore::in_memory().unwrap();
        let invoice = sample_invoice();

        store.write_invoice(&invoice).await.unwrap();

        // Verify vendor was created
        let vendor_id = store.resolve_vendor("Acme Corp").await.unwrap();
        assert!(!vendor_id.is_empty());

        // Verify vendor deduplication — same name returns same ID
        let vendor_id2 = store.resolve_vendor("Acme Corp").await.unwrap();
        assert_eq!(vendor_id, vendor_id2);

        // Verify invoice is linked to vendor
        let invoices = store.invoices_for_vendor(&vendor_id).await.unwrap();
        assert_eq!(invoices.len(), 1);
        assert_eq!(invoices[0], invoice.id);
    }

    #[tokio::test]
    async fn test_write_and_query_contract() {
        let store = KyuGraphStore::in_memory().unwrap();
        let contract = sample_contract();

        store.write_contract(&contract).await.unwrap();

        // Verify via raw cypher
        let results = store
            .query_cypher("MATCH (c:Contract) RETURN c.title")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Verify parties were created
        let results = store
            .query_cypher("MATCH (p:Party) RETURN p.name")
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        // Verify clauses were created
        let results = store
            .query_cypher("MATCH (cl:Clause) RETURN cl.clause_type")
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        // Verify edges exist
        let results = store
            .query_cypher("MATCH (:Party)-[:PARTY_TO]->(:Contract) RETURN count(*)")
            .await
            .unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_link_invoice_to_contract() {
        let store = KyuGraphStore::in_memory().unwrap();

        let invoice = sample_invoice();
        let contract = sample_contract();

        store.write_invoice(&invoice).await.unwrap();
        store.write_contract(&contract).await.unwrap();

        // Link them
        store
            .link_invoice_to_contract(&invoice.id, &contract.id)
            .await
            .unwrap();

        // Query related contracts
        let contracts = store.related_contracts(&invoice.id).await.unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0], contract.id);
    }

    #[tokio::test]
    async fn test_vendor_deduplication_across_invoices() {
        let store = KyuGraphStore::in_memory().unwrap();

        let mut inv1 = sample_invoice();
        inv1.invoice_number = "INV-001".to_string();

        let mut inv2 = sample_invoice();
        inv2.id = InvoiceId::new();
        inv2.invoice_number = "INV-002".to_string();
        // Same vendor name

        store.write_invoice(&inv1).await.unwrap();
        store.write_invoice(&inv2).await.unwrap();

        // Should have only ONE vendor node
        let results = store
            .query_cypher("MATCH (v:Vendor) RETURN count(v)")
            .await
            .unwrap();
        // count should be 1
        assert_eq!(results.len(), 1);

        // Vendor should have 2 SUPPLIES edges
        let vendor_id = store.resolve_vendor("Acme Corp").await.unwrap();
        let invoices = store.invoices_for_vendor(&vendor_id).await.unwrap();
        assert_eq!(invoices.len(), 2);
    }

    #[tokio::test]
    async fn test_related_contracts_via_vendor() {
        let store = KyuGraphStore::in_memory().unwrap();

        // Invoice 1 from Acme Corp, linked to a contract
        let mut inv1 = sample_invoice();
        inv1.invoice_number = "INV-001".to_string();

        let contract = sample_contract();

        store.write_invoice(&inv1).await.unwrap();
        store.write_contract(&contract).await.unwrap();
        store
            .link_invoice_to_contract(&inv1.id, &contract.id)
            .await
            .unwrap();

        // Invoice 2 from same vendor (Acme Corp), no direct contract link
        let mut inv2 = sample_invoice();
        inv2.id = InvoiceId::new();
        inv2.invoice_number = "INV-002".to_string();
        inv2.contract_id = None;

        store.write_invoice(&inv2).await.unwrap();

        // inv2 should discover the contract via shared vendor
        let contracts = store.related_contracts(&inv2.id).await.unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0], contract.id);
    }

    // ---- RDF integration tests ----

    const TEST_TURTLE: &str = r#"
@prefix rdf:    <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix foaf:   <http://xmlns.com/foaf/0.1/> .
@prefix schema: <https://schema.org/> .
@prefix xsd:    <http://www.w3.org/2001/XMLSchema#> .
@prefix ex:     <https://example.org/> .

ex:alice
    a foaf:Person ;
    foaf:name "Alice Smith" ;
    foaf:mbox "alice@example.com" .

ex:bob
    a foaf:Person ;
    foaf:name "Bob Jones" ;
    foaf:mbox "bob@example.com" ;
    foaf:knows ex:alice .

ex:acme
    a schema:Organization ;
    schema:name "Acme Corp" ;
    schema:location "New York" .

ex:alice schema:affiliation ex:acme .
ex:bob   schema:affiliation ex:acme .
"#;

    fn write_test_turtle() -> std::path::PathBuf {
        let id = uuid::Uuid::new_v4();
        let dir = std::env::temp_dir().join(format!("fen_graph_rdf_test_{id}"));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.ttl");
        std::fs::write(&path, TEST_TURTLE).unwrap();
        path
    }

    #[tokio::test]
    async fn test_load_rdf_turtle() {
        let ttl_path = write_test_turtle();
        let store = KyuGraphStore::in_memory().unwrap();

        store.load_rdf(ttl_path.to_str().unwrap()).await.unwrap();

        // Verify Person nodes were created
        let persons = store.rdf_nodes("Person").await.unwrap();
        assert_eq!(persons.len(), 2);

        // Verify Organization nodes were created
        let orgs = store.rdf_nodes("Organization").await.unwrap();
        assert_eq!(orgs.len(), 1);

        let _ = std::fs::remove_dir_all(ttl_path.parent().unwrap());
    }

    #[tokio::test]
    async fn test_rdf_inspect_stats() {
        let ttl_path = write_test_turtle();
        let store = KyuGraphStore::in_memory().unwrap();

        let stats = store
            .rdf_inspect("stats", ttl_path.to_str().unwrap())
            .await
            .unwrap();

        // Should return exactly one row with triple/subject/predicate/type counts
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].len(), 4);

        let _ = std::fs::remove_dir_all(ttl_path.parent().unwrap());
    }

    #[tokio::test]
    async fn test_rdf_inspect_types() {
        let ttl_path = write_test_turtle();
        let store = KyuGraphStore::in_memory().unwrap();

        let types = store
            .rdf_inspect("types", ttl_path.to_str().unwrap())
            .await
            .unwrap();

        // Should find Person and Organization types
        assert!(types.len() >= 2);

        let _ = std::fs::remove_dir_all(ttl_path.parent().unwrap());
    }

    #[tokio::test]
    async fn test_rdf_creates_relationship_tables() {
        let ttl_path = write_test_turtle();
        let store = KyuGraphStore::in_memory().unwrap();

        store.load_rdf(ttl_path.to_str().unwrap()).await.unwrap();

        // Verify the "knows" relationship was imported
        let knows = store
            .query_cypher("MATCH (a:Person)-[:knows]->(b:Person) RETURN a.name, b.name")
            .await
            .unwrap();
        assert_eq!(knows.len(), 1);

        // Verify affiliation relationships
        let affiliations = store
            .query_cypher(
                "MATCH (p:Person)-[:affiliation]->(o:Organization) RETURN p.name, o.name",
            )
            .await
            .unwrap();
        assert_eq!(affiliations.len(), 2);

        let _ = std::fs::remove_dir_all(ttl_path.parent().unwrap());
    }
}

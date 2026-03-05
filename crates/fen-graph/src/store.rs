use std::path::Path;
use std::sync::Arc;

use kyu_graph::Database;

use fen_core::domain::{
    Contract, ContractId, Invoice, InvoiceId,
};

use crate::error::GraphError;
use crate::query;
use crate::schema;
use crate::writer;
use crate::GraphStore;

/// KyuGraph-backed knowledge graph store.
///
/// KyuGraph is pure Rust — Connection is Send+Sync, but graph queries can be
/// CPU/IO-intensive, so we still use `spawn_blocking` to avoid blocking the
/// async runtime. Each operation creates a fresh Connection via `db.connect()`.
pub struct KyuGraphStore {
    db: Arc<Database>,
}

impl KyuGraphStore {
    /// Create a new persistent graph store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, GraphError> {
        let mut db = Database::open(path.as_ref())?;
        db.register_extension(Box::new(ext_rdf::RdfExtension::new()));
        {
            let conn = db.connect();
            schema::initialize_schema(&conn)?;
        }
        Ok(Self { db: Arc::new(db) })
    }

    /// Create an in-memory graph store (for testing).
    pub fn in_memory() -> Result<Self, GraphError> {
        let mut db = Database::in_memory();
        db.register_extension(Box::new(ext_rdf::RdfExtension::new()));
        {
            let conn = db.connect();
            schema::initialize_schema(&conn)?;
        }
        Ok(Self { db: Arc::new(db) })
    }

}

#[async_trait::async_trait]
impl GraphStore for KyuGraphStore {
    async fn write_invoice(&self, invoice: &Invoice) -> Result<(), GraphError> {
        let db = self.db.clone();
        let invoice = invoice.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            writer::write_invoice(&conn, &invoice)
        })
        .await?
    }

    async fn write_contract(&self, contract: &Contract) -> Result<(), GraphError> {
        let db = self.db.clone();
        let contract = contract.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            writer::write_contract(&conn, &contract)
        })
        .await?
    }

    async fn link_invoice_to_contract(
        &self,
        invoice_id: &InvoiceId,
        contract_id: &ContractId,
    ) -> Result<(), GraphError> {
        let db = self.db.clone();
        let iid = invoice_id.to_string();
        let cid = contract_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            writer::link_invoice_to_contract(&conn, &iid, &cid, None)
        })
        .await?
    }

    async fn resolve_vendor(&self, name: &str) -> Result<String, GraphError> {
        let db = self.db.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            writer::resolve_vendor(&conn, &name, None)
        })
        .await?
    }

    async fn invoices_for_vendor(
        &self,
        vendor_id: &str,
    ) -> Result<Vec<InvoiceId>, GraphError> {
        let db = self.db.clone();
        let vid = vendor_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            query::invoices_for_vendor(&conn, &vid)
        })
        .await?
    }

    async fn related_contracts(
        &self,
        invoice_id: &InvoiceId,
    ) -> Result<Vec<ContractId>, GraphError> {
        let db = self.db.clone();
        let iid = invoice_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            query::related_contracts(&conn, &iid)
        })
        .await?
    }

    async fn query_cypher(
        &self,
        cypher: &str,
    ) -> Result<Vec<Vec<String>>, GraphError> {
        let db = self.db.clone();
        let cypher = cypher.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            query::execute_cypher(&conn, &cypher)
        })
        .await?
    }

    async fn load_rdf(&self, path: &str) -> Result<(), GraphError> {
        let db = self.db.clone();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            let cypher = format!("LOAD FROM '{}'", writer::escape_cypher(&path));
            conn.query(&cypher)
                .map_err(|e| GraphError::Write(format!("RDF load failed: {e}")))?;
            Ok(())
        })
        .await?
    }

    async fn rdf_inspect(
        &self,
        procedure: &str,
        path: &str,
    ) -> Result<Vec<Vec<String>>, GraphError> {
        let db = self.db.clone();
        let procedure = procedure.to_string();
        let path = path.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            let cypher = format!(
                "CALL rdf.{}('{}')",
                writer::escape_cypher(&procedure),
                writer::escape_cypher(&path),
            );
            query::execute_cypher(&conn, &cypher)
        })
        .await?
    }

    async fn rdf_nodes(
        &self,
        table_name: &str,
    ) -> Result<Vec<Vec<String>>, GraphError> {
        let db = self.db.clone();
        let table_name = table_name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.connect();
            let cypher = format!("MATCH (n:{table_name}) RETURN n");
            query::execute_cypher(&conn, &cypher)
        })
        .await?
    }
}

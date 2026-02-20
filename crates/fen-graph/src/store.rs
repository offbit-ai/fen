use std::path::Path;
use std::sync::Arc;

use ryugraph::{Connection, Database, SystemConfig};

use fen_core::domain::{
    Contract, ContractId, Invoice, InvoiceId,
};

use crate::error::GraphError;
use crate::query;
use crate::schema;
use crate::writer;
use crate::GraphStore;

/// RyuGraph-backed knowledge graph store.
///
/// RyuGraph's `Connection` is `!Send` (inherited from Kuzu's C++ thread model),
/// so all graph operations run inside `tokio::task::spawn_blocking` with a fresh
/// `Connection` per operation — the same pattern used for redb in the hot tier.
pub struct RyuGraphStore {
    db: Arc<Database>,
}

impl RyuGraphStore {
    /// Create a new persistent graph store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, GraphError> {
        let db = Database::new(path, SystemConfig::default())?;
        {
            let conn = Connection::new(&db)?;
            schema::initialize_schema(&conn)?;
        }
        Ok(Self { db: Arc::new(db) })
    }

    /// Create an in-memory graph store (for testing).
    pub fn in_memory() -> Result<Self, GraphError> {
        let db = Database::in_memory(SystemConfig::default())?;
        {
            let conn = Connection::new(&db)?;
            schema::initialize_schema(&conn)?;
        }
        Ok(Self { db: Arc::new(db) })
    }

}

#[async_trait::async_trait]
impl GraphStore for RyuGraphStore {
    async fn write_invoice(&self, invoice: &Invoice) -> Result<(), GraphError> {
        let db = self.db.clone();
        let invoice = invoice.clone();
        tokio::task::spawn_blocking(move || {
            let conn = Connection::new(&db)?;
            writer::write_invoice(&conn, &invoice)
        })
        .await?
    }

    async fn write_contract(&self, contract: &Contract) -> Result<(), GraphError> {
        let db = self.db.clone();
        let contract = contract.clone();
        tokio::task::spawn_blocking(move || {
            let conn = Connection::new(&db)?;
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
            let conn = Connection::new(&db)?;
            writer::link_invoice_to_contract(&conn, &iid, &cid, None)
        })
        .await?
    }

    async fn resolve_vendor(&self, name: &str) -> Result<String, GraphError> {
        let db = self.db.clone();
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = Connection::new(&db)?;
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
            let conn = Connection::new(&db)?;
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
            let conn = Connection::new(&db)?;
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
            let conn = Connection::new(&db)?;
            query::execute_cypher(&conn, &cypher)
        })
        .await?
    }
}

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;
use crate::traits::DocumentStore;

// Table definitions - key is UUID bytes, value is JSON-serialized data
const INVOICES_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("invoices");
const CONTRACTS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("contracts");

/// redb-based hot storage for ACID transactions
pub struct RedbStorage {
    db: Arc<Database>,
}

impl RedbStorage {
    /// Create a new redb storage at the given path
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let db = Database::create(path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(INVOICES_TABLE)?;
            let _ = write_txn.open_table(CONTRACTS_TABLE)?;
        }
        write_txn.commit()?;

        tracing::info!("Initialized redb storage");

        Ok(Self { db: Arc::new(db) })
    }

    /// Create an in-memory redb storage (for testing)
    pub fn in_memory() -> Result<Self, StorageError> {
        let db = Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(INVOICES_TABLE)?;
            let _ = write_txn.open_table(CONTRACTS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db: Arc::new(db) })
    }

    fn serialize_invoice(invoice: &Invoice) -> Result<Vec<u8>, StorageError> {
        serde_json::to_vec(invoice)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn deserialize_invoice(bytes: &[u8]) -> Result<Invoice, StorageError> {
        serde_json::from_slice(bytes)
            .map_err(|e| StorageError::Deserialization(e.to_string()))
    }

    fn serialize_contract(contract: &Contract) -> Result<Vec<u8>, StorageError> {
        serde_json::to_vec(contract)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn deserialize_contract(bytes: &[u8]) -> Result<Contract, StorageError> {
        serde_json::from_slice(bytes)
            .map_err(|e| StorageError::Deserialization(e.to_string()))
    }
}

#[async_trait]
impl DocumentStore for RedbStorage {
    async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError> {
        let key = invoice.id.as_bytes();
        let value = Self::serialize_invoice(invoice)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(INVOICES_TABLE)?;
            table.insert(key.as_slice(), value.as_slice())?;
        }
        write_txn.commit()?;

        tracing::debug!(invoice_id = %invoice.id, "Stored invoice");
        Ok(())
    }

    async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError> {
        let key = id.as_bytes();

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(INVOICES_TABLE)?;

        match table.get(key.as_slice())? {
            Some(value) => {
                let invoice = Self::deserialize_invoice(value.value())?;
                Ok(Some(invoice))
            }
            None => Ok(None),
        }
    }

    async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
        let key = id.as_bytes();

        let write_txn = self.db.begin_write()?;
        let deleted = {
            let mut table = write_txn.open_table(INVOICES_TABLE)?;
            let result = table.remove(key.as_slice())?.is_some();
            result
        };
        write_txn.commit()?;

        Ok(deleted)
    }

    async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError> {
        let key = contract.id.as_bytes();
        let value = Self::serialize_contract(contract)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CONTRACTS_TABLE)?;
            table.insert(key.as_slice(), value.as_slice())?;
        }
        write_txn.commit()?;

        tracing::debug!(contract_id = %contract.id, "Stored contract");
        Ok(())
    }

    async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError> {
        let key = id.as_bytes();

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTRACTS_TABLE)?;

        match table.get(key.as_slice())? {
            Some(value) => {
                let contract = Self::deserialize_contract(value.value())?;
                Ok(Some(contract))
            }
            None => Ok(None),
        }
    }

    async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError> {
        let key = id.as_bytes();

        let write_txn = self.db.begin_write()?;
        let deleted = {
            let mut table = write_txn.open_table(CONTRACTS_TABLE)?;
            let result = table.remove(key.as_slice())?.is_some();
            result
        };
        write_txn.commit()?;

        Ok(deleted)
    }

    async fn list_invoices(&self, limit: usize, offset: usize) -> Result<Vec<Invoice>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(INVOICES_TABLE)?;

        let invoices: Result<Vec<Invoice>, StorageError> = table
            .iter()?
            .skip(offset)
            .take(limit)
            .map(|result| {
                let (_, value) = result?;
                Self::deserialize_invoice(value.value())
            })
            .collect();

        invoices
    }

    async fn list_contracts(&self, limit: usize, offset: usize) -> Result<Vec<Contract>, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTRACTS_TABLE)?;

        let contracts: Result<Vec<Contract>, StorageError> = table
            .iter()?
            .skip(offset)
            .take(limit)
            .map(|result| {
                let (_, value) = result?;
                Self::deserialize_contract(value.value())
            })
            .collect();

        contracts
    }

    async fn count_invoices(&self) -> Result<usize, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(INVOICES_TABLE)?;
        Ok(table.len()? as usize)
    }

    async fn count_contracts(&self) -> Result<usize, StorageError> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CONTRACTS_TABLE)?;
        Ok(table.len()? as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[tokio::test]
    async fn test_invoice_crud() {
        let storage = RedbStorage::in_memory().unwrap();

        // Create
        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        storage.store_invoice(&invoice).await.unwrap();

        // Read
        let loaded = storage.get_invoice(&invoice.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().invoice_number, "INV-001");

        // Count
        let count = storage.count_invoices().await.unwrap();
        assert_eq!(count, 1);

        // Delete
        let deleted = storage.delete_invoice(&invoice.id).await.unwrap();
        assert!(deleted);

        // Verify deleted
        let loaded = storage.get_invoice(&invoice.id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_contract_crud() {
        let storage = RedbStorage::in_memory().unwrap();

        // Create
        let contract = Contract::new("Test Contract", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        storage.store_contract(&contract).await.unwrap();

        // Read
        let loaded = storage.get_contract(&contract.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().title, "Test Contract");

        // Count
        let count = storage.count_contracts().await.unwrap();
        assert_eq!(count, 1);
    }
}

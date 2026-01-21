use async_trait::async_trait;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

use crate::error::StorageError;

/// Generic document storage operations
#[async_trait]
pub trait DocumentStore: Send + Sync {
    /// Store an invoice
    async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError>;

    /// Retrieve an invoice by ID
    async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError>;

    /// Delete an invoice by ID
    async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError>;

    /// Store a contract
    async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError>;

    /// Retrieve a contract by ID
    async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError>;

    /// Delete a contract by ID
    async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError>;

    /// List invoices with pagination
    async fn list_invoices(&self, limit: usize, offset: usize) -> Result<Vec<Invoice>, StorageError>;

    /// List contracts with pagination
    async fn list_contracts(&self, limit: usize, offset: usize) -> Result<Vec<Contract>, StorageError>;

    /// Count total invoices
    async fn count_invoices(&self) -> Result<usize, StorageError>;

    /// Count total contracts
    async fn count_contracts(&self) -> Result<usize, StorageError>;
}

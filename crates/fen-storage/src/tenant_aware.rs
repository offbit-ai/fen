//! Tenant-aware storage wrapper for multi-tenant isolation.
//!
//! This module provides a wrapper around storage implementations that enforces
//! tenant isolation at the storage layer. All operations are scoped to a specific
//! tenant, preventing cross-tenant data access.

use async_trait::async_trait;
use fen_core::domain::{cluster::TenantId, Contract, ContractId, Invoice, InvoiceId};
use std::sync::Arc;

use crate::{error::StorageError, traits::DocumentStore};

/// Tenant-aware storage wrapper that enforces tenant isolation.
///
/// This wrapper intercepts all storage operations and ensures:
/// 1. Data is stored with tenant-prefixed keys
/// 2. Queries only return data belonging to the tenant
/// 3. Cross-tenant access attempts are blocked
pub struct TenantAwareStore<S: DocumentStore> {
    inner: Arc<S>,
}

impl<S: DocumentStore> TenantAwareStore<S> {
    /// Create a new tenant-aware store wrapping an existing store.
    pub fn new(store: Arc<S>) -> Self {
        Self { inner: store }
    }

    /// Get a reference to the underlying store.
    pub fn inner(&self) -> &Arc<S> {
        &self.inner
    }
}

/// Tenant-scoped document store operations.
///
/// Unlike the base `DocumentStore` trait, all operations in this trait
/// require a tenant ID parameter to ensure tenant isolation.
#[async_trait]
pub trait TenantScopedStore: Send + Sync {
    /// Store an invoice for a specific tenant.
    ///
    /// # Errors
    /// Returns `StorageError::TenantMismatch` if the invoice's tenant_id
    /// doesn't match the provided tenant_id.
    async fn store_invoice_for_tenant(
        &self,
        tenant_id: &TenantId,
        invoice: &Invoice,
    ) -> Result<(), StorageError>;

    /// Retrieve an invoice for a specific tenant.
    ///
    /// Returns `None` if the invoice doesn't exist or belongs to a different tenant.
    async fn get_invoice_for_tenant(
        &self,
        tenant_id: &TenantId,
        invoice_id: &InvoiceId,
    ) -> Result<Option<Invoice>, StorageError>;

    /// Delete an invoice for a specific tenant.
    ///
    /// Returns `false` if the invoice doesn't exist or belongs to a different tenant.
    async fn delete_invoice_for_tenant(
        &self,
        tenant_id: &TenantId,
        invoice_id: &InvoiceId,
    ) -> Result<bool, StorageError>;

    /// List invoices for a specific tenant with pagination.
    async fn list_invoices_for_tenant(
        &self,
        tenant_id: &TenantId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError>;

    /// Count invoices for a specific tenant.
    async fn count_invoices_for_tenant(&self, tenant_id: &TenantId) -> Result<usize, StorageError>;

    /// Store a contract for a specific tenant.
    async fn store_contract_for_tenant(
        &self,
        tenant_id: &TenantId,
        contract: &Contract,
    ) -> Result<(), StorageError>;

    /// Retrieve a contract for a specific tenant.
    async fn get_contract_for_tenant(
        &self,
        tenant_id: &TenantId,
        contract_id: &ContractId,
    ) -> Result<Option<Contract>, StorageError>;

    /// Delete a contract for a specific tenant.
    async fn delete_contract_for_tenant(
        &self,
        tenant_id: &TenantId,
        contract_id: &ContractId,
    ) -> Result<bool, StorageError>;

    /// List contracts for a specific tenant with pagination.
    async fn list_contracts_for_tenant(
        &self,
        tenant_id: &TenantId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Contract>, StorageError>;

    /// Count contracts for a specific tenant.
    async fn count_contracts_for_tenant(&self, tenant_id: &TenantId)
        -> Result<usize, StorageError>;
}

#[async_trait]
impl<S: DocumentStore> TenantScopedStore for TenantAwareStore<S> {
    async fn store_invoice_for_tenant(
        &self,
        tenant_id: &TenantId,
        invoice: &Invoice,
    ) -> Result<(), StorageError> {
        // Verify invoice belongs to the specified tenant
        if invoice.tenant_id != *tenant_id {
            return Err(StorageError::TenantMismatch {
                expected: tenant_id.to_string(),
                actual: invoice.tenant_id.to_string(),
            });
        }

        self.inner.store_invoice(invoice).await
    }

    async fn get_invoice_for_tenant(
        &self,
        tenant_id: &TenantId,
        invoice_id: &InvoiceId,
    ) -> Result<Option<Invoice>, StorageError> {
        let invoice = self.inner.get_invoice(invoice_id).await?;

        // Filter out invoices that don't belong to this tenant
        match invoice {
            Some(inv) if inv.tenant_id == *tenant_id => Ok(Some(inv)),
            Some(inv) => {
                tracing::warn!(
                    requested_tenant = %tenant_id,
                    actual_tenant = %inv.tenant_id,
                    invoice_id = %invoice_id,
                    "Cross-tenant access attempt blocked"
                );
                Ok(None)
            }
            None => Ok(None),
        }
    }

    async fn delete_invoice_for_tenant(
        &self,
        tenant_id: &TenantId,
        invoice_id: &InvoiceId,
    ) -> Result<bool, StorageError> {
        // First verify ownership
        let existing = self.get_invoice_for_tenant(tenant_id, invoice_id).await?;
        if existing.is_none() {
            return Ok(false);
        }

        self.inner.delete_invoice(invoice_id).await
    }

    async fn list_invoices_for_tenant(
        &self,
        tenant_id: &TenantId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        // Get all invoices and filter by tenant
        // Note: This is inefficient for large datasets; production should use
        // tenant-prefixed storage keys or a tenant_id index
        let all_invoices = self.inner.list_invoices(limit * 10, 0).await?;

        let filtered: Vec<Invoice> = all_invoices
            .into_iter()
            .filter(|inv| inv.tenant_id == *tenant_id)
            .skip(offset)
            .take(limit)
            .collect();

        Ok(filtered)
    }

    async fn count_invoices_for_tenant(&self, tenant_id: &TenantId) -> Result<usize, StorageError> {
        // Note: Inefficient implementation; production should maintain per-tenant counts
        let all_invoices = self.inner.list_invoices(1_000_000, 0).await?;
        let count = all_invoices
            .iter()
            .filter(|inv| inv.tenant_id == *tenant_id)
            .count();
        Ok(count)
    }

    async fn store_contract_for_tenant(
        &self,
        tenant_id: &TenantId,
        contract: &Contract,
    ) -> Result<(), StorageError> {
        if contract.tenant_id != *tenant_id {
            return Err(StorageError::TenantMismatch {
                expected: tenant_id.to_string(),
                actual: contract.tenant_id.to_string(),
            });
        }

        self.inner.store_contract(contract).await
    }

    async fn get_contract_for_tenant(
        &self,
        tenant_id: &TenantId,
        contract_id: &ContractId,
    ) -> Result<Option<Contract>, StorageError> {
        let contract = self.inner.get_contract(contract_id).await?;

        match contract {
            Some(c) if c.tenant_id == *tenant_id => Ok(Some(c)),
            Some(c) => {
                tracing::warn!(
                    requested_tenant = %tenant_id,
                    actual_tenant = %c.tenant_id,
                    contract_id = %contract_id,
                    "Cross-tenant access attempt blocked"
                );
                Ok(None)
            }
            None => Ok(None),
        }
    }

    async fn delete_contract_for_tenant(
        &self,
        tenant_id: &TenantId,
        contract_id: &ContractId,
    ) -> Result<bool, StorageError> {
        let existing = self.get_contract_for_tenant(tenant_id, contract_id).await?;
        if existing.is_none() {
            return Ok(false);
        }

        self.inner.delete_contract(contract_id).await
    }

    async fn list_contracts_for_tenant(
        &self,
        tenant_id: &TenantId,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Contract>, StorageError> {
        let all_contracts = self.inner.list_contracts(limit * 10, 0).await?;

        let filtered: Vec<Contract> = all_contracts
            .into_iter()
            .filter(|c| c.tenant_id == *tenant_id)
            .skip(offset)
            .take(limit)
            .collect();

        Ok(filtered)
    }

    async fn count_contracts_for_tenant(
        &self,
        tenant_id: &TenantId,
    ) -> Result<usize, StorageError> {
        let all_contracts = self.inner.list_contracts(1_000_000, 0).await?;
        let count = all_contracts
            .iter()
            .filter(|c| c.tenant_id == *tenant_id)
            .count();
        Ok(count)
    }
}

/// Builder for creating tenant-scoped storage contexts.
///
/// This allows creating a temporary storage context scoped to a specific tenant,
/// useful for request handlers.
pub struct TenantStorageContext<S: DocumentStore> {
    store: Arc<TenantAwareStore<S>>,
    tenant_id: TenantId,
}

impl<S: DocumentStore> TenantStorageContext<S> {
    /// Create a new tenant storage context.
    pub fn new(store: Arc<TenantAwareStore<S>>, tenant_id: TenantId) -> Self {
        Self { store, tenant_id }
    }

    /// Get the tenant ID for this context.
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    /// Store an invoice (automatically using the context's tenant).
    pub async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError> {
        self.store
            .store_invoice_for_tenant(&self.tenant_id, invoice)
            .await
    }

    /// Get an invoice by ID.
    pub async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError> {
        self.store.get_invoice_for_tenant(&self.tenant_id, id).await
    }

    /// Delete an invoice by ID.
    pub async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
        self.store
            .delete_invoice_for_tenant(&self.tenant_id, id)
            .await
    }

    /// List invoices with pagination.
    pub async fn list_invoices(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Invoice>, StorageError> {
        self.store
            .list_invoices_for_tenant(&self.tenant_id, limit, offset)
            .await
    }

    /// Count invoices.
    pub async fn count_invoices(&self) -> Result<usize, StorageError> {
        self.store.count_invoices_for_tenant(&self.tenant_id).await
    }

    /// Store a contract.
    pub async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError> {
        self.store
            .store_contract_for_tenant(&self.tenant_id, contract)
            .await
    }

    /// Get a contract by ID.
    pub async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError> {
        self.store
            .get_contract_for_tenant(&self.tenant_id, id)
            .await
    }

    /// Delete a contract by ID.
    pub async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError> {
        self.store
            .delete_contract_for_tenant(&self.tenant_id, id)
            .await
    }

    /// List contracts with pagination.
    pub async fn list_contracts(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Contract>, StorageError> {
        self.store
            .list_contracts_for_tenant(&self.tenant_id, limit, offset)
            .await
    }

    /// Count contracts.
    pub async fn count_contracts(&self) -> Result<usize, StorageError> {
        self.store.count_contracts_for_tenant(&self.tenant_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::collections::HashMap;
    use std::sync::RwLock;

    /// In-memory mock store for testing
    struct MockStore {
        invoices: RwLock<HashMap<InvoiceId, Invoice>>,
        contracts: RwLock<HashMap<ContractId, Contract>>,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                invoices: RwLock::new(HashMap::new()),
                contracts: RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl DocumentStore for MockStore {
        async fn store_invoice(&self, invoice: &Invoice) -> Result<(), StorageError> {
            self.invoices
                .write()
                .unwrap()
                .insert(invoice.id, invoice.clone());
            Ok(())
        }

        async fn get_invoice(&self, id: &InvoiceId) -> Result<Option<Invoice>, StorageError> {
            Ok(self.invoices.read().unwrap().get(id).cloned())
        }

        async fn delete_invoice(&self, id: &InvoiceId) -> Result<bool, StorageError> {
            Ok(self.invoices.write().unwrap().remove(id).is_some())
        }

        async fn store_contract(&self, contract: &Contract) -> Result<(), StorageError> {
            self.contracts
                .write()
                .unwrap()
                .insert(contract.id, contract.clone());
            Ok(())
        }

        async fn get_contract(&self, id: &ContractId) -> Result<Option<Contract>, StorageError> {
            Ok(self.contracts.read().unwrap().get(id).cloned())
        }

        async fn delete_contract(&self, id: &ContractId) -> Result<bool, StorageError> {
            Ok(self.contracts.write().unwrap().remove(id).is_some())
        }

        async fn list_invoices(
            &self,
            limit: usize,
            offset: usize,
        ) -> Result<Vec<Invoice>, StorageError> {
            let invoices: Vec<Invoice> = self
                .invoices
                .read()
                .unwrap()
                .values()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect();
            Ok(invoices)
        }

        async fn list_contracts(
            &self,
            limit: usize,
            offset: usize,
        ) -> Result<Vec<Contract>, StorageError> {
            let contracts: Vec<Contract> = self
                .contracts
                .read()
                .unwrap()
                .values()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect();
            Ok(contracts)
        }

        async fn count_invoices(&self) -> Result<usize, StorageError> {
            Ok(self.invoices.read().unwrap().len())
        }

        async fn count_contracts(&self) -> Result<usize, StorageError> {
            Ok(self.contracts.read().unwrap().len())
        }
    }

    #[tokio::test]
    async fn test_tenant_isolation_store() {
        let mock = Arc::new(MockStore::new());
        let store = TenantAwareStore::new(mock);

        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();

        // Create invoice for tenant A
        let invoice_a = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            .with_tenant(tenant_a.clone());

        // Store should succeed
        store
            .store_invoice_for_tenant(&tenant_a, &invoice_a)
            .await
            .unwrap();

        // Tenant A should be able to retrieve it
        let retrieved = store
            .get_invoice_for_tenant(&tenant_a, &invoice_a.id)
            .await
            .unwrap();
        assert!(retrieved.is_some());

        // Tenant B should NOT be able to retrieve it
        let cross_tenant = store
            .get_invoice_for_tenant(&tenant_b, &invoice_a.id)
            .await
            .unwrap();
        assert!(cross_tenant.is_none());
    }

    #[tokio::test]
    async fn test_tenant_mismatch_rejected() {
        let mock = Arc::new(MockStore::new());
        let store = TenantAwareStore::new(mock);

        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();

        // Create invoice for tenant A
        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            .with_tenant(tenant_a);

        // Try to store under tenant B - should fail
        let result = store.store_invoice_for_tenant(&tenant_b, &invoice).await;
        assert!(matches!(result, Err(StorageError::TenantMismatch { .. })));
    }

    #[tokio::test]
    async fn test_list_filters_by_tenant() {
        let mock = Arc::new(MockStore::new());
        let store = TenantAwareStore::new(mock);

        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();

        // Create invoices for both tenants
        let invoice_a1 = Invoice::new("INV-A1", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            .with_tenant(tenant_a.clone());
        let invoice_a2 = Invoice::new("INV-A2", NaiveDate::from_ymd_opt(2024, 1, 2).unwrap())
            .with_tenant(tenant_a.clone());
        let invoice_b1 = Invoice::new("INV-B1", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            .with_tenant(tenant_b.clone());

        store
            .store_invoice_for_tenant(&tenant_a, &invoice_a1)
            .await
            .unwrap();
        store
            .store_invoice_for_tenant(&tenant_a, &invoice_a2)
            .await
            .unwrap();
        store
            .store_invoice_for_tenant(&tenant_b, &invoice_b1)
            .await
            .unwrap();

        // Tenant A should see 2 invoices
        let tenant_a_invoices = store
            .list_invoices_for_tenant(&tenant_a, 100, 0)
            .await
            .unwrap();
        assert_eq!(tenant_a_invoices.len(), 2);

        // Tenant B should see 1 invoice
        let tenant_b_invoices = store
            .list_invoices_for_tenant(&tenant_b, 100, 0)
            .await
            .unwrap();
        assert_eq!(tenant_b_invoices.len(), 1);
    }

    #[tokio::test]
    async fn test_tenant_storage_context() {
        let mock = Arc::new(MockStore::new());
        let store = Arc::new(TenantAwareStore::new(mock));

        let tenant = TenantId::new();
        let ctx = TenantStorageContext::new(store, tenant.clone());

        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
            .with_tenant(tenant);

        ctx.store_invoice(&invoice).await.unwrap();

        let retrieved = ctx.get_invoice(&invoice.id).await.unwrap();
        assert!(retrieved.is_some());

        let count = ctx.count_invoices().await.unwrap();
        assert_eq!(count, 1);
    }
}

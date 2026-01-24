//! Document location index for tracking which tier each document resides in.
//!
//! This index provides O(1) lookups for document locations and ensures accurate
//! counting across tiers without duplicates.

use std::collections::HashSet;

use dashmap::DashMap;

use fen_core::domain::{ContractId, InvoiceId};

use crate::traits::StorageTier;

/// Tracks the current storage tier for each document.
///
/// This index is the single source of truth for document locations,
/// preventing double-counting during tier migrations.
pub struct DocumentLocationIndex {
    /// Invoice ID -> current tier
    invoices: DashMap<InvoiceId, StorageTier>,
    /// Contract ID -> current tier
    contracts: DashMap<ContractId, StorageTier>,
}

impl DocumentLocationIndex {
    /// Create a new empty location index
    pub fn new() -> Self {
        Self {
            invoices: DashMap::new(),
            contracts: DashMap::new(),
        }
    }

    /// Register an invoice in a specific tier
    pub fn register_invoice(&self, id: InvoiceId, tier: StorageTier) {
        self.invoices.insert(id, tier);
    }

    /// Get the tier for an invoice
    pub fn get_invoice_tier(&self, id: &InvoiceId) -> Option<StorageTier> {
        self.invoices.get(id).map(|r| *r)
    }

    /// Move an invoice to a new tier (atomic operation)
    pub fn move_invoice(&self, id: &InvoiceId, new_tier: StorageTier) -> Option<StorageTier> {
        self.invoices.insert(*id, new_tier)
    }

    /// Remove an invoice from the index
    pub fn remove_invoice(&self, id: &InvoiceId) -> Option<StorageTier> {
        self.invoices.remove(id).map(|(_, tier)| tier)
    }

    /// Check if an invoice exists in the index
    pub fn contains_invoice(&self, id: &InvoiceId) -> bool {
        self.invoices.contains_key(id)
    }

    /// Register a contract in a specific tier
    pub fn register_contract(&self, id: ContractId, tier: StorageTier) {
        self.contracts.insert(id, tier);
    }

    /// Get the tier for a contract
    pub fn get_contract_tier(&self, id: &ContractId) -> Option<StorageTier> {
        self.contracts.get(id).map(|r| *r)
    }

    /// Move a contract to a new tier (atomic operation)
    pub fn move_contract(&self, id: &ContractId, new_tier: StorageTier) -> Option<StorageTier> {
        self.contracts.insert(*id, new_tier)
    }

    /// Remove a contract from the index
    pub fn remove_contract(&self, id: &ContractId) -> Option<StorageTier> {
        self.contracts.remove(id).map(|(_, tier)| tier)
    }

    /// Check if a contract exists in the index
    pub fn contains_contract(&self, id: &ContractId) -> bool {
        self.contracts.contains_key(id)
    }

    /// Count invoices (deduplicated - each invoice counted exactly once)
    pub fn count_invoices(&self) -> usize {
        self.invoices.len()
    }

    /// Count contracts (deduplicated - each contract counted exactly once)
    pub fn count_contracts(&self) -> usize {
        self.contracts.len()
    }

    /// Count invoices in a specific tier
    pub fn count_invoices_in_tier(&self, tier: StorageTier) -> usize {
        self.invoices.iter().filter(|r| *r.value() == tier).count()
    }

    /// Count contracts in a specific tier
    pub fn count_contracts_in_tier(&self, tier: StorageTier) -> usize {
        self.contracts.iter().filter(|r| *r.value() == tier).count()
    }

    /// Get all invoice IDs in a specific tier
    pub fn invoices_in_tier(&self, tier: StorageTier) -> Vec<InvoiceId> {
        self.invoices
            .iter()
            .filter(|r| *r.value() == tier)
            .map(|r| *r.key())
            .collect()
    }

    /// Get all contract IDs in a specific tier
    pub fn contracts_in_tier(&self, tier: StorageTier) -> Vec<ContractId> {
        self.contracts
            .iter()
            .filter(|r| *r.value() == tier)
            .map(|r| *r.key())
            .collect()
    }

    /// Get tier distribution for invoices
    pub fn invoice_tier_distribution(&self) -> TierDistribution {
        let mut hot = 0;
        let mut warm = 0;
        let mut cold = 0;

        for entry in self.invoices.iter() {
            match *entry.value() {
                StorageTier::Hot => hot += 1,
                StorageTier::Warm => warm += 1,
                StorageTier::Cold => cold += 1,
            }
        }

        TierDistribution { hot, warm, cold }
    }

    /// Get tier distribution for contracts
    pub fn contract_tier_distribution(&self) -> TierDistribution {
        let mut hot = 0;
        let mut warm = 0;
        let mut cold = 0;

        for entry in self.contracts.iter() {
            match *entry.value() {
                StorageTier::Hot => hot += 1,
                StorageTier::Warm => warm += 1,
                StorageTier::Cold => cold += 1,
            }
        }

        TierDistribution { hot, warm, cold }
    }

    /// Get all tiers that contain at least one invoice
    pub fn invoice_tiers(&self) -> HashSet<StorageTier> {
        self.invoices.iter().map(|r| *r.value()).collect()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.invoices.clear();
        self.contracts.clear();
    }
}

impl Default for DocumentLocationIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Distribution of documents across tiers
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TierDistribution {
    pub hot: usize,
    pub warm: usize,
    pub cold: usize,
}

impl TierDistribution {
    /// Total documents across all tiers
    pub fn total(&self) -> usize {
        self.hot + self.warm + self.cold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fen_core::domain::Invoice;

    #[test]
    fn test_register_and_get() {
        let index = DocumentLocationIndex::new();
        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());

        index.register_invoice(invoice.id, StorageTier::Hot);

        assert_eq!(index.get_invoice_tier(&invoice.id), Some(StorageTier::Hot));
        assert!(index.contains_invoice(&invoice.id));
    }

    #[test]
    fn test_move_tier() {
        let index = DocumentLocationIndex::new();
        let invoice = Invoice::new("INV-002", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());

        index.register_invoice(invoice.id, StorageTier::Hot);
        assert_eq!(index.get_invoice_tier(&invoice.id), Some(StorageTier::Hot));

        let old_tier = index.move_invoice(&invoice.id, StorageTier::Warm);
        assert_eq!(old_tier, Some(StorageTier::Hot));
        assert_eq!(index.get_invoice_tier(&invoice.id), Some(StorageTier::Warm));
    }

    #[test]
    fn test_count_no_duplicates() {
        let index = DocumentLocationIndex::new();

        let inv1 = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        let inv2 = Invoice::new("INV-002", NaiveDate::from_ymd_opt(2024, 1, 2).unwrap());
        let inv3 = Invoice::new("INV-003", NaiveDate::from_ymd_opt(2024, 1, 3).unwrap());

        index.register_invoice(inv1.id, StorageTier::Hot);
        index.register_invoice(inv2.id, StorageTier::Hot);
        index.register_invoice(inv3.id, StorageTier::Warm);

        // Total count is exactly 3, not hot_count + warm_count
        assert_eq!(index.count_invoices(), 3);
        assert_eq!(index.count_invoices_in_tier(StorageTier::Hot), 2);
        assert_eq!(index.count_invoices_in_tier(StorageTier::Warm), 1);
    }

    #[test]
    fn test_tier_distribution() {
        let index = DocumentLocationIndex::new();

        for i in 0..5 {
            let inv = Invoice::new(
                format!("HOT-{}", i),
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            );
            index.register_invoice(inv.id, StorageTier::Hot);
        }
        for i in 0..3 {
            let inv = Invoice::new(
                format!("WARM-{}", i),
                NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            );
            index.register_invoice(inv.id, StorageTier::Warm);
        }

        let dist = index.invoice_tier_distribution();
        assert_eq!(dist.hot, 5);
        assert_eq!(dist.warm, 3);
        assert_eq!(dist.cold, 0);
        assert_eq!(dist.total(), 8);
    }

    #[test]
    fn test_remove() {
        let index = DocumentLocationIndex::new();
        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());

        index.register_invoice(invoice.id, StorageTier::Hot);
        assert_eq!(index.count_invoices(), 1);

        let removed_tier = index.remove_invoice(&invoice.id);
        assert_eq!(removed_tier, Some(StorageTier::Hot));
        assert_eq!(index.count_invoices(), 0);
        assert!(!index.contains_invoice(&invoice.id));
    }
}

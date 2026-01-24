use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use dashmap::DashMap;

use fen_core::domain::{Contract, ContractId, Invoice, InvoiceId};

/// Time-to-live for cached entries
const DEFAULT_TTL_SECS: u64 = 300; // 5 minutes

/// Cached entry with expiration
struct CacheEntry<T> {
    value: T,
    inserted_at: Instant,
    ttl: Duration,
}

impl<T: Clone> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }

    fn get(&self) -> Option<T> {
        if self.is_expired() {
            None
        } else {
            Some(self.value.clone())
        }
    }
}

/// LRU-style query cache with TTL
pub struct QueryCache {
    invoices: DashMap<InvoiceId, CacheEntry<Invoice>>,
    contracts: DashMap<ContractId, CacheEntry<Contract>>,
    max_entries: usize,
    ttl: Duration,
    hits: AtomicUsize,
    misses: AtomicUsize,
}

impl QueryCache {
    /// Create a new query cache with the specified max entries
    pub fn new(max_entries: usize) -> Self {
        Self {
            invoices: DashMap::new(),
            contracts: DashMap::new(),
            max_entries,
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }

    /// Create a cache with custom TTL
    pub fn with_ttl(max_entries: usize, ttl_secs: u64) -> Self {
        Self {
            invoices: DashMap::new(),
            contracts: DashMap::new(),
            max_entries,
            ttl: Duration::from_secs(ttl_secs),
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
        }
    }

    /// Get an invoice from cache
    pub fn get_invoice(&self, id: &InvoiceId) -> Option<Invoice> {
        match self.invoices.get(id) {
            Some(entry) => {
                if let Some(value) = entry.get() {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    Some(value)
                } else {
                    // Entry expired, remove it
                    drop(entry);
                    self.invoices.remove(id);
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Put an invoice in cache
    pub fn put_invoice(&self, invoice: Invoice) {
        // Evict if at capacity
        if self.invoices.len() >= self.max_entries {
            self.evict_invoices();
        }

        self.invoices
            .insert(invoice.id, CacheEntry::new(invoice, self.ttl));
    }

    /// Invalidate a cached invoice
    pub fn invalidate_invoice(&self, id: &InvoiceId) {
        self.invoices.remove(id);
    }

    /// Get a contract from cache
    pub fn get_contract(&self, id: &ContractId) -> Option<Contract> {
        match self.contracts.get(id) {
            Some(entry) => {
                if let Some(value) = entry.get() {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    Some(value)
                } else {
                    drop(entry);
                    self.contracts.remove(id);
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Put a contract in cache
    pub fn put_contract(&self, contract: Contract) {
        if self.contracts.len() >= self.max_entries {
            self.evict_contracts();
        }

        self.contracts
            .insert(contract.id, CacheEntry::new(contract, self.ttl));
    }

    /// Invalidate a cached contract
    pub fn invalidate_contract(&self, id: &ContractId) {
        self.contracts.remove(id);
    }

    /// Evict expired entries from invoice cache
    fn evict_invoices(&self) {
        // Use retain to remove expired entries (more efficient with DashMap)
        self.invoices.retain(|_, v| !v.is_expired());

        // If still at capacity, remove entries until under limit
        // Collect all keys first to avoid holding locks during iteration
        if self.invoices.len() >= self.max_entries {
            let keys: Vec<_> = self.invoices.iter().map(|e| *e.key()).collect();
            // Remove entries until we're under the limit
            for key in keys {
                if self.invoices.len() < self.max_entries {
                    break;
                }
                self.invoices.remove(&key);
            }
        }
    }

    /// Evict expired entries from contract cache
    fn evict_contracts(&self) {
        // Use retain to remove expired entries (more efficient with DashMap)
        self.contracts.retain(|_, v| !v.is_expired());

        // If still at capacity, remove entries until under limit
        if self.contracts.len() >= self.max_entries {
            let keys: Vec<_> = self.contracts.iter().map(|e| *e.key()).collect();
            for key in keys {
                if self.contracts.len() < self.max_entries {
                    break;
                }
                self.contracts.remove(&key);
            }
        }
    }

    /// Clear all cached entries
    pub fn clear(&self) {
        self.invoices.clear();
        self.contracts.clear();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    /// Get cache statistics (hits, misses)
    pub fn stats(&self) -> (usize, usize) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    /// Get hit rate as a percentage
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;

        if total == 0.0 {
            0.0
        } else {
            (hits / total) * 100.0
        }
    }

    /// Get current cache sizes
    pub fn sizes(&self) -> (usize, usize) {
        (self.invoices.len(), self.contracts.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_cache_invoice() {
        let cache = QueryCache::new(100);

        let invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        let id = invoice.id;

        // Not in cache
        assert!(cache.get_invoice(&id).is_none());

        // Put in cache
        cache.put_invoice(invoice.clone());

        // Should be in cache
        let cached = cache.get_invoice(&id);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().invoice_number, "INV-001");
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = QueryCache::new(100);

        let invoice = Invoice::new("INV-002", NaiveDate::from_ymd_opt(2024, 1, 20).unwrap());
        let id = invoice.id;

        cache.put_invoice(invoice);
        assert!(cache.get_invoice(&id).is_some());

        cache.invalidate_invoice(&id);
        assert!(cache.get_invoice(&id).is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = QueryCache::new(100);

        let invoice = Invoice::new("INV-003", NaiveDate::from_ymd_opt(2024, 1, 25).unwrap());
        let id = invoice.id;

        // Miss
        cache.get_invoice(&id);
        assert_eq!(cache.stats(), (0, 1));

        // Put and hit
        cache.put_invoice(invoice);
        cache.get_invoice(&id);
        assert_eq!(cache.stats(), (1, 1));
    }

    #[test]
    fn test_cache_ttl() {
        let cache = QueryCache::with_ttl(100, 0); // Immediate expiry

        let invoice = Invoice::new("INV-004", NaiveDate::from_ymd_opt(2024, 1, 30).unwrap());
        let id = invoice.id;

        cache.put_invoice(invoice);

        // Should be expired immediately
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(cache.get_invoice(&id).is_none());
    }

    #[test]
    fn test_cache_eviction() {
        let cache = QueryCache::new(2);

        let inv1 = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        let inv2 = Invoice::new("INV-002", NaiveDate::from_ymd_opt(2024, 1, 2).unwrap());
        let inv3 = Invoice::new("INV-003", NaiveDate::from_ymd_opt(2024, 1, 3).unwrap());

        cache.put_invoice(inv1);
        cache.put_invoice(inv2);

        assert_eq!(cache.sizes().0, 2);

        // Adding third should trigger eviction
        cache.put_invoice(inv3);
        assert!(cache.sizes().0 <= 2);
    }
}

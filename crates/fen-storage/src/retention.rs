//! Retention policy enforcement for multi-tenant data lifecycle management.
//!
//! This module provides mechanisms for enforcing per-tenant data retention policies
//! across the tiered storage architecture (hot -> warm -> archive -> delete).

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use fen_core::domain::{
    cluster::TenantId,
    tenant_config::{RetentionPolicy, TenantConfig},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::StorageError;
use crate::traits::StorageTier;

/// Statistics from a retention cleanup operation.
#[derive(Debug, Default, Clone)]
pub struct RetentionStats {
    /// Number of items moved from hot to warm storage
    pub hot_to_warm: usize,
    /// Number of items moved from warm to archive storage
    pub warm_to_archive: usize,
    /// Number of items soft-deleted (marked as deleted)
    pub soft_deleted: usize,
    /// Number of items hard-deleted (permanently removed)
    pub hard_deleted: usize,
    /// Total bytes reclaimed
    pub bytes_reclaimed: u64,
    /// Errors encountered during cleanup
    pub errors: usize,
}

impl RetentionStats {
    /// Merge stats from another cleanup operation.
    pub fn merge(&mut self, other: &RetentionStats) {
        self.hot_to_warm += other.hot_to_warm;
        self.warm_to_archive += other.warm_to_archive;
        self.soft_deleted += other.soft_deleted;
        self.hard_deleted += other.hard_deleted;
        self.bytes_reclaimed += other.bytes_reclaimed;
        self.errors += other.errors;
    }
}

/// Tracks document timestamps for retention decisions.
#[derive(Debug, Clone)]
pub struct DocumentMetadata {
    /// When the document was created/ingested
    pub created_at: DateTime<Utc>,
    /// When the document was last accessed
    pub last_accessed_at: DateTime<Utc>,
    /// Current storage tier
    pub tier: StorageTier,
    /// Size in bytes
    pub size_bytes: u64,
    /// Whether the document is soft-deleted
    pub deleted: bool,
}

/// Service for managing retention-related document metadata.
#[async_trait]
pub trait RetentionMetadataStore: Send + Sync {
    /// Get metadata for a document.
    async fn get_metadata(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<Option<DocumentMetadata>, StorageError>;

    /// Update metadata for a document.
    async fn set_metadata(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        resource_id: &str,
        metadata: DocumentMetadata,
    ) -> Result<(), StorageError>;

    /// List documents older than a given timestamp in a specific tier.
    async fn list_documents_before(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        tier: StorageTier,
        before: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<(String, DocumentMetadata)>, StorageError>;

    /// Mark a document as deleted (soft delete).
    async fn mark_deleted(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<(), StorageError>;
}

/// In-memory implementation of retention metadata store.
///
/// Production deployments should use a persistent store.
pub struct InMemoryRetentionMetadata {
    /// Map of tenant_id -> resource_type -> resource_id -> metadata
    #[allow(clippy::type_complexity)]
    data: RwLock<HashMap<String, HashMap<String, HashMap<String, DocumentMetadata>>>>,
}

impl InMemoryRetentionMetadata {
    /// Create a new in-memory metadata store.
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    fn make_key(tenant_id: &TenantId, resource_type: &str) -> String {
        format!("{}:{}", tenant_id, resource_type)
    }
}

impl Default for InMemoryRetentionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RetentionMetadataStore for InMemoryRetentionMetadata {
    async fn get_metadata(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<Option<DocumentMetadata>, StorageError> {
        let data = self.data.read().await;
        let key = Self::make_key(tenant_id, resource_type);
        Ok(data
            .get(&key)
            .and_then(|types| types.get(resource_type))
            .and_then(|resources| resources.get(resource_id))
            .cloned())
    }

    async fn set_metadata(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        resource_id: &str,
        metadata: DocumentMetadata,
    ) -> Result<(), StorageError> {
        let mut data = self.data.write().await;
        let key = Self::make_key(tenant_id, resource_type);
        data.entry(key)
            .or_default()
            .entry(resource_type.to_string())
            .or_default()
            .insert(resource_id.to_string(), metadata);
        Ok(())
    }

    async fn list_documents_before(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        tier: StorageTier,
        before: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<(String, DocumentMetadata)>, StorageError> {
        let data = self.data.read().await;
        let key = Self::make_key(tenant_id, resource_type);

        let results: Vec<(String, DocumentMetadata)> = data
            .get(&key)
            .and_then(|types| types.get(resource_type))
            .map(|resources| {
                resources
                    .iter()
                    .filter(|(_, meta)| {
                        meta.tier == tier && meta.created_at < before && !meta.deleted
                    })
                    .take(limit)
                    .map(|(id, meta)| (id.clone(), meta.clone()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }

    async fn mark_deleted(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<(), StorageError> {
        let mut data = self.data.write().await;
        let key = Self::make_key(tenant_id, resource_type);

        if let Some(types) = data.get_mut(&key) {
            if let Some(resources) = types.get_mut(resource_type) {
                if let Some(meta) = resources.get_mut(resource_id) {
                    meta.deleted = true;
                }
            }
        }
        Ok(())
    }
}

/// Retention policy enforcer that manages data lifecycle across storage tiers.
pub struct RetentionEnforcer<M: RetentionMetadataStore> {
    metadata_store: Arc<M>,
    tenant_configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
}

impl<M: RetentionMetadataStore> RetentionEnforcer<M> {
    /// Create a new retention enforcer.
    pub fn new(
        metadata_store: Arc<M>,
        tenant_configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>>,
    ) -> Self {
        Self {
            metadata_store,
            tenant_configs,
        }
    }

    /// Run retention cleanup for all tenants.
    ///
    /// This should be called periodically (e.g., daily) by a background job.
    pub async fn run_cleanup(&self) -> Result<RetentionStats, StorageError> {
        let configs = self.tenant_configs.read().await;
        let mut total_stats = RetentionStats::default();

        for (tenant_id, config) in configs.iter() {
            match self.cleanup_tenant(tenant_id, config).await {
                Ok(stats) => {
                    info!(
                        tenant_id = %tenant_id,
                        hot_to_warm = stats.hot_to_warm,
                        warm_to_archive = stats.warm_to_archive,
                        soft_deleted = stats.soft_deleted,
                        hard_deleted = stats.hard_deleted,
                        "Retention cleanup completed for tenant"
                    );
                    total_stats.merge(&stats);
                }
                Err(e) => {
                    warn!(
                        tenant_id = %tenant_id,
                        error = %e,
                        "Retention cleanup failed for tenant"
                    );
                    total_stats.errors += 1;
                }
            }
        }

        info!(
            total_hot_to_warm = total_stats.hot_to_warm,
            total_warm_to_archive = total_stats.warm_to_archive,
            total_soft_deleted = total_stats.soft_deleted,
            total_hard_deleted = total_stats.hard_deleted,
            total_bytes_reclaimed = total_stats.bytes_reclaimed,
            errors = total_stats.errors,
            "Global retention cleanup completed"
        );

        Ok(total_stats)
    }

    /// Run retention cleanup for a specific tenant.
    async fn cleanup_tenant(
        &self,
        tenant_id: &TenantId,
        config: &TenantConfig,
    ) -> Result<RetentionStats, StorageError> {
        let mut stats = RetentionStats::default();
        let now = Utc::now();

        // Process each resource type with its retention policy
        for (resource_type, policy) in &config.retention_policies {
            let type_stats = self
                .cleanup_resource_type(tenant_id, resource_type, policy, now)
                .await?;
            stats.merge(&type_stats);
        }

        // Also process default retention for resource types without explicit policy
        let default_policy = RetentionPolicy::default();
        for resource_type in &["invoice", "contract", "document", "anomaly"] {
            if !config.retention_policies.contains_key(*resource_type) {
                let type_stats = self
                    .cleanup_resource_type(tenant_id, resource_type, &default_policy, now)
                    .await?;
                stats.merge(&type_stats);
            }
        }

        Ok(stats)
    }

    /// Apply retention policy to a specific resource type.
    async fn cleanup_resource_type(
        &self,
        tenant_id: &TenantId,
        resource_type: &str,
        policy: &RetentionPolicy,
        now: DateTime<Utc>,
    ) -> Result<RetentionStats, StorageError> {
        let mut stats = RetentionStats::default();

        // Hot -> Warm transition
        let hot_cutoff = now - Duration::days(i64::from(policy.hot_retention_days));
        let hot_candidates = self
            .metadata_store
            .list_documents_before(tenant_id, resource_type, StorageTier::Hot, hot_cutoff, 1000)
            .await?;

        for (resource_id, mut metadata) in hot_candidates {
            debug!(
                tenant_id = %tenant_id,
                resource_type = %resource_type,
                resource_id = %resource_id,
                "Moving document from hot to warm storage"
            );
            // In production, this would actually move data between tiers
            metadata.tier = StorageTier::Warm;
            self.metadata_store
                .set_metadata(tenant_id, resource_type, &resource_id, metadata)
                .await?;
            stats.hot_to_warm += 1;
        }

        // Warm -> Archive transition
        let warm_cutoff = now - Duration::days(i64::from(policy.warm_retention_days));
        let warm_candidates = self
            .metadata_store
            .list_documents_before(
                tenant_id,
                resource_type,
                StorageTier::Warm,
                warm_cutoff,
                1000,
            )
            .await?;

        for (resource_id, mut metadata) in warm_candidates {
            debug!(
                tenant_id = %tenant_id,
                resource_type = %resource_type,
                resource_id = %resource_id,
                "Moving document from warm to archive storage"
            );
            metadata.tier = StorageTier::Cold;
            self.metadata_store
                .set_metadata(tenant_id, resource_type, &resource_id, metadata)
                .await?;
            stats.warm_to_archive += 1;
        }

        // Archive deletion
        let archive_cutoff = now - Duration::days(i64::from(policy.archive_retention_days));
        let archive_candidates = self
            .metadata_store
            .list_documents_before(
                tenant_id,
                resource_type,
                StorageTier::Cold,
                archive_cutoff,
                1000,
            )
            .await?;

        for (resource_id, metadata) in archive_candidates {
            if policy.hard_delete {
                debug!(
                    tenant_id = %tenant_id,
                    resource_type = %resource_type,
                    resource_id = %resource_id,
                    "Hard deleting archived document"
                );
                // In production, this would actually delete the data
                stats.hard_deleted += 1;
                stats.bytes_reclaimed += metadata.size_bytes;
            } else {
                debug!(
                    tenant_id = %tenant_id,
                    resource_type = %resource_type,
                    resource_id = %resource_id,
                    "Soft deleting archived document"
                );
                self.metadata_store
                    .mark_deleted(tenant_id, resource_type, &resource_id)
                    .await?;
                stats.soft_deleted += 1;
            }
        }

        Ok(stats)
    }

    /// Get the current tier for a document based on its age and policy.
    pub fn determine_tier(
        &self,
        created_at: DateTime<Utc>,
        policy: &RetentionPolicy,
    ) -> StorageTier {
        let now = Utc::now();
        let age = now - created_at;

        if age < Duration::days(i64::from(policy.hot_retention_days)) {
            StorageTier::Hot
        } else if age < Duration::days(i64::from(policy.warm_retention_days)) {
            StorageTier::Warm
        } else {
            StorageTier::Cold
        }
    }

    /// Check if a document should be deleted based on its age and policy.
    pub fn should_delete(&self, created_at: DateTime<Utc>, policy: &RetentionPolicy) -> bool {
        let now = Utc::now();
        let age = now - created_at;
        age >= Duration::days(i64::from(policy.archive_retention_days))
    }
}

/// Background job for periodic retention enforcement.
pub struct RetentionJob<M: RetentionMetadataStore> {
    enforcer: Arc<RetentionEnforcer<M>>,
    interval: std::time::Duration,
}

impl<M: RetentionMetadataStore + 'static> RetentionJob<M> {
    /// Create a new retention job.
    pub fn new(enforcer: Arc<RetentionEnforcer<M>>, interval: std::time::Duration) -> Self {
        Self { enforcer, interval }
    }

    /// Start the retention job as a background task.
    ///
    /// Returns a handle that can be used to cancel the job.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                info!("Starting scheduled retention cleanup");
                match self.enforcer.run_cleanup().await {
                    Ok(stats) => {
                        info!(
                            hot_to_warm = stats.hot_to_warm,
                            warm_to_archive = stats.warm_to_archive,
                            deleted = stats.soft_deleted + stats.hard_deleted,
                            "Retention cleanup job completed"
                        );
                    }
                    Err(e) => {
                        warn!(error = %e, "Retention cleanup job failed");
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fen_core::domain::tenant_config::TenantConfig;

    #[tokio::test]
    async fn test_retention_tier_determination() {
        let metadata = Arc::new(InMemoryRetentionMetadata::new());
        let configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let enforcer = RetentionEnforcer::new(metadata, configs);

        let policy = RetentionPolicy {
            hot_retention_days: 30,
            warm_retention_days: 90,
            archive_retention_days: 365,
            hard_delete: false,
        };

        // Recent document should be Hot
        let recent = Utc::now() - Duration::days(5);
        assert_eq!(enforcer.determine_tier(recent, &policy), StorageTier::Hot);

        // 60-day old document should be Warm
        let warm_age = Utc::now() - Duration::days(60);
        assert_eq!(
            enforcer.determine_tier(warm_age, &policy),
            StorageTier::Warm
        );

        // 200-day old document should be Cold
        let cold_age = Utc::now() - Duration::days(200);
        assert_eq!(
            enforcer.determine_tier(cold_age, &policy),
            StorageTier::Cold
        );
    }

    #[tokio::test]
    async fn test_should_delete() {
        let metadata = Arc::new(InMemoryRetentionMetadata::new());
        let configs: Arc<RwLock<HashMap<TenantId, TenantConfig>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let enforcer = RetentionEnforcer::new(metadata, configs);

        let policy = RetentionPolicy {
            hot_retention_days: 30,
            warm_retention_days: 90,
            archive_retention_days: 365,
            hard_delete: true,
        };

        // Recent document should not be deleted
        let recent = Utc::now() - Duration::days(100);
        assert!(!enforcer.should_delete(recent, &policy));

        // Old document should be deleted
        let old = Utc::now() - Duration::days(400);
        assert!(enforcer.should_delete(old, &policy));
    }

    #[tokio::test]
    async fn test_metadata_store() {
        let store = InMemoryRetentionMetadata::new();
        let tenant_id = TenantId::new();

        let metadata = DocumentMetadata {
            created_at: Utc::now() - Duration::days(10),
            last_accessed_at: Utc::now(),
            tier: StorageTier::Hot,
            size_bytes: 1024,
            deleted: false,
        };

        store
            .set_metadata(&tenant_id, "invoice", "inv-001", metadata.clone())
            .await
            .unwrap();

        let retrieved = store
            .get_metadata(&tenant_id, "invoice", "inv-001")
            .await
            .unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().tier, StorageTier::Hot);
    }
}

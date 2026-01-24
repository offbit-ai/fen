//! Cluster domain types for distributed Fen deployment.
//!
//! These types enable multi-tenant, sharded storage with consistent hashing
//! for partition assignment across a distributed cluster.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a tenant in multi-tenant deployments.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub Uuid);

impl TenantId {
    /// Create a new random tenant ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse a tenant ID from a string.
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    /// Get the bytes of the tenant ID for hashing.
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a shard in the distributed storage layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShardId(pub u32);

impl ShardId {
    /// Create a new shard ID.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the shard ID as a u32.
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl fmt::Display for ShardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shard-{}", self.0)
    }
}

/// Unique identifier for a node in the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    /// Create a new random node ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse a node ID from a string.
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Document type for partition key computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum DocumentType {
    /// Invoice document.
    Invoice = 0,
    /// Contract document.
    Contract = 1,
}

impl DocumentType {
    /// Get the document type as a byte for hashing.
    pub fn as_byte(&self) -> u8 {
        *self as u8
    }
}

impl fmt::Display for DocumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentType::Invoice => write!(f, "invoice"),
            DocumentType::Contract => write!(f, "contract"),
        }
    }
}

/// Partition key for consistent hashing.
///
/// Used to determine which shard a document belongs to based on
/// tenant ID and document type. This ensures all documents of the
/// same type for a tenant are co-located on the same shard.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartitionKey {
    /// The tenant this document belongs to.
    pub tenant_id: TenantId,
    /// The type of document.
    pub document_type: DocumentType,
}

impl PartitionKey {
    /// Create a new partition key.
    pub fn new(tenant_id: TenantId, document_type: DocumentType) -> Self {
        Self {
            tenant_id,
            document_type,
        }
    }

    /// Convert the partition key to bytes for hashing.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(17); // 16 bytes UUID + 1 byte doc type
        bytes.extend_from_slice(self.tenant_id.as_bytes());
        bytes.push(self.document_type.as_byte());
        bytes
    }

    /// Compute the shard ID for this partition key using XXH3 consistent hashing.
    ///
    /// # Arguments
    /// * `num_shards` - Total number of shards in the cluster
    ///
    /// # Returns
    /// The shard ID where documents with this partition key should be stored.
    #[cfg(feature = "cluster")]
    pub fn shard_id(&self, num_shards: u32) -> ShardId {
        use xxhash_rust::xxh3::xxh3_64;
        let hash = xxh3_64(&self.to_bytes());
        ShardId((hash % num_shards as u64) as u32)
    }

    /// Compute the shard ID without xxhash (fallback for non-cluster builds).
    #[cfg(not(feature = "cluster"))]
    pub fn shard_id(&self, num_shards: u32) -> ShardId {
        // Simple fallback hash using std
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();
        ShardId((hash % num_shards as u64) as u32)
    }
}

impl fmt::Display for PartitionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.tenant_id, self.document_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_id_creation() {
        let tenant1 = TenantId::new();
        let tenant2 = TenantId::new();
        assert_ne!(tenant1, tenant2);
    }

    #[test]
    fn test_tenant_id_from_string() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let tenant = TenantId::from_string(uuid_str).unwrap();
        assert_eq!(tenant.to_string(), uuid_str);
    }

    #[test]
    fn test_shard_id() {
        let shard = ShardId::new(5);
        assert_eq!(shard.as_u32(), 5);
        assert_eq!(shard.to_string(), "shard-5");
    }

    #[test]
    fn test_partition_key_to_bytes() {
        let tenant = TenantId::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = PartitionKey::new(tenant, DocumentType::Invoice);
        let bytes = key.to_bytes();
        assert_eq!(bytes.len(), 17);
        assert_eq!(bytes[16], 0); // Invoice = 0
    }

    #[test]
    fn test_partition_key_shard_assignment() {
        let tenant = TenantId::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = PartitionKey::new(tenant, DocumentType::Invoice);

        // Same key should always map to same shard
        let shard1 = key.shard_id(16);
        let shard2 = key.shard_id(16);
        assert_eq!(shard1, shard2);

        // Shard should be within range
        assert!(shard1.as_u32() < 16);
    }

    #[test]
    fn test_document_type_display() {
        assert_eq!(DocumentType::Invoice.to_string(), "invoice");
        assert_eq!(DocumentType::Contract.to_string(), "contract");
    }
}

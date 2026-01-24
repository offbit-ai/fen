//! Consistent hashing and routing table for shard assignment.
//!
//! This module implements consistent hashing using XXH3 to distribute
//! tenants and documents across shards while minimizing rebalancing
//! when cluster membership changes.

use fen_core::domain::{NodeId, PartitionKey, ShardId, TenantId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// A node's assignment of shards.
#[derive(Debug, Clone)]
pub struct NodeAssignment {
    /// The node ID.
    pub node_id: NodeId,
    /// Shards assigned to this node (primary).
    pub primary_shards: Vec<ShardId>,
    /// Shards this node holds as replica.
    pub replica_shards: Vec<ShardId>,
    /// Node address for gRPC communication.
    pub address: String,
    /// Whether the node is healthy.
    pub healthy: bool,
}

/// Routing table for shard-to-node mapping.
///
/// The routing table maintains the current assignment of shards to nodes
/// and provides efficient lookup for request routing.
#[derive(Debug)]
pub struct RoutingTable {
    /// Total number of shards in the cluster.
    num_shards: u32,
    /// Replication factor (number of copies per shard).
    replication_factor: u32,
    /// Shard to primary node mapping.
    shard_to_primary: HashMap<ShardId, NodeId>,
    /// Shard to replica nodes mapping.
    shard_to_replicas: HashMap<ShardId, Vec<NodeId>>,
    /// Node assignments.
    node_assignments: HashMap<NodeId, NodeAssignment>,
    /// Version number for optimistic locking.
    version: u64,
}

impl RoutingTable {
    /// Create a new routing table.
    pub fn new(num_shards: u32, replication_factor: u32) -> Self {
        Self {
            num_shards,
            replication_factor,
            shard_to_primary: HashMap::new(),
            shard_to_replicas: HashMap::new(),
            node_assignments: HashMap::new(),
            version: 0,
        }
    }

    /// Get the total number of shards.
    pub fn num_shards(&self) -> u32 {
        self.num_shards
    }

    /// Get the replication factor.
    pub fn replication_factor(&self) -> u32 {
        self.replication_factor
    }

    /// Get the current version.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get the primary node for a shard.
    pub fn get_primary(&self, shard_id: &ShardId) -> Option<&NodeId> {
        self.shard_to_primary.get(shard_id)
    }

    /// Get replica nodes for a shard.
    pub fn get_replicas(&self, shard_id: &ShardId) -> Option<&Vec<NodeId>> {
        self.shard_to_replicas.get(shard_id)
    }

    /// Get all nodes that hold a shard (primary + replicas).
    pub fn get_all_nodes(&self, shard_id: &ShardId) -> Vec<NodeId> {
        let mut nodes = Vec::new();
        if let Some(primary) = self.shard_to_primary.get(shard_id) {
            nodes.push(*primary);
        }
        if let Some(replicas) = self.shard_to_replicas.get(shard_id) {
            nodes.extend(replicas.iter().cloned());
        }
        nodes
    }

    /// Get node assignment by node ID.
    pub fn get_node_assignment(&self, node_id: &NodeId) -> Option<&NodeAssignment> {
        self.node_assignments.get(node_id)
    }

    /// List all known nodes.
    pub fn list_nodes(&self) -> Vec<&NodeAssignment> {
        self.node_assignments.values().collect()
    }

    /// List all healthy nodes.
    pub fn list_healthy_nodes(&self) -> Vec<&NodeAssignment> {
        self.node_assignments
            .values()
            .filter(|n| n.healthy)
            .collect()
    }

    /// Add a node to the routing table.
    pub fn add_node(&mut self, assignment: NodeAssignment) {
        info!(
            node_id = %assignment.node_id,
            address = %assignment.address,
            primary_shards = ?assignment.primary_shards,
            "Adding node to routing table"
        );

        // Update shard mappings
        for shard_id in &assignment.primary_shards {
            self.shard_to_primary.insert(*shard_id, assignment.node_id);
        }
        for shard_id in &assignment.replica_shards {
            self.shard_to_replicas
                .entry(*shard_id)
                .or_default()
                .push(assignment.node_id);
        }

        self.node_assignments.insert(assignment.node_id, assignment);
        self.version += 1;
    }

    /// Remove a node from the routing table.
    pub fn remove_node(&mut self, node_id: &NodeId) -> Option<NodeAssignment> {
        if let Some(assignment) = self.node_assignments.remove(node_id) {
            info!(node_id = %node_id, "Removing node from routing table");

            // Clean up shard mappings
            for shard_id in &assignment.primary_shards {
                self.shard_to_primary.remove(shard_id);
            }
            for shard_id in &assignment.replica_shards {
                if let Some(replicas) = self.shard_to_replicas.get_mut(shard_id) {
                    replicas.retain(|n| n != node_id);
                }
            }

            self.version += 1;
            Some(assignment)
        } else {
            None
        }
    }

    /// Mark a node as healthy or unhealthy.
    pub fn set_node_health(&mut self, node_id: &NodeId, healthy: bool) {
        if let Some(assignment) = self.node_assignments.get_mut(node_id) {
            if assignment.healthy != healthy {
                debug!(
                    node_id = %node_id,
                    healthy = healthy,
                    "Node health status changed"
                );
                assignment.healthy = healthy;
                self.version += 1;
            }
        }
    }

    /// Compute shard ID for a partition key using consistent hashing.
    pub fn compute_shard(&self, key: &PartitionKey) -> ShardId {
        key.shard_id(self.num_shards)
    }

    /// Compute shard ID for a tenant ID.
    pub fn compute_tenant_shard(&self, tenant_id: &TenantId) -> ShardId {
        use xxhash_rust::xxh3::xxh3_64;
        let hash = xxh3_64(tenant_id.0.as_bytes());
        ShardId((hash % self.num_shards as u64) as u32)
    }
}

/// Thread-safe routing table manager.
pub struct RoutingTableManager {
    table: Arc<RwLock<RoutingTable>>,
}

impl RoutingTableManager {
    /// Create a new routing table manager.
    pub fn new(num_shards: u32, replication_factor: u32) -> Self {
        Self {
            table: Arc::new(RwLock::new(RoutingTable::new(
                num_shards,
                replication_factor,
            ))),
        }
    }

    /// Get read access to the routing table.
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, RoutingTable> {
        self.table.read().await
    }

    /// Get write access to the routing table.
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, RoutingTable> {
        self.table.write().await
    }

    /// Route a partition key to its primary node.
    pub async fn route_to_primary(&self, key: &PartitionKey) -> Option<NodeAssignment> {
        let table = self.table.read().await;
        let shard_id = table.compute_shard(key);
        table
            .get_primary(&shard_id)
            .and_then(|node_id| table.get_node_assignment(node_id))
            .cloned()
    }

    /// Route a tenant to its primary node.
    pub async fn route_tenant_to_primary(&self, tenant_id: &TenantId) -> Option<NodeAssignment> {
        let table = self.table.read().await;
        let shard_id = table.compute_tenant_shard(tenant_id);
        table
            .get_primary(&shard_id)
            .and_then(|node_id| table.get_node_assignment(node_id))
            .cloned()
    }

    /// Get all nodes for a shard (for queries that need to check replicas).
    pub async fn get_shard_nodes(&self, shard_id: &ShardId) -> Vec<NodeAssignment> {
        let table = self.table.read().await;
        table
            .get_all_nodes(shard_id)
            .iter()
            .filter_map(|node_id| table.get_node_assignment(node_id).cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_table_add_remove_node() {
        let mut table = RoutingTable::new(16, 3);

        let node_id = NodeId::new();
        let assignment = NodeAssignment {
            node_id,
            primary_shards: vec![ShardId(0), ShardId(1), ShardId(2)],
            replica_shards: vec![ShardId(5), ShardId(6)],
            address: "localhost:9000".to_string(),
            healthy: true,
        };

        table.add_node(assignment);

        assert!(table.get_node_assignment(&node_id).is_some());
        assert_eq!(table.get_primary(&ShardId(0)), Some(&node_id));
        assert_eq!(table.version(), 1);

        table.remove_node(&node_id);
        assert!(table.get_node_assignment(&node_id).is_none());
        assert_eq!(table.get_primary(&ShardId(0)), None);
        assert_eq!(table.version(), 2);
    }

    #[test]
    fn test_routing_table_health_status() {
        let mut table = RoutingTable::new(16, 3);

        let node_id = NodeId::new();
        let assignment = NodeAssignment {
            node_id,
            primary_shards: vec![ShardId(0)],
            replica_shards: vec![],
            address: "localhost:9000".to_string(),
            healthy: true,
        };

        table.add_node(assignment);
        assert!(table.get_node_assignment(&node_id).unwrap().healthy);

        table.set_node_health(&node_id, false);
        assert!(!table.get_node_assignment(&node_id).unwrap().healthy);

        assert_eq!(table.list_healthy_nodes().len(), 0);
    }

    #[test]
    fn test_consistent_hashing() {
        let table = RoutingTable::new(16, 3);
        let tenant_id = TenantId::new();

        // Same tenant should always hash to same shard
        let shard1 = table.compute_tenant_shard(&tenant_id);
        let shard2 = table.compute_tenant_shard(&tenant_id);
        assert_eq!(shard1, shard2);

        // Shard should be within bounds
        assert!(shard1.0 < 16);
    }

    #[tokio::test]
    async fn test_routing_table_manager() {
        let manager = RoutingTableManager::new(16, 3);

        let node_id = NodeId::new();
        let assignment = NodeAssignment {
            node_id,
            primary_shards: vec![ShardId(0)],
            replica_shards: vec![],
            address: "localhost:9000".to_string(),
            healthy: true,
        };

        {
            let mut table = manager.write().await;
            table.add_node(assignment);
        }

        let nodes = manager.get_shard_nodes(&ShardId(0)).await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, node_id);
    }
}

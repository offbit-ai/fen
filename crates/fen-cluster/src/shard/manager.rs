//! Shard manager for request routing and shard lifecycle.
//!
//! The shard manager is responsible for:
//! - Routing requests to the appropriate shard/node
//! - Managing local shard handles
//! - Maintaining connections to remote nodes
//! - Coordinating shard rebalancing

use async_trait::async_trait;
use dashmap::DashMap;
use fen_core::domain::{NodeId, PartitionKey, ShardId, TenantId};
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

use super::routing::{NodeAssignment, RoutingTableManager};

/// Errors that can occur during shard operations.
#[derive(Debug, Error)]
pub enum ShardError {
    /// Shard not found.
    #[error("Shard {0:?} not found")]
    ShardNotFound(ShardId),

    /// Node not available.
    #[error("Node {0:?} not available")]
    NodeNotAvailable(NodeId),

    /// No healthy nodes for shard.
    #[error("No healthy nodes for shard {0:?}")]
    NoHealthyNodes(ShardId),

    /// Routing error.
    #[error("Routing error: {0}")]
    RoutingError(String),

    /// Storage error.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Connection error.
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Handle to a local shard's storage.
///
/// This trait abstracts the storage operations that can be performed
/// on a local shard. Implementations should be thread-safe.
#[async_trait]
pub trait LocalShardHandle: Send + Sync {
    /// Get the shard ID.
    fn shard_id(&self) -> ShardId;

    /// Check if the shard is ready to serve requests.
    fn is_ready(&self) -> bool;

    /// Perform a health check on the shard.
    async fn health_check(&self) -> Result<(), ShardError>;

    /// Get approximate size in bytes.
    async fn size_bytes(&self) -> Result<u64, ShardError>;

    /// Get number of documents in the shard.
    async fn document_count(&self) -> Result<u64, ShardError>;
}

/// Handle to a remote shard (via gRPC).
///
/// This trait abstracts the operations that can be performed
/// on a remote shard through gRPC calls.
#[async_trait]
pub trait RemoteShardClient: Send + Sync {
    /// Get the node this client connects to.
    fn node_id(&self) -> &NodeId;

    /// Get the node's address.
    fn address(&self) -> &str;

    /// Check connection health.
    async fn health_check(&self) -> Result<(), ShardError>;

    /// Check if connection is ready.
    fn is_connected(&self) -> bool;
}

/// Configuration for the shard manager.
#[derive(Debug, Clone)]
pub struct ShardManagerConfig {
    /// This node's ID.
    pub node_id: NodeId,
    /// Total number of shards in the cluster.
    pub num_shards: u32,
    /// Replication factor (number of copies per shard).
    pub replication_factor: u32,
    /// Health check interval in seconds.
    pub health_check_interval_secs: u64,
}

impl Default for ShardManagerConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::new(),
            num_shards: 16,
            replication_factor: 3,
            health_check_interval_secs: 10,
        }
    }
}

/// Shard manager for routing and shard lifecycle management.
pub struct ShardManager {
    /// This node's ID.
    node_id: NodeId,
    /// Routing table manager.
    routing: Arc<RoutingTableManager>,
    /// Local shard handles.
    local_shards: DashMap<ShardId, Arc<dyn LocalShardHandle>>,
    /// Remote shard clients (by node ID).
    remote_clients: DashMap<NodeId, Arc<dyn RemoteShardClient>>,
}

impl ShardManager {
    /// Create a new shard manager.
    pub fn new(config: ShardManagerConfig) -> Self {
        Self {
            node_id: config.node_id,
            routing: Arc::new(RoutingTableManager::new(
                config.num_shards,
                config.replication_factor,
            )),
            local_shards: DashMap::new(),
            remote_clients: DashMap::new(),
        }
    }

    /// Get this node's ID.
    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// Get the routing table manager.
    pub fn routing(&self) -> &Arc<RoutingTableManager> {
        &self.routing
    }

    /// Register a local shard handle.
    pub fn register_local_shard(&self, handle: Arc<dyn LocalShardHandle>) {
        let shard_id = handle.shard_id();
        info!(shard_id = ?shard_id, "Registering local shard");
        self.local_shards.insert(shard_id, handle);
    }

    /// Unregister a local shard handle.
    pub fn unregister_local_shard(&self, shard_id: &ShardId) -> Option<Arc<dyn LocalShardHandle>> {
        self.local_shards.remove(shard_id).map(|(_, v)| v)
    }

    /// Get a local shard handle.
    pub fn get_local_shard(&self, shard_id: &ShardId) -> Option<Arc<dyn LocalShardHandle>> {
        self.local_shards.get(shard_id).map(|v| v.clone())
    }

    /// Check if a shard is local.
    pub fn is_local_shard(&self, shard_id: &ShardId) -> bool {
        self.local_shards.contains_key(shard_id)
    }

    /// List all local shard IDs.
    pub fn list_local_shards(&self) -> Vec<ShardId> {
        self.local_shards.iter().map(|r| *r.key()).collect()
    }

    /// Register a remote shard client.
    pub fn register_remote_client(&self, client: Arc<dyn RemoteShardClient>) {
        let node_id = *client.node_id();
        info!(node_id = ?node_id, address = %client.address(), "Registering remote client");
        self.remote_clients.insert(node_id, client);
    }

    /// Unregister a remote shard client.
    pub fn unregister_remote_client(&self, node_id: &NodeId) -> Option<Arc<dyn RemoteShardClient>> {
        self.remote_clients.remove(node_id).map(|(_, v)| v)
    }

    /// Get a remote shard client.
    pub fn get_remote_client(&self, node_id: &NodeId) -> Option<Arc<dyn RemoteShardClient>> {
        self.remote_clients.get(node_id).map(|v| v.clone())
    }

    /// Route a request to the appropriate handler.
    ///
    /// Returns either a local shard handle or information about which
    /// remote node to contact.
    pub async fn route(&self, key: &PartitionKey) -> Result<ShardRoute, ShardError> {
        let table = self.routing.read().await;
        let shard_id = table.compute_shard(key);

        // Check if local
        if let Some(handle) = self.local_shards.get(&shard_id) {
            return Ok(ShardRoute::Local(handle.clone()));
        }

        // Find remote node
        if let Some(node_id) = table.get_primary(&shard_id) {
            if let Some(client) = self.remote_clients.get(node_id) {
                return Ok(ShardRoute::Remote {
                    shard_id,
                    node_id: *node_id,
                    client: client.clone(),
                });
            }

            // Node exists but we don't have a client
            return Err(ShardError::ConnectionError(format!(
                "No connection to node {:?}",
                node_id
            )));
        }

        Err(ShardError::ShardNotFound(shard_id))
    }

    /// Route by tenant ID.
    pub async fn route_tenant(&self, tenant_id: &TenantId) -> Result<ShardRoute, ShardError> {
        let table = self.routing.read().await;
        let shard_id = table.compute_tenant_shard(tenant_id);

        // Check if local
        if let Some(handle) = self.local_shards.get(&shard_id) {
            return Ok(ShardRoute::Local(handle.clone()));
        }

        // Find remote node
        if let Some(node_id) = table.get_primary(&shard_id) {
            if let Some(client) = self.remote_clients.get(node_id) {
                return Ok(ShardRoute::Remote {
                    shard_id,
                    node_id: *node_id,
                    client: client.clone(),
                });
            }
        }

        Err(ShardError::ShardNotFound(shard_id))
    }

    /// Get healthy nodes for a shard (for read queries that can use replicas).
    pub async fn get_healthy_nodes_for_shard(
        &self,
        shard_id: &ShardId,
    ) -> Result<Vec<ShardRoute>, ShardError> {
        let table = self.routing.read().await;
        let nodes = table.get_all_nodes(shard_id);

        if nodes.is_empty() {
            return Err(ShardError::ShardNotFound(*shard_id));
        }

        let mut routes = Vec::new();

        for node_id in nodes {
            // Check if local
            if node_id == self.node_id {
                if let Some(handle) = self.local_shards.get(shard_id) {
                    if handle.is_ready() {
                        routes.push(ShardRoute::Local(handle.clone()));
                    }
                }
            } else {
                // Check remote
                if let Some(client) = self.remote_clients.get(&node_id) {
                    if client.is_connected() {
                        routes.push(ShardRoute::Remote {
                            shard_id: *shard_id,
                            node_id,
                            client: client.clone(),
                        });
                    }
                }
            }
        }

        if routes.is_empty() {
            Err(ShardError::NoHealthyNodes(*shard_id))
        } else {
            Ok(routes)
        }
    }

    /// Run health checks on all local shards.
    pub async fn health_check_local_shards(&self) -> Vec<(ShardId, Result<(), ShardError>)> {
        let mut results = Vec::new();

        for entry in self.local_shards.iter() {
            let shard_id = *entry.key();
            let result = entry.value().health_check().await;

            if result.is_err() {
                warn!(shard_id = ?shard_id, "Local shard health check failed");
            }

            results.push((shard_id, result));
        }

        results
    }

    /// Run health checks on all remote connections.
    pub async fn health_check_remote_clients(&self) -> Vec<(NodeId, Result<(), ShardError>)> {
        let mut results = Vec::new();

        for entry in self.remote_clients.iter() {
            let node_id = *entry.key();
            let result = entry.value().health_check().await;

            if result.is_err() {
                warn!(node_id = ?node_id, "Remote client health check failed");
            }

            results.push((node_id, result));
        }

        results
    }

    /// Update the routing table with new node assignments.
    pub async fn update_routing_table(&self, assignments: Vec<NodeAssignment>) {
        let mut table = self.routing.write().await;

        for assignment in assignments {
            table.add_node(assignment);
        }

        info!(version = table.version(), "Routing table updated");
    }

    /// Handle node failure by updating routing and cleaning up connections.
    pub async fn handle_node_failure(&self, node_id: &NodeId) {
        warn!(node_id = ?node_id, "Handling node failure");

        // Update routing table health status
        {
            let mut table = self.routing.write().await;
            table.set_node_health(node_id, false);
        }

        // Remove the remote client
        self.unregister_remote_client(node_id);
    }

    /// Get cluster statistics.
    pub async fn cluster_stats(&self) -> ClusterStats {
        let table = self.routing.read().await;

        ClusterStats {
            num_shards: table.num_shards(),
            replication_factor: table.replication_factor(),
            local_shards: self.local_shards.len(),
            remote_connections: self.remote_clients.len(),
            routing_table_version: table.version(),
            healthy_nodes: table.list_healthy_nodes().len(),
            total_nodes: table.list_nodes().len(),
        }
    }
}

/// Result of routing a request.
pub enum ShardRoute {
    /// The shard is local.
    Local(Arc<dyn LocalShardHandle>),
    /// The shard is on a remote node.
    Remote {
        shard_id: ShardId,
        node_id: NodeId,
        client: Arc<dyn RemoteShardClient>,
    },
}

impl std::fmt::Debug for ShardRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShardRoute::Local(handle) => f
                .debug_struct("ShardRoute::Local")
                .field("shard_id", &handle.shard_id())
                .finish(),
            ShardRoute::Remote {
                shard_id,
                node_id,
                client,
            } => f
                .debug_struct("ShardRoute::Remote")
                .field("shard_id", shard_id)
                .field("node_id", node_id)
                .field("address", &client.address())
                .finish(),
        }
    }
}

/// Cluster statistics.
#[derive(Debug, Clone)]
pub struct ClusterStats {
    /// Total number of shards.
    pub num_shards: u32,
    /// Replication factor.
    pub replication_factor: u32,
    /// Number of local shards on this node.
    pub local_shards: usize,
    /// Number of active remote connections.
    pub remote_connections: usize,
    /// Routing table version.
    pub routing_table_version: u64,
    /// Number of healthy nodes.
    pub healthy_nodes: usize,
    /// Total nodes known.
    pub total_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLocalShard {
        shard_id: ShardId,
    }

    #[async_trait]
    impl LocalShardHandle for MockLocalShard {
        fn shard_id(&self) -> ShardId {
            self.shard_id
        }

        fn is_ready(&self) -> bool {
            true
        }

        async fn health_check(&self) -> Result<(), ShardError> {
            Ok(())
        }

        async fn size_bytes(&self) -> Result<u64, ShardError> {
            Ok(0)
        }

        async fn document_count(&self) -> Result<u64, ShardError> {
            Ok(0)
        }
    }

    #[test]
    fn test_shard_manager_creation() {
        let config = ShardManagerConfig::default();
        let manager = ShardManager::new(config);

        assert!(manager.list_local_shards().is_empty());
    }

    #[test]
    fn test_register_local_shard() {
        let manager = ShardManager::new(ShardManagerConfig::default());
        let shard = Arc::new(MockLocalShard {
            shard_id: ShardId(0),
        });

        manager.register_local_shard(shard);

        assert!(manager.is_local_shard(&ShardId(0)));
        assert!(!manager.is_local_shard(&ShardId(1)));
    }

    #[tokio::test]
    async fn test_cluster_stats() {
        let manager = ShardManager::new(ShardManagerConfig::default());
        let shard = Arc::new(MockLocalShard {
            shard_id: ShardId(0),
        });

        manager.register_local_shard(shard);

        let stats = manager.cluster_stats().await;
        assert_eq!(stats.local_shards, 1);
        assert_eq!(stats.num_shards, 16);
    }
}

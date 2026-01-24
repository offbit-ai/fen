//! Connection pool for efficient client management.

use crate::client::{ShardClient, ShardClientConfig, ShardClientError, ShardClientWrapper};
use dashmap::DashMap;
use fen_cluster::RemoteShardClient;
use fen_core::domain::NodeId;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Configuration for the connection pool.
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Default connection timeout.
    pub connect_timeout: Duration,
    /// Default request timeout.
    pub request_timeout: Duration,
    /// Keep-alive interval.
    pub keepalive_interval: Duration,
    /// Maximum connections per node.
    pub max_connections_per_node: usize,
    /// Connection idle timeout before removal.
    pub idle_timeout: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(10),
            max_connections_per_node: 4,
            idle_timeout: Duration::from_secs(300),
        }
    }
}

/// Connection pool for managing client connections to remote nodes.
pub struct ConnectionPool {
    /// Configuration.
    config: ConnectionPoolConfig,
    /// Active connections by node ID.
    connections: DashMap<NodeId, Arc<ShardClientWrapper>>,
    /// Node addresses.
    addresses: DashMap<NodeId, String>,
}

impl ConnectionPool {
    /// Create a new connection pool.
    pub fn new(config: ConnectionPoolConfig) -> Self {
        Self {
            config,
            connections: DashMap::new(),
            addresses: DashMap::new(),
        }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ConnectionPoolConfig::default())
    }

    /// Register a node's address.
    pub fn register_node(&self, node_id: NodeId, address: String) {
        info!(node_id = ?node_id, address = %address, "Registering node in connection pool");
        self.addresses.insert(node_id, address);
    }

    /// Unregister a node and close its connections.
    pub fn unregister_node(&self, node_id: &NodeId) {
        self.addresses.remove(node_id);
        self.connections.remove(node_id);
        info!(node_id = ?node_id, "Unregistered node from connection pool");
    }

    /// Get or create a connection to a node.
    pub async fn get_connection(
        &self,
        node_id: &NodeId,
    ) -> Result<Arc<ShardClientWrapper>, ShardClientError> {
        // Check for existing connection
        if let Some(conn) = self.connections.get(node_id) {
            return Ok(conn.clone());
        }

        // Get address
        let address = self
            .addresses
            .get(node_id)
            .map(|a| a.clone())
            .ok_or_else(|| {
                ShardClientError::ConnectionFailed(format!("Unknown node: {:?}", node_id))
            })?;

        // Create new connection
        let config = ShardClientConfig {
            address,
            node_id: node_id.clone(),
            connect_timeout: self.config.connect_timeout,
            request_timeout: self.config.request_timeout,
            keepalive_interval: self.config.keepalive_interval,
            ..Default::default()
        };

        let client = ShardClient::connect(config).await?;
        let wrapper = Arc::new(ShardClientWrapper::new(client));

        // Store and return
        self.connections.insert(node_id.clone(), wrapper.clone());

        Ok(wrapper)
    }

    /// Get a connection as RemoteShardClient trait object.
    pub async fn get_remote_client(
        &self,
        node_id: &NodeId,
    ) -> Result<Arc<dyn RemoteShardClient>, ShardClientError> {
        let wrapper = self.get_connection(node_id).await?;
        Ok(wrapper as Arc<dyn RemoteShardClient>)
    }

    /// Remove a connection (e.g., after failure).
    pub fn remove_connection(&self, node_id: &NodeId) {
        self.connections.remove(node_id);
        debug!(node_id = ?node_id, "Removed connection from pool");
    }

    /// Get number of active connections.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get number of registered nodes.
    pub fn node_count(&self) -> usize {
        self.addresses.len()
    }

    /// List all registered node IDs.
    pub fn list_nodes(&self) -> Vec<NodeId> {
        self.addresses.iter().map(|r| r.key().clone()).collect()
    }

    /// Check health of all connections.
    pub async fn health_check_all(&self) -> Vec<(NodeId, Result<(), String>)> {
        let mut results = Vec::new();

        for entry in self.connections.iter() {
            let node_id = entry.key().clone();
            let conn = entry.value().clone();

            match conn.health_check().await {
                Ok(()) => results.push((node_id, Ok(()))),
                Err(e) => {
                    warn!(node_id = ?node_id, error = %e, "Connection health check failed");
                    results.push((node_id, Err(e.to_string())));
                }
            }
        }

        results
    }

    /// Remove unhealthy connections.
    pub async fn cleanup_unhealthy(&self) {
        let health_results = self.health_check_all().await;

        for (node_id, result) in health_results {
            if result.is_err() {
                self.remove_connection(&node_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let pool = ConnectionPool::with_defaults();
        assert_eq!(pool.connection_count(), 0);
        assert_eq!(pool.node_count(), 0);
    }

    #[test]
    fn test_register_node() {
        let pool = ConnectionPool::with_defaults();
        let node_id = NodeId::new();

        pool.register_node(node_id.clone(), "http://localhost:9000".to_string());

        assert_eq!(pool.node_count(), 1);
        assert!(pool.list_nodes().contains(&node_id));
    }

    #[test]
    fn test_unregister_node() {
        let pool = ConnectionPool::with_defaults();
        let node_id = NodeId::new();

        pool.register_node(node_id.clone(), "http://localhost:9000".to_string());
        pool.unregister_node(&node_id);

        assert_eq!(pool.node_count(), 0);
    }
}

//! Cluster management for distributed Fen.
//!
//! This crate provides the infrastructure for running Fen as a distributed system:
//!
//! - **Shard Management**: Consistent hashing for data distribution, routing
//!   tables, and request routing to local or remote shards
//! - **Transaction Coordination**: Two-phase commit (2PC) protocol for cross-shard
//!   transactions ensuring ACID guarantees
//! - **Consensus**: Raft-based leader election and cluster membership management
//!   (with the `raft` feature)
//!
//! # Features
//!
//! - `raft` - Enable Raft consensus protocol support (requires `openraft` crate)
//!
//! # Example
//!
//! ```ignore
//! use fen_cluster::{ShardManager, ShardManagerConfig, TransactionCoordinator};
//! use fen_core::domain::ShardId;
//!
//! // Create shard manager
//! let config = ShardManagerConfig {
//!     num_shards: 16,
//!     replication_factor: 3,
//!     ..Default::default()
//! };
//! let manager = ShardManager::new(config);
//!
//! // Create transaction coordinator
//! let coordinator = TransactionCoordinator::default_config();
//!
//! // Route requests through the shard manager
//! let route = manager.route(&partition_key).await?;
//! ```

pub mod consensus;
pub mod shard;
pub mod transaction;

pub use shard::{
    ClusterStats, LocalShardHandle, NodeAssignment, RemoteShardClient, RoutingTable,
    RoutingTableManager, ShardError, ShardManager, ShardManagerConfig, ShardRoute,
};

pub use transaction::{
    Transaction, TransactionCoordinator, TransactionCoordinatorConfig, TransactionError,
    TransactionId, TransactionParticipant, TransactionState,
};

#[cfg(feature = "raft")]
pub use consensus::raft::{
    ClusterHealth, ClusterMember, RaftConfig, RaftCoordinator, RaftError, RaftState,
};

#[cfg(test)]
mod tests {
    use super::*;
    use fen_core::domain::{DocumentType, NodeId, PartitionKey, ShardId, TenantId};
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_shard_routing_integration() {
        let config = ShardManagerConfig {
            num_shards: 16,
            replication_factor: 3,
            ..Default::default()
        };
        let manager = ShardManager::new(config);

        // Add some nodes to the routing table
        let node1 = NodeId::new();
        let assignment = NodeAssignment {
            node_id: node1.clone(),
            primary_shards: vec![ShardId(0), ShardId(1), ShardId(2)],
            replica_shards: vec![],
            address: "localhost:9000".to_string(),
            healthy: true,
        };

        {
            let mut table = manager.routing().write().await;
            table.add_node(assignment);
        }

        // Verify routing table was updated
        let stats = manager.cluster_stats().await;
        assert_eq!(stats.total_nodes, 1);
    }

    #[tokio::test]
    async fn test_transaction_coordination_integration() {
        use async_trait::async_trait;

        struct MockParticipant {
            shard_id: ShardId,
        }

        #[async_trait]
        impl TransactionParticipant for MockParticipant {
            fn shard_id(&self) -> ShardId {
                self.shard_id.clone()
            }

            async fn prepare(&self, _tx_id: &TransactionId) -> Result<(), TransactionError> {
                Ok(())
            }

            async fn commit(&self, _tx_id: &TransactionId) -> Result<(), TransactionError> {
                Ok(())
            }

            async fn abort(&self, _tx_id: &TransactionId) -> Result<(), TransactionError> {
                Ok(())
            }
        }

        let coordinator = TransactionCoordinator::default_config();

        // Register participants
        coordinator.register_participant(std::sync::Arc::new(MockParticipant {
            shard_id: ShardId(0),
        }));
        coordinator.register_participant(std::sync::Arc::new(MockParticipant {
            shard_id: ShardId(1),
        }));

        // Create and execute transaction
        let mut participants = HashSet::new();
        participants.insert(ShardId(0));
        participants.insert(ShardId(1));

        let tx = coordinator.begin(participants).unwrap();
        let result = coordinator.execute_2pc(&tx.id).await;

        assert!(result.is_ok());

        let state = coordinator.get_state(&tx.id).await;
        assert_eq!(state, Some(TransactionState::Committed));
    }

    #[test]
    fn test_partition_key_sharding() {
        let tenant = TenantId::new();
        let key = PartitionKey {
            tenant_id: tenant.clone(),
            document_type: DocumentType::Invoice,
        };

        // Same key should always hash to same shard
        let shard1 = key.shard_id(16);
        let shard2 = key.shard_id(16);
        assert_eq!(shard1, shard2);

        // Shard should be within bounds
        assert!(shard1.0 < 16);
    }
}

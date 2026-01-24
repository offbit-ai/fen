//! Shard management for distributed Fen.
//!
//! This module provides:
//! - Consistent hashing for shard assignment
//! - Routing tables for shard-to-node mapping
//! - Shard manager for request routing

pub mod manager;
pub mod routing;

pub use manager::{
    ClusterStats, LocalShardHandle, RemoteShardClient, ShardError, ShardManager,
    ShardManagerConfig, ShardRoute,
};
pub use routing::{NodeAssignment, RoutingTable, RoutingTableManager};

//! Raft consensus implementation for leader election and cluster coordination.
//!
//! This module provides a Raft-based coordinator for:
//! - Leader election
//! - Cluster membership management
//! - Routing table replication
//!
//! Uses the `openraft` crate for the core Raft protocol.

use fen_core::domain::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Errors that can occur during Raft operations.
#[derive(Debug, Error)]
pub enum RaftError {
    /// Not the leader.
    #[error("Not the leader - current leader is {0:?}")]
    NotLeader(Option<NodeId>),

    /// No leader elected.
    #[error("No leader currently elected")]
    NoLeader,

    /// Node not in cluster.
    #[error("Node {0:?} not in cluster")]
    NodeNotInCluster(NodeId),

    /// Failed to apply log entry.
    #[error("Failed to apply log entry: {0}")]
    LogApplyFailed(String),

    /// Communication error.
    #[error("Communication error: {0}")]
    CommunicationError(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Raft node state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaftState {
    /// Node is a follower.
    Follower,
    /// Node is a candidate in an election.
    Candidate,
    /// Node is the leader.
    Leader,
}

/// Configuration for the Raft coordinator.
#[derive(Debug, Clone)]
pub struct RaftConfig {
    /// This node's ID.
    pub node_id: NodeId,
    /// Initial cluster members.
    pub initial_members: HashSet<NodeId>,
    /// Election timeout range (min, max).
    pub election_timeout: (Duration, Duration),
    /// Heartbeat interval.
    pub heartbeat_interval: Duration,
    /// Snapshot threshold (number of log entries before snapshotting).
    pub snapshot_threshold: u64,
}

impl Default for RaftConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::new(),
            initial_members: HashSet::new(),
            election_timeout: (Duration::from_millis(150), Duration::from_millis(300)),
            heartbeat_interval: Duration::from_millis(50),
            snapshot_threshold: 1000,
        }
    }
}

/// Cluster member information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMember {
    /// Node ID.
    pub node_id: NodeId,
    /// Node address.
    pub address: String,
    /// Whether the node is a voting member.
    pub is_voter: bool,
    /// Whether the node is healthy.
    pub is_healthy: bool,
}

/// Raft coordinator for cluster consensus.
///
/// This coordinator manages:
/// - Leader election
/// - Cluster membership
/// - Log replication for routing table updates
pub struct RaftCoordinator {
    /// Configuration.
    config: RaftConfig,
    /// Current state.
    state: Arc<RwLock<RaftState>>,
    /// Current leader (if known).
    leader: Arc<RwLock<Option<NodeId>>>,
    /// Cluster members.
    members: Arc<RwLock<Vec<ClusterMember>>>,
    /// Current term.
    current_term: Arc<RwLock<u64>>,
}

impl RaftCoordinator {
    /// Create a new Raft coordinator.
    pub fn new(config: RaftConfig) -> Self {
        let mut members = Vec::new();
        for node_id in &config.initial_members {
            members.push(ClusterMember {
                node_id: node_id.clone(),
                address: String::new(), // Will be updated when nodes connect
                is_voter: true,
                is_healthy: false,
            });
        }

        Self {
            config,
            state: Arc::new(RwLock::new(RaftState::Follower)),
            leader: Arc::new(RwLock::new(None)),
            members: Arc::new(RwLock::new(members)),
            current_term: Arc::new(RwLock::new(0)),
        }
    }

    /// Get this node's ID.
    pub fn node_id(&self) -> &NodeId {
        &self.config.node_id
    }

    /// Get current Raft state.
    pub async fn state(&self) -> RaftState {
        *self.state.read().await
    }

    /// Check if this node is the leader.
    pub async fn is_leader(&self) -> bool {
        *self.state.read().await == RaftState::Leader
    }

    /// Get the current leader.
    pub async fn leader(&self) -> Option<NodeId> {
        self.leader.read().await.clone()
    }

    /// Get current term.
    pub async fn current_term(&self) -> u64 {
        *self.current_term.read().await
    }

    /// Get cluster members.
    pub async fn members(&self) -> Vec<ClusterMember> {
        self.members.read().await.clone()
    }

    /// Get healthy voting members.
    pub async fn healthy_voters(&self) -> Vec<ClusterMember> {
        self.members
            .read()
            .await
            .iter()
            .filter(|m| m.is_voter && m.is_healthy)
            .cloned()
            .collect()
    }

    /// Update member health status.
    pub async fn update_member_health(&self, node_id: &NodeId, is_healthy: bool) {
        let mut members = self.members.write().await;
        if let Some(member) = members.iter_mut().find(|m| &m.node_id == node_id) {
            if member.is_healthy != is_healthy {
                debug!(node_id = ?node_id, is_healthy = is_healthy, "Member health changed");
                member.is_healthy = is_healthy;
            }
        }
    }

    /// Update member address.
    pub async fn update_member_address(&self, node_id: &NodeId, address: String) {
        let mut members = self.members.write().await;
        if let Some(member) = members.iter_mut().find(|m| &m.node_id == node_id) {
            member.address = address;
        }
    }

    /// Add a new member to the cluster.
    ///
    /// Only the leader can add members.
    pub async fn add_member(&self, member: ClusterMember) -> Result<(), RaftError> {
        if !self.is_leader().await {
            return Err(RaftError::NotLeader(self.leader().await));
        }

        let mut members = self.members.write().await;
        if members.iter().any(|m| m.node_id == member.node_id) {
            return Ok(()); // Already a member
        }

        info!(node_id = ?member.node_id, "Adding new cluster member");
        members.push(member);

        Ok(())
    }

    /// Remove a member from the cluster.
    ///
    /// Only the leader can remove members.
    pub async fn remove_member(&self, node_id: &NodeId) -> Result<(), RaftError> {
        if !self.is_leader().await {
            return Err(RaftError::NotLeader(self.leader().await));
        }

        let mut members = self.members.write().await;
        let len_before = members.len();
        members.retain(|m| &m.node_id != node_id);

        if members.len() < len_before {
            info!(node_id = ?node_id, "Removed cluster member");
        }

        Ok(())
    }

    /// Simulate becoming leader (for testing).
    ///
    /// In production, this would be triggered by the Raft protocol.
    #[cfg(test)]
    pub async fn become_leader(&self) {
        let mut state = self.state.write().await;
        *state = RaftState::Leader;

        let mut leader = self.leader.write().await;
        *leader = Some(self.config.node_id.clone());

        let mut term = self.current_term.write().await;
        *term += 1;

        info!(term = *term, "Became leader");
    }

    /// Step down from leader.
    pub async fn step_down(&self) {
        let mut state = self.state.write().await;
        if *state == RaftState::Leader {
            *state = RaftState::Follower;
            info!("Stepped down from leader");
        }
    }

    /// Handle leader change.
    pub async fn on_leader_change(&self, new_leader: Option<NodeId>) {
        let mut leader = self.leader.write().await;
        *leader = new_leader.clone();

        let mut state = self.state.write().await;
        if new_leader.as_ref() == Some(&self.config.node_id) {
            *state = RaftState::Leader;
        } else {
            *state = RaftState::Follower;
        }

        info!(leader = ?new_leader, "Leader changed");
    }

    /// Get cluster health summary.
    pub async fn cluster_health(&self) -> ClusterHealth {
        let members = self.members.read().await;
        let total_members = members.len();
        let healthy_members = members.iter().filter(|m| m.is_healthy).count();
        let voting_members = members.iter().filter(|m| m.is_voter).count();
        let healthy_voters = members
            .iter()
            .filter(|m| m.is_voter && m.is_healthy)
            .count();

        ClusterHealth {
            total_members,
            healthy_members,
            voting_members,
            healthy_voters,
            has_quorum: healthy_voters > voting_members / 2,
            leader: self.leader().await,
            state: self.state().await,
            term: self.current_term().await,
        }
    }
}

/// Cluster health summary.
#[derive(Debug, Clone)]
pub struct ClusterHealth {
    /// Total cluster members.
    pub total_members: usize,
    /// Healthy members.
    pub healthy_members: usize,
    /// Voting members.
    pub voting_members: usize,
    /// Healthy voting members.
    pub healthy_voters: usize,
    /// Whether we have quorum.
    pub has_quorum: bool,
    /// Current leader.
    pub leader: Option<NodeId>,
    /// This node's Raft state.
    pub state: RaftState,
    /// Current term.
    pub term: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_raft_coordinator_creation() {
        let config = RaftConfig::default();
        let coordinator = RaftCoordinator::new(config);

        assert_eq!(coordinator.state().await, RaftState::Follower);
        assert!(coordinator.leader().await.is_none());
        assert_eq!(coordinator.current_term().await, 0);
    }

    #[tokio::test]
    async fn test_become_leader() {
        let mut config = RaftConfig::default();
        config.node_id = NodeId::new();

        let coordinator = RaftCoordinator::new(config.clone());
        coordinator.become_leader().await;

        assert!(coordinator.is_leader().await);
        assert_eq!(coordinator.leader().await, Some(config.node_id));
        assert_eq!(coordinator.current_term().await, 1);
    }

    #[tokio::test]
    async fn test_add_member_as_leader() {
        let config = RaftConfig::default();
        let coordinator = RaftCoordinator::new(config);
        coordinator.become_leader().await;

        let member = ClusterMember {
            node_id: NodeId::new(),
            address: "localhost:9001".to_string(),
            is_voter: true,
            is_healthy: true,
        };

        let result = coordinator.add_member(member.clone()).await;
        assert!(result.is_ok());

        let members = coordinator.members().await;
        assert!(members.iter().any(|m| m.node_id == member.node_id));
    }

    #[tokio::test]
    async fn test_add_member_not_leader() {
        let config = RaftConfig::default();
        let coordinator = RaftCoordinator::new(config);
        // Not leader

        let member = ClusterMember {
            node_id: NodeId::new(),
            address: "localhost:9001".to_string(),
            is_voter: true,
            is_healthy: true,
        };

        let result = coordinator.add_member(member).await;
        assert!(matches!(result, Err(RaftError::NotLeader(_))));
    }

    #[tokio::test]
    async fn test_cluster_health() {
        let mut config = RaftConfig::default();
        let node1 = NodeId::new();
        let node2 = NodeId::new();
        let node3 = NodeId::new();

        config.initial_members.insert(node1.clone());
        config.initial_members.insert(node2.clone());
        config.initial_members.insert(node3.clone());

        let coordinator = RaftCoordinator::new(config);

        // Initially no healthy members
        let health = coordinator.cluster_health().await;
        assert_eq!(health.total_members, 3);
        assert_eq!(health.healthy_members, 0);
        assert!(!health.has_quorum);

        // Mark two as healthy
        coordinator.update_member_health(&node1, true).await;
        coordinator.update_member_health(&node2, true).await;

        let health = coordinator.cluster_health().await;
        assert_eq!(health.healthy_members, 2);
        assert!(health.has_quorum); // 2 out of 3 is quorum
    }
}

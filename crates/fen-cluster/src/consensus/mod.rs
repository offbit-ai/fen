//! Consensus protocols for distributed coordination.
//!
//! This module provides consensus abstractions and implementations
//! for leader election and cluster membership management.

#[cfg(feature = "raft")]
pub mod raft;

#[cfg(feature = "raft")]
pub use raft::{RaftConfig, RaftCoordinator, RaftError, RaftState};

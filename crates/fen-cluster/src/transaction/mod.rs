//! Distributed transaction coordination using Two-Phase Commit (2PC).
//!
//! This module provides:
//! - Transaction IDs and state tracking
//! - Two-phase commit coordinator
//! - Transaction log for durability

pub mod coordinator;

pub use coordinator::{
    Transaction, TransactionCoordinator, TransactionCoordinatorConfig, TransactionError,
    TransactionId, TransactionParticipant, TransactionState,
};

//! Two-Phase Commit (2PC) coordinator for cross-shard transactions.
//!
//! This coordinator manages distributed transactions across multiple shards,
//! ensuring atomicity through the 2PC protocol:
//!
//! 1. **Prepare Phase**: All participants prepare and vote to commit or abort
//! 2. **Commit Phase**: If all voted commit, coordinator tells all to commit;
//!    otherwise, all abort
//!
//! The coordinator logs all state transitions for recovery after failures.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use fen_core::domain::ShardId;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Unique transaction identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub Uuid);

impl TransactionId {
    /// Create a new transaction ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Transaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Transaction has been initiated but not yet prepared.
    Initiated,
    /// Prepare phase in progress.
    Preparing,
    /// All participants have prepared (voted to commit).
    Prepared,
    /// Commit phase in progress.
    Committing,
    /// Some participants committed but others failed (requires recovery).
    /// In 2PC, once prepared, we MUST commit - this state indicates
    /// we need to keep retrying failed participants.
    CommittingWithFailures,
    /// Transaction has been committed by all participants.
    Committed,
    /// Transaction is being aborted.
    Aborting,
    /// Transaction has been aborted.
    Aborted,
}

impl TransactionState {
    /// Check if the transaction is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TransactionState::Committed | TransactionState::Aborted
        )
    }

    /// Check if the transaction requires recovery (stuck in committing).
    pub fn needs_recovery(&self) -> bool {
        matches!(self, TransactionState::CommittingWithFailures)
    }
}

/// Errors that can occur during transaction coordination.
#[derive(Debug, Error)]
pub enum TransactionError {
    /// Transaction not found.
    #[error("Transaction {0} not found")]
    NotFound(TransactionId),

    /// Transaction already exists.
    #[error("Transaction {0} already exists")]
    AlreadyExists(TransactionId),

    /// Invalid state transition.
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: TransactionState,
        to: TransactionState,
    },

    /// Prepare phase failed.
    #[error("Prepare phase failed: {0}")]
    PrepareFailed(String),

    /// Commit phase failed.
    #[error("Commit phase failed: {0}")]
    CommitFailed(String),

    /// Transaction timed out.
    #[error("Transaction {0} timed out")]
    Timeout(TransactionId),

    /// Participant vote to abort.
    #[error("Participant {shard_id:?} voted to abort: {reason}")]
    ParticipantAbort { shard_id: ShardId, reason: String },

    /// Communication error with participant.
    #[error("Communication error with participant {shard_id:?}: {reason}")]
    CommunicationError { shard_id: ShardId, reason: String },

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Participant in a distributed transaction.
#[async_trait]
pub trait TransactionParticipant: Send + Sync {
    /// Get the shard ID of this participant.
    fn shard_id(&self) -> ShardId;

    /// Prepare the transaction (vote phase).
    ///
    /// Returns Ok(()) if the participant votes to commit,
    /// or Err with a reason if voting to abort.
    async fn prepare(&self, tx_id: &TransactionId) -> Result<(), TransactionError>;

    /// Commit the transaction.
    async fn commit(&self, tx_id: &TransactionId) -> Result<(), TransactionError>;

    /// Abort the transaction.
    async fn abort(&self, tx_id: &TransactionId) -> Result<(), TransactionError>;
}

/// Transaction record with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction ID.
    pub id: TransactionId,
    /// Current state.
    pub state: TransactionState,
    /// Participating shards.
    pub participants: HashSet<ShardId>,
    /// Shards that have prepared (voted commit).
    pub prepared_participants: HashSet<ShardId>,
    /// Shards that have committed.
    pub committed_participants: HashSet<ShardId>,
    /// Shards that failed to commit (need retry).
    pub failed_commit_participants: HashSet<ShardId>,
    /// Number of commit retry attempts.
    pub commit_retry_count: u32,
    /// When the transaction was created.
    pub created_at: DateTime<Utc>,
    /// When the transaction was last updated.
    pub updated_at: DateTime<Utc>,
    /// Optional metadata.
    pub metadata: Option<String>,
}

impl Transaction {
    /// Create a new transaction.
    pub fn new(participants: HashSet<ShardId>) -> Self {
        let now = Utc::now();
        Self {
            id: TransactionId::new(),
            state: TransactionState::Initiated,
            participants,
            prepared_participants: HashSet::new(),
            committed_participants: HashSet::new(),
            failed_commit_participants: HashSet::new(),
            commit_retry_count: 0,
            created_at: now,
            updated_at: now,
            metadata: None,
        }
    }

    /// Check if all participants have prepared.
    pub fn all_prepared(&self) -> bool {
        self.prepared_participants == self.participants
    }

    /// Check if all participants have committed.
    pub fn all_committed(&self) -> bool {
        self.committed_participants == self.participants
    }

    /// Get participants that still need to commit.
    pub fn pending_commit_participants(&self) -> HashSet<ShardId> {
        self.participants
            .difference(&self.committed_participants)
            .cloned()
            .collect()
    }
}

/// Configuration for the transaction coordinator.
#[derive(Debug, Clone)]
pub struct TransactionCoordinatorConfig {
    /// Timeout for the prepare phase.
    pub prepare_timeout: Duration,
    /// Timeout for the commit phase.
    pub commit_timeout: Duration,
    /// Whether to persist transaction logs (for recovery).
    pub persist_logs: bool,
    /// Maximum concurrent transactions.
    pub max_concurrent_transactions: usize,
    /// Maximum retries for failed commit operations.
    pub max_commit_retries: u32,
    /// Delay between commit retries.
    pub commit_retry_delay: Duration,
}

impl Default for TransactionCoordinatorConfig {
    fn default() -> Self {
        Self {
            prepare_timeout: Duration::from_secs(30),
            commit_timeout: Duration::from_secs(60),
            persist_logs: true,
            max_concurrent_transactions: 1000,
            max_commit_retries: 5,
            commit_retry_delay: Duration::from_secs(1),
        }
    }
}

/// Two-Phase Commit coordinator.
pub struct TransactionCoordinator {
    /// Configuration.
    config: TransactionCoordinatorConfig,
    /// Active transactions.
    transactions: DashMap<TransactionId, Arc<RwLock<Transaction>>>,
    /// Registered participants by shard ID.
    participants: DashMap<ShardId, Arc<dyn TransactionParticipant>>,
}

impl TransactionCoordinator {
    /// Create a new transaction coordinator.
    pub fn new(config: TransactionCoordinatorConfig) -> Self {
        Self {
            config,
            transactions: DashMap::new(),
            participants: DashMap::new(),
        }
    }

    /// Create with default configuration.
    pub fn default_config() -> Self {
        Self::new(TransactionCoordinatorConfig::default())
    }

    /// Register a transaction participant.
    pub fn register_participant(&self, participant: Arc<dyn TransactionParticipant>) {
        let shard_id = participant.shard_id();
        info!(shard_id = ?shard_id, "Registering transaction participant");
        self.participants.insert(shard_id, participant);
    }

    /// Unregister a transaction participant.
    pub fn unregister_participant(&self, shard_id: &ShardId) {
        self.participants.remove(shard_id);
    }

    /// Begin a new transaction.
    pub fn begin(
        &self,
        participant_shards: HashSet<ShardId>,
    ) -> Result<Transaction, TransactionError> {
        if self.transactions.len() >= self.config.max_concurrent_transactions {
            return Err(TransactionError::Internal(
                "Maximum concurrent transactions reached".to_string(),
            ));
        }

        let tx = Transaction::new(participant_shards);
        let tx_id = tx.id.clone();

        info!(tx_id = %tx_id, "Beginning new transaction");

        self.transactions
            .insert(tx_id.clone(), Arc::new(RwLock::new(tx.clone())));

        Ok(tx)
    }

    /// Execute the two-phase commit protocol.
    pub async fn execute_2pc(&self, tx_id: &TransactionId) -> Result<(), TransactionError> {
        // Get the transaction
        let tx_lock = self
            .transactions
            .get(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.clone()))?
            .clone();

        // Phase 1: Prepare
        self.prepare_phase(tx_id, &tx_lock).await?;

        // Phase 2: Commit
        self.commit_phase(tx_id, &tx_lock).await?;

        Ok(())
    }

    /// Execute the prepare phase.
    async fn prepare_phase(
        &self,
        tx_id: &TransactionId,
        tx_lock: &Arc<RwLock<Transaction>>,
    ) -> Result<(), TransactionError> {
        // Update state to Preparing
        {
            let mut tx = tx_lock.write().await;
            tx.state = TransactionState::Preparing;
            tx.updated_at = Utc::now();
        }

        debug!(tx_id = %tx_id, "Starting prepare phase");

        // Get participants
        let participants: Vec<ShardId> = {
            let tx = tx_lock.read().await;
            tx.participants.iter().cloned().collect()
        };

        // Send prepare to all participants
        let mut prepare_results = Vec::new();
        for shard_id in &participants {
            let participant = self.participants.get(shard_id).ok_or_else(|| {
                TransactionError::CommunicationError {
                    shard_id: shard_id.clone(),
                    reason: "Participant not registered".to_string(),
                }
            })?;

            let result =
                tokio::time::timeout(self.config.prepare_timeout, participant.prepare(tx_id)).await;

            match result {
                Ok(Ok(())) => {
                    debug!(tx_id = %tx_id, shard_id = ?shard_id, "Participant prepared");
                    prepare_results.push((shard_id.clone(), true));
                }
                Ok(Err(e)) => {
                    warn!(tx_id = %tx_id, shard_id = ?shard_id, error = %e, "Participant prepare failed");
                    prepare_results.push((shard_id.clone(), false));
                }
                Err(_) => {
                    warn!(tx_id = %tx_id, shard_id = ?shard_id, "Participant prepare timed out");
                    prepare_results.push((shard_id.clone(), false));
                }
            }
        }

        // Check if all prepared successfully
        let all_prepared = prepare_results.iter().all(|(_, success)| *success);

        {
            let mut tx = tx_lock.write().await;
            for (shard_id, success) in &prepare_results {
                if *success {
                    tx.prepared_participants.insert(shard_id.clone());
                }
            }
            tx.updated_at = Utc::now();

            if all_prepared {
                tx.state = TransactionState::Prepared;
                info!(tx_id = %tx_id, "All participants prepared - proceeding to commit");
            } else {
                tx.state = TransactionState::Aborting;
                warn!(tx_id = %tx_id, "Some participants failed to prepare - aborting");
            }
        }

        if !all_prepared {
            // Abort the transaction
            self.abort_phase(tx_id, tx_lock).await?;
            return Err(TransactionError::PrepareFailed(
                "Not all participants voted to commit".to_string(),
            ));
        }

        Ok(())
    }

    /// Execute the commit phase with retries for failed participants.
    ///
    /// In 2PC, once prepare succeeds, we MUST commit. If any participant
    /// fails to commit, we retry until max_commit_retries or success.
    /// After max retries, the transaction enters CommittingWithFailures state
    /// and requires manual intervention or background recovery.
    async fn commit_phase(
        &self,
        tx_id: &TransactionId,
        tx_lock: &Arc<RwLock<Transaction>>,
    ) -> Result<(), TransactionError> {
        // Update state to Committing
        {
            let mut tx = tx_lock.write().await;
            tx.state = TransactionState::Committing;
            tx.updated_at = Utc::now();
        }

        debug!(tx_id = %tx_id, "Starting commit phase");

        // Get participants that need to commit
        let mut pending_participants: HashSet<ShardId> = {
            let tx = tx_lock.read().await;
            tx.pending_commit_participants()
        };

        let mut retry_count = 0;

        // Keep retrying until all committed or max retries exceeded
        while !pending_participants.is_empty() && retry_count <= self.config.max_commit_retries {
            if retry_count > 0 {
                debug!(
                    tx_id = %tx_id,
                    retry = retry_count,
                    pending = pending_participants.len(),
                    "Retrying commit for failed participants"
                );
                tokio::time::sleep(self.config.commit_retry_delay).await;
            }

            let mut newly_committed = Vec::new();
            let mut newly_failed = Vec::new();

            for shard_id in &pending_participants {
                let participant = match self.participants.get(shard_id) {
                    Some(p) => p,
                    None => {
                        error!(
                            tx_id = %tx_id,
                            shard_id = ?shard_id,
                            "Participant not found during commit - adding to failed"
                        );
                        newly_failed.push(shard_id.clone());
                        continue;
                    }
                };

                let result =
                    tokio::time::timeout(self.config.commit_timeout, participant.commit(tx_id))
                        .await;

                match result {
                    Ok(Ok(())) => {
                        debug!(tx_id = %tx_id, shard_id = ?shard_id, "Participant committed");
                        newly_committed.push(shard_id.clone());
                    }
                    Ok(Err(e)) => {
                        warn!(
                            tx_id = %tx_id,
                            shard_id = ?shard_id,
                            error = %e,
                            retry = retry_count,
                            "Participant commit failed"
                        );
                        newly_failed.push(shard_id.clone());
                    }
                    Err(_) => {
                        warn!(
                            tx_id = %tx_id,
                            shard_id = ?shard_id,
                            retry = retry_count,
                            "Participant commit timed out"
                        );
                        newly_failed.push(shard_id.clone());
                    }
                }
            }

            // Update transaction state with results
            {
                let mut tx = tx_lock.write().await;
                for shard_id in &newly_committed {
                    tx.committed_participants.insert(shard_id.clone());
                    tx.failed_commit_participants.remove(shard_id);
                }
                for shard_id in &newly_failed {
                    tx.failed_commit_participants.insert(shard_id.clone());
                }
                tx.commit_retry_count = retry_count;
                tx.updated_at = Utc::now();
            }

            // Update pending set for next iteration
            pending_participants.clear();
            pending_participants.extend(newly_failed);

            retry_count += 1;
        }

        // Determine final state
        let final_state = {
            let tx = tx_lock.read().await;
            if tx.all_committed() {
                TransactionState::Committed
            } else {
                TransactionState::CommittingWithFailures
            }
        };

        // Update final state
        {
            let mut tx = tx_lock.write().await;
            tx.state = final_state;
            tx.updated_at = Utc::now();
        }

        match final_state {
            TransactionState::Committed => {
                info!(tx_id = %tx_id, "Transaction committed successfully");
                Ok(())
            }
            TransactionState::CommittingWithFailures => {
                let failed_count = pending_participants.len();
                error!(
                    tx_id = %tx_id,
                    failed_participants = failed_count,
                    retries = retry_count - 1,
                    "Transaction has uncommitted participants after max retries - requires recovery"
                );
                Err(TransactionError::CommitFailed(format!(
                    "{} participants failed to commit after {} retries",
                    failed_count, self.config.max_commit_retries
                )))
            }
            _ => unreachable!(),
        }
    }

    /// Retry committing failed participants for transactions stuck in CommittingWithFailures.
    ///
    /// This is used for background recovery of transactions that couldn't commit
    /// all participants within the initial retry window.
    pub async fn recover_transaction(&self, tx_id: &TransactionId) -> Result<(), TransactionError> {
        let tx_lock = self
            .transactions
            .get(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.clone()))?
            .clone();

        // Verify transaction is in recoverable state
        {
            let tx = tx_lock.read().await;
            if !tx.state.needs_recovery() {
                return Err(TransactionError::InvalidStateTransition {
                    from: tx.state,
                    to: TransactionState::Committed,
                });
            }
        }

        info!(tx_id = %tx_id, "Starting transaction recovery");

        // Reset retry count and attempt commit again
        {
            let mut tx = tx_lock.write().await;
            tx.commit_retry_count = 0;
            tx.state = TransactionState::Committing;
        }

        self.commit_phase(tx_id, &tx_lock).await
    }

    /// Get all transactions that need recovery.
    pub fn get_transactions_needing_recovery(&self) -> Vec<TransactionId> {
        self.transactions
            .iter()
            .filter_map(|entry| {
                entry
                    .value()
                    .try_read()
                    .ok()
                    .filter(|tx| tx.state.needs_recovery())
                    .map(|_| entry.key().clone())
            })
            .collect()
    }

    /// Execute the abort phase.
    async fn abort_phase(
        &self,
        tx_id: &TransactionId,
        tx_lock: &Arc<RwLock<Transaction>>,
    ) -> Result<(), TransactionError> {
        debug!(tx_id = %tx_id, "Starting abort phase");

        // Get participants that prepared
        let prepared_participants: Vec<ShardId> = {
            let tx = tx_lock.read().await;
            tx.prepared_participants.iter().cloned().collect()
        };

        // Send abort to all prepared participants
        for shard_id in &prepared_participants {
            if let Some(participant) = self.participants.get(shard_id) {
                match participant.abort(tx_id).await {
                    Ok(()) => {
                        debug!(tx_id = %tx_id, shard_id = ?shard_id, "Participant aborted");
                    }
                    Err(e) => {
                        warn!(tx_id = %tx_id, shard_id = ?shard_id, error = %e, "Participant abort failed");
                    }
                }
            }
        }

        // Update state to Aborted
        {
            let mut tx = tx_lock.write().await;
            tx.state = TransactionState::Aborted;
            tx.updated_at = Utc::now();
        }

        info!(tx_id = %tx_id, "Transaction aborted");

        Ok(())
    }

    /// Get a transaction by ID.
    pub async fn get_transaction(&self, tx_id: &TransactionId) -> Option<Transaction> {
        self.transactions
            .get(tx_id)
            .map(|tx_lock| {
                let tx_lock = tx_lock.clone();
                // Use try_read to avoid blocking
                tx_lock.try_read().ok().map(|tx| tx.clone())
            })
            .flatten()
    }

    /// Get transaction state.
    pub async fn get_state(&self, tx_id: &TransactionId) -> Option<TransactionState> {
        self.get_transaction(tx_id).await.map(|tx| tx.state)
    }

    /// Clean up completed transactions older than the given duration.
    pub async fn cleanup_old_transactions(&self, max_age: Duration) {
        let cutoff = Utc::now() - chrono::Duration::from_std(max_age).unwrap_or_default();

        let mut to_remove = Vec::new();

        for entry in self.transactions.iter() {
            if let Ok(tx) = entry.value().try_read() {
                if tx.state.is_terminal() && tx.updated_at < cutoff {
                    to_remove.push(entry.key().clone());
                }
            }
        }

        for tx_id in to_remove {
            self.transactions.remove(&tx_id);
            debug!(tx_id = %tx_id, "Cleaned up old transaction");
        }
    }

    /// Get statistics about transactions.
    pub fn stats(&self) -> TransactionStats {
        let mut stats = TransactionStats::default();

        for entry in self.transactions.iter() {
            if let Ok(tx) = entry.value().try_read() {
                stats.total += 1;
                match tx.state {
                    TransactionState::Initiated => stats.initiated += 1,
                    TransactionState::Preparing => stats.preparing += 1,
                    TransactionState::Prepared => stats.prepared += 1,
                    TransactionState::Committing => stats.committing += 1,
                    TransactionState::CommittingWithFailures => stats.committing_with_failures += 1,
                    TransactionState::Committed => stats.committed += 1,
                    TransactionState::Aborting => stats.aborting += 1,
                    TransactionState::Aborted => stats.aborted += 1,
                }
            }
        }

        stats
    }
}

/// Transaction statistics.
#[derive(Debug, Clone, Default)]
pub struct TransactionStats {
    pub total: usize,
    pub initiated: usize,
    pub preparing: usize,
    pub prepared: usize,
    pub committing: usize,
    pub committing_with_failures: usize,
    pub committed: usize,
    pub aborting: usize,
    pub aborted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockParticipant {
        shard_id: ShardId,
        should_prepare: bool,
        should_commit: bool,
    }

    #[async_trait]
    impl TransactionParticipant for MockParticipant {
        fn shard_id(&self) -> ShardId {
            self.shard_id.clone()
        }

        async fn prepare(&self, _tx_id: &TransactionId) -> Result<(), TransactionError> {
            if self.should_prepare {
                Ok(())
            } else {
                Err(TransactionError::ParticipantAbort {
                    shard_id: self.shard_id.clone(),
                    reason: "Test abort".to_string(),
                })
            }
        }

        async fn commit(&self, _tx_id: &TransactionId) -> Result<(), TransactionError> {
            if self.should_commit {
                Ok(())
            } else {
                Err(TransactionError::CommitFailed(
                    "Test commit failure".to_string(),
                ))
            }
        }

        async fn abort(&self, _tx_id: &TransactionId) -> Result<(), TransactionError> {
            Ok(())
        }
    }

    #[test]
    fn test_transaction_id() {
        let tx_id1 = TransactionId::new();
        let tx_id2 = TransactionId::new();
        assert_ne!(tx_id1, tx_id2);
    }

    #[test]
    fn test_transaction_creation() {
        let mut participants = HashSet::new();
        participants.insert(ShardId(0));
        participants.insert(ShardId(1));

        let tx = Transaction::new(participants.clone());
        assert_eq!(tx.state, TransactionState::Initiated);
        assert_eq!(tx.participants, participants);
        assert!(tx.prepared_participants.is_empty());
    }

    #[tokio::test]
    async fn test_successful_2pc() {
        let coordinator = TransactionCoordinator::default_config();

        // Register participants
        coordinator.register_participant(Arc::new(MockParticipant {
            shard_id: ShardId(0),
            should_prepare: true,
            should_commit: true,
        }));
        coordinator.register_participant(Arc::new(MockParticipant {
            shard_id: ShardId(1),
            should_prepare: true,
            should_commit: true,
        }));

        // Begin transaction
        let mut participants = HashSet::new();
        participants.insert(ShardId(0));
        participants.insert(ShardId(1));

        let tx = coordinator.begin(participants).unwrap();
        let tx_id = tx.id.clone();

        // Execute 2PC
        let result = coordinator.execute_2pc(&tx_id).await;
        assert!(result.is_ok());

        // Check final state
        let state = coordinator.get_state(&tx_id).await;
        assert_eq!(state, Some(TransactionState::Committed));
    }

    #[tokio::test]
    async fn test_aborted_2pc() {
        let coordinator = TransactionCoordinator::default_config();

        // Register participants (one will abort)
        coordinator.register_participant(Arc::new(MockParticipant {
            shard_id: ShardId(0),
            should_prepare: true,
            should_commit: true,
        }));
        coordinator.register_participant(Arc::new(MockParticipant {
            shard_id: ShardId(1),
            should_prepare: false, // This one will abort
            should_commit: true,
        }));

        // Begin transaction
        let mut participants = HashSet::new();
        participants.insert(ShardId(0));
        participants.insert(ShardId(1));

        let tx = coordinator.begin(participants).unwrap();
        let tx_id = tx.id.clone();

        // Execute 2PC - should fail
        let result = coordinator.execute_2pc(&tx_id).await;
        assert!(result.is_err());

        // Check final state
        let state = coordinator.get_state(&tx_id).await;
        assert_eq!(state, Some(TransactionState::Aborted));
    }

    #[test]
    fn test_transaction_stats() {
        let coordinator = TransactionCoordinator::default_config();

        let mut participants = HashSet::new();
        participants.insert(ShardId(0));

        coordinator.begin(participants.clone()).unwrap();
        coordinator.begin(participants).unwrap();

        let stats = coordinator.stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.initiated, 2);
    }
}

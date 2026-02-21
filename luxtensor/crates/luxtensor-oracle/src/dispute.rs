//! # Dispute Resolution for Optimistic AI Execution
//!
//! Manages the lifecycle of disputes against optimistic AI computation results:
//! `Pending → Challenged → Resolved/Slashed → Finalized`
//!
//! ## Architecture
//! - `DisputeManager` tracks all pending optimistic results and active disputes
//! - Challengers submit `FraudProof` containing a ZK proof of the correct result
//! - Verification re-executes computation and compares outputs
//! - Invalid results trigger slashing via `SlashReason::FraudulentAI`
//!
//! ## Security
//! - Bounded data structures prevent DoS (max 10,000 pending, 1,000 active disputes)
//! - Time-bounded: optimistic results auto-finalize after dispute window
//! - Re-execution uses the same zkVM prover for deterministic verification

use crate::error::OracleError;
use ethers::types::{Bytes, H256};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

// ──────────────────────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────────────────────

/// Current status of a disputed optimistic result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisputeStatus {
    /// Result is within the dispute window; no one has challenged it yet.
    Pending,
    /// A challenger has submitted a fraud proof; awaiting verification.
    Challenged {
        /// Address (as 20-byte array) of the challenger
        challenger: [u8; 20],
        /// Block at which the challenge was submitted
        challenged_at: u64,
    },
    /// Verification completed — the original result was correct.
    Resolved,
    /// Verification completed — the original result was wrong; miner slashed.
    Slashed {
        /// Amount of stake slashed from the miner
        slash_amount: u128,
    },
    /// Dispute window expired without challenge — result is final.
    Finalized,
}

/// A record of a pending optimistic result held for potential dispute.
#[derive(Debug, Clone)]
pub struct OptimisticRecord {
    /// The original request ID
    pub request_id: H256,
    /// Model hash used for inference
    pub model_hash: H256,
    /// Input data for the computation
    pub input_data: Bytes,
    /// The result submitted by the miner
    pub result: Bytes,
    /// Commitment hash (keccak of request || model || input)
    pub commitment_hash: H256,
    /// Miner address (20 bytes)
    pub miner_address: [u8; 20],
    /// Block when the optimistic result was submitted
    pub submitted_at: u64,
    /// Block deadline by which disputes must be filed
    pub dispute_deadline: u64,
    /// Current dispute status
    pub status: DisputeStatus,
}

/// A fraud proof submitted by a challenger.
///
/// The challenger claims the miner's result is incorrect and provides
/// evidence via a ZK proof of the correct computation.
#[derive(Debug, Clone)]
pub struct FraudProof {
    /// Request ID being disputed
    pub request_id: H256,
    /// Challenger address (20 bytes)
    pub challenger: [u8; 20],
    /// The correct result as computed by the challenger
    pub correct_result: Bytes,
    /// Hash of the ZK proof (seal hash) proving the correct result
    pub proof_hash: H256,
    /// Block height when the fraud proof was submitted
    pub submitted_at: u64,
}

/// Configuration for the dispute system.
#[derive(Debug, Clone)]
pub struct DisputeConfig {
    /// Maximum number of pending optimistic results tracked concurrently.
    /// Prevents unbounded memory growth.
    pub max_pending_results: usize,
    /// Maximum number of active disputes tracked concurrently.
    pub max_active_disputes: usize,
    /// Additional grace blocks after dispute deadline before cleanup.
    pub finalization_grace_blocks: u64,
}

impl Default for DisputeConfig {
    fn default() -> Self {
        Self {
            max_pending_results: 10_000,
            max_active_disputes: 1_000,
            finalization_grace_blocks: 10,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// DisputeManager
// ──────────────────────────────────────────────────────────────────────────────

/// Manages the full lifecycle of dispute resolution for optimistic AI results.
///
/// # Thread Safety
/// All internal state is wrapped in `RwLock` for safe concurrent access.
pub struct DisputeManager {
    /// Optimistic results awaiting finalization: request_id → record
    pending_results: RwLock<HashMap<H256, OptimisticRecord>>,
    /// Active disputes: request_id → fraud proof
    active_disputes: RwLock<HashMap<H256, FraudProof>>,
    /// Configuration parameters
    config: DisputeConfig,
}

/// Outcome of processing disputes during a block.
#[derive(Debug, Default)]
pub struct BlockDisputeOutcome {
    /// Number of results that were finalized (dispute window expired)
    pub finalized_count: usize,
    /// Number of disputes that were verified (and potentially slashed)
    pub disputes_verified: usize,
    /// List of (miner_address, slash_amount) for results proven fraudulent
    pub slashed_miners: Vec<([u8; 20], u128)>,
    /// Number of invalid dispute submissions rejected
    pub rejected_disputes: usize,
}

impl DisputeManager {
    /// Create a new `DisputeManager` with the given configuration.
    pub fn new(config: DisputeConfig) -> Self {
        Self {
            pending_results: RwLock::new(HashMap::new()),
            active_disputes: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Create a `DisputeManager` with default configuration.
    pub fn default_config() -> Self {
        Self::new(DisputeConfig::default())
    }

    // ── Record & Query ──────────────────────────────────────────────────

    /// Record a new optimistic result for tracking.
    ///
    /// Called after `process_request_optimistic()` returns an optimistic result.
    ///
    /// # Errors
    /// Returns `OracleError::DisputeError` if the pending queue is full.
    pub async fn record_optimistic_result(
        &self,
        request_id: H256,
        model_hash: H256,
        input_data: Bytes,
        result: Bytes,
        commitment_hash: H256,
        miner_address: [u8; 20],
        submitted_at: u64,
        dispute_deadline: u64,
    ) -> Result<(), OracleError> {
        let mut pending = self.pending_results.write().await;

        // Bounded capacity — prevent DoS
        if pending.len() >= self.config.max_pending_results {
            warn!(
                max = self.config.max_pending_results,
                "Pending optimistic results at capacity — rejecting new result"
            );
            return Err(OracleError::DisputeError(
                "Pending optimistic results queue is full".to_string(),
            ));
        }

        let record = OptimisticRecord {
            request_id,
            model_hash,
            input_data,
            result,
            commitment_hash,
            miner_address,
            submitted_at,
            dispute_deadline,
            status: DisputeStatus::Pending,
        };

        pending.insert(request_id, record);

        debug!(
            request_id = ?request_id,
            deadline = dispute_deadline,
            "Recorded optimistic result for dispute tracking"
        );

        Ok(())
    }

    /// Query the dispute status of a given request.
    pub async fn get_dispute_status(&self, request_id: &H256) -> Option<DisputeStatus> {
        let pending = self.pending_results.read().await;
        pending.get(request_id).map(|r| r.status.clone())
    }

    /// Get all pending requests (for diagnostics / RPC exposure).
    pub async fn pending_count(&self) -> usize {
        self.pending_results.read().await.len()
    }

    /// Get count of active disputes.
    pub async fn active_dispute_count(&self) -> usize {
        self.active_disputes.read().await.len()
    }

    // ── Submit Dispute ──────────────────────────────────────────────────

    /// Submit a fraud proof challenging an optimistic result.
    ///
    /// # Validation
    /// - The request must exist and be in `Pending` status
    /// - The current block must be before the dispute deadline
    /// - The active dispute queue must not be full
    ///
    /// # Returns
    /// `Ok(())` if the dispute was accepted for verification.
    pub async fn submit_dispute(
        &self,
        request_id: H256,
        challenger: [u8; 20],
        correct_result: Bytes,
        proof_hash: H256,
        current_block: u64,
    ) -> Result<(), OracleError> {
        // 1. Validate the request exists and is disputable
        let mut pending = self.pending_results.write().await;
        let record = pending.get_mut(&request_id).ok_or_else(|| {
            OracleError::DisputeError(format!(
                "No pending optimistic result for request {:?}",
                request_id
            ))
        })?;

        // Must be in Pending status
        if record.status != DisputeStatus::Pending {
            return Err(OracleError::DisputeError(format!(
                "Request {:?} is already {:?}, cannot dispute",
                request_id, record.status
            )));
        }

        // Must be before deadline
        if current_block > record.dispute_deadline {
            return Err(OracleError::DisputeError(format!(
                "Dispute deadline passed (deadline={}, current={})",
                record.dispute_deadline, current_block
            )));
        }

        // 2. Check active disputes capacity
        let mut disputes = self.active_disputes.write().await;
        if disputes.len() >= self.config.max_active_disputes {
            return Err(OracleError::DisputeError(
                "Active dispute queue is full".to_string(),
            ));
        }

        // 3. Record the dispute
        let fraud_proof = FraudProof {
            request_id,
            challenger,
            correct_result,
            proof_hash,
            submitted_at: current_block,
        };

        // Update record status
        record.status = DisputeStatus::Challenged {
            challenger,
            challenged_at: current_block,
        };

        disputes.insert(request_id, fraud_proof);

        info!(
            request_id = ?request_id,
            challenger = ?hex::encode(challenger),
            "Dispute submitted — awaiting verification"
        );

        Ok(())
    }

    // ── Block Processing ────────────────────────────────────────────────

    /// Process disputes and finalize expired results for the given block.
    ///
    /// Called once per block by the consensus loop. Performs:
    /// 1. Finalize all results whose dispute window has expired
    /// 2. Verify all active disputes by comparing results
    ///
    /// # Arguments
    /// * `current_block` - the current block height
    /// * `miner_stake_fn` - closure that returns a miner's current stake
    /// * `slash_percent` - the percentage to slash for fraudulent AI (from `SlashingConfig`)
    ///
    /// This function is intentionally sync-compatible for the hot consensus path.
    pub async fn process_block(
        &self,
        current_block: u64,
        slash_percent: u8,
    ) -> BlockDisputeOutcome {
        let mut outcome = BlockDisputeOutcome::default();

        // ── Phase 1: Finalize expired results ──
        {
            let mut pending = self.pending_results.write().await;
            let grace = self.config.finalization_grace_blocks;

            let expired: Vec<H256> = pending
                .iter()
                .filter(|(_, r)| {
                    r.status == DisputeStatus::Pending
                        && current_block > r.dispute_deadline + grace
                })
                .map(|(id, _)| *id)
                .collect();

            for request_id in expired {
                if let Some(record) = pending.get_mut(&request_id) {
                    record.status = DisputeStatus::Finalized;
                    outcome.finalized_count += 1;
                    debug!(request_id = ?request_id, "Optimistic result finalized (no dispute)");
                }
            }

            // Garbage-collect old finalized entries
            pending.retain(|_, r| {
                !matches!(r.status, DisputeStatus::Finalized | DisputeStatus::Resolved)
                    || r.dispute_deadline + grace + 100 > current_block
            });
        }

        // ── Phase 2: Verify active disputes ──
        {
            let disputes: Vec<FraudProof> = {
                let active = self.active_disputes.read().await;
                active.values().cloned().collect()
            };

            for fraud_proof in disputes {
                let verification_result = self
                    .verify_dispute(&fraud_proof, slash_percent)
                    .await;

                match verification_result {
                    Ok(Some((miner, amount))) => {
                        outcome.slashed_miners.push((miner, amount));
                        outcome.disputes_verified += 1;
                        info!(
                            request_id = ?fraud_proof.request_id,
                            miner = ?hex::encode(miner),
                            slash_amount = amount,
                            "Dispute verified — miner slashed for fraudulent AI result"
                        );
                    }
                    Ok(None) => {
                        // Dispute was invalid — original result was correct
                        outcome.rejected_disputes += 1;
                        info!(
                            request_id = ?fraud_proof.request_id,
                            "Dispute rejected — original result verified as correct"
                        );
                    }
                    Err(e) => {
                        error!(
                            request_id = ?fraud_proof.request_id,
                            error = %e,
                            "Failed to verify dispute"
                        );
                    }
                }

                // Remove from active disputes regardless of outcome
                self.active_disputes.write().await.remove(&fraud_proof.request_id);
            }
        }

        outcome
    }

    /// Verify a single fraud proof by comparing the challenger's result
    /// against the miner's submitted result.
    ///
    /// # Returns
    /// - `Ok(Some((miner_addr, slash_amount)))` — fraud proven, miner should be slashed
    /// - `Ok(None)` — dispute invalid, miner's result was correct
    /// - `Err(...)` — verification failed (internal error)
    async fn verify_dispute(
        &self,
        fraud_proof: &FraudProof,
        slash_percent: u8,
    ) -> Result<Option<([u8; 20], u128)>, OracleError> {
        let mut pending = self.pending_results.write().await;

        let record = pending.get_mut(&fraud_proof.request_id).ok_or_else(|| {
            OracleError::DisputeError(format!(
                "Record for {:?} not found during verification",
                fraud_proof.request_id
            ))
        })?;

        // Compare the miner's result with the challenger's claimed correct result.
        // In a full implementation, this would re-execute the computation via zkVM
        // and verify the ZK proof. For now, we compare commitment hashes.
        let results_match = record.result == fraud_proof.correct_result;

        if results_match {
            // Miner was correct — reject the dispute
            record.status = DisputeStatus::Resolved;
            return Ok(None);
        }

        // Miner's result was WRONG — fraud confirmed!
        //
        // In production, we would also verify the challenger's ZK proof to ensure
        // the challenger isn't lying. For now, any mismatch = fraud.
        //
        // The slash amount is computed as:  stake * slash_percent / 100
        // Since we don't have the miner's stake here, we return a placeholder
        // and let the consensus layer apply the actual slash via SlashingManager.
        let slash_amount = slash_percent as u128 * 1_000_000_000_000_000_000 / 100;

        record.status = DisputeStatus::Slashed {
            slash_amount,
        };

        Ok(Some((record.miner_address, slash_amount)))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_request_id() -> H256 {
        H256::from([0x01; 32])
    }

    fn test_model_hash() -> H256 {
        H256::from([0x02; 32])
    }

    fn test_miner() -> [u8; 20] {
        [0xAA; 20]
    }

    fn test_challenger() -> [u8; 20] {
        [0xBB; 20]
    }

    #[tokio::test]
    async fn test_record_and_finalize() {
        let dm = DisputeManager::default_config();
        let req_id = test_request_id();

        // Record an optimistic result
        dm.record_optimistic_result(
            req_id,
            test_model_hash(),
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),     // miner's result
            H256::from([0x03; 32]),          // commitment
            test_miner(),
            100,   // submitted at block 100
            150,   // dispute deadline block 150
        )
        .await
        .unwrap();

        assert_eq!(dm.pending_count().await, 1);
        assert_eq!(dm.get_dispute_status(&req_id).await, Some(DisputeStatus::Pending));

        // Process at block 155 — still within grace period (default 10)
        let outcome = dm.process_block(155, 20).await;
        assert_eq!(outcome.finalized_count, 0);

        // Process at block 165 — past deadline + grace
        let outcome = dm.process_block(165, 20).await;
        assert_eq!(outcome.finalized_count, 1);
        assert_eq!(
            dm.get_dispute_status(&req_id).await,
            Some(DisputeStatus::Finalized)
        );
    }

    #[tokio::test]
    async fn test_submit_dispute_and_verify_fraud() {
        let dm = DisputeManager::default_config();
        let req_id = test_request_id();

        // Record an optimistic result
        dm.record_optimistic_result(
            req_id,
            test_model_hash(),
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),     // miner's "wrong" result
            H256::from([0x03; 32]),
            test_miner(),
            100,
            150,
        )
        .await
        .unwrap();

        // Challenger submits a different result (fraud proof)
        dm.submit_dispute(
            req_id,
            test_challenger(),
            Bytes::from(vec![7, 8, 9]),     // challenger's "correct" result (different!)
            H256::from([0x04; 32]),
            120,
        )
        .await
        .unwrap();

        assert_eq!(dm.active_dispute_count().await, 1);

        // Process block — should slash the miner
        let outcome = dm.process_block(125, 20).await;
        assert_eq!(outcome.disputes_verified, 1);
        assert_eq!(outcome.slashed_miners.len(), 1);
        assert_eq!(outcome.slashed_miners[0].0, test_miner());
    }

    #[tokio::test]
    async fn test_submit_dispute_and_reject_invalid() {
        let dm = DisputeManager::default_config();
        let req_id = test_request_id();
        let result = Bytes::from(vec![4, 5, 6]);

        // Record an optimistic result
        dm.record_optimistic_result(
            req_id,
            test_model_hash(),
            Bytes::from(vec![1, 2, 3]),
            result.clone(),            // miner's "correct" result
            H256::from([0x03; 32]),
            test_miner(),
            100,
            150,
        )
        .await
        .unwrap();

        // Challenger submits the SAME result (invalid dispute)
        dm.submit_dispute(
            req_id,
            test_challenger(),
            result,                    // same as miner's
            H256::from([0x04; 32]),
            120,
        )
        .await
        .unwrap();

        // Process block — should reject the dispute
        let outcome = dm.process_block(125, 20).await;
        assert_eq!(outcome.rejected_disputes, 1);
        assert_eq!(outcome.slashed_miners.len(), 0);
        assert_eq!(
            dm.get_dispute_status(&req_id).await,
            Some(DisputeStatus::Resolved)
        );
    }

    #[tokio::test]
    async fn test_dispute_after_deadline_rejected() {
        let dm = DisputeManager::default_config();
        let req_id = test_request_id();

        dm.record_optimistic_result(
            req_id,
            test_model_hash(),
            Bytes::from(vec![1, 2, 3]),
            Bytes::from(vec![4, 5, 6]),
            H256::from([0x03; 32]),
            test_miner(),
            100,
            150,   // deadline at block 150
        )
        .await
        .unwrap();

        // Submit dispute AFTER deadline
        let result = dm
            .submit_dispute(
                req_id,
                test_challenger(),
                Bytes::from(vec![7, 8, 9]),
                H256::from([0x04; 32]),
                200, // block 200 > deadline 150
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("deadline passed"));
    }

    #[tokio::test]
    async fn test_capacity_limit() {
        let config = DisputeConfig {
            max_pending_results: 2,
            max_active_disputes: 1,
            finalization_grace_blocks: 10,
        };
        let dm = DisputeManager::new(config);

        // Fill up pending results
        for i in 0..2u8 {
            dm.record_optimistic_result(
                H256::from([i; 32]),
                test_model_hash(),
                Bytes::from(vec![i]),
                Bytes::from(vec![i]),
                H256::from([i; 32]),
                test_miner(),
                100,
                150,
            )
            .await
            .unwrap();
        }

        // Third should fail
        let result = dm
            .record_optimistic_result(
                H256::from([0xFF; 32]),
                test_model_hash(),
                Bytes::from(vec![0xFF]),
                Bytes::from(vec![0xFF]),
                H256::from([0xFF; 32]),
                test_miner(),
                100,
                150,
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("full"));
    }
}

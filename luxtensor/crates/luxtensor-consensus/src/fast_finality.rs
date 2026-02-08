// Fast finality mechanism with BFT-style guarantees
// Provides immediate finality for blocks with sufficient validator signatures
//
// View-Change Protocol (Phase 9):
// When a designated leader fails to produce a block within the timeout,
// validators initiate a view-change to elect a new leader. This prevents
// chain stalls due to offline/Byzantine leaders. The protocol:
//   1. Each validator monitors block production timeout
//   2. On timeout, validator sends ViewChange message for view+1
//   3. When ≥2/3 stake sends ViewChange for the same view, the new
//      leader (selected by view % validator_count) takes over
//   4. New leader produces a block with the accumulated ViewChange
//      certificate as proof of leader rotation

use crate::error::ConsensusError;
use crate::validator::ValidatorSet;
use luxtensor_core::types::{Address, Hash};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tracing::{debug, info};

// ============================================================================
// View-Change Types
// ============================================================================

/// Current phase of the BFT protocol for a given height
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BftPhase {
    /// Waiting for leader to propose a block
    WaitingForProposal,
    /// Collecting signatures for a proposed block
    CollectingSignatures,
    /// View-change in progress (leader timed out)
    ViewChange,
    /// Block finalized at this height
    Finalized,
}

/// A view-change message from a single validator
#[derive(Debug, Clone)]
pub struct ViewChangeMessage {
    /// The height this view-change is for
    pub height: u64,
    /// The view number being proposed (monotonically increasing)
    pub new_view: u64,
    /// Validator who sent this message
    pub validator: Address,
    /// Stake of the validator (for threshold calculation)
    pub stake: u128,
    /// Timestamp when this message was created
    pub timestamp: u64,
}

/// Accumulated view-change state for a particular (height, view) pair
#[derive(Debug, Clone)]
struct ViewChangeState {
    /// Validators who voted for this view
    voters: HashSet<Address>,
    /// Total stake accumulated
    total_stake: u128,
    /// Whether threshold has been reached
    threshold_reached: bool,
}

/// Per-height BFT tracking state
#[derive(Debug)]
struct HeightState {
    /// Current view number (0 => original leader, 1+ => after view-change)
    current_view: u64,
    /// Current BFT phase
    phase: BftPhase,
    /// When we started waiting for the current view's proposal
    proposal_deadline: Option<Instant>,
    /// The block hash proposed in the current view (if any)
    proposed_block: Option<Hash>,
}

/// Fast finality manager using validator signatures
pub struct FastFinality {
    /// Required percentage of stake for finality (e.g., 67 for 2/3)
    finality_threshold_percent: u8,
    /// Validator set
    validator_set: ValidatorSet,
    /// Signatures collected per block
    signatures: HashMap<Hash, BlockSignatures>,
    /// 🔧 FIX: Track which block each validator signed at each height
    /// Prevents equivocation — a validator signing conflicting blocks at the same height
    height_votes: HashMap<(Address, u64), Hash>,

    // === View-Change Protocol State (Phase 9) ===
    /// Per-height BFT state tracking
    height_states: HashMap<u64, HeightState>,
    /// View-change votes: (height, view) → ViewChangeState
    view_change_votes: HashMap<(u64, u64), ViewChangeState>,
    /// Block proposal timeout duration
    proposal_timeout: Duration,
    /// Maximum view number before giving up (prevents infinite view-changes)
    max_view: u64,
}

/// Signatures for a block
#[derive(Debug, Clone)]
struct BlockSignatures {
    /// Validators who signed
    signers: HashSet<Address>,
    /// Total stake that has signed
    total_stake: u128,
    /// Whether block reached finality
    finalized: bool,
}

impl FastFinality {
    /// Create a new fast finality instance
    pub fn new(finality_threshold_percent: u8, validator_set: ValidatorSet) -> Result<Self, ConsensusError> {
        if finality_threshold_percent <= 50 || finality_threshold_percent > 100 {
            return Err(ConsensusError::InvalidConfig(
                format!("Finality threshold must be between 51 and 100, got {}", finality_threshold_percent)
            ));
        }

        Ok(Self {
            finality_threshold_percent,
            validator_set,
            signatures: HashMap::new(),
            height_votes: HashMap::new(),
            height_states: HashMap::new(),
            view_change_votes: HashMap::new(),
            proposal_timeout: Duration::from_secs(10),
            max_view: 10,
        })
    }

    /// Add a validator signature for a block
    /// 🔧 FIX: Now takes block_height to detect equivocation (signing conflicting blocks)
    pub fn add_signature(
        &mut self,
        block_hash: Hash,
        block_height: u64,
        validator: Address,
    ) -> Result<bool, ConsensusError> {
        // Verify validator exists and get their stake
        let validator_info = self
            .validator_set
            .get_validator(&validator)
            .ok_or(ConsensusError::ValidatorNotFound(format!("{:?}", validator)))?;

        // 🔧 FIX: Check for equivocation — has this validator already voted for a
        // DIFFERENT block at this height?
        let vote_key = (validator, block_height);
        if let Some(previous_hash) = self.height_votes.get(&vote_key) {
            if *previous_hash != block_hash {
                return Err(ConsensusError::InvalidOperation(format!(
                    "Equivocation detected: validator {:?} signed conflicting blocks at height {}. \
                     Previous: {}, Current: {}",
                    validator, block_height,
                    hex::encode(previous_hash), hex::encode(&block_hash)
                )));
            }
        }
        // Record this validator's vote at this height
        self.height_votes.insert(vote_key, block_hash);

        // Get or create signature entry
        let entry = self.signatures.entry(block_hash).or_insert_with(|| {
            BlockSignatures {
                signers: HashSet::new(),
                total_stake: 0,
                finalized: false,
            }
        });

        // Check if already finalized
        if entry.finalized {
            return Ok(true);
        }

        // Check if validator already signed
        if entry.signers.contains(&validator) {
            return Ok(entry.finalized);
        }

        // Add signature
        entry.signers.insert(validator);
        entry.total_stake += validator_info.stake;

        debug!(
            "Added signature from validator {} for block {}",
            hex::encode(&validator),
            hex::encode(&block_hash)
        );

        // Check if finality threshold reached
        let total_stake = self.validator_set.total_stake();
        let required_stake = (total_stake * self.finality_threshold_percent as u128) / 100;

        if entry.total_stake >= required_stake {
            entry.finalized = true;
            info!(
                "Block {} reached fast finality with {}/{} stake ({}%)",
                hex::encode(&block_hash),
                entry.total_stake,
                total_stake,
                (entry.total_stake * 100) / total_stake
            );
            Ok(true)
        } else {
            debug!(
                "Block {} has {}/{} stake ({}%), needs {}% for finality",
                hex::encode(&block_hash),
                entry.total_stake,
                total_stake,
                (entry.total_stake * 100) / total_stake,
                self.finality_threshold_percent
            );
            Ok(false)
        }
    }

    /// Check if a block has reached finality
    pub fn is_finalized(&self, block_hash: &Hash) -> bool {
        self.signatures
            .get(block_hash)
            .map(|s| s.finalized)
            .unwrap_or(false)
    }

    /// Get finality progress for a block (percentage of stake signed)
    pub fn get_finality_progress(&self, block_hash: &Hash) -> Option<u8> {
        self.signatures.get(block_hash).map(|s| {
            let total_stake = self.validator_set.total_stake();
            if total_stake == 0 {
                0
            } else {
                ((s.total_stake * 100) / total_stake) as u8
            }
        })
    }

    /// Get number of validators who signed a block
    pub fn get_signer_count(&self, block_hash: &Hash) -> usize {
        self.signatures
            .get(block_hash)
            .map(|s| s.signers.len())
            .unwrap_or(0)
    }

    /// Get list of validators who signed a block
    pub fn get_signers(&self, block_hash: &Hash) -> Option<Vec<Address>> {
        self.signatures
            .get(block_hash)
            .map(|s| s.signers.iter().copied().collect())
    }

    /// Clear old signatures for blocks that are no longer needed
    pub fn prune_old_signatures(&mut self, keep_blocks: &[Hash]) {
        let keep_set: HashSet<_> = keep_blocks.iter().copied().collect();
        self.signatures.retain(|hash, _| keep_set.contains(hash));
        // 🔧 FIX: Also prune height_votes to prevent unbounded memory growth
        self.height_votes.retain(|_, hash| keep_set.contains(hash));
    }

    /// Get statistics about fast finality
    pub fn get_stats(&self) -> FastFinalityStats {
        let total_blocks = self.signatures.len();
        let finalized_blocks = self
            .signatures
            .values()
            .filter(|s| s.finalized)
            .count();

        let avg_stake = if total_blocks > 0 {
            let total: u128 = self.signatures.values().map(|s| s.total_stake).sum();
            total / total_blocks as u128
        } else {
            0
        };

        FastFinalityStats {
            total_blocks,
            finalized_blocks,
            pending_blocks: total_blocks - finalized_blocks,
            average_stake: avg_stake,
            threshold_percent: self.finality_threshold_percent,
        }
    }

    /// Update validator set
    pub fn update_validator_set(&mut self, validator_set: ValidatorSet) {
        self.validator_set = validator_set;

        // Re-check all pending blocks for finality
        let total_stake = self.validator_set.total_stake();
        let required_stake = (total_stake * self.finality_threshold_percent as u128) / 100;

        for entry in self.signatures.values_mut() {
            if !entry.finalized && entry.total_stake >= required_stake {
                entry.finalized = true;
            }
        }
    }

    // ========================================================================
    // View-Change Protocol Methods (Phase 9)
    // ========================================================================

    /// Set the proposal timeout duration
    pub fn set_proposal_timeout(&mut self, timeout: Duration) {
        self.proposal_timeout = timeout;
    }

    /// Set the maximum view number
    pub fn set_max_view(&mut self, max_view: u64) {
        self.max_view = max_view;
    }

    /// Begin tracking a new height. Called when the node expects a block at
    /// the given height. Starts the proposal timeout timer.
    pub fn begin_height(&mut self, height: u64) {
        if self.height_states.contains_key(&height) {
            return; // Already tracking
        }
        self.height_states.insert(height, HeightState {
            current_view: 0,
            phase: BftPhase::WaitingForProposal,
            proposal_deadline: Some(Instant::now() + self.proposal_timeout),
            proposed_block: None,
        });
        debug!("BFT: begin tracking height {} view 0", height);
    }

    /// Notify that a block has been proposed at the given height.
    /// Transitions from WaitingForProposal → CollectingSignatures.
    pub fn on_block_proposed(&mut self, height: u64, block_hash: Hash) {
        let state = self.height_states.entry(height).or_insert_with(|| HeightState {
            current_view: 0,
            phase: BftPhase::WaitingForProposal,
            proposal_deadline: Some(Instant::now() + self.proposal_timeout),
            proposed_block: None,
        });

        if state.phase == BftPhase::Finalized {
            return; // Already finalized, ignore
        }

        state.phase = BftPhase::CollectingSignatures;
        state.proposed_block = Some(block_hash);
        state.proposal_deadline = None; // Cancel timeout since we got a proposal
        debug!("BFT: block proposed at height {} view {}: {}",
            height, state.current_view, hex::encode(&block_hash));
    }

    /// Check if the proposal timeout has expired for any tracked height.
    /// Returns a list of (height, suggested_new_view) pairs that need view-change.
    pub fn check_timeouts(&self) -> Vec<(u64, u64)> {
        let now = Instant::now();
        let mut timed_out = Vec::new();

        for (&height, state) in &self.height_states {
            if state.phase == BftPhase::Finalized {
                continue;
            }
            if let Some(deadline) = state.proposal_deadline {
                if now >= deadline {
                    let next_view = state.current_view + 1;
                    if next_view <= self.max_view {
                        timed_out.push((height, next_view));
                    }
                }
            }
        }
        timed_out
    }

    /// Process a view-change vote from a validator.
    /// When ≥threshold stake votes for the same (height, new_view), the
    /// view-change completes and a new leader is elected.
    ///
    /// Returns `Ok(true)` if the view-change threshold was reached.
    pub fn add_view_change_vote(
        &mut self,
        msg: ViewChangeMessage,
    ) -> Result<bool, ConsensusError> {
        let height = msg.height;
        let new_view = msg.new_view;
        let validator = msg.validator;

        // Validate: new_view must be greater than current view
        if let Some(state) = self.height_states.get(&height) {
            if state.phase == BftPhase::Finalized {
                return Ok(false); // Already finalized, ignore
            }
            if new_view <= state.current_view {
                return Err(ConsensusError::InvalidOperation(format!(
                    "View-change for view {} but current view is already {}",
                    new_view, state.current_view
                )));
            }
        }

        if new_view > self.max_view {
            return Err(ConsensusError::InvalidOperation(format!(
                "View {} exceeds max_view {}", new_view, self.max_view
            )));
        }

        // Verify validator is in the set
        let validator_info = self.validator_set.get_validator(&validator)
            .ok_or_else(|| ConsensusError::ValidatorNotFound(format!("{:?}", validator)))?;

        let key = (height, new_view);
        let vc_state = self.view_change_votes.entry(key).or_insert_with(|| ViewChangeState {
            voters: HashSet::new(),
            total_stake: 0,
            threshold_reached: false,
        });

        if vc_state.threshold_reached {
            return Ok(true); // Already completed
        }

        if vc_state.voters.contains(&validator) {
            return Ok(false); // Duplicate vote
        }

        vc_state.voters.insert(validator);
        vc_state.total_stake += validator_info.stake;

        let total_stake = self.validator_set.total_stake();
        let required = (total_stake * self.finality_threshold_percent as u128) / 100;

        if vc_state.total_stake >= required {
            vc_state.threshold_reached = true;
            let completed_stake = vc_state.total_stake;

            // Compute leader index before borrowing self mutably via height_states
            let leader_index = self.get_leader_index(height, new_view);

            // Execute the view-change: update height state
            let height_state = self.height_states.entry(height).or_insert_with(|| HeightState {
                current_view: 0,
                phase: BftPhase::WaitingForProposal,
                proposal_deadline: None,
                proposed_block: None,
            });

            height_state.current_view = new_view;
            height_state.phase = BftPhase::WaitingForProposal;
            height_state.proposal_deadline = Some(Instant::now() + self.proposal_timeout);
            height_state.proposed_block = None;

            info!(
                "BFT: View-change completed for height {} → view {}. \
                 New leader: validator #{}. Stake: {}/{}",
                height, new_view,
                leader_index,
                completed_stake, total_stake
            );

            Ok(true)
        } else {
            debug!(
                "BFT: View-change vote for height {} view {}: {}/{} stake ({} voters)",
                height, new_view, vc_state.total_stake, total_stake, vc_state.voters.len()
            );
            Ok(false)
        }
    }

    /// Get the designated leader index for a given (height, view) pair.
    /// Leader = (height + view) % validator_count.
    /// This is deterministic so all honest nodes agree on who the leader is.
    pub fn get_leader_index(&self, height: u64, view: u64) -> usize {
        let count = self.validator_set.active_validators().len();
        if count == 0 { return 0; }
        ((height + view) % count as u64) as usize
    }

    /// Get the current view number for a height
    pub fn get_current_view(&self, height: u64) -> u64 {
        self.height_states.get(&height).map(|s| s.current_view).unwrap_or(0)
    }

    /// Get the current BFT phase for a height
    pub fn get_phase(&self, height: u64) -> BftPhase {
        self.height_states.get(&height).map(|s| s.phase).unwrap_or(BftPhase::WaitingForProposal)
    }

    /// Mark a height as finalized (called after block reaches finality threshold)
    pub fn mark_height_finalized(&mut self, height: u64) {
        if let Some(state) = self.height_states.get_mut(&height) {
            state.phase = BftPhase::Finalized;
            state.proposal_deadline = None;
        }
    }

    /// Get the number of view-change votes for a (height, view) pair
    pub fn get_view_change_vote_count(&self, height: u64, view: u64) -> usize {
        self.view_change_votes
            .get(&(height, view))
            .map(|s| s.voters.len())
            .unwrap_or(0)
    }

    /// Has a view-change completed for a (height, view)?
    pub fn is_view_change_complete(&self, height: u64, view: u64) -> bool {
        self.view_change_votes
            .get(&(height, view))
            .map(|s| s.threshold_reached)
            .unwrap_or(false)
    }

    /// Prune view-change state for heights below a given threshold
    pub fn prune_view_change_state(&mut self, below_height: u64) {
        self.height_states.retain(|h, _| *h >= below_height);
        self.view_change_votes.retain(|(h, _), _| *h >= below_height);
    }
}

/// Fast finality statistics
#[derive(Debug, Clone)]
pub struct FastFinalityStats {
    pub total_blocks: usize,
    pub finalized_blocks: usize,
    pub pending_blocks: usize,
    pub average_stake: u128,
    pub threshold_percent: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::Validator;
    use luxtensor_crypto::KeyPair;

    fn create_test_validator(stake: u128) -> (Address, Validator) {
        let keypair = KeyPair::generate();
        let address = Address::from(keypair.address());
        let mut public_key = [0u8; 32];
        let pk_bytes = keypair.public_key_bytes();
        public_key.copy_from_slice(&pk_bytes[..32.min(pk_bytes.len())]);

        let validator = Validator {
            address,
            stake,
            public_key,
            active: true,
            rewards: 0,
            last_active_slot: 0,
            activation_epoch: 0,
        };

        (address, validator)
    }

    fn create_validator_set(count: usize, stake_per_validator: u128) -> (ValidatorSet, Vec<Address>) {
        let mut set = ValidatorSet::new();
        let mut addresses = Vec::new();

        for _ in 0..count {
            let (addr, validator) = create_test_validator(stake_per_validator);
            set.add_validator(validator).unwrap();
            addresses.push(addr);
        }

        (set, addresses)
    }

    #[test]
    fn test_fast_finality_creation() {
        let (validator_set, _) = create_validator_set(4, 100);
        let finality = FastFinality::new(67, validator_set).unwrap();

        assert_eq!(finality.finality_threshold_percent, 67);
    }

    #[test]
    fn test_fast_finality_invalid_threshold() {
        let (validator_set, _) = create_validator_set(4, 100);
        let result = FastFinality::new(50, validator_set);
        assert!(result.is_err()); // Threshold <= 50 is rejected
    }

    #[test]
    fn test_add_signature() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];

        // Add first signature (33% stake)
        let finalized = finality.add_signature(block_hash, 1, addresses[0]).unwrap();
        assert!(!finalized);

        // Add second signature (67% stake) - should NOT be finalized yet (200/300 = 66.6%)
        let finalized = finality.add_signature(block_hash, 1, addresses[1]).unwrap();
        assert!(!finalized);

        // Add third signature (100% stake) - NOW it's finalized
        let finalized = finality.add_signature(block_hash, 1, addresses[2]).unwrap();
        assert!(finalized);
    }

    #[test]
    fn test_duplicate_signature() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];

        // Add signature
        finality.add_signature(block_hash, 1, addresses[0]).unwrap();

        // Add same signature again - should be idempotent
        finality.add_signature(block_hash, 1, addresses[0]).unwrap();

        // Should still only count once
        assert_eq!(finality.get_signer_count(&block_hash), 1);
    }

    #[test]
    fn test_is_finalized() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];

        assert!(!finality.is_finalized(&block_hash));

        // Add all 3 signatures for 100% stake
        finality.add_signature(block_hash, 1, addresses[0]).unwrap();
        finality.add_signature(block_hash, 1, addresses[1]).unwrap();
        finality.add_signature(block_hash, 1, addresses[2]).unwrap();

        assert!(finality.is_finalized(&block_hash));
    }

    #[test]
    fn test_get_finality_progress() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];

        // Initially 0%
        assert_eq!(finality.get_finality_progress(&block_hash), None);

        // After 1 signature: 33%
        finality.add_signature(block_hash, 1, addresses[0]).unwrap();
        assert_eq!(finality.get_finality_progress(&block_hash), Some(33));

        // After 2 signatures: 66%
        finality.add_signature(block_hash, 1, addresses[1]).unwrap();
        assert_eq!(finality.get_finality_progress(&block_hash), Some(66));

        // After 3 signatures: 100%
        finality.add_signature(block_hash, 1, addresses[2]).unwrap();
        assert_eq!(finality.get_finality_progress(&block_hash), Some(100));
    }

    #[test]
    fn test_get_signers() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];

        finality.add_signature(block_hash, 1, addresses[0]).unwrap();
        finality.add_signature(block_hash, 1, addresses[1]).unwrap();

        let signers = finality.get_signers(&block_hash).unwrap();
        assert_eq!(signers.len(), 2);
        assert!(signers.contains(&addresses[0]));
        assert!(signers.contains(&addresses[1]));
    }

    #[test]
    fn test_prune_old_signatures() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block1 = [1u8; 32];
        let block2 = [2u8; 32];

        finality.add_signature(block1, 1, addresses[0]).unwrap();
        finality.add_signature(block2, 2, addresses[0]).unwrap();

        assert_eq!(finality.signatures.len(), 2);

        // Prune block1
        finality.prune_old_signatures(&[block2]);

        assert_eq!(finality.signatures.len(), 1);
        assert!(finality.signatures.contains_key(&block2));
        assert!(!finality.signatures.contains_key(&block1));
    }

    #[test]
    fn test_get_stats() {
        let (validator_set, addresses) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block1 = [1u8; 32];
        let block2 = [2u8; 32];

        // Block 1: finalized (3/4 validators)
        finality.add_signature(block1, 1, addresses[0]).unwrap();
        finality.add_signature(block1, 1, addresses[1]).unwrap();
        finality.add_signature(block1, 1, addresses[2]).unwrap();

        // Block 2: not finalized (2/4 validators)
        finality.add_signature(block2, 2, addresses[0]).unwrap();
        finality.add_signature(block2, 2, addresses[1]).unwrap();

        let stats = finality.get_stats();
        assert_eq!(stats.total_blocks, 2);
        assert_eq!(stats.finalized_blocks, 1);
        assert_eq!(stats.pending_blocks, 1);
        assert_eq!(stats.threshold_percent, 67);
    }

    #[test]
    fn test_invalid_validator() {
        let (validator_set, _) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];
        let invalid_validator = Address::from([99u8; 20]);

        let result = finality.add_signature(block_hash, 1, invalid_validator);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_validator_set() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        let block_hash = [1u8; 32];

        // Add signatures but don't reach threshold
        finality.add_signature(block_hash, 1, addresses[0]).unwrap();
        assert!(!finality.is_finalized(&block_hash));

        // Create new validator set with lower total stake
        let (new_set, _) = create_validator_set(2, 100);

        // Update validator set - block might now be finalized if threshold reached
        finality.update_validator_set(new_set);
    }

    // === View-Change Protocol Tests ===

    #[test]
    fn test_begin_height() {
        let (validator_set, _) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        finality.begin_height(1);
        assert_eq!(finality.get_current_view(1), 0);
        assert_eq!(finality.get_phase(1), BftPhase::WaitingForProposal);
    }

    #[test]
    fn test_on_block_proposed() {
        let (validator_set, _) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        finality.begin_height(1);
        finality.on_block_proposed(1, [0xAA; 32]);
        assert_eq!(finality.get_phase(1), BftPhase::CollectingSignatures);
    }

    #[test]
    fn test_view_change_threshold() {
        let (validator_set, addresses) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        finality.begin_height(1);

        // 3 out of 4 validators (75% stake) vote for view-change
        let result1 = finality.add_view_change_vote(ViewChangeMessage {
            height: 1, new_view: 1, validator: addresses[0], stake: 100, timestamp: 0,
        }).unwrap();
        assert!(!result1);

        let result2 = finality.add_view_change_vote(ViewChangeMessage {
            height: 1, new_view: 1, validator: addresses[1], stake: 100, timestamp: 0,
        }).unwrap();
        assert!(!result2);

        let result3 = finality.add_view_change_vote(ViewChangeMessage {
            height: 1, new_view: 1, validator: addresses[2], stake: 100, timestamp: 0,
        }).unwrap();
        assert!(result3); // 300/400 = 75% > 67% threshold

        assert_eq!(finality.get_current_view(1), 1);
        assert!(finality.is_view_change_complete(1, 1));
    }

    #[test]
    fn test_view_change_duplicate_vote() {
        let (validator_set, addresses) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        finality.begin_height(1);

        finality.add_view_change_vote(ViewChangeMessage {
            height: 1, new_view: 1, validator: addresses[0], stake: 100, timestamp: 0,
        }).unwrap();

        // Same validator votes again — should be ignored
        let dup = finality.add_view_change_vote(ViewChangeMessage {
            height: 1, new_view: 1, validator: addresses[0], stake: 100, timestamp: 0,
        }).unwrap();
        assert!(!dup);
        assert_eq!(finality.get_view_change_vote_count(1, 1), 1);
    }

    #[test]
    fn test_leader_rotation() {
        let (validator_set, _) = create_validator_set(4, 100);
        let finality = FastFinality::new(67, validator_set).unwrap();

        // Leader index should change with view
        let leader_v0 = finality.get_leader_index(1, 0);
        let leader_v1 = finality.get_leader_index(1, 1);
        assert_ne!(leader_v0, leader_v1);
    }

    #[test]
    fn test_mark_height_finalized() {
        let (validator_set, _) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        finality.begin_height(1);
        finality.mark_height_finalized(1);
        assert_eq!(finality.get_phase(1), BftPhase::Finalized);
    }

    #[test]
    fn test_prune_view_change_state() {
        let (validator_set, _) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set).unwrap();

        finality.begin_height(1);
        finality.begin_height(2);
        finality.begin_height(3);

        finality.prune_view_change_state(2);
        // Height 1 should be pruned
        assert_eq!(finality.get_phase(1), BftPhase::WaitingForProposal); // default (pruned)
        assert_eq!(finality.get_current_view(2), 0); // still exists
    }
}

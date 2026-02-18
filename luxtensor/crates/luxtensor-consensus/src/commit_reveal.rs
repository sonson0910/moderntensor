// Commit-Reveal mechanism for validator weights
// Prevents weight manipulation by requiring validators to commit hash before revealing
//
// Flow:
// 1. Validator commits: hash(weights || salt) during commit window
// 2. After commit window ends, reveal window starts
// 3. Validator reveals: actual weights + salt, verified against commit hash
// 4. After reveal window, weights are finalized

use luxtensor_core::types::{Address, Hash};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use tracing::{info, warn};

/// Configuration for commit-reveal mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRevealConfig {
    /// Number of blocks for commit window
    pub commit_window: u64,
    /// Number of blocks for reveal window
    pub reveal_window: u64,
    /// Minimum number of validators required to commit
    pub min_commits: usize,
    /// Whether to slash validators who don't reveal
    pub slash_on_no_reveal: bool,
    /// Percentage to slash for not revealing (0-100)
    /// Per tokenomics: 80% of slashed amount is burned
    pub no_reveal_slash_percent: u8,
    /// Percentage of slashed amount to burn (0-100)
    /// Per tokenomics: 80% burned, 20% to treasury
    pub slash_burn_percent: u8,
}

impl Default for CommitRevealConfig {
    fn default() -> Self {
        Self {
            commit_window: 100, // ~20 minutes at 12s blocks
            reveal_window: 100, // ~20 minutes to reveal
            min_commits: 1,     // At least 1 validator (for testnets)
            slash_on_no_reveal: true,
            // Per tokenomics: validators who don't reveal get slashed
            no_reveal_slash_percent: 5, // 5% of stake slashed (was 80%, reduced to prevent catastrophic loss from transient issues)
            // Per tokenomics: 80% of slashed tokens are burned
            slash_burn_percent: 80,
        }
    }
}

/// Status of a commit-reveal epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpochPhase {
    /// Accepting commits
    Committing,
    /// Commit window closed, reveal window open
    Revealing,
    /// All windows closed, ready to finalize
    Finalizing,
    /// Epoch complete
    Finalized,
}

/// Result of slashing calculation per tokenomics
/// 80% of slashed tokens are burned, 20% go to treasury
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingResult {
    /// Validators to be slashed
    pub validators: Vec<Address>,
    /// Percentage of stake to slash (0-100)
    pub slash_percent: u8,
    /// Percentage of slashed amount to burn (0-100)
    /// Per tokenomics: 80%
    pub burn_percent: u8,
}

impl SlashingResult {
    /// Calculate burn and treasury amounts from total slashed
    pub fn calculate_distribution(&self, total_slashed: u128) -> (u128, u128) {
        let burn_amount = (total_slashed * self.burn_percent as u128) / 100;
        let treasury_amount = total_slashed - burn_amount;
        (burn_amount, treasury_amount)
    }
}

/// Result of epoch finalization including slashing info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochFinalizationResult {
    /// Aggregated weights from revealed commits
    pub weights: Vec<(u64, u16)>,
    /// Epoch number
    pub epoch_number: u64,
    /// Number of validators who revealed
    pub revealed_count: usize,
    /// Slashing info (if any validators didn't reveal)
    pub slashing: Option<SlashingResult>,
}

impl EpochFinalizationResult {
    /// Check if any slashing occurred
    pub fn has_slashing(&self) -> bool {
        self.slashing.is_some()
    }

    /// Get validators to slash
    pub fn slashed_validators(&self) -> Vec<Address> {
        self.slashing.as_ref().map(|s| s.validators.clone()).unwrap_or_default()
    }
}

/// A weight commit from a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightCommit {
    /// Validator address
    pub validator: Address,
    /// Subnet ID
    pub subnet_uid: u64,
    /// Commit hash (keccak256 of weights || salt)
    pub commit_hash: Hash,
    /// Block when committed
    pub committed_at: u64,
    /// Whether revealed
    pub revealed: bool,
    /// Revealed weights (if revealed)
    pub weights: Option<Vec<(u64, u16)>>,
    /// Salt used (if revealed)
    pub salt: Option<[u8; 32]>,
}

impl WeightCommit {
    /// Create new commit
    pub fn new(validator: Address, subnet_uid: u64, commit_hash: Hash, block: u64) -> Self {
        Self {
            validator,
            subnet_uid,
            commit_hash,
            committed_at: block,
            revealed: false,
            weights: None,
            salt: None,
        }
    }

    /// Verify revealed weights match commit hash
    pub fn verify_reveal(&self, weights: &[(u64, u16)], salt: &[u8; 32]) -> bool {
        let computed_hash = compute_commit_hash(weights, salt);
        computed_hash == self.commit_hash
    }
}

/// Epoch state for a subnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetEpochState {
    pub subnet_uid: u64,
    pub epoch_number: u64,
    pub phase: EpochPhase,
    pub commit_start_block: u64,
    pub reveal_start_block: u64,
    pub finalize_block: u64,
    pub commits: Vec<WeightCommit>,
    /// Cached aggregated weights - updated incrementally during reveal phase
    /// Avoids O(n*m) recomputation on each get_revealed_weights() call
    #[serde(default)]
    cached_weights: HashMap<u64, (u64, usize)>, // uid -> (sum, count)
}

impl SubnetEpochState {
    pub fn new(
        subnet_uid: u64,
        epoch_number: u64,
        start_block: u64,
        config: &CommitRevealConfig,
    ) -> Self {
        Self {
            subnet_uid,
            epoch_number,
            phase: EpochPhase::Committing,
            commit_start_block: start_block,
            reveal_start_block: start_block + config.commit_window,
            finalize_block: start_block + config.commit_window + config.reveal_window,
            commits: Vec::new(),
            cached_weights: HashMap::new(),
        }
    }

    /// Update phase based on current block
    pub fn update_phase(&mut self, current_block: u64) {
        if current_block >= self.finalize_block {
            self.phase = EpochPhase::Finalizing;
        } else if current_block >= self.reveal_start_block {
            self.phase = EpochPhase::Revealing;
        } else {
            self.phase = EpochPhase::Committing;
        }
    }

    /// Get revealed weights aggregated
    /// Optimized: Uses cached weights if available (O(1)), falls back to recompute if needed
    pub fn get_revealed_weights(&self) -> Vec<(u64, u16)> {
        // Use cached weights if populated (from incremental updates during reveal)
        if !self.cached_weights.is_empty() {
            return self
                .cached_weights
                .iter()
                .map(|(uid, (sum, count))| {
                    let avg = if *count > 0 {
                        ((*sum / *count as u64) as u128).min(u16::MAX as u128) as u16
                    } else {
                        0
                    };
                    (*uid, avg)
                })
                .collect();
        }

        // Fallback: recompute from commits (for backward compatibility)
        let mut weight_sums: HashMap<u64, (u64, usize)> = HashMap::new();
        for commit in &self.commits {
            if commit.revealed {
                if let Some(weights) = &commit.weights {
                    for (uid, weight) in weights {
                        let entry = weight_sums.entry(*uid).or_insert((0, 0));
                        entry.0 += *weight as u64;
                        entry.1 += 1;
                    }
                }
            }
        }

        weight_sums
            .iter()
            .map(|(uid, (sum, count))| {
                let avg = if *count > 0 {
                    ((*sum / *count as u64) as u128).min(u16::MAX as u128) as u16
                } else {
                    0
                };
                (*uid, avg)
            })
            .collect()
    }

    /// Update cached weights incrementally when a validator reveals
    /// Called during reveal phase to avoid O(n*m) recomputation
    pub fn update_cached_weights(&mut self, weights: &[(u64, u16)]) {
        for (uid, weight) in weights {
            let entry = self.cached_weights.entry(*uid).or_insert((0, 0));
            entry.0 += *weight as u64;
            entry.1 += 1;
        }
    }
}

/// Compute commit hash from weights and salt
pub fn compute_commit_hash(weights: &[(u64, u16)], salt: &[u8; 32]) -> Hash {
    let mut hasher = Keccak256::new();

    // Encode weights deterministically
    for (uid, weight) in weights {
        hasher.update(uid.to_be_bytes());
        hasher.update(weight.to_be_bytes());
    }

    // Add salt
    hasher.update(salt);

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Generate cryptographically secure random salt
pub fn generate_salt() -> [u8; 32] {
    use rand::Rng;

    let mut salt = [0u8; 32];
    let mut rng = rand::thread_rng();
    rng.fill(&mut salt);

    salt
}

/// Commit-Reveal Manager
pub struct CommitRevealManager {
    config: CommitRevealConfig,
    /// Epoch states per subnet
    epochs: RwLock<HashMap<u64, SubnetEpochState>>,
    /// Historical finalized weights
    history: RwLock<Vec<(u64, u64, Vec<(u64, u16)>)>>, // (subnet_uid, epoch, weights)
}

impl CommitRevealManager {
    /// Create new manager
    pub fn new(config: CommitRevealConfig) -> Self {
        Self { config, epochs: RwLock::new(HashMap::new()), history: RwLock::new(Vec::new()) }
    }

    /// Start new epoch for subnet
    pub fn start_epoch(&self, subnet_uid: u64, epoch_number: u64, current_block: u64) {
        let state = SubnetEpochState::new(subnet_uid, epoch_number, current_block, &self.config);

        self.epochs.write().insert(subnet_uid, state);
        info!(
            "Started commit-reveal epoch {} for subnet {} at block {}",
            epoch_number, subnet_uid, current_block
        );
    }

    /// Commit weights hash
    pub fn commit_weights(
        &self,
        subnet_uid: u64,
        validator: Address,
        commit_hash: Hash,
        current_block: u64,
    ) -> Result<(), CommitRevealError> {
        let mut epochs = self.epochs.write();

        let state = epochs.get_mut(&subnet_uid).ok_or(CommitRevealError::NoActiveEpoch)?;

        // Check phase
        if state.phase != EpochPhase::Committing {
            return Err(CommitRevealError::NotInCommitPhase);
        }

        // Check if already committed
        if state.commits.iter().any(|c| c.validator == validator) {
            return Err(CommitRevealError::AlreadyCommitted);
        }

        // Add commit
        let commit = WeightCommit::new(validator, subnet_uid, commit_hash, current_block);
        state.commits.push(commit);

        info!(
            "Validator {:?} committed weights for subnet {} (hash: {:?})",
            validator, subnet_uid, commit_hash
        );

        Ok(())
    }

    /// Reveal weights
    pub fn reveal_weights(
        &self,
        subnet_uid: u64,
        validator: Address,
        weights: Vec<(u64, u16)>,
        salt: [u8; 32],
        current_block: u64,
    ) -> Result<(), CommitRevealError> {
        let mut epochs = self.epochs.write();

        let state = epochs.get_mut(&subnet_uid).ok_or(CommitRevealError::NoActiveEpoch)?;

        // Update phase
        state.update_phase(current_block);

        // Check phase
        if state.phase != EpochPhase::Revealing {
            return Err(CommitRevealError::NotInRevealPhase);
        }

        // Find commit
        let commit = state
            .commits
            .iter_mut()
            .find(|c| c.validator == validator)
            .ok_or(CommitRevealError::NoCommitFound)?;

        // Check not already revealed
        if commit.revealed {
            return Err(CommitRevealError::AlreadyRevealed);
        }

        // Verify hash matches
        if !commit.verify_reveal(&weights, &salt) {
            warn!("Validator {:?} reveal hash mismatch for subnet {}", validator, subnet_uid);
            return Err(CommitRevealError::HashMismatch);
        }

        // Store revealed data
        commit.revealed = true;
        commit.weights = Some(weights.clone());
        commit.salt = Some(salt);

        // Update cached weights incrementally (optimized aggregation)
        state.update_cached_weights(&weights);

        info!(
            "Validator {:?} revealed weights for subnet {} ({} entries)",
            validator,
            subnet_uid,
            weights.len()
        );

        Ok(())
    }

    /// Finalize epoch and return aggregated weights
    pub fn finalize_epoch(
        &self,
        subnet_uid: u64,
        current_block: u64,
    ) -> Result<Vec<(u64, u16)>, CommitRevealError> {
        let result = self.finalize_epoch_with_slashing(subnet_uid, current_block)?;
        Ok(result.weights)
    }

    /// Finalize epoch with full slashing information
    /// Returns weights and slashing details for tokenomics compliance
    pub fn finalize_epoch_with_slashing(
        &self,
        subnet_uid: u64,
        current_block: u64,
    ) -> Result<EpochFinalizationResult, CommitRevealError> {
        let mut epochs = self.epochs.write();

        let state = epochs.get_mut(&subnet_uid).ok_or(CommitRevealError::NoActiveEpoch)?;

        // Update phase
        state.update_phase(current_block);

        // Check phase
        if state.phase != EpochPhase::Finalizing {
            return Err(CommitRevealError::NotInFinalizePhase);
        }

        // Check minimum commits
        let revealed_count = state.commits.iter().filter(|c| c.revealed).count();
        if revealed_count < self.config.min_commits {
            return Err(CommitRevealError::InsufficientReveals);
        }

        // Get revealed weights
        let weights = state.get_revealed_weights();

        // Record in history (pruned to prevent unbounded memory growth)
        {
            const MAX_HISTORY: usize = 1000;
            let mut history = self.history.write();
            history.push((subnet_uid, state.epoch_number, weights.clone()));
            if history.len() > MAX_HISTORY {
                let excess = history.len() - MAX_HISTORY;
                history.drain(..excess);
            }
        }

        // Mark as finalized
        state.phase = EpochPhase::Finalized;

        // Get non-revealers for slashing (per tokenomics: 80% burned)
        let non_revealers: Vec<Address> =
            state.commits.iter().filter(|c| !c.revealed).map(|c| c.validator).collect();

        // Calculate slashing per tokenomics
        let slashing =
            if self.config.slash_on_no_reveal && !non_revealers.is_empty() {
                let slash_percent = self.config.no_reveal_slash_percent;
                let burn_percent = self.config.slash_burn_percent;

                warn!(
                "Epoch {} for subnet {}: {} validators did not reveal, slashing {}% ({}% burned)",
                state.epoch_number, subnet_uid, non_revealers.len(), slash_percent, burn_percent
            );

                Some(SlashingResult {
                    validators: non_revealers,
                    slash_percent,
                    burn_percent,
                    // Burn = slash_amount * burn_percent / 100
                    // Treasury = slash_amount * (100 - burn_percent) / 100
                })
            } else {
                None
            };

        info!(
            "Finalized epoch {} for subnet {} with {} weight entries",
            state.epoch_number,
            subnet_uid,
            weights.len()
        );

        Ok(EpochFinalizationResult {
            weights,
            epoch_number: state.epoch_number,
            revealed_count,
            slashing,
        })
    }

    /// Get current epoch state
    pub fn get_epoch_state(&self, subnet_uid: u64) -> Option<SubnetEpochState> {
        self.epochs.read().get(&subnet_uid).cloned()
    }

    /// Get pending commits for subnet
    pub fn get_pending_commits(&self, subnet_uid: u64) -> Vec<WeightCommit> {
        self.epochs.read().get(&subnet_uid).map(|s| s.commits.clone()).unwrap_or_default()
    }

    /// Check if validator has committed
    pub fn has_committed(&self, subnet_uid: u64, validator: &Address) -> bool {
        self.epochs
            .read()
            .get(&subnet_uid)
            .map(|s| s.commits.iter().any(|c| &c.validator == validator))
            .unwrap_or(false)
    }

    /// Check if validator has revealed
    pub fn has_revealed(&self, subnet_uid: u64, validator: &Address) -> bool {
        self.epochs
            .read()
            .get(&subnet_uid)
            .map(|s| s.commits.iter().any(|c| &c.validator == validator && c.revealed))
            .unwrap_or(false)
    }

    /// Get configuration
    pub fn config(&self) -> &CommitRevealConfig {
        &self.config
    }
}

/// Errors for commit-reveal operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitRevealError {
    NoActiveEpoch,
    NotInCommitPhase,
    NotInRevealPhase,
    NotInFinalizePhase,
    AlreadyCommitted,
    AlreadyRevealed,
    NoCommitFound,
    HashMismatch,
    InsufficientReveals,
}

impl std::fmt::Display for CommitRevealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoActiveEpoch => write!(f, "No active epoch for this subnet"),
            Self::NotInCommitPhase => write!(f, "Not in commit phase"),
            Self::NotInRevealPhase => write!(f, "Not in reveal phase"),
            Self::NotInFinalizePhase => write!(f, "Not in finalize phase"),
            Self::AlreadyCommitted => write!(f, "Validator already committed"),
            Self::AlreadyRevealed => write!(f, "Validator already revealed"),
            Self::NoCommitFound => write!(f, "No commit found for validator"),
            Self::HashMismatch => write!(f, "Revealed weights hash does not match commit"),
            Self::InsufficientReveals => write!(f, "Insufficient number of reveals"),
        }
    }
}

impl std::error::Error for CommitRevealError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(n: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[0] = n;
        Address::from(addr)
    }

    #[test]
    fn test_compute_commit_hash() {
        let weights = vec![(1, 100u16), (2, 200u16)];
        let salt = [42u8; 32];

        let hash1 = compute_commit_hash(&weights, &salt);
        let hash2 = compute_commit_hash(&weights, &salt);

        // Same inputs = same hash
        assert_eq!(hash1, hash2);

        // Different salt = different hash
        let different_salt = [43u8; 32];
        let hash3 = compute_commit_hash(&weights, &different_salt);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_commit_reveal_flow() {
        let manager = CommitRevealManager::new(CommitRevealConfig {
            commit_window: 10,
            reveal_window: 10,
            min_commits: 1,
            ..Default::default()
        });

        let subnet_uid = 1u64;
        let validator = test_address(1);
        let weights = vec![(0, 500u16), (1, 500u16)];
        let salt = [99u8; 32];

        // Start epoch
        manager.start_epoch(subnet_uid, 1, 0);

        // Commit
        let commit_hash = compute_commit_hash(&weights, &salt);
        assert!(manager.commit_weights(subnet_uid, validator, commit_hash, 5).is_ok());

        // Cannot commit again
        assert_eq!(
            manager.commit_weights(subnet_uid, validator, commit_hash, 6),
            Err(CommitRevealError::AlreadyCommitted)
        );

        // Cannot reveal during commit phase
        assert_eq!(
            manager.reveal_weights(subnet_uid, validator, weights.clone(), salt, 5),
            Err(CommitRevealError::NotInRevealPhase)
        );

        // Reveal during reveal phase
        assert!(manager.reveal_weights(subnet_uid, validator, weights.clone(), salt, 15).is_ok());

        // Cannot reveal again
        assert_eq!(
            manager.reveal_weights(subnet_uid, validator, weights.clone(), salt, 16),
            Err(CommitRevealError::AlreadyRevealed)
        );

        // Finalize
        let final_weights = manager.finalize_epoch(subnet_uid, 25).unwrap();
        assert_eq!(final_weights.len(), 2);
    }

    #[test]
    fn test_hash_mismatch_rejected() {
        let manager = CommitRevealManager::new(CommitRevealConfig {
            commit_window: 10,
            reveal_window: 10,
            min_commits: 1,
            ..Default::default()
        });

        let subnet_uid = 1u64;
        let validator = test_address(1);
        let weights = vec![(0, 500u16)];
        let salt = [99u8; 32];
        let wrong_weights = vec![(0, 600u16)]; // Different weights

        manager.start_epoch(subnet_uid, 1, 0);

        // Commit with correct weights
        let commit_hash = compute_commit_hash(&weights, &salt);
        manager.commit_weights(subnet_uid, validator, commit_hash, 5).unwrap();

        // Try to reveal with different weights
        assert_eq!(
            manager.reveal_weights(subnet_uid, validator, wrong_weights, salt, 15),
            Err(CommitRevealError::HashMismatch)
        );
    }
}

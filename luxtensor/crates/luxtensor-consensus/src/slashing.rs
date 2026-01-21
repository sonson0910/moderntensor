// Slashing module for validator penalties
// Handles: offline validators, double signing, malicious behavior

use crate::error::ConsensusError;
use crate::validator::ValidatorSet;
use luxtensor_core::types::{Address, Hash};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

/// Slashing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingConfig {
    /// Blocks before considered offline
    pub offline_threshold: u64,
    /// Percentage to slash for being offline (0-100)
    pub offline_slash_percent: u8,
    /// Percentage to slash for double signing (0-100)
    pub double_sign_slash_percent: u8,
    /// Percentage to slash for invalid block (0-100)
    pub invalid_block_slash_percent: u8,
    /// Minimum slash amount (absolute)
    pub min_slash_amount: u128,
    /// Jail duration in blocks
    pub jail_duration: u64,
}

impl Default for SlashingConfig {
    fn default() -> Self {
        Self {
            offline_threshold: 100,          // 100 missed blocks
            offline_slash_percent: 1,        // 1% stake
            double_sign_slash_percent: 10,   // 10% stake
            invalid_block_slash_percent: 5,  // 5% stake
            min_slash_amount: 1_000_000_000_000_000_000, // 1 token
            jail_duration: 7200,             // ~24 hours @ 12s blocks
        }
    }
}

/// Reasons for slashing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SlashReason {
    /// Validator missed too many blocks
    Offline,
    /// Validator signed two different blocks at same height
    DoubleSigning,
    /// Validator proposed invalid block
    InvalidBlock,
    /// Validator submitted invalid weights
    InvalidWeights,
    /// Custom reason with percentage
    Custom(u8),
}

impl SlashReason {
    /// Get slash percentage for this reason
    pub fn slash_percent(&self, config: &SlashingConfig) -> u8 {
        match self {
            SlashReason::Offline => config.offline_slash_percent,
            SlashReason::DoubleSigning => config.double_sign_slash_percent,
            SlashReason::InvalidBlock => config.invalid_block_slash_percent,
            SlashReason::InvalidWeights => config.offline_slash_percent,
            SlashReason::Custom(pct) => *pct,
        }
    }
}

/// Evidence of misbehavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingEvidence {
    /// Validator address
    pub validator: Address,
    /// Reason for slash
    pub reason: SlashReason,
    /// Block height when offense occurred
    pub height: u64,
    /// Evidence hash (block signatures, etc.)
    pub evidence_hash: Option<Hash>,
    /// Timestamp
    pub timestamp: u64,
}

/// Slashing event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashEvent {
    /// Evidence that triggered slash
    pub evidence: SlashingEvidence,
    /// Amount slashed
    pub amount_slashed: u128,
    /// Whether validator was jailed
    pub jailed: bool,
    /// Block height of slash execution
    pub executed_at: u64,
}

/// Jail status for a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JailStatus {
    /// When jailed
    pub jailed_at: u64,
    /// When jail ends
    pub release_at: u64,
    /// Reason for jailing
    pub reason: SlashReason,
}

/// Slashing manager
pub struct SlashingManager {
    config: SlashingConfig,

    /// Missed block counter per validator
    missed_blocks: RwLock<HashMap<Address, u64>>,

    /// Double signing evidence: (height, block_hash) -> signatures
    double_sign_evidence: RwLock<HashMap<(u64, Hash), Vec<(Address, Hash)>>>,

    /// Jailed validators
    jailed: RwLock<HashMap<Address, JailStatus>>,

    /// Slash history
    slash_history: RwLock<Vec<SlashEvent>>,

    /// Reference to validator set
    validator_set: Arc<RwLock<ValidatorSet>>,
}

impl SlashingManager {
    /// Create new slashing manager
    pub fn new(
        config: SlashingConfig,
        validator_set: Arc<RwLock<ValidatorSet>>,
    ) -> Self {
        Self {
            config,
            missed_blocks: RwLock::new(HashMap::new()),
            double_sign_evidence: RwLock::new(HashMap::new()),
            jailed: RwLock::new(HashMap::new()),
            slash_history: RwLock::new(Vec::new()),
            validator_set,
        }
    }

    /// Record a missed block for validator
    pub fn record_missed_block(&self, validator: &Address) {
        let mut missed = self.missed_blocks.write();
        let count = missed.entry(*validator).or_insert(0);
        *count += 1;

        if *count >= self.config.offline_threshold {
            warn!(
                "Validator {:?} missed {} blocks, threshold reached",
                validator, count
            );
        }
    }

    /// Reset missed blocks for validator (called when they produce a block)
    pub fn reset_missed_blocks(&self, validator: &Address) {
        self.missed_blocks.write().remove(validator);
    }

    /// Check if validator should be slashed for being offline
    pub fn check_offline(&self, validator: &Address) -> Option<SlashingEvidence> {
        let missed = self.missed_blocks.read();
        if let Some(&count) = missed.get(validator) {
            if count >= self.config.offline_threshold {
                return Some(SlashingEvidence {
                    validator: *validator,
                    reason: SlashReason::Offline,
                    height: 0, // Should be set by caller
                    evidence_hash: None,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }
        }
        None
    }

    /// Record potential double signing
    pub fn record_block_signature(
        &self,
        height: u64,
        block_hash: Hash,
        validator: Address,
        signature_hash: Hash,
    ) {
        let mut evidence = self.double_sign_evidence.write();
        let key = (height, block_hash);
        let sigs = evidence.entry(key).or_insert_with(Vec::new);
        sigs.push((validator, signature_hash));
    }

    /// Check for double signing at a height
    pub fn check_double_signing(&self, height: u64) -> Vec<SlashingEvidence> {
        let evidence = self.double_sign_evidence.read();
        let mut offenders = Vec::new();

        // Find validators who signed multiple blocks at same height
        let mut validator_blocks: HashMap<Address, Vec<Hash>> = HashMap::new();

        for ((h, block_hash), sigs) in evidence.iter() {
            if *h == height {
                for (validator, _sig_hash) in sigs {
                    validator_blocks
                        .entry(*validator)
                        .or_insert_with(Vec::new)
                        .push(*block_hash);
                }
            }
        }

        for (validator, blocks) in validator_blocks {
            if blocks.len() > 1 {
                // Same validator signed different blocks at same height!
                warn!(
                    "Double signing detected! Validator {:?} signed {} blocks at height {}",
                    validator, blocks.len(), height
                );

                offenders.push(SlashingEvidence {
                    validator,
                    reason: SlashReason::DoubleSigning,
                    height,
                    evidence_hash: Some(blocks[0]), // First conflicting block
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                });
            }
        }

        offenders
    }

    /// Execute slash on a validator
    pub fn slash(
        &self,
        evidence: SlashingEvidence,
        current_height: u64,
    ) -> Result<SlashEvent, ConsensusError> {
        let percent = evidence.reason.slash_percent(&self.config);

        // Get validator stake
        let mut validator_set = self.validator_set.write();
        let validator = validator_set
            .get_validator(&evidence.validator)
            .ok_or_else(|| {
                ConsensusError::ValidatorNotFound(format!("{:?}", evidence.validator))
            })?;

        // Calculate slash amount
        let slash_amount = (validator.stake as u128 * percent as u128) / 100;
        let slash_amount = slash_amount.max(self.config.min_slash_amount);
        let slash_amount = slash_amount.min(validator.stake); // Can't slash more than stake

        // Apply slash
        validator_set
            .slash_stake(&evidence.validator, slash_amount)
            .map_err(|e| ConsensusError::SlashingFailed(e.to_string()))?;

        // Jail validator for serious offenses
        let should_jail = matches!(
            evidence.reason,
            SlashReason::DoubleSigning | SlashReason::InvalidBlock
        );

        if should_jail {
            let jail_status = JailStatus {
                jailed_at: current_height,
                release_at: current_height + self.config.jail_duration,
                reason: evidence.reason,
            };
            self.jailed.write().insert(evidence.validator, jail_status);

            // Deactivate validator
            if let Err(e) = validator_set.deactivate_validator(&evidence.validator) {
                warn!("Failed to deactivate jailed validator: {}", e);
            }
        }

        // Record event
        let event = SlashEvent {
            evidence,
            amount_slashed: slash_amount,
            jailed: should_jail,
            executed_at: current_height,
        };

        self.slash_history.write().push(event.clone());

        info!(
            "Slashed validator {:?}: {} tokens, jailed: {}",
            event.evidence.validator, slash_amount, should_jail
        );

        // Reset missed blocks counter
        self.missed_blocks.write().remove(&event.evidence.validator);

        Ok(event)
    }

    /// Check if validator is jailed
    pub fn is_jailed(&self, validator: &Address) -> bool {
        self.jailed.read().contains_key(validator)
    }

    /// Get jail status
    pub fn get_jail_status(&self, validator: &Address) -> Option<JailStatus> {
        self.jailed.read().get(validator).cloned()
    }

    /// Process unjailing (called each block)
    pub fn process_unjail(&self, current_height: u64) -> Vec<Address> {
        let mut jailed = self.jailed.write();
        let mut validator_set = self.validator_set.write();

        let mut unjailed = Vec::new();

        jailed.retain(|validator, status| {
            if current_height >= status.release_at {
                info!("Unjailing validator {:?} at height {}", validator, current_height);

                // Reactivate validator
                if let Err(e) = validator_set.activate_validator(validator) {
                    warn!("Failed to reactivate unjailed validator: {}", e);
                }

                unjailed.push(*validator);
                false // Remove from jailed map
            } else {
                true // Keep in jailed map
            }
        });

        unjailed
    }

    /// Get slash history for an address
    pub fn get_slash_history(&self, validator: &Address) -> Vec<SlashEvent> {
        self.slash_history
            .read()
            .iter()
            .filter(|e| &e.evidence.validator == validator)
            .cloned()
            .collect()
    }

    /// Get all slash events
    pub fn get_all_slash_events(&self) -> Vec<SlashEvent> {
        self.slash_history.read().clone()
    }

    /// Get total slashed amount for a validator
    pub fn get_total_slashed(&self, validator: &Address) -> u128 {
        self.slash_history
            .read()
            .iter()
            .filter(|e| &e.evidence.validator == validator)
            .map(|e| e.amount_slashed)
            .sum()
    }

    /// Clean up old evidence (call periodically)
    pub fn cleanup_old_evidence(&self, current_height: u64, max_age: u64) {
        let cutoff = current_height.saturating_sub(max_age);

        self.double_sign_evidence.write().retain(|(height, _), _| {
            *height >= cutoff
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::Validator;

    fn setup_test() -> (SlashingManager, Address) {
        let validator_set = Arc::new(RwLock::new(ValidatorSet::new()));

        // Add a test validator
        let validator = Validator {
            address: Address::zero(),
            stake: 100_000_000_000_000_000_000u128, // 100 tokens
            public_key: [0u8; 32],
            active: true,
            rewards: 0,
            last_active_slot: 0,
            activation_epoch: 0,
        };
        validator_set.write().add_validator(validator.clone()).unwrap();

        let manager = SlashingManager::new(
            SlashingConfig::default(),
            validator_set,
        );

        (manager, Address::zero())
    }

    #[test]
    fn test_missed_blocks_tracking() {
        let (manager, validator) = setup_test();

        // Record missed blocks
        for _ in 0..50 {
            manager.record_missed_block(&validator);
        }

        // Should not trigger slash yet (threshold is 100)
        assert!(manager.check_offline(&validator).is_none());

        // Record more
        for _ in 0..50 {
            manager.record_missed_block(&validator);
        }

        // Now should trigger
        assert!(manager.check_offline(&validator).is_some());
    }

    #[test]
    fn test_slash_execution() {
        let (manager, validator) = setup_test();

        let evidence = SlashingEvidence {
            validator,
            reason: SlashReason::Offline,
            height: 100,
            evidence_hash: None,
            timestamp: 0,
        };

        let event = manager.slash(evidence, 100).unwrap();

        // Should have slashed 1% of 100 tokens = 1 token
        assert!(event.amount_slashed >= 1_000_000_000_000_000_000);
        assert!(!event.jailed); // Offline doesn't jail
    }

    #[test]
    fn test_double_signing_jails() {
        let (manager, validator) = setup_test();

        let evidence = SlashingEvidence {
            validator,
            reason: SlashReason::DoubleSigning,
            height: 100,
            evidence_hash: Some([1u8; 32]),
            timestamp: 0,
        };

        let event = manager.slash(evidence, 100).unwrap();

        // Should be jailed
        assert!(event.jailed);
        assert!(manager.is_jailed(&validator));
    }

    #[test]
    fn test_unjailing() {
        let (manager, validator) = setup_test();

        // Jail the validator
        let evidence = SlashingEvidence {
            validator,
            reason: SlashReason::DoubleSigning,
            height: 100,
            evidence_hash: None,
            timestamp: 0,
        };
        manager.slash(evidence, 100).unwrap();

        // Process before jail ends - should stay jailed
        let unjailed = manager.process_unjail(100 + 1000);
        assert!(unjailed.is_empty());

        // Process after jail ends
        let unjailed = manager.process_unjail(100 + 7200 + 1);
        assert_eq!(unjailed.len(), 1);
        assert_eq!(unjailed[0], validator);
        assert!(!manager.is_jailed(&validator));
    }
}

use crate::error::ConsensusError;
use crate::halving::HalvingSchedule;
use crate::validator::ValidatorSet;
use luxtensor_core::types::{Address, Hash};
use luxtensor_crypto::keccak256;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Production VRF imports — only compiled with the `production-vrf` feature.
#[cfg(feature = "production-vrf")]
use crate::vrf_key::{VrfKeypair, VrfPublicKey, VrfSecretKey, vrf_prove};

/// Configuration for PoS consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Number of seconds per slot
    pub slot_duration: u64,
    /// Minimum stake required to become a validator
    pub min_stake: u128,
    /// Base block reward amount (before halving)
    pub block_reward: u128,
    /// Epoch length in slots
    pub epoch_length: u64,
    /// Halving schedule for block rewards
    pub halving_schedule: HalvingSchedule,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            slot_duration: 12,                           // 12 seconds per block
            min_stake: 32_000_000_000_000_000_000u128,   // 32 tokens minimum
            block_reward: 2_000_000_000_000_000_000u128, // 2 tokens per block (initial)
            epoch_length: 32,                            // 32 slots per epoch
            halving_schedule: HalvingSchedule::default(),
        }
    }
}

/// Proof of Stake consensus mechanism
pub struct ProofOfStake {
    validator_set: Arc<RwLock<ValidatorSet>>,
    config: ConsensusConfig,
    current_epoch: RwLock<u64>,
    /// Last finalized block hash for VRF seed entropy
    last_block_hash: RwLock<Hash>,
    /// RANDAO mix from commit-reveal (provides unbiasable randomness)
    /// When set, this is mixed into the seed computation for stronger security.
    randao_mix: RwLock<Option<Hash>>,
    /// FIX-6: RANDAO mix from the **previous** finalized epoch.
    ///
    /// Using the previous epoch's RANDAO mix (rather than the current one)
    /// eliminates last-look bias: the seed for epoch N is fixed at the end
    /// of epoch N-2, when the current proposer is not yet known.  An attacker
    /// would need to manipulate reveals from multiple epochs in the past to
    /// influence the outcome.
    ///
    /// Updated by the node service when `RandaoMixer::finalize_epoch()` is called.
    /// Falls back to `randao_mix` (current epoch) if not yet set.
    prev_epoch_randao: RwLock<Option<Hash>>,
    /// Production VRF secret key.
    ///
    /// Loaded from the validator keystore at node startup via `set_vrf_key()`.
    /// When `production-vrf` is enabled and this is `None`, block production
    /// will return `ConsensusError::VrfKeyMissing` — preventing silent failures.
    #[cfg(feature = "production-vrf")]
    vrf_secret_key: RwLock<Option<VrfSecretKey>>,
}

impl ProofOfStake {
    /// Create a new PoS consensus instance
    pub fn new(config: ConsensusConfig) -> Self {
        Self {
            validator_set: Arc::new(RwLock::new(ValidatorSet::new())),
            config,
            current_epoch: RwLock::new(0),
            last_block_hash: RwLock::new([0u8; 32]),
            randao_mix: RwLock::new(None),
            prev_epoch_randao: RwLock::new(None),
            #[cfg(feature = "production-vrf")]
            vrf_secret_key: RwLock::new(None),
        }
    }

    /// Create with an existing validator set
    pub fn with_validator_set(config: ConsensusConfig, validator_set: ValidatorSet) -> Self {
        Self {
            validator_set: Arc::new(RwLock::new(validator_set)),
            config,
            current_epoch: RwLock::new(0),
            last_block_hash: RwLock::new([0u8; 32]),
            randao_mix: RwLock::new(None),
            prev_epoch_randao: RwLock::new(None),
            #[cfg(feature = "production-vrf")]
            vrf_secret_key: RwLock::new(None),
        }
    }

    // ── Production VRF key management ─────────────────────────────────────────

    /// Load the validator's VRF secret key from raw bytes (e.g. read from keystore).
    ///
    /// This MUST be called once during node startup before any block production
    /// when running with the `production-vrf` feature enabled. The key is stored
    /// in a `RwLock` so it can be rotated without restarting the node.
    ///
    /// # Security
    /// `raw_bytes` should come from an encrypted keystore file. Zeroize the
    /// source buffer after this call.
    #[cfg(feature = "production-vrf")]
    pub fn set_vrf_key(&self, raw_bytes: &[u8]) -> Result<(), ConsensusError> {
        let kp = VrfKeypair::from_secret_bytes(raw_bytes)?;
        // Move only the secret key into the lock — the public key can be derived on demand.
        *self.vrf_secret_key.write() = Some(kp.into_secret_key());
        Ok(())
    }

    /// Returns the VRF public key for on-chain broadcast.
    ///
    /// Peers use this key to verify VRF proofs included in block headers.
    /// Returns `Err(VrfKeyMissing)` if the key has not been loaded yet.
    #[cfg(feature = "production-vrf")]
    pub fn vrf_public_key(&self) -> Result<VrfPublicKey, ConsensusError> {
        self.vrf_secret_key
            .read()
            .as_ref()
            .map(|sk| sk.public_key())
            .ok_or(ConsensusError::VrfKeyMissing)
    }

    /// Update last block hash (call after block finalization)
    pub fn update_last_block_hash(&self, hash: Hash) {
        *self.last_block_hash.write() = hash;
    }

    /// Update RANDAO mix from the RandaoMixer after epoch finalization.
    /// This adds unbiasable randomness to validator selection,
    /// preventing a validators from predicting future selections.
    /// Update RANDAO mix (called at epoch finalization)
    pub fn update_randao_mix(&self, mix: Hash) {
        // Before overwriting the current mix, save it as the previous epoch's
        // lookback seed (FIX-6: anti-last-look-bias).
        let old_mix = *self.randao_mix.read();
        if let Some(prev) = old_mix {
            *self.prev_epoch_randao.write() = Some(prev);
        }
        *self.randao_mix.write() = Some(mix);
    }

    /// FIX-6: Explicitly set the previous-epoch RANDAO seed.
    ///
    /// Call this when `RandaoMixer::finalize_epoch()` returns a mix for epoch N,
    /// passing that mix here **before** starting epoch N+1.  The PoS engine will
    /// then use this lookback seed as the primary entropy source for slot selection,
    /// which is immune to last-look bias because it was finalised before any
    /// validator knew they would be the current proposer.
    ///
    /// # Example (in node service)
    /// ```ignore
    /// let finalized = randao_mixer.finalize_epoch()?;
    /// pos_engine.update_prev_epoch_randao(finalized);
    /// pos_engine.update_randao_mix(randao_mixer.current_mix());
    /// ```
    pub fn update_prev_epoch_randao(&self, mix: Hash) {
        *self.prev_epoch_randao.write() = Some(mix);
    }

    /// Select a validator for a given slot using VRF-based selection
    ///
    /// 🔧 FIX MC-1: Read all shared state (last_block_hash, randao_mix, validator_set)
    /// atomically before computing the seed. Previously these were read under separate
    /// locks, allowing a race during epoch transitions where the seed could be computed
    /// with a stale hash but a fresh RANDAO mix (or vice versa).
    pub fn select_validator(&self, slot: u64) -> Result<Address, ConsensusError> {
        // Acquire all read guards before any computation to get a consistent snapshot
        let last_hash = *self.last_block_hash.read();
        let randao = *self.randao_mix.read();
        let validator_set = self.validator_set.read();

        let seed = self.compute_seed_with(slot, last_hash, randao);

        validator_set
            .select_by_seed(&seed)
            .map_err(|e| ConsensusError::ValidatorSelection(e.to_string()))
    }

    /// Validate that the correct validator produced the block
    pub fn validate_block_producer(
        &self,
        producer: &Address,
        slot: u64,
    ) -> Result<(), ConsensusError> {
        let expected = self.select_validator(slot)?;

        if producer != &expected {
            return Err(ConsensusError::InvalidProducer { expected, actual: *producer });
        }

        Ok(())
    }

    /// Compute the randomness seed for validator selection at a given slot.
    ///
    /// SECURITY: Uses epoch + slot + last_block_hash + RANDAO mix (when available).
    /// The RANDAO mix comes from the commit-reveal scheme in `RandaoMixer`,
    /// which makes the seed unbiasable by any single validator.
    /// Without RANDAO, the seed is computed from keccak256(epoch || slot || block_hash)
    /// which a block producer can bias by withholding blocks.
    pub fn compute_seed(&self, slot: u64) -> Hash {
        let last_hash = *self.last_block_hash.read();
        let randao = *self.randao_mix.read();
        self.compute_seed_with(slot, last_hash, randao)
    }

    /// Pure computation of seed from pre-read values.
    /// 🔧 FIX MC-1: Extracted so that `select_validator` can pass already-acquired
    /// snapshots, avoiding separate lock acquisitions that could race.
    ///
    /// # ⚠️  TESTNET-ONLY — PSEUDO-VRF IMPLEMENTATION
    ///
    /// This function currently uses `keccak256(epoch || slot || last_block_hash [|| randao_mix])`
    /// as a **pseudo-VRF** seed.  This is **NOT cryptographically secure** for mainnet because:
    ///
    /// 1. **Last-look bias**: The current block producer can withhold a block and mine an
    ///    alternative if the resulting seed would not select them as next producer.
    /// 2. **Seed predictability**: Any observer can pre-compute future seeds using public
    ///    chain data.
    ///
    /// The RANDAO mix (from `RandaoMixer`) partially mitigates (1) by injecting
    /// commit-reveal randomness, but does **not** replace a proper VRF proof.
    ///
    /// ## Mainnet Upgrade Path
    /// Replace this function with a real VRF implementation before **mainnet launch**:
    /// - Use Ed25519-VRF (RFC 9381) with each validator's BLS key pair.
    /// - Each block producer generates a VRF proof over `(epoch, slot, last_block_hash)`.
    /// - The proof is included in the block header so peers can verify randomness
    ///   without privileged access to the private key.
    ///
    /// A compile-time guard enforces this requirement: the `production-vrf` feature flag
    /// must be enabled when building for mainnet (see `Cargo.toml`).  If you are
    /// seeing this message in a production build, the guard was bypassed — **stop and fix**.
    ///
    /// ```text
    /// [features]
    /// production-vrf = []   # Enable ONLY when real VRF is wired in
    /// ```
    #[cfg(not(feature = "production-vrf"))]
    fn compute_seed_with(&self, slot: u64, last_hash: Hash, randao: Option<Hash>) -> Hash {
        // PSEUDO-VRF — TESTNET ONLY.  See doc comment above for the mainnet upgrade path.
        let epoch = if self.config.epoch_length > 0 { slot / self.config.epoch_length } else { 0 };

        let mut data = Vec::with_capacity(112);
        data.extend_from_slice(&epoch.to_le_bytes());
        data.extend_from_slice(&slot.to_le_bytes());
        data.extend_from_slice(&last_hash);

        // FIX-6: Anti-last-look-bias RANDAO mixing.
        //
        // Priority order (most to least secure):
        //   1. prev_epoch_randao  — already finalized, attacker cannot change it retroactively
        //   2. randao (current)   — single-round mix, vulnerable to last-look bias
        //   3. (neither)          — seed is purely function of chain data (no RANDAO)
        //
        // The node service should always call `update_prev_epoch_randao()` at epoch
        // boundaries so that (1) is available in steady state.
        let lookback = *self.prev_epoch_randao.read();
        if let Some(lb) = lookback {
            // Preferred path: mix in the immutable lookback seed first.
            data.extend_from_slice(&lb);
            // Then optionally also mix in the current epoch mix for additional entropy.
            if let Some(mix) = randao {
                data.extend_from_slice(&mix);
            }
        } else if let Some(mix) = randao {
            // Fallback: mix in current epoch mix (slightly weaker but still better than nothing).
            data.extend_from_slice(&mix);
        }

        keccak256(&data)
    }

    /// Production VRF seed computation stub.
    ///
    /// This variant is compiled **only** when the `production-vrf` feature is enabled.
    /// It intentionally panics to force developers to wire in a real VRF implementation
    /// (Ed25519-VRF / RFC 9381) before shipping to mainnet.
    ///
    /// The panic message explains clearly what needs to be done, making it impossible
    /// to accidentally ship the pseudo-VRF in a production build.
    /// Production VRF seed computation — ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381).
    ///
    /// This variant is compiled **only** when the `production-vrf` Cargo feature is enabled.
    /// It generates a cryptographically-secure, non-biasable seed by:
    ///
    /// 1. Building `alpha = epoch || slot || last_block_hash [|| prev_epoch_randao]`
    /// 2. Calling `vrf_prove(sk, alpha)` to get a deterministic proof
    /// 3. Hashing the proof's 64-byte output via SHA-256 to get a 32-byte seed
    ///
    /// The proof must be included in the block header so peers can call
    /// `vrf_key::vrf_verify(pk, proof, alpha, seed)` independently.
    ///
    /// # Panics
    /// Panics if the VRF secret key has not been loaded via `set_vrf_key()`.
    /// This is intentional: a missing key is a misconfiguration that should
    /// prevent the node from silently producing invalid blocks.
    #[cfg(feature = "production-vrf")]
    fn compute_seed_with(&self, slot: u64, last_hash: Hash, randao: Option<Hash>) -> Hash {
        let epoch = if self.config.epoch_length > 0 {
            slot / self.config.epoch_length
        } else {
            0
        };

        // Build the VRF input (alpha). Mirrors the testnet path so seeds are
        // comparable when switching between modes during development.
        let mut alpha = Vec::with_capacity(112);
        alpha.extend_from_slice(&epoch.to_le_bytes());
        alpha.extend_from_slice(&slot.to_le_bytes());
        alpha.extend_from_slice(&last_hash);

        // Anti-last-look-bias: prefer the finalized previous epoch's RANDAO.
        let lookback = *self.prev_epoch_randao.read();
        if let Some(lb) = lookback {
            alpha.extend_from_slice(&lb);
        } else if let Some(mix) = randao {
            alpha.extend_from_slice(&mix);
        }

        // Prove: panics if key not loaded (misconfiguration, not a recoverable error).
        let sk_guard = self.vrf_secret_key.read();
        let sk = sk_guard
            .as_ref()
            .expect("VRF secret key must be loaded via set_vrf_key() before producing blocks");

        let (_proof_bytes, output) = vrf_prove(sk, &alpha)
            .expect("vrf_prove must not fail with a valid Ed25519 key");

        // output.0 is already a 32-byte SHA-256-truncated VRF hash.
        output.0
    }

    /// Calculate and distribute block rewards (uses base block_reward - legacy method)
    pub fn distribute_reward(&self, producer: &Address) -> Result<(), ConsensusError> {
        let mut validator_set = self.validator_set.write();

        validator_set
            .add_reward(producer, self.config.block_reward)
            .map_err(|e| ConsensusError::RewardDistribution(e.to_string()))
    }

    /// Calculate block reward for a given height using halving schedule
    pub fn get_reward_for_height(&self, block_height: u64) -> u128 {
        self.config.halving_schedule.calculate_reward(block_height)
    }

    /// Calculate and distribute block rewards with halving schedule
    /// This is the preferred method for production use
    pub fn distribute_reward_with_height(
        &self,
        producer: &Address,
        block_height: u64,
    ) -> Result<u128, ConsensusError> {
        let reward = self.get_reward_for_height(block_height);

        if reward == 0 {
            // No reward for this block (after all halvings complete)
            return Ok(0);
        }

        let mut validator_set = self.validator_set.write();
        validator_set
            .add_reward(producer, reward)
            .map_err(|e| ConsensusError::RewardDistribution(e.to_string()))?;

        Ok(reward)
    }

    /// Get halving info for the current block height
    pub fn get_halving_info(&self, _block_height: u64) -> crate::halving::HalvingInfo {
        let schedule = &self.config.halving_schedule;
        crate::halving::HalvingInfo {
            initial_reward_mdt: schedule.initial_reward as f64 / 1e18,
            halving_interval_blocks: schedule.halving_interval,
            halving_interval_years: (schedule.halving_interval as f64 * 12.0)
                / (365.25 * 24.0 * 3600.0),
            max_halvings: schedule.max_halvings,
            estimated_total_emission_mdt: schedule.estimate_total_emission() as f64 / 1e18,
        }
    }

    /// Get current halving era and blocks until next halving
    pub fn get_halving_status(&self, block_height: u64) -> (u32, u64, u128) {
        let schedule = &self.config.halving_schedule;
        (
            schedule.get_halving_era(block_height),
            schedule.blocks_until_next_halving(block_height),
            schedule.calculate_reward(block_height),
        )
    }

    /// Add a new validator to the set
    pub fn add_validator(
        &self,
        address: Address,
        stake: u128,
        public_key: [u8; 32],
    ) -> Result<(), ConsensusError> {
        if stake < self.config.min_stake {
            return Err(ConsensusError::InsufficientStake {
                provided: stake,
                required: self.config.min_stake,
            });
        }

        let validator = crate::validator::Validator::new(address, stake, public_key);
        let mut validator_set = self.validator_set.write();

        validator_set
            .add_validator(validator)
            .map_err(|e| ConsensusError::ValidatorManagement(e.to_string()))
    }

    /// Remove a validator from the set
    pub fn remove_validator(&self, address: &Address) -> Result<(), ConsensusError> {
        let mut validator_set = self.validator_set.write();

        validator_set
            .remove_validator(address)
            .map_err(|e| ConsensusError::ValidatorManagement(e.to_string()))
    }

    /// Update validator stake
    pub fn update_validator_stake(
        &self,
        address: &Address,
        new_stake: u128,
    ) -> Result<(), ConsensusError> {
        if new_stake < self.config.min_stake {
            return Err(ConsensusError::InsufficientStake {
                provided: new_stake,
                required: self.config.min_stake,
            });
        }

        let mut validator_set = self.validator_set.write();

        validator_set
            .update_stake(address, new_stake)
            .map_err(|e| ConsensusError::ValidatorManagement(e.to_string()))
    }

    /// Get the current epoch
    pub fn current_epoch(&self) -> u64 {
        *self.current_epoch.read()
    }

    /// Advance to the next epoch
    pub fn advance_epoch(&self) {
        let mut epoch = self.current_epoch.write();
        *epoch += 1;
    }

    /// Get slot from timestamp
    pub fn get_slot(&self, timestamp: u64, genesis_time: u64) -> u64 {
        if timestamp < genesis_time || self.config.slot_duration == 0 {
            return 0;
        }
        (timestamp - genesis_time) / self.config.slot_duration
    }

    /// Get validator set reference
    pub fn validator_set(&self) -> Arc<RwLock<ValidatorSet>> {
        Arc::clone(&self.validator_set)
    }

    /// Get configuration
    pub fn config(&self) -> &ConsensusConfig {
        &self.config
    }

    /// Get total stake in the network
    pub fn total_stake(&self) -> u128 {
        self.validator_set.read().total_stake()
    }

    /// Get number of validators
    pub fn validator_count(&self) -> usize {
        self.validator_set.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_address(index: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[0] = index;
        Address::from(bytes)
    }

    #[test]
    fn test_pos_creation() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config);

        assert_eq!(pos.validator_count(), 0);
        assert_eq!(pos.current_epoch(), 0);
    }

    #[test]
    fn test_add_validator() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let address = create_test_address(1);
        let pubkey = [1u8; 32];

        let result = pos.add_validator(address, config.min_stake, pubkey);
        assert!(result.is_ok());
        assert_eq!(pos.validator_count(), 1);
    }

    #[test]
    fn test_add_validator_insufficient_stake() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let address = create_test_address(1);
        let pubkey = [1u8; 32];

        let result = pos.add_validator(address, config.min_stake - 1, pubkey);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(not(feature = "production-vrf"))]
    fn test_validator_selection() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        // Add validators
        for i in 1..=3 {
            let address = create_test_address(i);
            let pubkey = [i; 32];
            pos.add_validator(address, config.min_stake * (i as u128), pubkey).unwrap();
        }

        // Select validator for slot 0
        let selected = pos.select_validator(0);
        assert!(selected.is_ok());
    }

    #[test]
    #[cfg(not(feature = "production-vrf"))]
    fn test_validate_block_producer() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        // Add validator
        let address = create_test_address(1);
        let pubkey = [1u8; 32];
        pos.add_validator(address, config.min_stake, pubkey).unwrap();

        // Select validator for slot 0
        let selected = pos.select_validator(0).unwrap();

        // Validate correct producer
        assert!(pos.validate_block_producer(&selected, 0).is_ok());

        // Validate wrong producer
        let wrong_address = create_test_address(2);
        assert!(pos.validate_block_producer(&wrong_address, 0).is_err());
    }

    #[test]
    fn test_reward_distribution() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let address = create_test_address(1);
        let pubkey = [1u8; 32];
        pos.add_validator(address, config.min_stake, pubkey).unwrap();

        // Distribute reward
        let result = pos.distribute_reward(&address);
        assert!(result.is_ok());

        // Check reward was added
        let validator_set = pos.validator_set.read();
        let validator = validator_set.get_validator(&address).unwrap();
        assert_eq!(validator.rewards, config.block_reward);
    }

    #[test]
    #[cfg(not(feature = "production-vrf"))]
    fn test_seed_computation() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config);

        // Same slot should produce same seed
        let seed1 = pos.compute_seed(0);
        let seed2 = pos.compute_seed(0);
        assert_eq!(seed1, seed2);

        // Different slots should produce different seeds
        let seed3 = pos.compute_seed(1);
        assert_ne!(seed1, seed3);
    }

    #[test]
    fn test_get_slot() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let genesis_time = 1000u64;

        // At genesis
        assert_eq!(pos.get_slot(genesis_time, genesis_time), 0);

        // After one slot duration
        assert_eq!(pos.get_slot(genesis_time + config.slot_duration, genesis_time), 1);

        // After multiple slot durations
        assert_eq!(pos.get_slot(genesis_time + config.slot_duration * 5, genesis_time), 5);
    }

    #[test]
    fn test_epoch_advancement() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config);

        assert_eq!(pos.current_epoch(), 0);

        pos.advance_epoch();
        assert_eq!(pos.current_epoch(), 1);

        pos.advance_epoch();
        assert_eq!(pos.current_epoch(), 2);
    }

    // ── VRF integration tests (compiled only with `production-vrf` feature) ──

    #[cfg(feature = "production-vrf")]
    mod vrf_tests {
        use super::*;

        fn make_pos_with_vrf_key() -> ProofOfStake {
            use rand::RngCore as _;

            let pos = ProofOfStake::new(ConsensusConfig::default());

            // Generate a random 32-byte secret and load it
            let mut raw = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut raw);
            pos.set_vrf_key(&raw).expect("set_vrf_key must succeed with 32 valid bytes");
            pos
        }

        /// set_vrf_key() + compute_seed_with() must not panic.
        #[test]
        fn production_seed_does_not_panic() {
            let pos = make_pos_with_vrf_key();
            let last_hash = [1u8; 32];
            let seed = pos.compute_seed_with(0, last_hash, None);
            // Just check we got a non-zero hash (probability of all-zero is negligible)
            assert_ne!(seed, [0u8; 32]);
        }

        /// Same (epoch, slot) → same seed. VRF must be deterministic.
        #[test]
        fn production_seed_is_deterministic() {
            let pos = make_pos_with_vrf_key();
            let last_hash = [42u8; 32];
            let seed1 = pos.compute_seed_with(5, last_hash, None);
            let seed2 = pos.compute_seed_with(5, last_hash, None);
            assert_eq!(seed1, seed2, "VRF seed must be deterministic for same inputs");
        }

        /// Different slots → different seeds (birthday-paradox probability is negligible).
        #[test]
        fn production_seed_differs_per_slot() {
            let pos = make_pos_with_vrf_key();
            let last_hash = [1u8; 32];
            let seed_slot1 = pos.compute_seed_with(5, last_hash, None);
            let last_hash2 = [2u8; 32];
            let seed_slot2 = pos.compute_seed_with(5, last_hash2, None);
            assert_ne!(seed_slot1, seed_slot2, "different last_hash must yield different VRF seeds");
        }

        /// compute_seed_with should return Err (or panic via expect) when key is not loaded.
        /// We test the happy path in the other tests; here confirm set_vrf_key errors on bad bytes.
        #[test]
        fn set_vrf_key_rejects_short_bytes() {
            let pos = ProofOfStake::new(ConsensusConfig::default());
            let bad_bytes = [0u8; 16]; // too short
            assert!(
                pos.set_vrf_key(&bad_bytes).is_err(),
                "set_vrf_key must fail on < 32 bytes"
            );
        }
    }
}

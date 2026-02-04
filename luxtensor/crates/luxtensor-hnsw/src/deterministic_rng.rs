//! Deterministic Random Number Generator for Consensus-Safe HNSW Construction
//!
//! This module provides `DeterministicRng` - a PRNG seeded by blockchain consensus
//! artifacts to ensure identical graph construction across all validator nodes.
//!
//! ## Why Deterministic RNG?
//!
//! Standard HNSW implementations use system entropy (`rand::thread_rng()`) for:
//! - Level generation (which layer a new node belongs to)
//! - Random tie-breaking during search
//!
//! If Validator A assigns a node to Level 3 while Validator B assigns it to Level 2,
//! the resulting graph topologies differ, causing **consensus forks**.
//!
//! ## Solution: Cryptographic Seeding
//!
//! We derive the seed from immutable consensus artifacts:
//! ```text
//! seed = Keccak256(TxHash ⊕ BlockHash)
//! ```
//! Since all validators agree on `TxHash` and `BlockHash`, they all generate
//! exactly the same sequence of "random" numbers.

use luxtensor_crypto::{keccak256, Hash};
use serde::{Deserialize, Serialize};

/// A deterministic random number generator seeded by blockchain consensus artifacts.
///
/// This RNG produces identical sequences across all validators when given the same
/// transaction hash and block hash, ensuring deterministic HNSW graph construction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeterministicRng {
    /// Current state (256 bits)
    state: [u8; 32],
    /// Counter for additional entropy
    counter: u64,
}

impl DeterministicRng {
    /// Create a new deterministic RNG from consensus artifacts.
    ///
    /// # Arguments
    /// * `tx_hash` - The hash of the transaction containing the vector insert
    /// * `block_hash` - The hash of the previous block
    ///
    /// # Security
    /// The XOR of `tx_hash` and `block_hash` prevents attackers from "grinding"
    /// transactions to achieve a specific seed, since they cannot predict
    /// the future block hash.
    pub fn new(tx_hash: Hash, block_hash: Hash) -> Self {
        // XOR the two hashes together
        let mut xored = [0u8; 32];
        for i in 0..32 {
            xored[i] = tx_hash[i] ^ block_hash[i];
        }

        // Hash the XOR result to produce the initial state
        let state = keccak256(&xored);

        Self { state, counter: 0 }
    }

    /// Create a new RNG from raw bytes (for testing or deserialization).
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let state = keccak256(&seed);
        Self { state, counter: 0 }
    }

    /// Generate the next level for an HNSW node.
    ///
    /// Levels follow a geometric distribution where higher levels are exponentially
    /// less likely. This is the core of HNSW's hierarchical structure.
    ///
    /// # Arguments
    /// * `max_level` - The maximum allowed level (typically 16-32)
    /// * `ml` - Level multiplier (typically 1/ln(M) where M is connections per node)
    ///
    /// # Returns
    /// A level in [0, max_level] where 0 is most common.
    pub fn next_level(&mut self, max_level: u8, ml: f64) -> u8 {
        let random_float = self.next_f64();

        // Level calculation: floor(-ln(rand) * ml)
        // This produces a geometric distribution
        let level = (-random_float.ln() * ml).floor() as u8;

        level.min(max_level)
    }

    /// Generate the next random u64 value.
    pub fn next_u64(&mut self) -> u64 {
        self.advance_state();

        // Take the first 8 bytes of state as u64
        u64::from_le_bytes([
            self.state[0],
            self.state[1],
            self.state[2],
            self.state[3],
            self.state[4],
            self.state[5],
            self.state[6],
            self.state[7],
        ])
    }

    /// Generate a random f64 in [0, 1).
    ///
    /// Note: We only use this for level calculation which is then converted
    /// to a discrete level. The float is NOT used for distance calculations.
    pub fn next_f64(&mut self) -> f64 {
        let bits = self.next_u64();
        // Convert to f64 in [0, 1) using the standard technique
        (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Generate a random usize in [0, max).
    pub fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        (self.next_u64() as usize) % max
    }

    /// Advance the internal state using Keccak256.
    ///
    /// This is a cryptographically secure state transition that ensures
    /// the sequence is unpredictable but deterministic.
    #[inline]
    fn advance_state(&mut self) {
        // Concatenate current state with counter
        let mut input = [0u8; 40];
        input[..32].copy_from_slice(&self.state);
        input[32..40].copy_from_slice(&self.counter.to_le_bytes());

        // Hash to produce new state
        self.state = keccak256(&input);
        self.counter = self.counter.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash() -> Hash {
        [
            0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78,
            0x9a, 0xbc, 0xde, 0xf0,
        ]
    }

    fn different_hash() -> Hash {
        [
            0xFF, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc,
            0xde, 0xf0, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x12, 0x34, 0x56, 0x78,
            0x9a, 0xbc, 0xde, 0xf0,
        ]
    }

    #[test]
    fn test_determinism() {
        // Same inputs must produce same outputs
        let mut rng1 = DeterministicRng::new(test_hash(), test_hash());
        let mut rng2 = DeterministicRng::new(test_hash(), test_hash());

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_different_inputs_different_outputs() {
        let hash1 = test_hash();
        let hash2 = different_hash();

        let mut rng1 = DeterministicRng::new(hash1, hash1);
        let mut rng2 = DeterministicRng::new(hash2, hash1);

        // Should produce different sequences
        let val1 = rng1.next_u64();
        let val2 = rng2.next_u64();
        assert_ne!(val1, val2);
    }

    #[test]
    fn test_level_distribution() {
        let mut rng = DeterministicRng::from_seed([0u8; 32]);
        let ml = 1.0 / (16f64).ln(); // Standard HNSW parameter

        let mut level_counts = [0u32; 17];
        for _ in 0..10000 {
            let level = rng.next_level(16, ml);
            level_counts[level as usize] += 1;
        }

        // Level 0 should be most common (geometric distribution)
        assert!(level_counts[0] > level_counts[1]);
        assert!(level_counts[1] > level_counts[2]);
    }

    #[test]
    fn test_f64_range() {
        let mut rng = DeterministicRng::from_seed([0u8; 32]);

        for _ in 0..1000 {
            let val = rng.next_f64();
            assert!(val >= 0.0);
            assert!(val < 1.0);
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let rng = DeterministicRng::from_seed([42u8; 32]);
        let serialized = bincode::serialize(&rng).unwrap();
        let deserialized: DeterministicRng = bincode::deserialize(&serialized).unwrap();

        assert_eq!(rng.state, deserialized.state);
        assert_eq!(rng.counter, deserialized.counter);
    }
}

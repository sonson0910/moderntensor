// LuxTensor Light Client Protocol
//
// Provides header-only chain verification for resource-constrained nodes.
// A light client can:
//   1. Track and verify a chain of block headers.
//   2. Verify Merkle proofs against a trusted state root.
//   3. Perform header sync in ranges from full-node peers.

use std::collections::HashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use luxtensor_core::block::BlockHeader;
use luxtensor_core::types::Hash;

// ─── Error ───────────────────────────────────────────────────────────

/// Light client errors.
#[derive(Debug, thiserror::Error)]
pub enum LightClientError {
    #[error("header at height {0} not found")]
    HeaderNotFound(u64),

    #[error("header chain gap: expected parent at height {expected}, got {got}")]
    ChainGap { expected: u64, got: u64 },

    #[error("parent hash mismatch at height {0}")]
    ParentHashMismatch(u64),

    #[error("header validation failed at height {0}: {1}")]
    InvalidHeader(u64, String),

    #[error("merkle proof verification failed for key {0:?}")]
    InvalidMerkleProof(Vec<u8>),

    #[error("state root mismatch at height {0}")]
    StateRootMismatch(u64),

    #[error("header range request too large ({0} > max {1})")]
    RangeTooLarge(u64, u64),

    #[error("no trusted header available — bootstrap required")]
    NotBootstrapped,
}

pub type Result<T> = std::result::Result<T, LightClientError>;

// ─── Types ───────────────────────────────────────────────────────────

/// A Merkle inclusion proof for state queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The key being proved (e.g. account address).
    pub key: Vec<u8>,
    /// The value at the key (empty if absence proof).
    pub value: Vec<u8>,
    /// Sibling hashes from leaf to root, bottom-up.
    pub siblings: Vec<Hash>,
    /// Bitmap indicating left(0) / right(1) at each level.
    pub path_bits: Vec<bool>,
}

/// Compact header summary stored by the light client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedHeader {
    pub header: BlockHeader,
    pub header_hash: Hash,
}

/// Configuration for the light client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightClientConfig {
    /// Maximum number of headers to cache.
    pub max_cached_headers: usize,
    /// Maximum header range that can be requested in one sync call.
    pub max_sync_range: u64,
    /// Minimum number of confirmations before trusting a header.
    pub min_confirmations: u64,
}

impl Default for LightClientConfig {
    fn default() -> Self {
        Self {
            max_cached_headers: 10_000,
            max_sync_range: 1_000,
            min_confirmations: 6,
        }
    }
}

/// Sync status of the light client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// No trusted header yet.
    NotBootstrapped,
    /// Syncing — latest trusted height vs known chain tip.
    Syncing { trusted_height: u64, chain_tip: u64 },
    /// Fully synchronised.
    Synced { height: u64 },
}

// ─── Light Client State ──────────────────────────────────────────────

/// The in-memory light client state machine.
pub struct LightClientState {
    config: LightClientConfig,
    /// Map of height → trusted header.
    headers: RwLock<HashMap<u64, TrustedHeader>>,
    /// The highest verified height.
    latest_height: RwLock<u64>,
    /// Known chain tip reported by peers.
    chain_tip: RwLock<u64>,
}

impl LightClientState {
    pub fn new(config: LightClientConfig) -> Self {
        Self {
            config,
            headers: RwLock::new(HashMap::new()),
            latest_height: RwLock::new(0),
            chain_tip: RwLock::new(0),
        }
    }

    /// Bootstrap with a trusted genesis / checkpoint header.
    pub fn bootstrap(&self, header: BlockHeader) -> Result<()> {
        let hash = header.hash();
        let height = header.height;
        let th = TrustedHeader {
            header,
            header_hash: hash,
        };
        let mut headers = self.headers.write();
        headers.insert(height, th);
        *self.latest_height.write() = height;
        Ok(())
    }

    /// Verify and append a new header that extends the current trusted chain.
    ///
    /// Checks:
    /// 1. Height is exactly `latest + 1`.
    /// 2. `previous_hash` matches the hash of the latest trusted header.
    /// 3. Basic header validation (via `BlockHeader::validate()`).
    pub fn verify_and_append(&self, header: BlockHeader) -> Result<()> {
        let latest = *self.latest_height.read();
        if latest == 0 && self.headers.read().is_empty() {
            return Err(LightClientError::NotBootstrapped);
        }

        let expected_height = latest + 1;
        if header.height != expected_height {
            return Err(LightClientError::ChainGap {
                expected: expected_height,
                got: header.height,
            });
        }

        // Verify parent link
        {
            let headers = self.headers.read();
            let parent = headers
                .get(&latest)
                .ok_or(LightClientError::HeaderNotFound(latest))?;
            if header.previous_hash != parent.header_hash {
                return Err(LightClientError::ParentHashMismatch(header.height));
            }
        }

        // Validate header fields
        if let Err(e) = header.validate() {
            return Err(LightClientError::InvalidHeader(header.height, e.to_string()));
        }

        let hash = header.hash();
        let height = header.height;
        let th = TrustedHeader {
            header,
            header_hash: hash,
        };

        let mut headers = self.headers.write();
        headers.insert(height, th);
        *self.latest_height.write() = height;

        // Evict old headers if over capacity
        self.evict_old_headers(&mut headers, height);
        Ok(())
    }

    /// Process a batch of sequential headers (e.g. from a sync response).
    pub fn sync_headers(&self, batch: Vec<BlockHeader>) -> Result<u64> {
        if batch.len() as u64 > self.config.max_sync_range {
            return Err(LightClientError::RangeTooLarge(
                batch.len() as u64,
                self.config.max_sync_range,
            ));
        }

        let mut count = 0u64;
        for header in batch {
            self.verify_and_append(header)?;
            count += 1;
        }
        Ok(count)
    }

    /// Update the known chain tip height (reported by peers).
    pub fn set_chain_tip(&self, tip: u64) {
        let mut ct = self.chain_tip.write();
        if tip > *ct {
            *ct = tip;
        }
    }

    /// Return the current sync status.
    pub fn sync_status(&self) -> SyncStatus {
        let latest = *self.latest_height.read();
        if latest == 0 && self.headers.read().is_empty() {
            return SyncStatus::NotBootstrapped;
        }
        let tip = *self.chain_tip.read();
        if tip == 0 || latest + self.config.min_confirmations >= tip {
            SyncStatus::Synced { height: latest }
        } else {
            SyncStatus::Syncing {
                trusted_height: latest,
                chain_tip: tip,
            }
        }
    }

    /// Get a trusted header by height.
    pub fn get_header(&self, height: u64) -> Option<TrustedHeader> {
        self.headers.read().get(&height).cloned()
    }

    /// Get the latest trusted height.
    pub fn latest_height(&self) -> u64 {
        *self.latest_height.read()
    }

    /// Verify a Merkle inclusion proof against a trusted state root.
    ///
    /// The proof is verified against the state root of the header at
    /// `at_height`.
    pub fn verify_merkle_proof(
        &self,
        proof: &MerkleProof,
        at_height: u64,
    ) -> Result<bool> {
        let headers = self.headers.read();
        let th = headers
            .get(&at_height)
            .ok_or(LightClientError::HeaderNotFound(at_height))?;

        let computed_root = Self::compute_merkle_root(proof);
        if computed_root != th.header.state_root {
            return Err(LightClientError::StateRootMismatch(at_height));
        }
        Ok(true)
    }

    // ── Internal helpers ─────────────────────────────────────────────

    /// Compute the Merkle root from a proof.
    fn compute_merkle_root(proof: &MerkleProof) -> Hash {
        use sha3::{Digest, Keccak256};

        // Leaf = H(key || value)
        let mut hasher = Keccak256::new();
        hasher.update(&proof.key);
        hasher.update(&proof.value);
        let mut current: [u8; 32] = hasher.finalize().into();

        for (sibling, is_right) in proof.siblings.iter().zip(proof.path_bits.iter()) {
            let mut hasher = Keccak256::new();
            if *is_right {
                hasher.update(sibling);
                hasher.update(current);
            } else {
                hasher.update(current);
                hasher.update(sibling);
            }
            current = hasher.finalize().into();
        }
        current
    }

    fn evict_old_headers(&self, headers: &mut HashMap<u64, TrustedHeader>, current: u64) {
        if headers.len() <= self.config.max_cached_headers {
            return;
        }
        let cutoff = current.saturating_sub(self.config.max_cached_headers as u64);
        headers.retain(|h, _| *h >= cutoff);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(height: u64, previous_hash: Hash) -> BlockHeader {
        BlockHeader {
            version: 1,
            height,
            timestamp: 1_700_000_000 + height * 12,
            previous_hash,
            state_root: [0u8; 32],
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: 0,
            gas_limit: 30_000_000,
            extra_data: Vec::new(),
        }
    }

    fn lc() -> LightClientState {
        LightClientState::new(LightClientConfig {
            max_cached_headers: 100,
            max_sync_range: 50,
            min_confirmations: 2,
        })
    }

    #[test]
    fn test_bootstrap_and_append() {
        let light = lc();

        let genesis = make_header(0, [0u8; 32]);
        light.bootstrap(genesis.clone()).unwrap();
        assert_eq!(light.latest_height(), 0);

        let h1 = make_header(1, genesis.hash());
        light.verify_and_append(h1.clone()).unwrap();
        assert_eq!(light.latest_height(), 1);

        let h2 = make_header(2, h1.hash());
        light.verify_and_append(h2).unwrap();
        assert_eq!(light.latest_height(), 2);
    }

    #[test]
    fn test_rejects_gap() {
        let light = lc();
        let genesis = make_header(0, [0u8; 32]);
        light.bootstrap(genesis).unwrap();

        // Skipping height 1
        let bad = make_header(2, [0u8; 32]);
        assert!(light.verify_and_append(bad).is_err());
    }

    #[test]
    fn test_rejects_bad_parent_hash() {
        let light = lc();
        let genesis = make_header(0, [0u8; 32]);
        light.bootstrap(genesis).unwrap();

        let bad = make_header(1, [0xABu8; 32]); // wrong parent
        assert!(light.verify_and_append(bad).is_err());
    }

    #[test]
    fn test_sync_headers_batch() {
        let light = lc();
        let genesis = make_header(0, [0u8; 32]);
        light.bootstrap(genesis.clone()).unwrap();

        let mut batch = Vec::new();
        let mut prev_hash = genesis.hash();
        for h in 1..=10 {
            let hdr = make_header(h, prev_hash);
            prev_hash = hdr.hash();
            batch.push(hdr);
        }

        let synced = light.sync_headers(batch).unwrap();
        assert_eq!(synced, 10);
        assert_eq!(light.latest_height(), 10);
    }

    #[test]
    fn test_sync_status() {
        let light = lc();
        assert_eq!(light.sync_status(), SyncStatus::NotBootstrapped);

        let genesis = make_header(0, [0u8; 32]);
        light.bootstrap(genesis).unwrap();
        assert_eq!(light.sync_status(), SyncStatus::Synced { height: 0 });

        light.set_chain_tip(100);
        match light.sync_status() {
            SyncStatus::Syncing { trusted_height, chain_tip } => {
                assert_eq!(trusted_height, 0);
                assert_eq!(chain_tip, 100);
            }
            other => panic!("expected Syncing, got {:?}", other),
        }
    }

    #[test]
    fn test_merkle_proof_verification() {
        use sha3::{Digest, Keccak256};

        let light = lc();

        // Build a simple merkle tree: leaf = H(key||value), root = H(leaf||sibling)
        let key = b"account_1".to_vec();
        let value = b"balance_100".to_vec();

        let mut leaf_hasher = Keccak256::new();
        leaf_hasher.update(&key);
        leaf_hasher.update(&value);
        let leaf: [u8; 32] = leaf_hasher.finalize().into();

        let sibling: Hash = [0x42u8; 32];

        let mut root_hasher = Keccak256::new();
        root_hasher.update(leaf);
        root_hasher.update(sibling);
        let state_root: [u8; 32] = root_hasher.finalize().into();

        let mut genesis = make_header(0, [0u8; 32]);
        genesis.state_root = state_root;
        light.bootstrap(genesis).unwrap();

        let proof = MerkleProof {
            key,
            value,
            siblings: vec![sibling],
            path_bits: vec![false], // leaf is on the left
        };

        // Valid proof
        assert!(light.verify_merkle_proof(&proof, 0).unwrap());

        // Tampered proof
        let mut bad_proof = proof.clone();
        bad_proof.value = b"wrong".to_vec();
        assert!(light.verify_merkle_proof(&bad_proof, 0).is_err());
    }

    #[test]
    fn test_range_too_large() {
        let light = lc();
        let genesis = make_header(0, [0u8; 32]);
        light.bootstrap(genesis).unwrap();

        let batch: Vec<BlockHeader> = (0..51).map(|_| make_header(0, [0u8; 32])).collect();
        assert!(light.sync_headers(batch).is_err());
    }
}

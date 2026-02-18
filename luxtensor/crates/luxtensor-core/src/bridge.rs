// LuxTensor Cross-Chain Bridge Interface
//
// Defines the trait and data structures for bridging assets between
// LuxTensor and external chains (primarily Ethereum).
//
// Architecture: Lock-and-Mint / Burn-and-Release
//   - Lock native assets on source chain  → mint wrapped assets on target
//   - Burn wrapped assets on target chain → release native assets on source
//
// This module provides the *interface* and verification logic only.
// Actual bridge relayer / smart contract deployment is out of scope.

use crate::types::{Address, Hash};
use serde::{Deserialize, Serialize};

// ─── Error ───────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("bridge message {0:?} not found")]
    MessageNotFound(Hash),

    #[error("invalid bridge proof: {0}")]
    InvalidProof(String),

    #[error("bridge message already processed: {0:?}")]
    AlreadyProcessed(Hash),

    #[error("unsupported target chain: {0}")]
    UnsupportedChain(u64),

    #[error("amount below minimum bridge threshold ({0} < {1})")]
    BelowMinimum(u64, u64),

    #[error("bridge is paused")]
    Paused,

    #[error("insufficient relayer signatures ({0} < {1})")]
    InsufficientSignatures(usize, usize),

    #[error("nonce mismatch: expected {expected}, got {got}")]
    NonceMismatch { expected: u64, got: u64 },

    #[error("store error: {0}")]
    StoreError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),
}

pub type Result<T> = std::result::Result<T, BridgeError>;

// ─── Types ───────────────────────────────────────────────────────────

/// Supported external chains.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChainId {
    /// LuxTensor Mainnet (8899).
    LuxTensorMainnet,
    /// LuxTensor Testnet (9999).
    LuxTensorTestnet,
    /// Ethereum Mainnet (1).
    Ethereum,
    /// Ethereum Sepolia Testnet (11155111).
    EthereumSepolia,
}

impl ChainId {
    pub fn as_u64(&self) -> u64 {
        match self {
            ChainId::LuxTensorMainnet => 8899,
            ChainId::LuxTensorTestnet => 9999,
            ChainId::Ethereum => 1,
            ChainId::EthereumSepolia => 11_155_111,
        }
    }
}

/// Direction of a bridge transfer.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BridgeDirection {
    /// LuxTensor → External Chain (lock on LuxTensor, mint on target).
    Outbound,
    /// External Chain → LuxTensor (burn on source, release on LuxTensor).
    Inbound,
}

/// Status of a bridge message.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BridgeMessageStatus {
    /// Pending — waiting for relayer confirmations.
    Pending,
    /// Confirmed — enough relayers have attested.
    Confirmed,
    /// Executed on the target chain.
    Executed,
    /// Failed — proof rejected or expired.
    Failed,
}

/// A cross-chain bridge message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMessage {
    /// Unique hash of this message (keccak256 of canonical encoding).
    pub message_hash: Hash,
    /// Strictly increasing sequence number per direction+route.
    pub nonce: u64,
    pub direction: BridgeDirection,
    pub source_chain: ChainId,
    pub target_chain: ChainId,
    /// Sender address on the source chain.
    pub sender: Address,
    /// Recipient address on the target chain (20-byte).
    pub recipient: Address,
    /// Amount in base units (wei / smallest unit).
    pub amount: u64,
    /// Optional data payload (e.g. for contract calls).
    pub data: Vec<u8>,
    /// Block height on source chain where the deposit was made.
    pub source_block: u64,
    /// Timestamp of the source transaction.
    pub source_timestamp: u64,
    /// State root of the source chain at `source_block`.
    /// A zero root signals attestation-only mode (no Merkle verification).
    pub source_state_root: Hash,
    pub status: BridgeMessageStatus,
}

/// A relayer attestation for a bridge message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayerAttestation {
    pub message_hash: Hash,
    pub relayer: Address,
    /// ECDSA signature over the message hash.
    pub signature: Vec<u8>,
    pub attested_at: u64,
}

/// Proof submitted to the target chain to execute a bridge transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeProof {
    pub message: BridgeMessage,
    /// Merkle proof of the deposit/burn tx on the source chain.
    pub merkle_proof: Vec<Hash>,
    /// Relayer attestations (threshold signature scheme).
    pub attestations: Vec<RelayerAttestation>,
}

// ─── Config ──────────────────────────────────────────────────────────

/// Bridge configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Minimum number of relayer attestations required.
    pub min_attestations: usize,
    /// Minimum transfer amount (in base units).
    pub min_transfer_amount: u64,
    /// Maximum age (in blocks) of a bridge message before it expires.
    pub max_message_age_blocks: u64,
    /// Set of authorised relayer addresses.
    pub relayers: Vec<Address>,
    /// Whether the bridge is currently paused.
    pub paused: bool,
    /// When `true`, skip Merkle proof verification and rely only on relayer
    /// attestations.  This flag replaces the previous magic "zero state root"
    /// sentinel.  It exists for backward compatibility until the source-chain
    /// light client is fully integrated, at which point it should be removed.
    ///
    /// # Security
    /// Enabling this reduces the bridge security model to attestation-only.
    /// Do NOT enable in production once Merkle verification is available.
    pub attestation_only_mode: bool,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            min_attestations: 3,
            min_transfer_amount: 1_000_000_000_000_000, // 0.001 MDT
            max_message_age_blocks: 50_400,             // ~7 days
            relayers: Vec::new(),
            paused: false,
            attestation_only_mode: false,
        }
    }
}

// ─── Bridge Trait ────────────────────────────────────────────────────

/// Core bridge interface.
///
/// Implementations may talk to smart contracts, relay services, or
/// be used for testing / simulation.
pub trait Bridge {
    /// Initiate an outbound transfer (lock on LuxTensor).
    fn initiate_transfer(
        &self,
        sender: Address,
        recipient: Address,
        amount: u64,
        target_chain: ChainId,
        data: Vec<u8>,
        current_block: u64,
        current_timestamp: u64,
    ) -> Result<BridgeMessage>;

    /// Submit a proof for an inbound transfer (release on LuxTensor).
    fn submit_proof(&self, proof: BridgeProof, current_block: u64) -> Result<BridgeMessage>;

    /// Query the status of a bridge message.
    fn get_message(&self, message_hash: Hash) -> Result<BridgeMessage>;
}

// ─── Compute Hash (shared helper) ────────────────────────────────────

/// Compute a deterministic message hash.
///
/// SECURITY: Includes domain separator, chain IDs, and direction to prevent
/// cross-route replay attacks. Without these, an outbound message on route
/// A→B could be replayed on route A→C.
pub fn compute_message_hash(
    nonce: u64,
    sender: &Address,
    recipient: &Address,
    amount: u64,
    source_chain: ChainId,
    target_chain: ChainId,
    direction: BridgeDirection,
    data: &[u8],
) -> Hash {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(b"LUXTENSOR_BRIDGE_MSG_V1"); // Domain separator
    hasher.update(source_chain.as_u64().to_le_bytes());
    hasher.update(target_chain.as_u64().to_le_bytes());
    hasher.update([direction as u8]);
    hasher.update(nonce.to_le_bytes());
    hasher.update(sender);
    hasher.update(recipient);
    hasher.update(amount.to_le_bytes());
    hasher.update(&(data.len() as u64).to_le_bytes());
    hasher.update(data);
    hasher.finalize().into()
}

/// Verify that a proof has enough valid relayer attestations.
///
/// SECURITY: Deduplicates by relayer address — the same relayer cannot
/// contribute more than one attestation to the threshold count.
fn verify_attestations_with_config(config: &BridgeConfig, proof: &BridgeProof) -> Result<()> {
    use std::collections::HashSet;
    let mut seen_relayers = HashSet::new();
    let valid_count = proof
        .attestations
        .iter()
        .filter(|a| {
            if a.message_hash != proof.message.message_hash {
                return false;
            }
            if !config.relayers.contains(&a.relayer) {
                return false;
            }
            if a.signature.len() != 65 {
                return false;
            }
            if !seen_relayers.insert(a.relayer) {
                return false;
            }
            match luxtensor_crypto::recover_address(&a.message_hash, &a.signature) {
                Ok(recovered) => recovered.as_bytes() == a.relayer.as_bytes(),
                Err(_) => false,
            }
        })
        .count();

    if valid_count < config.min_attestations {
        return Err(BridgeError::InsufficientSignatures(
            valid_count,
            config.min_attestations,
        ));
    }
    Ok(())
}

// ─── BridgeStore Trait ──────────────────────────────────────────────

use std::sync::Arc;

/// Abstraction over the storage backend for bridge messages and nonces.
///
/// Implementations can use in-memory maps, RocksDB, or any other KV store.
pub trait BridgeStore: Send + Sync {
    /// Retrieve a bridge message by its hash.
    fn get_message(&self, hash: &Hash) -> Result<Option<BridgeMessage>>;
    /// Persist a bridge message keyed by its hash.
    fn put_message(&self, hash: &Hash, msg: &BridgeMessage) -> Result<()>;
    /// Get a named nonce counter (e.g. "outbound", "inbound").
    fn get_nonce(&self, key: &str) -> Result<u64>;
    /// Set a named nonce counter.
    fn put_nonce(&self, key: &str, val: u64) -> Result<()>;
    /// List all messages with a given status.
    fn list_by_status(&self, status: BridgeMessageStatus) -> Result<Vec<BridgeMessage>>;
    /// List all bridge messages.
    fn list_all(&self) -> Result<Vec<BridgeMessage>>;
}

// ─── InMemoryBridgeStore ────────────────────────────────────────────

use parking_lot::RwLock;
use std::collections::HashMap;

/// In-memory implementation of `BridgeStore` backed by `HashMap`.
pub struct InMemoryBridgeStore {
    messages: RwLock<HashMap<Hash, BridgeMessage>>,
    nonces: RwLock<HashMap<String, u64>>,
}

impl InMemoryBridgeStore {
    pub fn new() -> Self {
        Self {
            messages: RwLock::new(HashMap::new()),
            nonces: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryBridgeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BridgeStore for InMemoryBridgeStore {
    fn get_message(&self, hash: &Hash) -> Result<Option<BridgeMessage>> {
        Ok(self.messages.read().get(hash).cloned())
    }

    fn put_message(&self, hash: &Hash, msg: &BridgeMessage) -> Result<()> {
        self.messages.write().insert(*hash, msg.clone());
        Ok(())
    }

    fn get_nonce(&self, key: &str) -> Result<u64> {
        Ok(*self.nonces.read().get(key).unwrap_or(&1))
    }

    fn put_nonce(&self, key: &str, val: u64) -> Result<()> {
        self.nonces.write().insert(key.to_string(), val);
        Ok(())
    }

    fn list_by_status(&self, status: BridgeMessageStatus) -> Result<Vec<BridgeMessage>> {
        Ok(self
            .messages
            .read()
            .values()
            .filter(|m| m.status == status)
            .cloned()
            .collect())
    }

    fn list_all(&self) -> Result<Vec<BridgeMessage>> {
        Ok(self.messages.read().values().cloned().collect())
    }
}

// ─── In-Memory Implementation ────────────────────────────────────────

/// Simple in-memory bridge for testing and validation.
pub struct InMemoryBridge {
    config: BridgeConfig,
    messages: RwLock<HashMap<Hash, BridgeMessage>>,
    nonce: RwLock<u64>,
    inbound_nonce: RwLock<u64>,
}

impl InMemoryBridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self {
            config,
            messages: RwLock::new(HashMap::new()),
            nonce: RwLock::new(1),
            inbound_nonce: RwLock::new(1),
        }
    }

    /// Verify that a proof has enough valid relayer attestations.
    /// Delegates to the shared `verify_attestations_with_config` helper.
    fn verify_attestations(&self, proof: &BridgeProof) -> Result<()> {
        verify_attestations_with_config(&self.config, proof)
    }

    /// Compute a deterministic message hash.
    /// Delegates to the shared `compute_message_hash` helper.
    fn compute_hash(
        nonce: u64,
        sender: &Address,
        recipient: &Address,
        amount: u64,
        source_chain: ChainId,
        target_chain: ChainId,
        direction: BridgeDirection,
        data: &[u8],
    ) -> Hash {
        compute_message_hash(nonce, sender, recipient, amount, source_chain, target_chain, direction, data)
    }

    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// List all messages with a given status.
    pub fn list_messages(&self, status: Option<BridgeMessageStatus>) -> Vec<BridgeMessage> {
        self.messages
            .read()
            .values()
            .filter(|m| status.map_or(true, |s| m.status == s))
            .cloned()
            .collect()
    }
}

/// Verify a Merkle inclusion proof for a bridge message.
///
/// The proof demonstrates that `message_hash` is included in the Merkle tree
/// with root `expected_root`. Each element in `proof` is a sibling hash that,
/// combined with the current hash, produces the next level's hash.
///
/// Uses the same keccak256 hash function as the rest of the chain.
fn verify_merkle_proof(message_hash: &Hash, proof: &[Hash], expected_root: &Hash) -> bool {
    if proof.is_empty() {
        // Single-element tree: message_hash should equal root
        return message_hash == expected_root;
    }

    let mut current = *message_hash;
    for sibling in proof {
        // Canonical ordering: smaller hash first to ensure determinism
        let combined = if current <= *sibling {
            let mut data = [0u8; 64];
            data[..32].copy_from_slice(&current);
            data[32..].copy_from_slice(sibling);
            data
        } else {
            let mut data = [0u8; 64];
            data[..32].copy_from_slice(sibling);
            data[32..].copy_from_slice(&current);
            data
        };
        current = luxtensor_crypto::keccak256(&combined);
    }
    current == *expected_root
}

impl Bridge for InMemoryBridge {
    fn initiate_transfer(
        &self,
        sender: Address,
        recipient: Address,
        amount: u64,
        target_chain: ChainId,
        data: Vec<u8>,
        current_block: u64,
        current_timestamp: u64,
    ) -> Result<BridgeMessage> {
        if self.config.paused {
            return Err(BridgeError::Paused);
        }
        if amount < self.config.min_transfer_amount {
            return Err(BridgeError::BelowMinimum(amount, self.config.min_transfer_amount));
        }

        let mut nonce = self.nonce.write();
        let n = *nonce;
        *nonce += 1;

        let message_hash = Self::compute_hash(
            n,
            &sender,
            &recipient,
            amount,
            ChainId::LuxTensorMainnet,
            target_chain,
            BridgeDirection::Outbound,
            &data,
        );

        let msg = BridgeMessage {
            message_hash,
            nonce: n,
            direction: BridgeDirection::Outbound,
            source_chain: ChainId::LuxTensorMainnet,
            target_chain,
            sender,
            recipient,
            amount,
            data,
            source_block: current_block,
            source_timestamp: current_timestamp,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        self.messages.write().insert(message_hash, msg.clone());
        Ok(msg)
    }

    fn submit_proof(&self, proof: BridgeProof, current_block: u64) -> Result<BridgeMessage> {
        if self.config.paused {
            return Err(BridgeError::Paused);
        }

        // Check if already processed
        {
            let messages = self.messages.read();
            if let Some(existing) = messages.get(&proof.message.message_hash) {
                if existing.status == BridgeMessageStatus::Executed {
                    return Err(BridgeError::AlreadyProcessed(proof.message.message_hash));
                }
            }
        }

        // Check message age
        if current_block > proof.message.source_block + self.config.max_message_age_blocks {
            return Err(BridgeError::InvalidProof("message expired".into()));
        }

        // C-1: Recompute message hash from fields and verify integrity
        let expected_hash = Self::compute_hash(
            proof.message.nonce,
            &proof.message.sender,
            &proof.message.recipient,
            proof.message.amount,
            proof.message.source_chain,
            proof.message.target_chain,
            proof.message.direction,
            &proof.message.data,
        );
        if expected_hash != proof.message.message_hash {
            return Err(BridgeError::InvalidProof(
                "message hash does not match message fields".into(),
            ));
        }

        // H-1 FIX: Use explicit config flag instead of magic zero-root sentinel.
        // When attestation_only_mode is false, Merkle proof is always verified.
        if !self.config.attestation_only_mode {
            if !verify_merkle_proof(
                &proof.message.message_hash,
                &proof.merkle_proof,
                &proof.message.source_state_root,
            ) {
                return Err(BridgeError::InvalidProof(
                    "Merkle proof verification failed: message not included in source state"
                        .to_string(),
                ));
            }
        }

        // H-3: Validate inbound nonce (sequential ordering)
        {
            let expected = *self.inbound_nonce.read();
            if proof.message.nonce != expected {
                return Err(BridgeError::NonceMismatch {
                    expected,
                    got: proof.message.nonce,
                });
            }
        }

        // Verify attestations
        self.verify_attestations(&proof)?;

        // Accept the message
        let mut msg = proof.message;
        msg.status = BridgeMessageStatus::Executed;
        msg.direction = BridgeDirection::Inbound;
        self.messages.write().insert(msg.message_hash, msg.clone());

        // H-3: Increment inbound nonce after successful execution
        *self.inbound_nonce.write() += 1;

        Ok(msg)
    }

    fn get_message(&self, message_hash: Hash) -> Result<BridgeMessage> {
        self.messages
            .read()
            .get(&message_hash)
            .cloned()
            .ok_or(BridgeError::MessageNotFound(message_hash))
    }
}

// ─── PersistentBridge ─────────────────────────────────────────────────

/// A persistent bridge implementation backed by any `BridgeStore`.
///
/// Uses a write-through cache: recent messages are kept in an in-memory
/// `HashMap` for fast reads, and every write goes to both the cache and
/// the underlying store.
pub struct PersistentBridge {
    config: BridgeConfig,
    store: Arc<dyn BridgeStore>,
    /// Write-through cache for fast reads of recent messages.
    cache: RwLock<HashMap<Hash, BridgeMessage>>,
    /// Guards nonce read-increment-write for outbound transfers.
    outbound_nonce_lock: RwLock<()>,
    /// Guards nonce read-increment-write for inbound transfers.
    inbound_nonce_lock: RwLock<()>,
}

/// Nonce key constants used in the `BridgeStore`.
const OUTBOUND_NONCE_KEY: &str = "outbound_nonce";
const INBOUND_NONCE_KEY: &str = "inbound_nonce";

impl PersistentBridge {
    /// Create a new `PersistentBridge` backed by the given store.
    ///
    /// On startup the cache is populated from the store so that hot reads
    /// are served from memory.
    pub fn new(store: Arc<dyn BridgeStore>, config: BridgeConfig) -> Result<Self> {
        // Pre-populate cache from the store.
        let existing = store.list_all()?;
        let mut cache_map = HashMap::with_capacity(existing.len());
        for msg in existing {
            cache_map.insert(msg.message_hash, msg);
        }

        // Ensure nonce keys exist with default value 1.
        if store.get_nonce(OUTBOUND_NONCE_KEY)? == 0 {
            store.put_nonce(OUTBOUND_NONCE_KEY, 1)?;
        }
        if store.get_nonce(INBOUND_NONCE_KEY)? == 0 {
            store.put_nonce(INBOUND_NONCE_KEY, 1)?;
        }

        Ok(Self {
            config,
            store,
            cache: RwLock::new(cache_map),
            outbound_nonce_lock: RwLock::new(()),
            inbound_nonce_lock: RwLock::new(()),
        })
    }

    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// List all messages, optionally filtered by status.
    ///
    /// Reads from the cache first; falls back to the store if the cache
    /// is empty (shouldn't happen after `new()`, but defensive).
    pub fn list_messages(&self, status: Option<BridgeMessageStatus>) -> Result<Vec<BridgeMessage>> {
        let cache = self.cache.read();
        if cache.is_empty() {
            drop(cache);
            return match status {
                Some(s) => self.store.list_by_status(s),
                None => self.store.list_all(),
            };
        }
        Ok(cache
            .values()
            .filter(|m| status.map_or(true, |s| m.status == s))
            .cloned()
            .collect())
    }

    /// Write a message to both cache and store (write-through).
    fn write_through(&self, msg: &BridgeMessage) -> Result<()> {
        self.store.put_message(&msg.message_hash, msg)?;
        self.cache.write().insert(msg.message_hash, msg.clone());
        Ok(())
    }
}

impl Bridge for PersistentBridge {
    fn initiate_transfer(
        &self,
        sender: Address,
        recipient: Address,
        amount: u64,
        target_chain: ChainId,
        data: Vec<u8>,
        current_block: u64,
        current_timestamp: u64,
    ) -> Result<BridgeMessage> {
        if self.config.paused {
            return Err(BridgeError::Paused);
        }
        if amount < self.config.min_transfer_amount {
            return Err(BridgeError::BelowMinimum(amount, self.config.min_transfer_amount));
        }

        // Atomic nonce read + increment + write under a write-lock.
        let _guard = self.outbound_nonce_lock.write();
        let n = self.store.get_nonce(OUTBOUND_NONCE_KEY)?;
        self.store.put_nonce(OUTBOUND_NONCE_KEY, n + 1)?;

        let message_hash = compute_message_hash(
            n,
            &sender,
            &recipient,
            amount,
            ChainId::LuxTensorMainnet,
            target_chain,
            BridgeDirection::Outbound,
            &data,
        );

        let msg = BridgeMessage {
            message_hash,
            nonce: n,
            direction: BridgeDirection::Outbound,
            source_chain: ChainId::LuxTensorMainnet,
            target_chain,
            sender,
            recipient,
            amount,
            data,
            source_block: current_block,
            source_timestamp: current_timestamp,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        self.write_through(&msg)?;
        Ok(msg)
    }

    fn submit_proof(&self, proof: BridgeProof, current_block: u64) -> Result<BridgeMessage> {
        if self.config.paused {
            return Err(BridgeError::Paused);
        }

        // Check if already processed (cache first, then store).
        {
            let cache = self.cache.read();
            if let Some(existing) = cache.get(&proof.message.message_hash) {
                if existing.status == BridgeMessageStatus::Executed {
                    return Err(BridgeError::AlreadyProcessed(proof.message.message_hash));
                }
            } else {
                drop(cache);
                if let Some(existing) = self.store.get_message(&proof.message.message_hash)? {
                    if existing.status == BridgeMessageStatus::Executed {
                        return Err(BridgeError::AlreadyProcessed(proof.message.message_hash));
                    }
                }
            }
        }

        // Check message age.
        if current_block > proof.message.source_block + self.config.max_message_age_blocks {
            return Err(BridgeError::InvalidProof("message expired".into()));
        }

        // Recompute and verify message hash integrity.
        let expected_hash = compute_message_hash(
            proof.message.nonce,
            &proof.message.sender,
            &proof.message.recipient,
            proof.message.amount,
            proof.message.source_chain,
            proof.message.target_chain,
            proof.message.direction,
            &proof.message.data,
        );
        if expected_hash != proof.message.message_hash {
            return Err(BridgeError::InvalidProof(
                "message hash does not match message fields".into(),
            ));
        }

        // H-1 FIX: Use explicit config flag instead of magic zero-root sentinel.
        if !self.config.attestation_only_mode {
            if !verify_merkle_proof(
                &proof.message.message_hash,
                &proof.merkle_proof,
                &proof.message.source_state_root,
            ) {
                return Err(BridgeError::InvalidProof(
                    "Merkle proof verification failed: message not included in source state"
                        .to_string(),
                ));
            }
        }

        // Validate inbound nonce (sequential ordering) — atomic under lock.
        let _guard = self.inbound_nonce_lock.write();
        let expected_nonce = self.store.get_nonce(INBOUND_NONCE_KEY)?;
        if proof.message.nonce != expected_nonce {
            return Err(BridgeError::NonceMismatch {
                expected: expected_nonce,
                got: proof.message.nonce,
            });
        }

        // Verify attestations.
        verify_attestations_with_config(&self.config, &proof)?;

        // Accept the message.
        let mut msg = proof.message;
        msg.status = BridgeMessageStatus::Executed;
        msg.direction = BridgeDirection::Inbound;
        self.write_through(&msg)?;

        // Increment inbound nonce after successful execution.
        self.store.put_nonce(INBOUND_NONCE_KEY, expected_nonce + 1)?;

        Ok(msg)
    }

    fn get_message(&self, message_hash: Hash) -> Result<BridgeMessage> {
        // Try cache first.
        if let Some(msg) = self.cache.read().get(&message_hash) {
            return Ok(msg.clone());
        }
        // Fall back to store.
        self.store
            .get_message(&message_hash)?
            .ok_or(BridgeError::MessageNotFound(message_hash))
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_crypto::KeyPair;

    fn addr(b: u8) -> Address {
        let mut a = [0u8; 20];
        a[0] = b;
        Address::new(a)
    }

    /// Generate a deterministic relayer keypair and matching Address.
    fn relayer_keypair(seed: u8) -> (KeyPair, Address) {
        // Deterministic 32-byte secret (non-zero, valid for secp256k1)
        let mut secret = [0u8; 32];
        secret[31] = seed;
        secret[0] = 0x01; // ensure non-zero scalar
        let kp = KeyPair::from_secret(&secret).expect("valid secret");
        let addr = Address::from(kp.address());
        (kp, addr)
    }

    /// Sign a message hash with the relayer keypair, returning a 65-byte r‖s‖v
    /// signature that `recover_address` can verify.
    fn sign_attestation(kp: &KeyPair, message_hash: &Hash) -> Vec<u8> {
        let sig_64 = kp.sign(message_hash).expect("signing");
        // Try recovery ids 0 and 1 to find which one recovers to our address
        let expected_addr = kp.address();
        for v in 0u8..=1u8 {
            let mut sig65 = Vec::with_capacity(65);
            sig65.extend_from_slice(&sig_64);
            sig65.push(v);
            if let Ok(recovered) = luxtensor_crypto::recover_address(message_hash, &sig65) {
                if recovered == expected_addr {
                    return sig65;
                }
            }
        }
        panic!("Could not find recovery id for signature");
    }

    fn make_attestation(message_hash: Hash, kp: &KeyPair, relayer: Address) -> RelayerAttestation {
        RelayerAttestation {
            message_hash,
            relayer,
            signature: sign_attestation(kp, &message_hash),
            attested_at: 100,
        }
    }

    fn relayer_set_with_keys() -> Vec<(KeyPair, Address)> {
        vec![relayer_keypair(100), relayer_keypair(101), relayer_keypair(102)]
    }

    fn test_bridge_with_keys() -> (InMemoryBridge, Vec<(KeyPair, Address)>) {
        let keys = relayer_set_with_keys();
        let relayer_addrs: Vec<Address> = keys.iter().map(|(_, a)| *a).collect();
        let bridge = InMemoryBridge::new(BridgeConfig {
            min_attestations: 2,
            min_transfer_amount: 1_000,
            max_message_age_blocks: 1_000,
            relayers: relayer_addrs,
            paused: false,
            attestation_only_mode: true, // Tests use zero state roots
        });
        (bridge, keys)
    }

    #[test]
    fn test_outbound_transfer() {
        let (bridge, _keys) = test_bridge_with_keys();
        let msg = bridge
            .initiate_transfer(
                addr(1),
                addr(2),
                10_000,
                ChainId::Ethereum,
                Vec::new(),
                100,
                1_700_000_000,
            )
            .unwrap();

        assert_eq!(msg.nonce, 1);
        assert_eq!(msg.direction, BridgeDirection::Outbound);
        assert_eq!(msg.status, BridgeMessageStatus::Pending);
        assert_eq!(msg.amount, 10_000);

        // Can retrieve
        let fetched = bridge.get_message(msg.message_hash).unwrap();
        assert_eq!(fetched.nonce, 1);
    }

    #[test]
    fn test_below_minimum() {
        let (bridge, _keys) = test_bridge_with_keys();
        let result =
            bridge.initiate_transfer(addr(1), addr(2), 500, ChainId::Ethereum, Vec::new(), 100, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_paused_bridge() {
        let bridge = InMemoryBridge::new(BridgeConfig { paused: true, ..BridgeConfig::default() });
        let result = bridge.initiate_transfer(
            addr(1),
            addr(2),
            1_000_000,
            ChainId::Ethereum,
            Vec::new(),
            100,
            0,
        );
        assert!(matches!(result, Err(BridgeError::Paused)));
    }

    #[test]
    fn test_inbound_with_proof() {
        let (bridge, keys) = test_bridge_with_keys();

        // Create an inbound message
        let message_hash = InMemoryBridge::compute_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );
        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 1_700_000_000,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg,
            merkle_proof: vec![[0u8; 32]],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        let result = bridge.submit_proof(proof, 200).unwrap();
        assert_eq!(result.status, BridgeMessageStatus::Executed);
    }

    #[test]
    fn test_insufficient_attestations() {
        let (bridge, keys) = test_bridge_with_keys();

        let message_hash = InMemoryBridge::compute_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );
        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 0,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg,
            merkle_proof: vec![],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                // Only 1 valid attestation, need 2
            ],
        };

        assert!(bridge.submit_proof(proof, 200).is_err());
    }

    #[test]
    fn test_expired_message() {
        let (bridge, keys) = test_bridge_with_keys();

        let message_hash = InMemoryBridge::compute_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );
        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 0,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg,
            merkle_proof: vec![],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        // current_block = 1200 > source_block(100) + max_age(1000)
        assert!(bridge.submit_proof(proof, 1_200).is_err());
    }

    #[test]
    fn test_chain_ids() {
        assert_eq!(ChainId::LuxTensorMainnet.as_u64(), 8899);
        assert_eq!(ChainId::LuxTensorTestnet.as_u64(), 9999);
        assert_eq!(ChainId::Ethereum.as_u64(), 1);
        assert_eq!(ChainId::EthereumSepolia.as_u64(), 11_155_111);
    }

    #[test]
    fn test_double_execution_prevented() {
        let (bridge, keys) = test_bridge_with_keys();
        let message_hash = InMemoryBridge::compute_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );
        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 0,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg.clone(),
            merkle_proof: vec![],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        // First execution succeeds
        bridge.submit_proof(proof.clone(), 200).unwrap();

        // Second execution fails
        assert!(matches!(bridge.submit_proof(proof, 200), Err(BridgeError::AlreadyProcessed(_))));
    }

    #[test]
    fn test_merkle_proof_verification() {
        use luxtensor_crypto::keccak256;

        // Single element tree: message_hash equals root
        let leaf = keccak256(b"test message");
        assert!(verify_merkle_proof(&leaf, &[], &leaf));

        // Two-element tree
        let leaf_a = keccak256(b"message_a");
        let leaf_b = keccak256(b"message_b");
        let (first, second) = if leaf_a <= leaf_b {
            (leaf_a, leaf_b)
        } else {
            (leaf_b, leaf_a)
        };
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&first);
        combined[32..].copy_from_slice(&second);
        let root = keccak256(&combined);

        assert!(verify_merkle_proof(&leaf_a, &[leaf_b], &root));
        assert!(verify_merkle_proof(&leaf_b, &[leaf_a], &root));

        // Invalid proof: wrong root
        let wrong_root = [0u8; 32];
        assert!(!verify_merkle_proof(&leaf_a, &[leaf_b], &wrong_root));
    }

    #[test]
    fn test_submit_proof_with_merkle_verification() {
        let (bridge, keys) = test_bridge_with_keys();

        let message_hash = InMemoryBridge::compute_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );

        // Build a valid single-leaf Merkle tree (leaf == root)
        let source_state_root = message_hash;

        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 1_700_000_000,
            source_state_root,
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg,
            merkle_proof: vec![],  // empty proof ⇒ leaf must equal root
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        let result = bridge.submit_proof(proof, 200).unwrap();
        assert_eq!(result.status, BridgeMessageStatus::Executed);
    }

    #[test]
    fn test_submit_proof_invalid_merkle() {
        let (bridge, keys) = test_bridge_with_keys();

        let message_hash = InMemoryBridge::compute_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );

        // Non-zero state root with a proof that does NOT match
        let bad_state_root = [0xFFu8; 32];

        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 1_700_000_000,
            source_state_root: bad_state_root,
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg,
            merkle_proof: vec![[0xABu8; 32]],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        let result = bridge.submit_proof(proof, 200);
        assert!(matches!(result, Err(BridgeError::InvalidProof(_))));
    }

    // ── PersistentBridge tests ──────────────────────────────────────

    fn persistent_bridge_with_keys() -> (PersistentBridge, Vec<(KeyPair, Address)>) {
        let keys = relayer_set_with_keys();
        let relayer_addrs: Vec<Address> = keys.iter().map(|(_, a)| *a).collect();
        let store = Arc::new(InMemoryBridgeStore::new());
        let bridge = PersistentBridge::new(
            store,
            BridgeConfig {
                min_attestations: 2,
                min_transfer_amount: 1_000,
                max_message_age_blocks: 1_000,
                relayers: relayer_addrs,
                paused: false,
                attestation_only_mode: true, // Tests use zero state roots
            },
        )
        .unwrap();
        (bridge, keys)
    }

    #[test]
    fn test_persistent_outbound_transfer() {
        let (bridge, _keys) = persistent_bridge_with_keys();
        let msg = bridge
            .initiate_transfer(
                addr(1),
                addr(2),
                10_000,
                ChainId::Ethereum,
                Vec::new(),
                100,
                1_700_000_000,
            )
            .unwrap();

        assert_eq!(msg.nonce, 1);
        assert_eq!(msg.direction, BridgeDirection::Outbound);
        assert_eq!(msg.status, BridgeMessageStatus::Pending);
        assert_eq!(msg.amount, 10_000);

        // Can retrieve via Bridge trait
        let fetched = bridge.get_message(msg.message_hash).unwrap();
        assert_eq!(fetched.nonce, 1);

        // Also via store
        let from_store = bridge.store.get_message(&msg.message_hash).unwrap();
        assert!(from_store.is_some());
    }

    #[test]
    fn test_persistent_nonce_increments() {
        let (bridge, _keys) = persistent_bridge_with_keys();
        let m1 = bridge
            .initiate_transfer(addr(1), addr(2), 10_000, ChainId::Ethereum, vec![], 100, 0)
            .unwrap();
        let m2 = bridge
            .initiate_transfer(addr(1), addr(2), 20_000, ChainId::Ethereum, vec![], 101, 0)
            .unwrap();
        assert_eq!(m1.nonce, 1);
        assert_eq!(m2.nonce, 2);
    }

    #[test]
    fn test_persistent_below_minimum() {
        let (bridge, _keys) = persistent_bridge_with_keys();
        let result =
            bridge.initiate_transfer(addr(1), addr(2), 500, ChainId::Ethereum, Vec::new(), 100, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_persistent_paused() {
        let store = Arc::new(InMemoryBridgeStore::new());
        let bridge = PersistentBridge::new(
            store,
            BridgeConfig { paused: true, ..BridgeConfig::default() },
        )
        .unwrap();
        let result = bridge.initiate_transfer(
            addr(1),
            addr(2),
            1_000_000,
            ChainId::Ethereum,
            Vec::new(),
            100,
            0,
        );
        assert!(matches!(result, Err(BridgeError::Paused)));
    }

    #[test]
    fn test_persistent_inbound_with_proof() {
        let (bridge, keys) = persistent_bridge_with_keys();

        let message_hash = compute_message_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );
        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 1_700_000_000,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg,
            merkle_proof: vec![[0u8; 32]],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        let result = bridge.submit_proof(proof, 200).unwrap();
        assert_eq!(result.status, BridgeMessageStatus::Executed);
    }

    #[test]
    fn test_persistent_double_execution_prevented() {
        let (bridge, keys) = persistent_bridge_with_keys();
        let message_hash = compute_message_hash(
            1,
            &addr(10),
            &addr(11),
            50_000,
            ChainId::Ethereum,
            ChainId::LuxTensorMainnet,
            BridgeDirection::Inbound,
            &[],
        );
        let msg = BridgeMessage {
            message_hash,
            nonce: 1,
            direction: BridgeDirection::Inbound,
            source_chain: ChainId::Ethereum,
            target_chain: ChainId::LuxTensorMainnet,
            sender: addr(10),
            recipient: addr(11),
            amount: 50_000,
            data: Vec::new(),
            source_block: 100,
            source_timestamp: 0,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };

        let proof = BridgeProof {
            message: msg.clone(),
            merkle_proof: vec![],
            attestations: vec![
                make_attestation(message_hash, &keys[0].0, keys[0].1),
                make_attestation(message_hash, &keys[1].0, keys[1].1),
            ],
        };

        bridge.submit_proof(proof.clone(), 200).unwrap();
        assert!(matches!(bridge.submit_proof(proof, 200), Err(BridgeError::AlreadyProcessed(_))));
    }

    #[test]
    fn test_persistent_list_messages() {
        let (bridge, _keys) = persistent_bridge_with_keys();
        bridge
            .initiate_transfer(addr(1), addr(2), 10_000, ChainId::Ethereum, vec![], 100, 0)
            .unwrap();
        bridge
            .initiate_transfer(addr(1), addr(3), 20_000, ChainId::Ethereum, vec![], 101, 0)
            .unwrap();

        let all = bridge.list_messages(None).unwrap();
        assert_eq!(all.len(), 2);

        let pending = bridge.list_messages(Some(BridgeMessageStatus::Pending)).unwrap();
        assert_eq!(pending.len(), 2);

        let executed = bridge.list_messages(Some(BridgeMessageStatus::Executed)).unwrap();
        assert_eq!(executed.len(), 0);
    }

    #[test]
    fn test_inmemory_bridge_store_standalone() {
        let store = InMemoryBridgeStore::new();
        // Nonces default to 1
        assert_eq!(store.get_nonce("test").unwrap(), 1);
        store.put_nonce("test", 42).unwrap();
        assert_eq!(store.get_nonce("test").unwrap(), 42);

        // Messages
        let hash = [0xABu8; 32];
        assert!(store.get_message(&hash).unwrap().is_none());

        let msg = BridgeMessage {
            message_hash: hash,
            nonce: 1,
            direction: BridgeDirection::Outbound,
            source_chain: ChainId::LuxTensorMainnet,
            target_chain: ChainId::Ethereum,
            sender: addr(1),
            recipient: addr(2),
            amount: 5_000,
            data: vec![],
            source_block: 10,
            source_timestamp: 0,
            source_state_root: [0u8; 32],
            status: BridgeMessageStatus::Pending,
        };
        store.put_message(&hash, &msg).unwrap();
        assert!(store.get_message(&hash).unwrap().is_some());
        assert_eq!(store.list_all().unwrap().len(), 1);
        assert_eq!(store.list_by_status(BridgeMessageStatus::Pending).unwrap().len(), 1);
        assert_eq!(store.list_by_status(BridgeMessageStatus::Executed).unwrap().len(), 0);
    }

    #[test]
    fn test_compute_message_hash_matches_inmemory() {
        // Verify the standalone function produces the same hash as InMemoryBridge::compute_hash
        let h1 = compute_message_hash(
            1, &addr(1), &addr(2), 100,
            ChainId::Ethereum, ChainId::LuxTensorMainnet, BridgeDirection::Inbound, &[0xAB],
        );
        let h2 = InMemoryBridge::compute_hash(
            1, &addr(1), &addr(2), 100,
            ChainId::Ethereum, ChainId::LuxTensorMainnet, BridgeDirection::Inbound, &[0xAB],
        );
        assert_eq!(h1, h2);
    }
}

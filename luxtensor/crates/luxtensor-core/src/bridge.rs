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
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            min_attestations: 3,
            min_transfer_amount: 1_000_000_000_000_000, // 0.001 MDT
            max_message_age_blocks: 50_400,             // ~7 days
            relayers: Vec::new(),
            paused: false,
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

// ─── In-Memory Implementation ────────────────────────────────────────

use parking_lot::RwLock;
use std::collections::HashMap;

/// Simple in-memory bridge for testing and validation.
pub struct InMemoryBridge {
    config: BridgeConfig,
    messages: RwLock<HashMap<Hash, BridgeMessage>>,
    nonce: RwLock<u64>,
}

impl InMemoryBridge {
    pub fn new(config: BridgeConfig) -> Self {
        Self { config, messages: RwLock::new(HashMap::new()), nonce: RwLock::new(1) }
    }

    /// Verify that a proof has enough valid relayer attestations.
    ///
    /// SECURITY: Deduplicates by relayer address — the same relayer cannot
    /// contribute more than one attestation to the threshold count.
    /// Without dedup, an attacker controlling a single relayer could submit
    /// N copies of the same attestation to bypass the M-of-N threshold.
    ///
    /// Each attestation is verified by recovering the ECDSA signer address
    /// from the signature and checking it matches the declared relayer.
    fn verify_attestations(&self, proof: &BridgeProof) -> Result<()> {
        use std::collections::HashSet;
        let mut seen_relayers = HashSet::new();
        let valid_count = proof
            .attestations
            .iter()
            .filter(|a| {
                // 1. Message hash must match
                if a.message_hash != proof.message.message_hash {
                    return false;
                }
                // 2. Relayer must be in the allowed set
                if !self.config.relayers.contains(&a.relayer) {
                    return false;
                }
                // 3. Signature must be exactly 65 bytes (r[32] + s[32] + v[1])
                if a.signature.len() != 65 {
                    return false;
                }
                // 4. Dedup: same relayer can only attest once
                if !seen_relayers.insert(a.relayer) {
                    return false;
                }
                // 5. ECDSA: recover signer address and verify it matches a.relayer
                match luxtensor_crypto::recover_address(&a.message_hash, &a.signature) {
                    Ok(recovered) => recovered == *a.relayer.as_bytes(),
                    Err(_) => false,
                }
            })
            .count();

        if valid_count < self.config.min_attestations {
            return Err(BridgeError::InsufficientSignatures(
                valid_count,
                self.config.min_attestations,
            ));
        }
        Ok(())
    }

    /// Compute a deterministic message hash.
    ///
    /// SECURITY: Includes domain separator, chain IDs, and direction to prevent
    /// cross-route replay attacks. Without these, an outbound message on route
    /// A→B could be replayed on route A→C.
    fn compute_hash(
        nonce: u64,
        sender: &Address,
        recipient: &Address,
        amount: u64,
        source_chain: ChainId,
        target_chain: ChainId,
        direction: BridgeDirection,
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
        hasher.finalize().into()
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

        // Verify attestations
        self.verify_attestations(&proof)?;

        // Accept the message
        let mut msg = proof.message;
        msg.status = BridgeMessageStatus::Executed;
        msg.direction = BridgeDirection::Inbound;
        self.messages.write().insert(msg.message_hash, msg.clone());
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
        let addr = Address::new(kp.address());
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
}

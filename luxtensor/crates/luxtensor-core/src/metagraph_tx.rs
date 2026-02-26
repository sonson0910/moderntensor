//! Metagraph Transaction Payload
//!
//! When a client calls `neuron_register`, `subnet_create`, or `staking_registerValidator`,
//! the RPC handler encodes the operation as a `MetagraphTxPayload` (bincode-serialized)
//! and submits it as a standard Ethereum transaction where:
//!   `to` = PRECOMPILE_METAGRAPH (0x000...4d455441)
//!   `data` = bincode::serialize(&MetagraphTxPayload)
//!   `value` = 0  (stake is tracked in MetagraphDB, NOT as on-chain token transfer)
//!
//! The executor detects this address and calls the metagraph precompile instead of EVM.
//! This ensures ALL nodes process the same operations when they receive the block.

use serde::{Deserialize, Serialize};
use bincode;

/// Payload for metagraph precompile transactions.
/// Encoded as `tx.data` field using bincode with fixint encoding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetagraphTxPayload {
    /// Register a new neuron in a subnet
    RegisterNeuron {
        subnet_id: u64,
        uid: u64,
        /// 20-byte Ethereum address (hotkey)
        hotkey: [u8; 20],
        /// 20-byte Ethereum address (coldkey)
        coldkey: [u8; 20],
        endpoint: String,
        stake: u128,
        active: bool,
    },

    /// Register validator metadata (in addition to neuron registration)
    RegisterValidator {
        /// 20-byte Ethereum address
        hotkey: [u8; 20],
        name: String,
        stake: u128,
    },

    /// Create a new subnet
    CreateSubnet {
        subnet_id: u64,
        /// 20-byte Ethereum address (owner)
        owner: [u8; 20],
        name: String,
        min_stake: u128,
    },

    /// Set weights from a validator UID to miner UIDs
    SetWeights {
        subnet_id: u64,
        /// Validator UID setting the weights
        uid: u64,
        /// Vec of (target_uid, weight_u16) pairs
        weights: Vec<(u64, u16)>,
    },
}

impl MetagraphTxPayload {
    /// Serialize using bincode
    pub fn encode(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize using bincode
    pub fn decode(data: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(data)
    }

    /// Returns true if this is a metagraph tx payload (starts with valid variant tag)
    pub fn is_metagraph_data(data: &[u8]) -> bool {
        // The first 8 bytes in fixint encoding contain the enum variant index.
        // We accept variant indices 0-3 (RegisterNeuron, RegisterValidator, CreateSubnet, SetWeights).
        if data.len() < 8 {
            return false;
        }
        // Try to decode — if it succeeds, it's valid metagraph data
        Self::decode(data).is_ok()
    }
}

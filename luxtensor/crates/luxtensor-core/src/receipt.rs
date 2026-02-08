//! Transaction Receipt types shared between node and RPC crates.
//!
//! These types are serialized with `bincode` when stored in RocksDB
//! and deserialized by the RPC layer to return real receipt data.

use crate::Address;
use serde::{Deserialize, Serialize};

/// Transaction receipt stored after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub transaction_hash: [u8; 32],
    pub block_height: u64,
    pub block_hash: [u8; 32],
    pub transaction_index: usize,
    pub from: Address,
    pub to: Option<Address>,
    pub gas_used: u64,
    pub status: ExecutionStatus,
    pub logs: Vec<Log>,
    /// Contract address if this was a contract deployment
    pub contract_address: Option<Address>,
}

/// Execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Success = 1,
    Failed = 0,
}

/// Transaction log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    pub address: Address,
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

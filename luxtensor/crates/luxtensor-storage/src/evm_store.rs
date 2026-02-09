// EVM state persistence traits and types
// Defines the interface for EVM state storage (implemented by BlockchainDB).
// This lives in the storage crate to avoid a layer inversion where infrastructure
// (storage) would depend on application (contracts).

use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};

/// Serializable EVM account record for RocksDB storage
#[derive(SerdeSerialize, SerdeDeserialize, Debug, Clone)]
pub struct EvmAccountRecord {
    pub balance: [u8; 32], // U256 as big-endian bytes (to survive serde)
    pub nonce: u64,
    pub code_hash: [u8; 32],
    pub code: Option<Vec<u8>>,
}

/// Trait for EVM state persistence (implemented by BlockchainDB)
pub trait EvmStateStore: Send + Sync {
    /// Load all EVM accounts from persistent storage
    fn load_all_evm_accounts(&self) -> Result<Vec<([u8; 20], EvmAccountRecord)>, String>;
    /// Load all EVM storage slots from persistent storage
    fn load_all_evm_storage(&self) -> Result<Vec<([u8; 20], [u8; 32], [u8; 32])>, String>;
    /// Flush EVM state to persistent storage atomically
    fn flush_evm_state(
        &self,
        accounts: &[([u8; 20], Vec<u8>)],
        storage: &[([u8; 20], [u8; 32], [u8; 32])],
        deleted: &[[u8; 20]],
    ) -> Result<(), String>;
}

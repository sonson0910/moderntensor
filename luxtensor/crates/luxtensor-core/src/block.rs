use crate::{Hash, Transaction};
use luxtensor_crypto::keccak256;
use serde::{Deserialize, Serialize};

/// Block header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub height: u64,
    pub timestamp: u64,
    pub previous_hash: Hash,
    pub state_root: Hash,
    pub txs_root: Hash,
    pub receipts_root: Hash,

    pub validator: [u8; 32],
    pub signature: Vec<u8>, // 64 bytes signature

    pub gas_used: u64,
    pub gas_limit: u64,
    pub extra_data: Vec<u8>,
}

impl BlockHeader {
    pub fn new(
        version: u32,
        height: u64,
        timestamp: u64,
        previous_hash: Hash,
        state_root: Hash,
        txs_root: Hash,
        receipts_root: Hash,
        validator: [u8; 32],
        signature: [u8; 64],
        gas_used: u64,
        gas_limit: u64,
        extra_data: Vec<u8>,
    ) -> Self {
        Self {
            version,
            height,
            timestamp,
            previous_hash,
            state_root,
            txs_root,
            receipts_root,
            validator,
            signature: signature.to_vec(),
            gas_used,
            gas_limit,
            extra_data,
        }
    }

    /// Maximum extra_data size in bytes
    pub const MAX_EXTRA_DATA_SIZE: usize = 1024;

    /// Calculate block header hash (excludes signature for signing stability)
    pub fn hash(&self) -> Hash {
        // Hash all fields EXCEPT signature to allow stable block hash before/after signing
        let mut data = Vec::new();
        data.extend_from_slice(&self.version.to_le_bytes());
        data.extend_from_slice(&self.height.to_le_bytes());
        data.extend_from_slice(&self.timestamp.to_le_bytes());
        data.extend_from_slice(&self.previous_hash);
        data.extend_from_slice(&self.state_root);
        data.extend_from_slice(&self.txs_root);
        data.extend_from_slice(&self.receipts_root);
        data.extend_from_slice(&self.validator);
        data.extend_from_slice(&self.gas_used.to_le_bytes());
        data.extend_from_slice(&self.gas_limit.to_le_bytes());
        data.extend_from_slice(&(self.extra_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&self.extra_data);
        keccak256(&data)
    }

    /// Validate block header fields
    pub fn validate(&self) -> crate::Result<()> {
        if self.extra_data.len() > Self::MAX_EXTRA_DATA_SIZE {
            return Err(crate::CoreError::InvalidBlock(format!(
                "extra_data too large: {} > {}",
                self.extra_data.len(),
                Self::MAX_EXTRA_DATA_SIZE
            )));
        }
        if self.gas_used > self.gas_limit {
            return Err(crate::CoreError::InvalidBlock(format!(
                "gas_used {} exceeds gas_limit {}",
                self.gas_used, self.gas_limit
            )));
        }
        Ok(())
    }
}

/// Block structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

/// Network-wide block gas limit (30M gas per Ethereum Mainnet convention)
pub const BLOCK_GAS_LIMIT: u64 = 30_000_000;

/// Maximum transactions per block
pub const MAX_TXS_PER_BLOCK: usize = 1_000;

impl Block {
    pub fn new(header: BlockHeader, transactions: Vec<Transaction>) -> Self {
        Self { header, transactions }
    }

    pub fn hash(&self) -> Hash {
        self.header.hash()
    }

    pub fn height(&self) -> u64 {
        self.header.height
    }

    pub fn timestamp(&self) -> u64 {
        self.header.timestamp
    }

    pub fn header(&self) -> &BlockHeader {
        &self.header
    }

    pub fn header_mut(&mut self) -> &mut BlockHeader {
        &mut self.header
    }

    /// Validate block structure including gas limits and transaction count.
    ///
    /// SECURITY: Enforces:
    /// 1. Header validation (gas_used <= gas_limit, extra_data size)
    /// 2. Block gas limit doesn't exceed network maximum
    /// 3. Transaction count doesn't exceed per-block maximum
    /// 4. Cumulative transaction gas_limit doesn't exceed block gas_limit
    pub fn validate(&self) -> crate::Result<()> {
        // Validate header
        self.header.validate()?;

        // SECURITY: Reject blocks with gas_limit above network maximum
        if self.header.gas_limit > BLOCK_GAS_LIMIT {
            return Err(crate::CoreError::InvalidBlock(format!(
                "block gas_limit {} exceeds network maximum {}",
                self.header.gas_limit, BLOCK_GAS_LIMIT
            )));
        }

        // SECURITY: Reject blocks with too many transactions (DoS protection)
        if self.transactions.len() > MAX_TXS_PER_BLOCK {
            return Err(crate::CoreError::InvalidBlock(format!(
                "block has {} transactions, exceeds maximum {}",
                self.transactions.len(),
                MAX_TXS_PER_BLOCK
            )));
        }

        // SECURITY: Verify cumulative transaction gas doesn't exceed block gas limit
        // Also validate each transaction has a reasonable gas_limit
        let cumulative_gas: u64 = self
            .transactions
            .iter()
            .map(|tx| tx.gas_limit)
            .fold(0u64, |acc, g| acc.saturating_add(g));

        if cumulative_gas > self.header.gas_limit {
            return Err(crate::CoreError::InvalidBlock(format!(
                "cumulative tx gas {} exceeds block gas_limit {}",
                cumulative_gas, self.header.gas_limit
            )));
        }

        // SECURITY: Reject individual transactions with gas_limit exceeding block limit
        for (i, tx) in self.transactions.iter().enumerate() {
            if tx.gas_limit > self.header.gas_limit {
                return Err(crate::CoreError::InvalidBlock(format!(
                    "tx[{}] gas_limit {} exceeds block gas_limit {}",
                    i, tx.gas_limit, self.header.gas_limit
                )));
            }
            if tx.gas_limit == 0 {
                return Err(crate::CoreError::InvalidBlock(format!(
                    "tx[{}] has zero gas_limit",
                    i
                )));
            }
        }

        Ok(())
    }

    /// Create genesis block
    pub fn genesis() -> Self {
        let header = BlockHeader {
            version: 1,
            height: 0,
            timestamp: 0,
            previous_hash: [0u8; 32],
            state_root: [0u8; 32],
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: 0,
            gas_limit: 10_000_000,
            extra_data: b"LuxTensor Genesis Block".to_vec(),
        };

        Self::new(header, vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() {
        let genesis = Block::genesis();
        assert_eq!(genesis.height(), 0);
        assert_eq!(genesis.transactions.len(), 0);
    }

    #[test]
    fn test_block_hash() {
        let genesis = Block::genesis();
        let hash = genesis.hash();
        assert_eq!(hash.len(), 32);
    }
}

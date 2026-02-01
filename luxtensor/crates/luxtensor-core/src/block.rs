use serde::{Deserialize, Serialize};
use crate::{Hash, Transaction};
use luxtensor_crypto::keccak256;

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

    // Consensus
    pub validator: [u8; 32],
    pub signature: Vec<u8>,  // 64 bytes signature

    // Metadata
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

    pub fn hash(&self) -> Hash {
        let bytes = bincode::serialize(self)
            .expect("BlockHeader serialization should never fail");
        keccak256(&bytes)
    }
}

/// Block structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn new(header: BlockHeader, transactions: Vec<Transaction>) -> Self {
        Self {
            header,
            transactions,
        }
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

// Block structure for LuxTensor blockchain
//
// Migrated from: sdk/blockchain/block.py

use crate::types::{Hash, Address, BlockNumber, Timestamp};
use crate::transaction::Transaction;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// Block header containing metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHeader {
    /// Block number (height)
    pub number: BlockNumber,
    
    /// Hash of parent block
    pub parent_hash: Hash,
    
    /// Merkle root of transactions
    pub transactions_root: Hash,
    
    /// Merkle root of state
    pub state_root: Hash,
    
    /// Merkle root of receipts
    pub receipts_root: Hash,
    
    /// Block timestamp
    pub timestamp: Timestamp,
    
    /// Address of block producer (validator)
    pub validator: Address,
    
    /// Gas limit for this block
    pub gas_limit: u64,
    
    /// Total gas used in this block
    pub gas_used: u64,
    
    /// Extra data (up to 32 bytes)
    pub extra_data: Vec<u8>,
}

impl BlockHeader {
    /// Calculate the hash of this block header
    pub fn hash(&self) -> Hash {
        let serialized = bincode::serialize(self)
            .expect("Failed to serialize block header for hashing - this should never fail for valid BlockHeader");
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
}

/// Complete block with header and transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Block header
    pub header: BlockHeader,
    
    /// List of transactions in this block
    pub transactions: Vec<Transaction>,
    
    /// Block signature by validator
    pub signature: Option<Vec<u8>>,
}

impl Block {
    /// Create a new block
    pub fn new(
        number: BlockNumber,
        parent_hash: Hash,
        validator: Address,
        transactions: Vec<Transaction>,
        state_root: Hash,
        timestamp: Timestamp,
    ) -> Self {
        let transactions_root = Self::calculate_transactions_root(&transactions);
        let receipts_root = [0u8; 32]; // Will be calculated after execution
        
        let gas_used: u64 = transactions.iter().map(|tx| tx.gas_limit).sum();
        
        let header = BlockHeader {
            number,
            parent_hash,
            transactions_root,
            state_root,
            receipts_root,
            timestamp,
            validator,
            gas_limit: 10_000_000, // Default gas limit
            gas_used,
            extra_data: Vec::new(),
        };
        
        Block {
            header,
            transactions,
            signature: None,
        }
    }
    
    /// Create genesis block (block 0)
    pub fn genesis(state_root: Hash) -> Self {
        let header = BlockHeader {
            number: 0,
            parent_hash: [0u8; 32],
            transactions_root: [0u8; 32],
            state_root,
            receipts_root: [0u8; 32],
            timestamp: 0,
            validator: [0u8; 20],
            gas_limit: 10_000_000,
            gas_used: 0,
            extra_data: Vec::new(),
        };
        
        Block {
            header,
            transactions: Vec::new(),
            signature: None,
        }
    }
    
    /// Get the hash of this block
    pub fn hash(&self) -> Hash {
        self.header.hash()
    }
    
    /// Calculate Merkle root of transactions
    fn calculate_transactions_root(transactions: &[Transaction]) -> Hash {
        if transactions.is_empty() {
            return [0u8; 32];
        }
        
        let tx_hashes: Vec<Hash> = transactions.iter().map(|tx| tx.hash()).collect();
        
        // Simple merkle root calculation (should use proper Merkle tree)
        let mut hasher = Sha256::new();
        for hash in tx_hashes {
            hasher.update(&hash);
        }
        let result = hasher.finalize();
        let mut root = [0u8; 32];
        root.copy_from_slice(&result);
        root
    }
    
    /// Sign the block with validator's private key
    pub fn sign(&mut self, _signature: Vec<u8>) {
        self.signature = Some(_signature);
    }
    
    /// Verify block signature
    pub fn verify_signature(&self) -> bool {
        // TODO: Implement proper signature verification
        self.signature.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_genesis_block() {
        let state_root = [1u8; 32];
        let genesis = Block::genesis(state_root);
        
        assert_eq!(genesis.header.number, 0);
        assert_eq!(genesis.header.parent_hash, [0u8; 32]);
        assert_eq!(genesis.transactions.len(), 0);
    }
    
    #[test]
    fn test_block_hash() {
        let state_root = [1u8; 32];
        let block1 = Block::genesis(state_root);
        let block2 = Block::genesis(state_root);
        
        // Same blocks should have same hash
        assert_eq!(block1.hash(), block2.hash());
    }
}

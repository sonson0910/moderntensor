// Transaction structure for LuxTensor blockchain
//
// Migrated from: sdk/blockchain/transaction.py

use crate::types::{Hash, Address, Nonce, Gas, Balance, Signature};
use crate::errors::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// Transaction type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    /// Transaction nonce (replay protection)
    pub nonce: Nonce,
    
    /// Sender address
    pub from: Address,
    
    /// Recipient address (None for contract creation)
    pub to: Option<Address>,
    
    /// Amount of tokens to transfer
    pub value: Balance,
    
    /// Gas limit for this transaction
    pub gas_limit: Gas,
    
    /// Gas price
    pub gas_price: u64,
    
    /// Transaction data (for contract calls or deployment)
    pub data: Vec<u8>,
    
    /// Chain ID (for replay protection across chains)
    pub chain_id: u64,
    
    /// Signature components
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Transaction {
    /// Create a new unsigned transaction
    pub fn new(
        nonce: Nonce,
        from: Address,
        to: Option<Address>,
        value: Balance,
        gas_limit: Gas,
        gas_price: u64,
        data: Vec<u8>,
        chain_id: u64,
    ) -> Self {
        Transaction {
            nonce,
            from,
            to,
            value,
            gas_limit,
            gas_price,
            data,
            chain_id,
            v: 0,
            r: [0u8; 32],
            s: [0u8; 32],
        }
    }
    
    /// Calculate transaction hash
    pub fn hash(&self) -> Hash {
        let serialized = bincode::serialize(self).expect("Serialization should not fail");
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }
    
    /// Check if this is a contract creation transaction
    pub fn is_contract_creation(&self) -> bool {
        self.to.is_none()
    }
    
    /// Calculate intrinsic gas cost
    pub fn intrinsic_gas(&self) -> Gas {
        let mut gas: Gas = 21000; // Base transaction cost
        
        // Add cost for data
        for byte in &self.data {
            if *byte == 0 {
                gas += 4; // Zero byte cost
            } else {
                gas += 16; // Non-zero byte cost
            }
        }
        
        // Add cost for contract creation
        if self.is_contract_creation() {
            gas += 32000;
        }
        
        gas
    }
    
    /// Verify transaction signature
    pub fn verify_signature(&self) -> CoreResult<()> {
        // TODO: Implement proper ECDSA signature verification
        if self.v == 0 && self.r == [0u8; 32] && self.s == [0u8; 32] {
            return Err(CoreError::InvalidSignature);
        }
        Ok(())
    }
    
    /// Sign transaction with private key
    pub fn sign(&mut self, _private_key: &[u8]) -> CoreResult<()> {
        // TODO: Implement proper ECDSA signing
        self.v = 27;
        self.r = [1u8; 32];
        self.s = [1u8; 32];
        Ok(())
    }
}

/// Transaction receipt containing execution results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    /// Transaction hash
    pub transaction_hash: Hash,
    
    /// Block number where transaction was included
    pub block_number: u64,
    
    /// Transaction index in block
    pub transaction_index: u32,
    
    /// Sender address
    pub from: Address,
    
    /// Recipient address
    pub to: Option<Address>,
    
    /// Gas used by this transaction
    pub gas_used: Gas,
    
    /// Cumulative gas used in block up to this transaction
    pub cumulative_gas_used: Gas,
    
    /// Contract address (if contract was created)
    pub contract_address: Option<Address>,
    
    /// Execution status (true = success, false = failure)
    pub status: bool,
    
    /// Event logs emitted during execution
    pub logs: Vec<Log>,
}

/// Event log emitted during transaction execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    /// Address of contract that emitted the log
    pub address: Address,
    
    /// Indexed topics
    pub topics: Vec<Hash>,
    
    /// Non-indexed data
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_transaction_creation() {
        let tx = Transaction::new(
            0,
            [1u8; 20],
            Some([2u8; 20]),
            1000,
            21000,
            1_000_000_000,
            vec![],
            1,
        );
        
        assert_eq!(tx.nonce, 0);
        assert_eq!(tx.value, 1000);
        assert!(!tx.is_contract_creation());
    }
    
    #[test]
    fn test_contract_creation() {
        let tx = Transaction::new(
            0,
            [1u8; 20],
            None,
            0,
            100000,
            1_000_000_000,
            vec![0x60, 0x80, 0x60, 0x40], // Contract bytecode
            1,
        );
        
        assert!(tx.is_contract_creation());
    }
    
    #[test]
    fn test_intrinsic_gas() {
        let tx = Transaction::new(
            0,
            [1u8; 20],
            Some([2u8; 20]),
            1000,
            21000,
            1_000_000_000,
            vec![0, 0, 1, 2],
            1,
        );
        
        // Base cost (21000) + zero bytes (2 * 4) + non-zero bytes (2 * 16)
        let expected_gas = 21000 + 8 + 32;
        assert_eq!(tx.intrinsic_gas(), expected_gas);
    }
}

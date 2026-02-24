// Comprehensive Unit Tests for luxtensor-core
// Tests for Transaction, Block, Account, State, and types

use luxtensor_core::{
    Account, Address, Block, BlockHeader, Hash, Transaction,
};
use luxtensor_crypto::keccak256;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================
// Transaction Unit Tests
// ============================================================

#[cfg(test)]
mod transaction_tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let from = Address::zero();
        let to = Some(Address::zero());

        let tx = Transaction::new(
            0,      // nonce
            from,
            to,
            1000,   // value
            1,      // gas_price
            21000,  // gas_limit
            vec![], // data
        );

        assert_eq!(tx.nonce, 0);
        assert_eq!(tx.value, 1000);
        assert_eq!(tx.gas_limit, 21000);
    }

    #[test]
    fn test_transaction_hash_deterministic() {
        let from = Address::zero();
        let to = Some(Address::zero());

        let tx1 = Transaction::new(0, from, to, 1000, 1, 21000, vec![]);
        let tx2 = Transaction::new(0, from, to, 1000, 1, 21000, vec![]);

        // Same transaction data should produce same hash
        assert_eq!(tx1.hash(), tx2.hash());
    }

    #[test]
    fn test_transaction_hash_unique() {
        let from = Address::zero();
        let to = Some(Address::zero());

        let tx1 = Transaction::new(0, from, to, 1000, 1, 21000, vec![]);
        let tx2 = Transaction::new(1, from, to, 1000, 1, 21000, vec![]); // Different nonce

        // Different nonce should produce different hash
        assert_ne!(tx1.hash(), tx2.hash());
    }

    #[test]
    fn test_transaction_with_data() {
        let from = Address::zero();
        let to = Some(Address::zero());
        let data = vec![0x60, 0x80, 0x60, 0x40]; // Sample bytecode

        let tx = Transaction::new(0, from, to, 0, 1, 100000, data.clone());

        assert_eq!(tx.data, data);
    }

    #[test]
    fn test_transaction_contract_creation() {
        let from = Address::zero();
        let data = vec![0x60, 0x80, 0x60, 0x40]; // Contract bytecode

        // Contract creation has no 'to' address
        let tx = Transaction::new(0, from, None, 0, 1, 1000000, data);

        assert!(tx.to.is_none());
    }

    #[test]
    fn test_transaction_high_value() {
        let from = Address::zero();
        let to = Some(Address::zero());
        let high_value = u128::MAX;

        let tx = Transaction::new(0, from, to, high_value, 1, 21000, vec![]);

        assert_eq!(tx.value, high_value);
    }
}

// ============================================================
// Block Unit Tests
// ============================================================

#[cfg(test)]
mod block_tests {
    use super::*;

    #[test]
    fn test_block_genesis() {
        let genesis = Block::genesis();

        assert_eq!(genesis.height(), 0);
        assert_eq!(genesis.header.previous_hash, [0u8; 32]);
    }

    #[test]
    fn test_genesis_hash_deterministic() {
        let genesis1 = Block::genesis();
        let genesis2 = Block::genesis();

        assert_eq!(genesis1.hash(), genesis2.hash());
    }

    #[test]
    fn test_block_creation() {
        let genesis = Block::genesis();

        let header = BlockHeader {
            version: 1,
            height: 1,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            previous_hash: genesis.hash(),
            state_root: [0u8; 32],
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: 0,
            gas_limit: 10_000_000,
            extra_data: vec![],
            vrf_proof: None,
        };

        let block = Block::new(header, vec![]);

        assert_eq!(block.height(), 1);
        assert_eq!(block.header.previous_hash, genesis.hash());
    }

    #[test]
    fn test_block_with_transactions() {
        let genesis = Block::genesis();
        let from = Address::zero();
        let to = Some(Address::zero());

        let txs = vec![
            Transaction::new(0, from, to, 1000, 1, 21000, vec![]),
            Transaction::new(1, from, to, 2000, 1, 21000, vec![]),
        ];

        let header = BlockHeader {
            version: 1,
            height: 1,
            timestamp: 12345,
            previous_hash: genesis.hash(),
            state_root: [0u8; 32],
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![],
            gas_used: 42000,
            gas_limit: 10_000_000,
            extra_data: vec![],
            vrf_proof: None,
        };

        let block = Block::new(header, txs);

        assert_eq!(block.transactions.len(), 2);
    }

    #[test]
    fn test_block_hash_unique() {
        let genesis = Block::genesis();

        let header1 = BlockHeader {
            version: 1,
            height: 1,
            timestamp: 12345,
            previous_hash: genesis.hash(),
            state_root: [0u8; 32],
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![],
            gas_used: 0,
            gas_limit: 10_000_000,
            extra_data: vec![],
            vrf_proof: None,
        };

        let header2 = BlockHeader {
            version: 1,
            height: 2,  // Different height
            timestamp: 12345,
            previous_hash: genesis.hash(),
            state_root: [0u8; 32],
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![],
            gas_used: 0,
            gas_limit: 10_000_000,
            extra_data: vec![],
            vrf_proof: None,
        };

        let block1 = Block::new(header1, vec![]);
        let block2 = Block::new(header2, vec![]);

        assert_ne!(block1.hash(), block2.hash());
    }
}

// ============================================================
// Account Unit Tests
// ============================================================

#[cfg(test)]
mod account_tests {
    use super::*;

    #[test]
    fn test_account_creation() {
        let account = Account {
            nonce: 0,
            balance: 1_000_000_000_000_000_000, // 1 token
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        assert_eq!(account.nonce, 0);
        assert_eq!(account.balance, 1_000_000_000_000_000_000);
    }

    #[test]
    fn test_account_empty() {
        let account = Account {
            nonce: 0,
            balance: 0,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        assert_eq!(account.balance, 0);
    }

    #[test]
    fn test_account_high_balance() {
        let account = Account {
            nonce: 0,
            balance: u128::MAX,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        assert_eq!(account.balance, u128::MAX);
    }

    #[test]
    fn test_account_nonce_increment() {
        let mut account = Account {
            nonce: 5,
            balance: 1000,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        account.nonce += 1;
        assert_eq!(account.nonce, 6);
    }
}

// ============================================================
// Address Unit Tests
// ============================================================

#[cfg(test)]
mod address_tests {
    use super::*;

    #[test]
    fn test_address_zero() {
        let zero = Address::zero();
        assert_eq!(zero.as_bytes(), &[0u8; 20]);
    }

    #[test]
    fn test_address_from_bytes() {
        let bytes = [1u8; 20];
        let addr = Address::new(bytes);
        assert_eq!(addr.as_bytes(), &bytes);
    }

    #[test]
    fn test_address_equality() {
        let addr1 = Address::new([1u8; 20]);
        let addr2 = Address::new([1u8; 20]);
        let addr3 = Address::new([2u8; 20]);

        assert_eq!(addr1, addr2);
        assert_ne!(addr1, addr3);
    }

    #[test]
    fn test_address_from_slice() {
        let slice = [5u8; 24]; // Longer than 20 bytes
        let addr = Address::from_slice(&slice);

        // Should take first 20 bytes
        let expected = [5u8; 20];
        assert_eq!(addr.as_bytes(), &expected);
    }
}

// ============================================================
// Hash Unit Tests
// ============================================================

#[cfg(test)]
mod hash_tests {
    use super::*;

    #[test]
    fn test_keccak256_empty() {
        let hash = keccak256(&[]);
        // Hash of empty input should not be all zeros
        assert!(!hash.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_keccak256_deterministic() {
        let data = b"hello world";

        let hash1 = keccak256(data);
        let hash2 = keccak256(data);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_keccak256_different_input() {
        let hash1 = keccak256(b"hello");
        let hash2 = keccak256(b"world");

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_keccak256_length() {
        let hash = keccak256(b"test");
        assert_eq!(hash.len(), 32);
    }
}

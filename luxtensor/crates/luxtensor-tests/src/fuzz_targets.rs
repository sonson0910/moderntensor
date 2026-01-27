//! Fuzzing Targets for Luxtensor
//!
//! This module provides fuzz targets for cargo-fuzz.
//!
//! Setup:
//! ```bash
//! cargo install cargo-fuzz
//! cd crates/luxtensor-tests
//! cargo +nightly fuzz run tx_parser
//! cargo +nightly fuzz run block_validator
//! cargo +nightly fuzz run rpc_input
//! ```

use luxtensor_core::{Transaction, Block, BlockHeader};
use sha3::{Keccak256, Digest};

/// Fuzz the transaction parser with arbitrary bytes
pub fn fuzz_tx_parser(data: &[u8]) -> bool {
    // Try to parse as a transaction
    match bincode::deserialize::<Transaction>(data) {
        Ok(tx) => {
            // If it parses, validate it doesn't panic on any operations
            let _ = tx.hash();
            let _ = tx.to.clone();
            let _ = tx.from.clone();
            let _ = tx.value;
            let _ = tx.nonce;
            let _ = format!("{:?}", tx);
            true
        }
        Err(_) => false, // Invalid data is expected
    }
}

/// Fuzz the block validator with arbitrary bytes
pub fn fuzz_block_validator(data: &[u8]) -> bool {
    // Try to parse as a block
    match bincode::deserialize::<Block>(data) {
        Ok(block) => {
            // If it parses, validate it doesn't panic
            let _ = block.hash.clone();
            let _ = block.header.height;
            let _ = block.header.timestamp;
            let _ = block.header.parent_hash.clone();
            let _ = block.transactions.len();
            let _ = format!("{:?}", block.header);
            true
        }
        Err(_) => false,
    }
}

/// Fuzz block header parsing
pub fn fuzz_block_header(data: &[u8]) -> bool {
    match bincode::deserialize::<BlockHeader>(data) {
        Ok(header) => {
            let _ = header.height;
            let _ = header.timestamp;
            let _ = header.parent_hash.clone();
            let _ = header.state_root.clone();
            let _ = header.transactions_root.clone();
            true
        }
        Err(_) => false,
    }
}

/// Fuzz RPC JSON parsing
pub fn fuzz_rpc_json(data: &[u8]) -> bool {
    // Try to parse as JSON-RPC request
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(s) {
            // Check for required JSON-RPC fields
            let _ = value.get("method");
            let _ = value.get("params");
            let _ = value.get("id");
            let _ = value.get("jsonrpc");
            return true;
        }
    }
    false
}

/// Fuzz address parsing (hex to bytes)
pub fn fuzz_address_parser(data: &[u8]) -> bool {
    if let Ok(s) = std::str::from_utf8(data) {
        // Try to parse as hex address
        let trimmed = s.trim_start_matches("0x");
        if let Ok(bytes) = hex::decode(trimmed) {
            // Valid hex, check if it's address length
            if bytes.len() == 20 {
                // Valid address length
                let _ = format!("0x{}", hex::encode(&bytes));
                return true;
            }
        }
    }
    false
}

/// Fuzz hash computation
pub fn fuzz_keccak256(data: &[u8]) -> bool {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();

    // Verify determinism
    let mut hasher2 = Keccak256::new();
    hasher2.update(data);
    let result2 = hasher2.finalize();

    result == result2
}

/// Fuzz numeric overflow in value parsing
pub fn fuzz_value_parser(data: &[u8]) -> bool {
    if data.len() >= 8 {
        // Try to parse as u64
        let value = u64::from_le_bytes([
            data[0], data[1], data[2], data[3],
            data[4], data[5], data[6], data[7],
        ]);

        // Test arithmetic operations don't panic
        let _ = value.saturating_add(1);
        let _ = value.saturating_sub(1);
        let _ = value.saturating_mul(2);
        let _ = value.checked_add(u64::MAX);
        let _ = value.checked_mul(u64::MAX);

        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz_tx_parser_with_random() {
        // Test with random bytes
        let random_bytes = vec![0u8; 256];
        let _ = fuzz_tx_parser(&random_bytes);

        let random_bytes2: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let _ = fuzz_tx_parser(&random_bytes2);
    }

    #[test]
    fn test_fuzz_block_with_random() {
        let random_bytes = vec![255u8; 512];
        let _ = fuzz_block_validator(&random_bytes);
    }

    #[test]
    fn test_fuzz_rpc_json() {
        // Valid JSON-RPC
        let valid = r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#;
        assert!(fuzz_rpc_json(valid.as_bytes()));

        // Invalid JSON
        let invalid = "not json at all {{{{";
        assert!(!fuzz_rpc_json(invalid.as_bytes()));
    }

    #[test]
    fn test_fuzz_address_parser() {
        // Valid address
        let valid = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
        assert!(fuzz_address_parser(valid.as_bytes()));

        // Invalid address
        let invalid = "not an address";
        assert!(!fuzz_address_parser(invalid.as_bytes()));
    }

    #[test]
    fn test_fuzz_keccak256_determinism() {
        let data = b"test data for hashing";
        assert!(fuzz_keccak256(data));
    }

    #[test]
    fn test_fuzz_value_parser() {
        let data = [0u8, 0, 0, 0, 0, 0, 0, 0];
        assert!(fuzz_value_parser(&data));

        let max_data = [255u8, 255, 255, 255, 255, 255, 255, 255];
        assert!(fuzz_value_parser(&max_data));
    }
}

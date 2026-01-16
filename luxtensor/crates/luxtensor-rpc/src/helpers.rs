// RPC helper functions for parsing parameters
// Extracted from server.rs to reduce file size

use luxtensor_core::Address;

/// Parse block number from JSON value
/// Supports: "latest", "pending", "earliest", hex string, or numeric
pub fn parse_block_number(value: &serde_json::Value) -> std::result::Result<u64, jsonrpc_core::Error> {
    match value {
        serde_json::Value::String(s) => {
            if s == "latest" || s == "pending" {
                // In real implementation, get latest block
                Ok(0)
            } else if s == "earliest" {
                Ok(0)
            } else {
                let s = s.trim_start_matches("0x");
                u64::from_str_radix(s, 16)
                    .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid block number"))
            }
        }
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid block number")),
        _ => Err(jsonrpc_core::Error::invalid_params(
            "Block number must be string or number",
        )),
    }
}

/// Parse address from hex string (with or without 0x prefix)
pub fn parse_address(s: &str) -> std::result::Result<Address, jsonrpc_core::Error> {
    let s = s.trim_start_matches("0x");
    let bytes = hex::decode(s)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid address format"))?;

    if bytes.len() != 20 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Address must be 20 bytes",
        ));
    }

    Ok(Address::from_slice(&bytes))
}

/// Parse hash from hex string (32 bytes)
pub fn parse_hash(s: &str) -> std::result::Result<[u8; 32], jsonrpc_core::Error> {
    let s = s.trim_start_matches("0x");
    let bytes = hex::decode(s)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hash format"))?;

    if bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Hash must be 32 bytes",
        ));
    }

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

/// Parse amount from hex string
pub fn parse_amount(s: &str) -> std::result::Result<u128, jsonrpc_core::Error> {
    let s = s.trim_start_matches("0x");
    u128::from_str_radix(s, 16)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid amount format"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_block_number() {
        // Hex string
        let val = serde_json::json!("0x10");
        assert_eq!(parse_block_number(&val).unwrap(), 16);

        // "latest"
        let val = serde_json::json!("latest");
        assert_eq!(parse_block_number(&val).unwrap(), 0);

        // Number
        let val = serde_json::json!(42);
        assert_eq!(parse_block_number(&val).unwrap(), 42);
    }

    #[test]
    fn test_parse_address() {
        let addr = parse_address("0x0000000000000000000000000000000000000001").unwrap();
        assert_eq!(addr.as_bytes()[19], 1);
    }

    #[test]
    fn test_parse_address_invalid() {
        assert!(parse_address("invalid").is_err());
        assert!(parse_address("0x1234").is_err()); // Too short
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("0x64").unwrap(), 100);
        assert_eq!(parse_amount("1000").unwrap(), 4096);
    }
}

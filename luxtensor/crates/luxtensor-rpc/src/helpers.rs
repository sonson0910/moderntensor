// RPC helper functions for parsing parameters
// Extracted from server.rs to reduce file size
// Security: Added signature verification for RPC authentication

use luxtensor_core::Address;
use luxtensor_crypto::{keccak256, recover_public_key, address_from_public_key};

/// Parse block number from JSON value
/// Supports: "latest", "pending", "earliest", hex string, or numeric
///
/// # Arguments
/// * `value` - JSON value containing the block number tag or literal
/// * `latest_height` - The current chain tip height used to resolve "latest" and "pending" tags
pub fn parse_block_number_with_latest(
    value: &serde_json::Value,
    latest_height: u64,
) -> std::result::Result<u64, jsonrpc_core::Error> {
    match value {
        serde_json::Value::String(s) => {
            if s == "latest" || s == "pending" {
                Ok(latest_height)
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

/// Parse block number from JSON value.
///
/// **DEPRECATED**: Use [`parse_block_number_with_latest`] for correct "latest"/"pending" resolution.
/// This function resolves "latest" to 0 (genesis), which is incorrect for most use cases.
/// It is retained only for backward compatibility with callers that resolve the tag externally.
pub fn parse_block_number(value: &serde_json::Value) -> std::result::Result<u64, jsonrpc_core::Error> {
    parse_block_number_with_latest(value, 0)
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

/// Verify that caller owns the address by recovering signer from signature
///
/// # Security
/// This function prevents impersonation attacks by verifying that the caller
/// actually owns the private key corresponding to the claimed address.
///
/// # Arguments
/// * `claimed_address` - The address the caller claims to own
/// * `message` - The message that was signed (must include nonce/timestamp for replay protection)
/// * `signature_hex` - The signature in hex format (65 bytes: r(32) + s(32) + v(1))
/// * `recovery_id` - The recovery ID (v value, typically 27 or 28, we use 0 or 1)
///
/// # Returns
/// * `Ok(())` if signature is valid and matches claimed address
/// * `Err` if signature is invalid or doesn't match
pub fn verify_caller_signature(
    claimed_address: &Address,
    message: &str,
    signature_hex: &str,
    recovery_id: u8,
) -> std::result::Result<(), jsonrpc_core::Error> {
    // Parse signature bytes
    let sig_hex = signature_hex.trim_start_matches("0x");
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid signature hex format"))?;

    if sig_bytes.len() != 64 && sig_bytes.len() != 65 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Signature must be 64 or 65 bytes"
        ));
    }

    // Get signature portion (64 bytes)
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes[..64]);

    // Hash the message (Ethereum personal_sign format)
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let prefixed_msg = [prefix.as_bytes(), message.as_bytes()].concat();
    let message_hash = keccak256(&prefixed_msg);

    // Recover public key from signature
    let public_key = recover_public_key(&message_hash, &sig_arr, recovery_id)
        .map_err(|e| jsonrpc_core::Error::invalid_params(
            format!("Failed to recover public key: {:?}", e)
        ))?;

    // Derive address from recovered public key
    let recovered_address = address_from_public_key(&public_key)
        .map_err(|e| jsonrpc_core::Error::invalid_params(
            format!("Failed to derive address: {:?}", e)
        ))?;

    // Compare addresses
    if recovered_address.as_bytes() != claimed_address.as_bytes() {
        return Err(jsonrpc_core::Error::invalid_params(
            "Signature does not match claimed address"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_block_number() {
        // Hex string
        let val = serde_json::json!("0x10");
        assert_eq!(parse_block_number(&val).unwrap(), 16);

        // "latest" with default (backward compat)
        let val = serde_json::json!("latest");
        assert_eq!(parse_block_number(&val).unwrap(), 0);

        // Number
        let val = serde_json::json!(42);
        assert_eq!(parse_block_number(&val).unwrap(), 42);
    }

    #[test]
    fn test_parse_block_number_with_latest() {
        // "latest" resolves to the provided height
        let val = serde_json::json!("latest");
        assert_eq!(parse_block_number_with_latest(&val, 12345).unwrap(), 12345);

        // "pending" resolves the same way
        let val = serde_json::json!("pending");
        assert_eq!(parse_block_number_with_latest(&val, 9999).unwrap(), 9999);

        // "earliest" is always 0 regardless of latest_height
        let val = serde_json::json!("earliest");
        assert_eq!(parse_block_number_with_latest(&val, 12345).unwrap(), 0);

        // Hex number ignores latest_height
        let val = serde_json::json!("0xff");
        assert_eq!(parse_block_number_with_latest(&val, 12345).unwrap(), 255);
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

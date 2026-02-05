// RPC validation module for input sanitization and size limits
// Prevents malformed requests and DoS attacks

use serde_json::Value;

/// RPC request limits
pub struct RpcLimits {
    /// Maximum request body size in bytes
    pub max_request_size: usize,
    /// Maximum array parameter length
    pub max_array_length: usize,
    /// Maximum string parameter length
    pub max_string_length: usize,
    /// Maximum hex string length (for tx data)
    pub max_hex_length: usize,
}

impl Default for RpcLimits {
    fn default() -> Self {
        Self {
            max_request_size: 1024 * 1024,     // 1 MB
            max_array_length: 1000,
            max_string_length: 256,
            max_hex_length: 1024 * 128,        // 128 KB for contract data
        }
    }
}

/// Validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub message: String,
}

impl ValidationError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl From<ValidationError> for jsonrpc_core::Error {
    fn from(e: ValidationError) -> Self {
        jsonrpc_core::Error::invalid_params(e.message)
    }
}

/// Validate an Ethereum-style address (0x + 40 hex chars)
pub fn validate_address(s: &str) -> Result<[u8; 20], ValidationError> {
    let s = s.trim();

    if !s.starts_with("0x") && !s.starts_with("0X") {
        return Err(ValidationError::new("Address must start with 0x"));
    }

    let hex_part = &s[2..];
    if hex_part.len() != 40 {
        return Err(ValidationError::new("Address must be 40 hex characters"));
    }

    let mut bytes = [0u8; 20];
    hex::decode_to_slice(hex_part, &mut bytes)
        .map_err(|_| ValidationError::new("Invalid hex in address"))?;

    Ok(bytes)
}

/// Validate a 32-byte hash (0x + 64 hex chars)
pub fn validate_hash(s: &str) -> Result<[u8; 32], ValidationError> {
    let s = s.trim();

    if !s.starts_with("0x") && !s.starts_with("0X") {
        return Err(ValidationError::new("Hash must start with 0x"));
    }

    let hex_part = &s[2..];
    if hex_part.len() != 64 {
        return Err(ValidationError::new("Hash must be 64 hex characters"));
    }

    let mut bytes = [0u8; 32];
    hex::decode_to_slice(hex_part, &mut bytes)
        .map_err(|_| ValidationError::new("Invalid hex in hash"))?;

    Ok(bytes)
}

/// Validate hex data (variable length)
pub fn validate_hex_data(s: &str, limits: &RpcLimits) -> Result<Vec<u8>, ValidationError> {
    let s = s.trim();

    if !s.starts_with("0x") && !s.starts_with("0X") {
        return Err(ValidationError::new("Hex data must start with 0x"));
    }

    let hex_part = &s[2..];
    if hex_part.len() > limits.max_hex_length {
        return Err(ValidationError::new(format!(
            "Hex data too long: {} > {}",
            hex_part.len(),
            limits.max_hex_length
        )));
    }

    hex::decode(hex_part)
        .map_err(|_| ValidationError::new("Invalid hex data"))
}

/// Validate u64 from hex string
pub fn validate_u64_hex(s: &str) -> Result<u64, ValidationError> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");

    u64::from_str_radix(s, 16)
        .map_err(|_| ValidationError::new("Invalid u64 hex value"))
}

/// Validate u128 from hex string
pub fn validate_u128_hex(s: &str) -> Result<u128, ValidationError> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");

    u128::from_str_radix(s, 16)
        .map_err(|_| ValidationError::new("Invalid u128 hex value"))
}

/// Validate block number parameter
pub fn validate_block_number(value: &Value) -> Result<u64, ValidationError> {
    match value {
        Value::String(s) => {
            match s.as_str() {
                "latest" | "pending" => Ok(u64::MAX), // Special handling needed
                "earliest" => Ok(0),
                _ => validate_u64_hex(s),
            }
        }
        Value::Number(n) => {
            n.as_u64().ok_or_else(|| ValidationError::new("Invalid block number"))
        }
        _ => Err(ValidationError::new("Block number must be string or number")),
    }
}

/// Validate array length
pub fn validate_array_length<T>(arr: &[T], limits: &RpcLimits) -> Result<(), ValidationError> {
    if arr.len() > limits.max_array_length {
        return Err(ValidationError::new(format!(
            "Array too long: {} > {}",
            arr.len(),
            limits.max_array_length
        )));
    }
    Ok(())
}

/// Validate string length
pub fn validate_string_length(s: &str, limits: &RpcLimits) -> Result<(), ValidationError> {
    if s.len() > limits.max_string_length {
        return Err(ValidationError::new(format!(
            "String too long: {} > {}",
            s.len(),
            limits.max_string_length
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address() {
        // Valid address
        let addr = validate_address("0x0000000000000000000000000000000000000000");
        assert!(addr.is_ok());

        // Invalid - no 0x prefix
        let addr = validate_address("0000000000000000000000000000000000000000");
        assert!(addr.is_err());

        // Invalid - wrong length
        let addr = validate_address("0x00");
        assert!(addr.is_err());
    }

    #[test]
    fn test_validate_hash() {
        // Valid hash
        let hash = validate_hash("0x0000000000000000000000000000000000000000000000000000000000000000");
        assert!(hash.is_ok());

        // Invalid - wrong length
        let hash = validate_hash("0x00");
        assert!(hash.is_err());
    }

    #[test]
    fn test_validate_u64_hex() {
        assert_eq!(validate_u64_hex("0x10").unwrap(), 16);
        assert_eq!(validate_u64_hex("0xff").unwrap(), 255);
        assert_eq!(validate_u64_hex("100").unwrap(), 256);
    }

    #[test]
    fn test_validate_block_number() {
        let _limits = RpcLimits::default();

        assert_eq!(validate_block_number(&Value::String("earliest".into())).unwrap(), 0);
        assert_eq!(validate_block_number(&Value::String("0x10".into())).unwrap(), 16);
        assert_eq!(validate_block_number(&Value::Number(100.into())).unwrap(), 100);
    }
}

//! RLP encoding, parsing, and cryptographic helpers for the CLI.
//!
//! These utilities are used by the command handlers to build and sign
//! transactions, parse hex values, and manage keystores.

use anyhow::Result;

// ============================================================
// RLP encoding helpers
// ============================================================

/// Trim leading zero bytes from a byte slice.
pub fn trim_leading_zeros(data: &[u8]) -> &[u8] {
    let start = data.iter().position(|&b| b != 0).unwrap_or(data.len());
    &data[start..]
}

/// Convert u64 to big-endian bytes with leading zeros trimmed.
pub fn u64_to_be_trimmed(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![];
    }
    trim_leading_zeros(&val.to_be_bytes()).to_vec()
}

/// RLP encode a byte string.
pub fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return data.to_vec();
    }
    if data.is_empty() {
        return vec![0x80];
    }
    if data.len() <= 55 {
        let mut out = vec![0x80 + data.len() as u8];
        out.extend_from_slice(data);
        out
    } else {
        let len_bytes = u64_to_be_trimmed(data.len() as u64);
        let mut out = vec![0xb7 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(data);
        out
    }
}

/// RLP encode a u64 integer value.
pub fn rlp_encode_u64(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80];
    }
    rlp_encode_bytes(&u64_to_be_trimmed(val))
}

/// RLP encode a list of already-encoded items.
pub fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = Vec::new();
    for item in items {
        payload.extend_from_slice(item);
    }
    if payload.len() <= 55 {
        let mut out = vec![0xc0 + payload.len() as u8];
        out.extend_from_slice(&payload);
        out
    } else {
        let len_bytes = u64_to_be_trimmed(payload.len() as u64);
        let mut out = vec![0xf7 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(&payload);
        out
    }
}

// ============================================================
// Parsing helpers
// ============================================================

/// Parse a hex-encoded private key string into 32 bytes.
pub fn parse_private_key(hex_str: &str) -> Result<[u8; 32]> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes =
        hex::decode(hex_str).map_err(|e| anyhow::anyhow!("Invalid private key hex: {}", e))?;
    if bytes.len() != 32 {
        anyhow::bail!("Private key must be 32 bytes, got {}", bytes.len());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

/// Parse a hex-encoded u64 value (e.g., "0x5" or "0x1a2b").
pub fn parse_hex_u64(s: &str) -> Result<u64> {
    let s = s.trim_matches('"');
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    if s.is_empty() {
        return Ok(0);
    }
    Ok(u64::from_str_radix(s, 16)?)
}

/// Parse a wei amount from a decimal or hex string into trimmed big-endian bytes.
pub fn parse_wei_amount(s: &str) -> Result<Vec<u8>> {
    if let Some(hex_str) = s.strip_prefix("0x") {
        let bytes = hex::decode(hex_str)?;
        Ok(trim_leading_zeros(&bytes).to_vec())
    } else {
        let val: u128 = s.parse().map_err(|e| anyhow::anyhow!("Invalid amount '{}': {}", s, e))?;
        if val == 0 {
            return Ok(vec![]);
        }
        Ok(trim_leading_zeros(&val.to_be_bytes()).to_vec())
    }
}

// ============================================================
// Cryptographic helpers
// ============================================================

/// Legacy KDF: iterated keccak256 (kept for backward compatibility with v1 keystores).
pub fn derive_key_legacy(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = luxtensor_crypto::keccak256(&[password, salt].concat());
    for _ in 1..iterations {
        key = luxtensor_crypto::keccak256(&key);
    }
    key
}

/// Derive a 32-byte encryption key using scrypt KDF (secure replacement for iterated keccak256).
pub fn derive_key_scrypt(password: &[u8], salt: &[u8]) -> Result<[u8; 32]> {
    // scrypt params: log_n=14 (N=16384), r=8, p=1 — matches Ethereum keystore v3 defaults
    let params = scrypt::Params::new(14, 8, 1, 32)
        .map_err(|e| anyhow::anyhow!("Invalid scrypt params: {}", e))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(password, salt, &params, &mut key)
        .map_err(|e| anyhow::anyhow!("scrypt KDF failed: {}", e))?;
    Ok(key)
}

/// AES-128-CTR encrypt/decrypt (symmetric — same operation for both directions).
pub fn aes128_ctr_apply(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    use aes::cipher::{KeyIvInit, StreamCipher};
    type Aes128Ctr = ctr::Ctr128BE<aes::Aes128>;

    let mut cipher = Aes128Ctr::new_from_slices(key, iv)
        .map_err(|e| anyhow::anyhow!("AES-128-CTR init failed: {}", e))?;
    let mut buffer = data.to_vec();
    cipher.apply_keystream(&mut buffer);
    Ok(buffer)
}

/// Constant-time comparison of two byte slices to prevent timing side-channel attacks.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Read a private key from CLI argument, environment variable, or interactive prompt.
/// Priority: CLI arg > LUXTENSOR_PRIVATE_KEY env var > interactive prompt (hidden input).
pub fn read_private_key(cli_value: Option<String>) -> Result<String> {
    if let Some(key) = cli_value {
        eprintln!("\u{26a0}\u{fe0f}  WARNING: Passing private keys via CLI arguments is insecure.");
        eprintln!("   Your key may be visible in shell history and process listings.");
        eprintln!("   Consider using LUXTENSOR_PRIVATE_KEY env var or interactive prompt instead.");
        return Ok(key);
    }

    if let Ok(key) = std::env::var("LUXTENSOR_PRIVATE_KEY") {
        if !key.is_empty() {
            eprintln!(
                "\u{1f511} Using private key from LUXTENSOR_PRIVATE_KEY environment variable."
            );
            return Ok(key);
        }
    }

    let key = rpassword::prompt_password("\u{1f511} Enter private key (hex): ")
        .map_err(|e| anyhow::anyhow!("Failed to read private key: {}", e))?;

    if key.trim().is_empty() {
        anyhow::bail!("Private key cannot be empty");
    }

    Ok(key.trim().to_string())
}

// ============================================================
// RPC client
// ============================================================

/// Make a JSON-RPC call to the LuxTensor node.
pub async fn rpc_call(
    rpc: &str,
    method: &str,
    params: Vec<serde_json::Value>,
) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });

    let resp: serde_json::Value = client.post(rpc).json(&body).send().await?.json().await?;

    if let Some(error) = resp.get("error") {
        anyhow::bail!("RPC error: {}", error);
    }

    Ok(resp.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_leading_zeros() {
        assert_eq!(trim_leading_zeros(&[0, 0, 1, 2]), &[1, 2]);
        assert_eq!(trim_leading_zeros(&[0, 0, 0, 0]), &[] as &[u8]);
        assert_eq!(trim_leading_zeros(&[1, 2, 3]), &[1, 2, 3]);
    }

    #[test]
    fn test_u64_to_be_trimmed() {
        assert_eq!(u64_to_be_trimmed(0), Vec::<u8>::new());
        assert_eq!(u64_to_be_trimmed(1), vec![1]);
        assert_eq!(u64_to_be_trimmed(256), vec![1, 0]);
    }

    #[test]
    fn test_rlp_encode_u64() {
        assert_eq!(rlp_encode_u64(0), vec![0x80]);
        assert_eq!(rlp_encode_u64(1), vec![0x01]);
        assert_eq!(rlp_encode_u64(0x7f), vec![0x7f]);
        assert_eq!(rlp_encode_u64(0x80), vec![0x81, 0x80]);
    }

    #[test]
    fn test_parse_private_key_valid() {
        let hex = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        assert!(parse_private_key(hex).is_ok());
        assert!(parse_private_key(&format!("0x{}", hex)).is_ok());
    }

    #[test]
    fn test_parse_private_key_invalid() {
        assert!(parse_private_key("tooshort").is_err());
        assert!(parse_private_key("zzzz").is_err());
    }

    #[test]
    fn test_parse_hex_u64() {
        assert_eq!(parse_hex_u64("0x5").unwrap(), 5);
        assert_eq!(parse_hex_u64("0x1a2b").unwrap(), 0x1a2b);
        assert_eq!(parse_hex_u64("").unwrap(), 0);
    }

    #[test]
    fn test_parse_wei_amount() {
        assert_eq!(parse_wei_amount("0").unwrap(), Vec::<u8>::new());
        assert_eq!(parse_wei_amount("256").unwrap(), vec![1, 0]);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(&[1, 2, 3], &[1, 2, 3]));
        assert!(!constant_time_eq(&[1, 2, 3], &[1, 2, 4]));
        assert!(!constant_time_eq(&[1, 2], &[1, 2, 3]));
    }
}

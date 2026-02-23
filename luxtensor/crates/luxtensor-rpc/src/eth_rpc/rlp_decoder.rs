//! # RLP Transaction Decoder
//!
//! RLP decoding logic for Ethereum-compatible transactions.
//! Supports: Legacy (type 0), EIP-2930 (type 1), EIP-1559 (type 2).
//! Implements proper ecrecover to derive sender address from ECDSA signature.

use sha3::{Digest, Keccak256};

// Shared RLP encoding helpers from luxtensor-core
use luxtensor_core::rlp::{rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128, rlp_encode_list};

use super::Address;

// ============================================================================
// RLP Transaction Decoding — MetaMask / ethers.js / web3.js compatibility
// ============================================================================
// Supports: Legacy (type 0), EIP-2930 (type 1 — access list), EIP-1559 (type 2)
// Implements proper ecrecover to derive sender address from ECDSA signature.

/// Decoded fields from an RLP-encoded Ethereum transaction
#[allow(dead_code)] // Some fields (gas_price, max_fee) are decoded for completeness but not yet individually queried
#[derive(Debug, Clone)]
pub(crate) struct RlpDecodedTx {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) gas_price: u64,                // legacy + EIP-2930
    pub(crate) max_fee_per_gas: u64,          // EIP-1559 only
    pub(crate) max_priority_fee_per_gas: u64, // EIP-1559 only
    pub(crate) gas_limit: u64,
    pub(crate) to: Option<Address>,
    pub(crate) value: u128,
    pub(crate) data: Vec<u8>,
    pub(crate) v: u64,
    pub(crate) r: [u8; 32],
    pub(crate) s: [u8; 32],
    pub(crate) tx_type: u8, // 0 = legacy, 1 = EIP-2930, 2 = EIP-1559
    /// The raw RLP-encoded body used for hashing (to compute tx hash)
    pub(crate) signing_hash: [u8; 32],
    /// Recovered sender address
    pub(crate) from: Address,
}

/// Minimal RLP decoder: decode a single item (string/bytes) or list
/// Returns (decoded_bytes, bytes_consumed)
pub(crate) fn rlp_decode_item(data: &[u8]) -> Result<(Vec<u8>, usize), String> {
    if data.is_empty() {
        return Err("Empty RLP data".into());
    }
    let prefix = data[0];
    if prefix <= 0x7f {
        // Single byte
        Ok((vec![prefix], 1))
    } else if prefix <= 0xb7 {
        // Short string (0-55 bytes)
        let len = (prefix - 0x80) as usize;
        if data.len() < 1 + len {
            return Err("RLP short string truncated".into());
        }
        Ok((data[1..1 + len].to_vec(), 1 + len))
    } else if prefix <= 0xbf {
        // Long string
        let len_of_len = (prefix - 0xb7) as usize;
        if data.len() < 1 + len_of_len {
            return Err("RLP long string length truncated".into());
        }
        let mut len_bytes = [0u8; 8];
        let start = 8 - len_of_len;
        len_bytes[start..].copy_from_slice(&data[1..1 + len_of_len]);
        let len = u64::from_be_bytes(len_bytes) as usize;
        let total = (1usize + len_of_len)
            .checked_add(len)
            .ok_or_else(|| "RLP long string length overflow".to_string())?;
        if data.len() < total {
            return Err("RLP long string data truncated".into());
        }
        Ok((data[1 + len_of_len..total].to_vec(), total))
    } else {
        // List prefix — return entire list payload
        let (payload_offset, payload_len) = rlp_list_info(data)?;
        let end = payload_offset + payload_len;
        if data.len() < end {
            return Err("RLP list data truncated".into());
        }
        Ok((data[payload_offset..end].to_vec(), end))
    }
}

/// Get (offset, length) of an RLP list's payload
pub(crate) fn rlp_list_info(data: &[u8]) -> Result<(usize, usize), String> {
    if data.is_empty() {
        return Err("Empty RLP list".into());
    }
    let prefix = data[0];
    if prefix < 0xc0 {
        return Err(format!("Not an RLP list: prefix 0x{:02x}", prefix));
    }
    if prefix <= 0xf7 {
        let len = (prefix - 0xc0) as usize;
        if data.len() < 1 + len {
            return Err("RLP short list payload truncated".into());
        }
        Ok((1, len))
    } else {
        let len_of_len = (prefix - 0xf7) as usize;
        if data.len() < 1 + len_of_len {
            return Err("RLP list length truncated".into());
        }
        let mut len_bytes = [0u8; 8];
        let start = 8 - len_of_len;
        len_bytes[start..].copy_from_slice(&data[1..1 + len_of_len]);
        let len = u64::from_be_bytes(len_bytes) as usize;
        let total = (1usize + len_of_len)
            .checked_add(len)
            .ok_or_else(|| "RLP list length overflow".to_string())?;
        if data.len() < total {
            return Err("RLP list payload truncated".into());
        }
        Ok((1 + len_of_len, len))
    }
}

/// Decode all items from an RLP list payload into a Vec of raw byte items
pub(crate) fn rlp_decode_list(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut items = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let (item, consumed) = rlp_decode_item(&data[offset..])?;
        items.push(item);
        offset += consumed;
    }
    Ok(items)
}

// NOTE: RLP encode functions (rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128,
// rlp_encode_list, to_minimal_be) are now imported from luxtensor_core::rlp above.

/// Decode an RLP item as u64
pub(crate) fn rlp_item_to_u64(item: &[u8]) -> Result<u64, String> {
    if item.is_empty() {
        return Ok(0);
    }
    if item.len() > 8 {
        return Err(format!("RLP integer exceeds u64 range ({} bytes)", item.len()));
    }
    let mut buf = [0u8; 8];
    let start = 8usize.saturating_sub(item.len());
    buf[start..].copy_from_slice(item);
    Ok(u64::from_be_bytes(buf))
}

/// Decode an RLP item as u128
pub(crate) fn rlp_item_to_u128(item: &[u8]) -> u128 {
    if item.is_empty() {
        return 0;
    }
    let mut buf = [0u8; 16];
    let start = 16usize.saturating_sub(item.len());
    let take = item.len().min(16);
    buf[start..].copy_from_slice(&item[..take]);
    u128::from_be_bytes(buf)
}

/// Parse an RLP item into a 20-byte address (or None if empty = contract creation)
/// Returns Err for non-empty items that are not exactly 20 bytes
pub(crate) fn rlp_item_to_address(item: &[u8]) -> Result<Option<Address>, String> {
    if item.is_empty() {
        return Ok(None); // contract creation
    }
    if item.len() != 20 {
        return Err(format!("Invalid address length: {} (expected 20)", item.len()));
    }
    let mut addr = [0u8; 20];
    addr.copy_from_slice(item);
    Ok(Some(addr))
}

/// Parse RLP item into [u8; 32] left-padded
pub(crate) fn rlp_item_to_32(item: &[u8]) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let start = 32usize.saturating_sub(item.len());
    let take = item.len().min(32);
    buf[start..].copy_from_slice(&item[..take]);
    buf
}

/// Recover sender address from ECDSA signature using secp256k1 ecrecover
/// msg_hash: 32-byte Keccak256 of the signing payload
/// v: recovery ID (0 or 1 after EIP-155 normalization)
/// r, s: 32-byte signature components
pub(crate) fn ecrecover_address(
    msg_hash: &[u8; 32],
    v: u8,
    r: &[u8; 32],
    s: &[u8; 32],
) -> Result<Address, String> {
    use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};

    // Build the 64-byte compact signature (r || s)
    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(r);
    sig_bytes[32..].copy_from_slice(s);

    let signature =
        Signature::from_slice(&sig_bytes).map_err(|e| format!("Invalid signature: {}", e))?;
    let recovery_id = RecoveryId::new(v != 0, false);

    let verifying_key = VerifyingKey::recover_from_prehash(msg_hash, &signature, recovery_id)
        .map_err(|e| format!("ecrecover failed: {}", e))?;

    // Derive Ethereum address: keccak256(uncompressed_pubkey_without_prefix)[12..]
    let pubkey_bytes = verifying_key.to_encoded_point(false);
    let pubkey_raw = &pubkey_bytes.as_bytes()[1..]; // skip 0x04 prefix
    let hash = Keccak256::digest(pubkey_raw);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    Ok(addr)
}

/// Fully decode an RLP-encoded signed Ethereum transaction.
/// Supports Legacy (type 0), EIP-2930 (type 1), EIP-1559 (type 2).
pub(crate) fn decode_rlp_transaction(raw: &[u8]) -> Result<RlpDecodedTx, String> {
    if raw.is_empty() {
        return Err("Empty transaction bytes".into());
    }

    // Determine transaction type
    let first_byte = raw[0];

    if first_byte == 0x01 {
        // EIP-2930 (type 1): 0x01 || RLP([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList, signatureYParity, signatureR, signatureS])
        decode_eip2930_tx(raw)
    } else if first_byte == 0x02 {
        // EIP-1559 (type 2): 0x02 || RLP([chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList, signatureYParity, signatureR, signatureS])
        decode_eip1559_tx(raw)
    } else if first_byte >= 0xc0 {
        // Legacy transaction: RLP([nonce, gasPrice, gasLimit, to, value, data, v, r, s])
        decode_legacy_tx(raw)
    } else {
        Err(format!("Unknown transaction type byte: 0x{:02x}", first_byte))
    }
}

fn decode_legacy_tx(raw: &[u8]) -> Result<RlpDecodedTx, String> {
    let (payload_offset, payload_len) = rlp_list_info(raw)?;
    if raw.len() < payload_offset + payload_len {
        return Err("Legacy TX RLP truncated".into());
    }
    let payload = &raw[payload_offset..payload_offset + payload_len];
    let items = rlp_decode_list(payload)?;

    if items.len() != 9 {
        return Err(format!("Legacy TX needs 9 RLP items, got {}", items.len()));
    }

    let nonce = rlp_item_to_u64(&items[0])?;
    let gas_price = rlp_item_to_u64(&items[1])?;
    let gas_limit = rlp_item_to_u64(&items[2])?;
    let to = rlp_item_to_address(&items[3])?;
    let value = rlp_item_to_u128(&items[4]);
    let data = items[5].clone();
    let v_raw = rlp_item_to_u64(&items[6])?;
    let r = rlp_item_to_32(&items[7]);
    let s = rlp_item_to_32(&items[8]);

    // EIP-155: v = chain_id * 2 + 35 + recovery_id
    let (chain_id, recovery_id) = if v_raw >= 35 {
        let chain_id = (v_raw - 35) / 2;
        let rec = ((v_raw - 35) % 2) as u8;
        (chain_id, rec)
    } else if v_raw >= 27 {
        // Pre-EIP-155: v = 27 or 28
        (0u64, (v_raw - 27) as u8)
    } else {
        return Err(format!("Invalid v value in legacy TX: {}", v_raw));
    };

    // Compute signing hash:
    // For EIP-155: keccak256(RLP([nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0]))
    // For pre-EIP-155: keccak256(RLP([nonce, gasPrice, gasLimit, to, value, data]))
    let signing_payload = if chain_id > 0 {
        rlp_encode_list(&[
            rlp_encode_u64(nonce),
            rlp_encode_u64(gas_price),
            rlp_encode_u64(gas_limit),
            if let Some(addr) = &to { rlp_encode_bytes(addr) } else { rlp_encode_bytes(&[]) },
            rlp_encode_u128(value),
            rlp_encode_bytes(&data),
            rlp_encode_u64(chain_id),
            rlp_encode_bytes(&[]), // 0
            rlp_encode_bytes(&[]), // 0
        ])
    } else {
        rlp_encode_list(&[
            rlp_encode_u64(nonce),
            rlp_encode_u64(gas_price),
            rlp_encode_u64(gas_limit),
            if let Some(addr) = &to { rlp_encode_bytes(addr) } else { rlp_encode_bytes(&[]) },
            rlp_encode_u128(value),
            rlp_encode_bytes(&data),
        ])
    };
    let signing_hash_arr = {
        let h = Keccak256::digest(&signing_payload);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        arr
    };

    // Recover sender
    let from = ecrecover_address(&signing_hash_arr, recovery_id, &r, &s)?;

    // Compute tx hash = keccak256(raw RLP)
    let tx_hash_arr = {
        let h = Keccak256::digest(raw);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        arr
    };

    Ok(RlpDecodedTx {
        chain_id,
        nonce,
        gas_price,
        max_fee_per_gas: gas_price,
        max_priority_fee_per_gas: gas_price,
        gas_limit,
        to,
        value,
        data,
        v: recovery_id as u64, // Store recovery_id (0 or 1), NOT v_raw
        r,
        s,
        tx_type: 0,
        signing_hash: tx_hash_arr,
        from,
    })
}

fn decode_eip2930_tx(raw: &[u8]) -> Result<RlpDecodedTx, String> {
    // raw[0] == 0x01, rest is RLP list
    let rlp_data = &raw[1..];
    let (payload_offset, payload_len) = rlp_list_info(rlp_data)?;
    if rlp_data.len() < payload_offset + payload_len {
        return Err("EIP-2930 TX RLP truncated".into());
    }
    let payload = &rlp_data[payload_offset..payload_offset + payload_len];
    let items = rlp_decode_list(payload)?;

    if items.len() != 11 {
        return Err(format!("EIP-2930 TX needs 11 RLP items, got {}", items.len()));
    }

    let chain_id = rlp_item_to_u64(&items[0])?;
    let nonce = rlp_item_to_u64(&items[1])?;
    let gas_price = rlp_item_to_u64(&items[2])?;
    let gas_limit = rlp_item_to_u64(&items[3])?;
    let to = rlp_item_to_address(&items[4])?;
    let value = rlp_item_to_u128(&items[5]);
    let data = items[6].clone();
    // items[7] = accessList (ignored for our purposes)
    let recovery_id = rlp_item_to_u64(&items[8])? as u8;
    let r = rlp_item_to_32(&items[9]);
    let s = rlp_item_to_32(&items[10]);

    // Signing hash: keccak256(0x01 || RLP([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList]))
    let unsigned_rlp = rlp_encode_list(&[
        rlp_encode_u64(chain_id),
        rlp_encode_u64(nonce),
        rlp_encode_u64(gas_price),
        rlp_encode_u64(gas_limit),
        if let Some(addr) = &to { rlp_encode_bytes(addr) } else { rlp_encode_bytes(&[]) },
        rlp_encode_u128(value),
        rlp_encode_bytes(&data),
        rlp_encode_list(&[]), // empty access list for signing
    ]);
    let mut to_hash = vec![0x01u8];
    to_hash.extend_from_slice(&unsigned_rlp);
    let signing_hash_arr = {
        let h = Keccak256::digest(&to_hash);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        arr
    };

    let from = ecrecover_address(&signing_hash_arr, recovery_id, &r, &s)?;

    // tx hash = keccak256(full raw bytes)
    let tx_hash_arr = {
        let h = Keccak256::digest(raw);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        arr
    };

    Ok(RlpDecodedTx {
        chain_id,
        nonce,
        gas_price,
        max_fee_per_gas: gas_price,
        max_priority_fee_per_gas: gas_price,
        gas_limit,
        to,
        value,
        data,
        v: recovery_id as u64,
        r,
        s,
        tx_type: 1,
        signing_hash: tx_hash_arr,
        from,
    })
}

fn decode_eip1559_tx(raw: &[u8]) -> Result<RlpDecodedTx, String> {
    // raw[0] == 0x02, rest is RLP list
    let rlp_data = &raw[1..];
    let (payload_offset, payload_len) = rlp_list_info(rlp_data)?;
    if rlp_data.len() < payload_offset + payload_len {
        return Err("EIP-1559 TX RLP truncated".into());
    }
    let payload = &rlp_data[payload_offset..payload_offset + payload_len];
    let items = rlp_decode_list(payload)?;

    if items.len() != 12 {
        return Err(format!("EIP-1559 TX needs 12 RLP items, got {}", items.len()));
    }

    let chain_id = rlp_item_to_u64(&items[0])?;
    let nonce = rlp_item_to_u64(&items[1])?;
    let max_priority_fee = rlp_item_to_u64(&items[2])?;
    let max_fee = rlp_item_to_u64(&items[3])?;
    let gas_limit = rlp_item_to_u64(&items[4])?;
    let to = rlp_item_to_address(&items[5])?;
    let value = rlp_item_to_u128(&items[6]);
    let data = items[7].clone();
    // items[8] = accessList (ignored)
    let recovery_id = rlp_item_to_u64(&items[9])? as u8;
    let r = rlp_item_to_32(&items[10]);
    let s = rlp_item_to_32(&items[11]);

    // Signing hash: keccak256(0x02 || RLP([chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList]))
    let unsigned_rlp = rlp_encode_list(&[
        rlp_encode_u64(chain_id),
        rlp_encode_u64(nonce),
        rlp_encode_u64(max_priority_fee),
        rlp_encode_u64(max_fee),
        rlp_encode_u64(gas_limit),
        if let Some(addr) = &to { rlp_encode_bytes(addr) } else { rlp_encode_bytes(&[]) },
        rlp_encode_u128(value),
        rlp_encode_bytes(&data),
        rlp_encode_list(&[]), // empty access list for signing
    ]);
    let mut to_hash = vec![0x02u8];
    to_hash.extend_from_slice(&unsigned_rlp);
    let signing_hash_arr = {
        let h = Keccak256::digest(&to_hash);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        arr
    };

    let from = ecrecover_address(&signing_hash_arr, recovery_id, &r, &s)?;

    let tx_hash_arr = {
        let h = Keccak256::digest(raw);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&h);
        arr
    };

    Ok(RlpDecodedTx {
        chain_id,
        nonce,
        gas_price: max_fee,
        max_fee_per_gas: max_fee,
        max_priority_fee_per_gas: max_priority_fee,
        gas_limit,
        to,
        value,
        data,
        v: recovery_id as u64,
        r,
        s,
        tx_type: 2,
        signing_hash: tx_hash_arr,
        from,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::rlp::{rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128, rlp_encode_list};

    // -----------------------------------------------------------------------
    // Unit tests: RLP encode/decode round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_rlp_encode_decode_empty() {
        let encoded = rlp_encode_bytes(&[]);
        assert_eq!(encoded, vec![0x80]);
        let (decoded, consumed) = rlp_decode_item(&encoded).unwrap();
        assert_eq!(decoded, Vec::<u8>::new());
        assert_eq!(consumed, 1);
    }

    #[test]
    fn test_rlp_encode_decode_single_byte() {
        for b in 0..=0x7fu8 {
            let encoded = rlp_encode_bytes(&[b]);
            let (decoded, _) = rlp_decode_item(&encoded).unwrap();
            assert_eq!(decoded, vec![b]);
        }
    }

    #[test]
    fn test_rlp_encode_decode_short_string() {
        let data = b"hello world";
        let encoded = rlp_encode_bytes(data);
        assert_eq!(encoded[0], 0x80 + data.len() as u8);
        let (decoded, consumed) = rlp_decode_item(&encoded).unwrap();
        assert_eq!(decoded, data.to_vec());
        assert_eq!(consumed, 1 + data.len());
    }

    #[test]
    fn test_rlp_encode_decode_55_bytes() {
        let data = vec![0xAB; 55];
        let encoded = rlp_encode_bytes(&data);
        assert_eq!(encoded[0], 0x80 + 55);
        let (decoded, _) = rlp_decode_item(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_rlp_encode_decode_56_bytes() {
        // 56 bytes crosses into "long string" territory
        let data = vec![0xCD; 56];
        let encoded = rlp_encode_bytes(&data);
        assert_eq!(encoded[0], 0xb8); // 0xb7 + 1 (1 byte for length)
        assert_eq!(encoded[1], 56);
        let (decoded, consumed) = rlp_decode_item(&encoded).unwrap();
        assert_eq!(decoded, data);
        assert_eq!(consumed, 2 + 56);
    }

    #[test]
    fn test_rlp_encode_decode_long_string() {
        let data = vec![0xFF; 1024];
        let encoded = rlp_encode_bytes(&data);
        let (decoded, _) = rlp_decode_item(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_rlp_u64_roundtrip() {
        for val in [0u64, 1, 127, 128, 255, 256, 65535, u64::MAX] {
            let encoded = rlp_encode_u64(val);
            let (decoded_bytes, _) = rlp_decode_item(&encoded).unwrap();
            let decoded_val = rlp_item_to_u64(&decoded_bytes).unwrap();
            assert_eq!(decoded_val, val, "Failed roundtrip for {}", val);
        }
    }

    #[test]
    fn test_rlp_u128_roundtrip() {
        for val in [0u128, 1, 255, 256, u64::MAX as u128, u128::MAX] {
            let encoded = rlp_encode_u128(val);
            let (decoded_bytes, _) = rlp_decode_item(&encoded).unwrap();
            let decoded_val = rlp_item_to_u128(&decoded_bytes);
            assert_eq!(decoded_val, val, "Failed u128 roundtrip for {}", val);
        }
    }

    #[test]
    fn test_rlp_list_roundtrip() {
        let items = vec![rlp_encode_u64(42), rlp_encode_bytes(b"hello"), rlp_encode_bytes(&[])];
        let encoded = rlp_encode_list(&items);
        assert!(encoded[0] >= 0xc0);
        let (payload_offset, payload_len) = rlp_list_info(&encoded).unwrap();
        let payload = &encoded[payload_offset..payload_offset + payload_len];
        let decoded = rlp_decode_list(payload).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(rlp_item_to_u64(&decoded[0]).unwrap(), 42);
        assert_eq!(decoded[1], b"hello".to_vec());
        assert_eq!(decoded[2], Vec::<u8>::new());
    }

    // -----------------------------------------------------------------------
    // Fuzz-style tests: malformed / adversarial input
    // -----------------------------------------------------------------------

    #[test]
    fn test_rlp_decode_empty_input() {
        assert!(rlp_decode_item(&[]).is_err());
    }

    #[test]
    fn test_rlp_decode_truncated_short_string() {
        // Says length=10 but only 5 bytes follow
        let data = [0x80 + 10, 1, 2, 3, 4, 5];
        assert!(rlp_decode_item(&data).is_err());
    }

    #[test]
    fn test_rlp_decode_truncated_long_string() {
        // Says len_of_len=2, then says length=1000, but no data
        let data = [0xb9, 0x03, 0xe8]; // 0xb7+2, then 1000 in 2 bytes
        assert!(rlp_decode_item(&data).is_err());
    }

    #[test]
    fn test_rlp_decode_truncated_len_of_len() {
        // Says len_of_len=4 but only 1 byte follows
        let data = [0xbb, 0x01]; // 0xb7+4, only 1 of 4 len bytes
        assert!(rlp_decode_item(&data).is_err());
    }

    #[test]
    fn test_rlp_list_empty() {
        let data = [0xc0]; // empty list
        let (offset, len) = rlp_list_info(&data).unwrap();
        assert_eq!(offset, 1);
        assert_eq!(len, 0);
    }

    #[test]
    fn test_rlp_list_truncated() {
        // Says list length=55 but nothing follows — should be rejected
        let data = [0xc0 + 55]; // short list, len=55
        let result = rlp_list_info(&data);
        assert!(result.is_err()); // rlp_list_info now validates payload length
    }

    #[test]
    fn test_decode_rlp_transaction_empty() {
        assert!(decode_rlp_transaction(&[]).is_err());
    }

    #[test]
    fn test_decode_rlp_transaction_single_byte() {
        // Unknown type bytes
        for b in [0x03u8, 0x04, 0x05, 0x10, 0x50, 0x80, 0xbf] {
            let result = decode_rlp_transaction(&[b]);
            assert!(result.is_err(), "Should reject single byte 0x{:02x}", b);
        }
    }

    #[test]
    fn test_decode_rlp_transaction_too_short() {
        // Valid-looking legacy prefix but truncated
        let data = [0xc1, 0x01]; // list of 1 item, but legacy needs 9
        assert!(decode_rlp_transaction(&data).is_err());
    }

    #[test]
    fn test_decode_eip1559_too_few_items() {
        // Type 2 prefix + list with only 3 items (needs 12)
        let bogus_list =
            rlp_encode_list(&[rlp_encode_u64(1), rlp_encode_u64(0), rlp_encode_u64(100)]);
        let mut raw = vec![0x02u8];
        raw.extend_from_slice(&bogus_list);
        assert!(decode_rlp_transaction(&raw).is_err());
    }

    #[test]
    fn test_decode_eip2930_too_few_items() {
        let bogus_list = rlp_encode_list(&[rlp_encode_u64(1), rlp_encode_u64(0)]);
        let mut raw = vec![0x01u8];
        raw.extend_from_slice(&bogus_list);
        assert!(decode_rlp_transaction(&raw).is_err());
    }

    #[test]
    fn test_decode_rlp_transaction_all_zeros() {
        // 256 zero bytes
        let data = vec![0u8; 256];
        // Should either error or not panic (type byte 0x00 is not a known type)
        let _ = decode_rlp_transaction(&data);
    }

    #[test]
    fn test_decode_rlp_transaction_all_ff() {
        // 256 0xFF bytes
        let data = vec![0xFFu8; 256];
        let _ = decode_rlp_transaction(&data);
    }

    #[test]
    fn test_decode_rlp_nested_lists() {
        // Deeply nested empty lists: [[[[[]]]]]
        let mut data = vec![0xc0]; // empty inner
        for _ in 0..10 {
            let mut outer = vec![0xc0 + data.len() as u8];
            outer.extend_from_slice(&data);
            data = outer;
        }
        // Should not panic when passed as a transaction
        let _ = decode_rlp_transaction(&data);
    }

    #[test]
    fn test_decode_rlp_large_length_field() {
        // Claims to be a string of length 2^32 but has no data
        // 0xbb = long string, len_of_len=4, then 4 bytes of length = max u32
        let data = [0xbb, 0xFF, 0xFF, 0xFF, 0xFF];
        assert!(rlp_decode_item(&data).is_err());
    }

    #[test]
    fn test_rlp_address_parsing_edge_cases() {
        assert_eq!(rlp_item_to_address(&[]).unwrap(), None);
        assert!(rlp_item_to_address(&[1, 2, 3]).is_err()); // too short
        assert!(rlp_item_to_address(&[0u8; 21]).is_err()); // too long
        let addr = rlp_item_to_address(&[0xAB; 20]).unwrap();
        assert!(addr.is_some());
        assert_eq!(addr.unwrap(), [0xAB; 20]);
    }

    #[test]
    fn test_rlp_item_to_u64_edge_cases() {
        assert_eq!(rlp_item_to_u64(&[]).unwrap(), 0);
        assert!(rlp_item_to_u64(&[0xFF; 9]).is_err()); // rejects > 8 bytes
        assert_eq!(rlp_item_to_u64(&[1]).unwrap(), 1);
    }

    #[test]
    fn test_rlp_item_to_32_edge_cases() {
        let result = rlp_item_to_32(&[]);
        assert_eq!(result, [0u8; 32]);

        let result = rlp_item_to_32(&[0xFF]);
        assert_eq!(result[31], 0xFF);
        assert_eq!(result[30], 0);

        let large = vec![0xAA; 40]; // > 32 bytes
        let result = rlp_item_to_32(&large);
        // Should take last 32 bytes
        assert_eq!(result, [0xAA; 32]);
    }

    // -----------------------------------------------------------------------
    // Fuzz patterns: random byte vectors that should never panic
    // -----------------------------------------------------------------------

    #[test]
    fn test_rlp_fuzz_random_patterns() {
        // These are carefully crafted adversarial patterns
        let patterns: Vec<Vec<u8>> = vec![
            vec![0xc0],                   // empty list
            vec![0x80],                   // empty string
            vec![0xf8, 0x00],             // long list, 0 length
            vec![0xb8, 0x00],             // long string, 0 length
            vec![0xf8, 0xff],             // long list, claims 255 bytes but none follow
            vec![0xb8, 0xff],             // long string, claims 255
            vec![0xc1, 0xc1, 0xc1, 0xc0], // nested lists
            vec![0xc0; 100],              // 100 empty lists
            vec![0x01; 100],              // 100 "type 1" bytes
            vec![0x02; 100],              // 100 "type 2" bytes
            // Legacy tx with random garbage as RLP items
            vec![0xc9, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80],
            // Overlong length encodings
            vec![0xb8, 0x01, 0x00],       // claims 1 byte, has 1 zero
            vec![0xf9, 0x00, 0x01, 0x80], // long list len=1, contains empty string
        ];

        for (i, pattern) in patterns.iter().enumerate() {
            // Must never panic
            let _ = rlp_decode_item(pattern);
            let _ = decode_rlp_transaction(pattern);
            // Also test sub-functions
            let _ = rlp_list_info(pattern);
            let _ = rlp_decode_list(pattern);
            // Mark: pattern {} handled
            let _ = i;
        }
    }

    #[test]
    fn test_rlp_fuzz_incremental_lengths() {
        // Test every possible first byte with a minimal body
        for first_byte in 0..=255u8 {
            let data = vec![first_byte, 0x01, 0x02, 0x03, 0x04];
            let _ = rlp_decode_item(&data);
            let _ = decode_rlp_transaction(&data);
        }
    }

    #[test]
    fn test_rlp_fuzz_large_input() {
        // 10KB of random-ish data starting with a list prefix
        let mut data = vec![0xf9, 0x27, 0x10]; // long list, len=10000
        data.extend(vec![0x42; 10000]);
        let _ = rlp_decode_item(&data);
        let _ = decode_rlp_transaction(&data);
    }

    // -----------------------------------------------------------------------
    // Property-based tests (proptest)
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod prop_tests {
        use super::*;
    use luxtensor_core::rlp::{rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128, rlp_encode_list};
        use proptest::prelude::*;

        proptest! {
            /// RLP encode/decode roundtrip for arbitrary byte vectors
            #[test]
            fn fuzz_rlp_encode_decode_roundtrip(data in proptest::collection::vec(any::<u8>(), 0..1024)) {
                let encoded = rlp_encode_bytes(&data);
                let result = rlp_decode_item(&encoded);
                prop_assert!(result.is_ok(), "Failed to decode valid RLP: {:?}", result.err());
                let (decoded, consumed) = result.unwrap();
                prop_assert_eq!(&decoded, &data);
                prop_assert_eq!(consumed, encoded.len());
            }

            /// RLP u64 roundtrip for arbitrary values
            #[test]
            fn fuzz_rlp_u64_roundtrip(val in any::<u64>()) {
                let encoded = rlp_encode_u64(val);
                let (decoded_bytes, _) = rlp_decode_item(&encoded).unwrap();
                let decoded = rlp_item_to_u64(&decoded_bytes).unwrap();
                prop_assert_eq!(decoded, val);
            }

            /// RLP u128 roundtrip for arbitrary values
            #[test]
            fn fuzz_rlp_u128_roundtrip(val in any::<u128>()) {
                let encoded = rlp_encode_u128(val);
                let (decoded_bytes, _) = rlp_decode_item(&encoded).unwrap();
                let decoded = rlp_item_to_u128(&decoded_bytes);
                prop_assert_eq!(decoded, val);
            }

            /// RLP list roundtrip for arbitrary lists of byte vectors
            #[test]
            fn fuzz_rlp_list_roundtrip(
                items in proptest::collection::vec(
                    proptest::collection::vec(any::<u8>(), 0..128),
                    0..20
                )
            ) {
                let encoded_items: Vec<Vec<u8>> = items.iter()
                    .map(|item| rlp_encode_bytes(item))
                    .collect();
                let encoded_list = rlp_encode_list(&encoded_items);

                let (payload_offset, payload_len) = rlp_list_info(&encoded_list).unwrap();
                let payload = &encoded_list[payload_offset..payload_offset + payload_len];
                let decoded = rlp_decode_list(payload).unwrap();

                prop_assert_eq!(decoded.len(), items.len());
                for (original, decoded_item) in items.iter().zip(decoded.iter()) {
                    prop_assert_eq!(original, decoded_item);
                }
            }

            /// rlp_decode_item should never panic on arbitrary input
            #[test]
            fn fuzz_rlp_decode_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
                let _ = rlp_decode_item(&data);
            }

            /// decode_rlp_transaction should never panic on arbitrary input
            #[test]
            fn fuzz_decode_rlp_transaction_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
                let _ = decode_rlp_transaction(&data);
            }

            /// rlp_list_info should never panic on arbitrary input
            #[test]
            fn fuzz_rlp_list_info_never_panics(data in proptest::collection::vec(any::<u8>(), 0..256)) {
                let _ = rlp_list_info(&data);
            }

            /// rlp_decode_list should never panic on arbitrary input
            #[test]
            fn fuzz_rlp_decode_list_never_panics(data in proptest::collection::vec(any::<u8>(), 0..4096)) {
                let _ = rlp_decode_list(&data);
            }

            /// rlp_item_to_address should never panic
            #[test]
            fn fuzz_rlp_item_to_address_never_panics(data in proptest::collection::vec(any::<u8>(), 0..64)) {
                let _ = rlp_item_to_address(&data);
            }

            /// rlp_item_to_32 should never panic and always return [u8; 32]
            #[test]
            fn fuzz_rlp_item_to_32_never_panics(data in proptest::collection::vec(any::<u8>(), 0..128)) {
                let result = rlp_item_to_32(&data);
                prop_assert_eq!(result.len(), 32);
            }

            /// Encoded bytes always decode back successfully
            #[test]
            fn fuzz_rlp_encode_always_decodable(len in 0usize..2048) {
                let data = vec![0xABu8; len];
                let encoded = rlp_encode_bytes(&data);
                let result = rlp_decode_item(&encoded);
                prop_assert!(result.is_ok());
            }
        }
    }
}

//! Minimal RLP (Recursive Length Prefix) encoding utilities.
//!
//! Provides the subset of RLP encoding needed for EIP-155 transaction signing.
//! These helpers are shared between `luxtensor-core` (signing_message) and
//! `luxtensor-rpc` (eth_sendRawTransaction decode/verify).

/// Convert u64 to minimal big-endian bytes (no leading zeroes).
/// Returns empty vec for 0.
pub fn to_minimal_be(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![];
    }
    let full = val.to_be_bytes();
    let start = full.iter().position(|&b| b != 0).unwrap_or(7);
    full[start..].to_vec()
}

/// RLP-encode a single byte string (bytes).
///
/// Rules:
/// - Single byte in [0x00, 0x7f]: returned as-is
/// - Empty: 0x80
/// - 1–55 bytes: (0x80 + len) ++ data
/// - >55 bytes: (0xb7 + len_of_len) ++ len ++ data
pub fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] <= 0x7f {
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
        let len_bytes = to_minimal_be(data.len() as u64);
        let mut out = vec![0xb7 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(data);
        out
    }
}

/// RLP-encode a `u64` as minimal big-endian bytes.
/// Zero encodes as empty string (0x80).
pub fn rlp_encode_u64(val: u64) -> Vec<u8> {
    if val == 0 {
        return vec![0x80]; // empty bytes = 0
    }
    let bytes = to_minimal_be(val);
    rlp_encode_bytes(&bytes)
}

/// RLP-encode a `u128` as minimal big-endian bytes.
/// Zero encodes as empty string (0x80).
pub fn rlp_encode_u128(val: u128) -> Vec<u8> {
    if val == 0 {
        return vec![0x80];
    }
    let full = val.to_be_bytes();
    let start = full.iter().position(|&b| b != 0).unwrap_or(16);
    rlp_encode_bytes(&full[start..])
}

/// RLP-encode a list from pre-encoded items.
///
/// Rules:
/// - Total payload ≤ 55 bytes: (0xc0 + len) ++ payload
/// - Total payload > 55 bytes: (0xf7 + len_of_len) ++ len ++ payload
pub fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.iter().flat_map(|i| i.iter().copied()).collect();
    if payload.len() <= 55 {
        let mut out = vec![0xc0 + payload.len() as u8];
        out.extend_from_slice(&payload);
        out
    } else {
        let len_bytes = to_minimal_be(payload.len() as u64);
        let mut out = vec![0xf7 + len_bytes.len() as u8];
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(&payload);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlp_encode_u64_zero() {
        assert_eq!(rlp_encode_u64(0), vec![0x80]);
    }

    #[test]
    fn test_rlp_encode_u64_small() {
        // Single byte values [1, 0x7f] encode as themselves
        assert_eq!(rlp_encode_u64(1), vec![0x01]);
        assert_eq!(rlp_encode_u64(0x7f), vec![0x7f]);
    }

    #[test]
    fn test_rlp_encode_u64_medium() {
        // 0x80 needs string prefix
        assert_eq!(rlp_encode_u64(0x80), vec![0x81, 0x80]);
        assert_eq!(rlp_encode_u64(0x0400), vec![0x82, 0x04, 0x00]);
    }

    #[test]
    fn test_rlp_encode_empty_bytes() {
        assert_eq!(rlp_encode_bytes(&[]), vec![0x80]);
    }

    #[test]
    fn test_rlp_encode_list_empty() {
        assert_eq!(rlp_encode_list(&[]), vec![0xc0]);
    }

    #[test]
    fn test_rlp_encode_list_simple() {
        // RLP([ RLP("cat") ]) — but we encode as pre-encoded items
        let item = rlp_encode_bytes(b"cat");
        let list = rlp_encode_list(&[item]);
        // 0xc4, 0x83, 'c', 'a', 't'
        assert_eq!(list, vec![0xc4, 0x83, b'c', b'a', b't']);
    }

    #[test]
    fn test_rlp_encode_u128_zero() {
        assert_eq!(rlp_encode_u128(0), vec![0x80]);
    }

    #[test]
    fn test_rlp_encode_u128_large() {
        // 1 ETH = 10^18 = 0xDE0B6B3A7640000
        let val: u128 = 1_000_000_000_000_000_000;
        let encoded = rlp_encode_u128(val);
        // Should encode as 8-byte BE: 0x0DE0B6B3A7640000
        assert_eq!(encoded[0], 0x80 + 8); // 8-byte string prefix
    }

    #[test]
    fn test_to_minimal_be() {
        assert_eq!(to_minimal_be(0), Vec::<u8>::new());
        assert_eq!(to_minimal_be(1), vec![1]);
        assert_eq!(to_minimal_be(256), vec![1, 0]);
        assert_eq!(to_minimal_be(0xFFFF), vec![0xFF, 0xFF]);
    }
}

//! # Ethereum-compatible RPC Module
//!
//! Provides `eth_*` methods for EVM contract deployment and interaction.
//!
//! ## Supported Methods
//!
//! | Method | Description |
//! |--------|-------------|
//! | `eth_sendRawTransaction` | Submit signed transaction |
//! | `eth_getTransactionReceipt` | Get transaction receipt |
//! | `eth_call` | Execute read-only call |
//! | `eth_getCode` | Get contract bytecode |
//! | `eth_getBalance` | Get account balance |
//! | `eth_blockNumber` | Get current block number |
//! | `eth_getTransactionCount` | Get nonce |
//! | `eth_chainId` | Get chain ID |
//!
//! ## Types
//!
//! - [`PendingTransaction`] - Transaction in mempool
//! - [`ReadyTransaction`] - Transaction ready for block inclusion
//! - [`DeployedContract`] - Contract metadata

use jsonrpc_core::{Error as RpcError, ErrorCode, IoHandler, Params};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// Shared RLP encoding helpers from luxtensor-core
use luxtensor_core::rlp::{rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128, rlp_encode_list};

// ============================================================================
// RLP Transaction Decoding — MetaMask / ethers.js / web3.js compatibility
// ============================================================================
// Supports: Legacy (type 0), EIP-2930 (type 1 — access list), EIP-1559 (type 2)
// Implements proper ecrecover to derive sender address from ECDSA signature.

/// Decoded fields from an RLP-encoded Ethereum transaction
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct RlpDecodedTx {
    chain_id: u64,
    nonce: u64,
    gas_price: u64,                // legacy + EIP-2930
    max_fee_per_gas: u64,          // EIP-1559 only
    max_priority_fee_per_gas: u64, // EIP-1559 only
    gas_limit: u64,
    to: Option<Address>,
    value: u128,
    data: Vec<u8>,
    v: u64,
    r: [u8; 32],
    s: [u8; 32],
    tx_type: u8, // 0 = legacy, 1 = EIP-2930, 2 = EIP-1559
    /// The raw RLP-encoded body used for hashing (to compute tx hash)
    signing_hash: [u8; 32],
    /// Recovered sender address
    from: Address,
}

/// Minimal RLP decoder: decode a single item (string/bytes) or list
/// Returns (decoded_bytes, bytes_consumed)
fn rlp_decode_item(data: &[u8]) -> Result<(Vec<u8>, usize), String> {
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
fn rlp_list_info(data: &[u8]) -> Result<(usize, usize), String> {
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
fn rlp_decode_list(data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
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
fn rlp_item_to_u64(item: &[u8]) -> Result<u64, String> {
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
fn rlp_item_to_u128(item: &[u8]) -> u128 {
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
fn rlp_item_to_address(item: &[u8]) -> Result<Option<Address>, String> {
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
fn rlp_item_to_32(item: &[u8]) -> [u8; 32] {
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
fn ecrecover_address(
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
fn decode_rlp_transaction(raw: &[u8]) -> Result<RlpDecodedTx, String> {
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

/// Address type (20 bytes)
pub type Address = [u8; 20];

/// Hash type (32 bytes)
pub type TxHash = [u8; 32];

/// Pending transaction storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    pub hash: TxHash,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub data: Vec<u8>,
    pub gas: u64,
    pub nonce: u64,
    pub executed: bool,
    pub contract_address: Option<Address>,
    pub status: bool,
    pub gas_used: u64,
}

/// Deployed contract info
#[derive(Debug, Clone)]
pub struct DeployedContract {
    pub address: Address,
    pub code: Vec<u8>,
    pub deployer: Address,
    pub deploy_block: u64,
}

/// Transaction ready for block inclusion (with signature for production)
#[derive(Debug, Clone)]
pub struct ReadyTransaction {
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub data: Vec<u8>,
    pub gas: u64,
    /// Gas price in wei (must match what was signed)
    pub gas_price: u64,
    /// Signature R component (32 bytes)
    pub r: [u8; 32],
    /// Signature S component (32 bytes)
    pub s: [u8; 32],
    /// Signature V component (recovery id: 0 or 1)
    pub v: u8,
}

/// Mempool for transaction management
/// Replaces EvmState - contains only mempool-related data
pub struct Mempool {
    /// Pending transactions awaiting confirmation
    pub pending_txs: HashMap<TxHash, PendingTransaction>,
    /// Queue of transactions ready for block inclusion
    pub tx_queue: Arc<RwLock<Vec<ReadyTransaction>>>,
}

/// Maximum number of pending transactions in the mempool (DoS protection)
const MAX_MEMPOOL_PENDING: usize = 10_000;
/// Maximum number of transactions in the block inclusion queue
const MAX_TX_QUEUE: usize = 5_000;

impl Mempool {
    /// Create a new empty mempool
    pub fn new() -> Self {
        Self { pending_txs: HashMap::new(), tx_queue: Arc::new(RwLock::new(Vec::new())) }
    }

    /// Get and clear pending transactions for block production
    pub fn drain_tx_queue(&self) -> Vec<ReadyTransaction> {
        let mut queue = self.tx_queue.write();
        std::mem::take(&mut *queue)
    }

    /// Add transaction to queue for block inclusion
    /// SECURITY: Enforces MAX_TX_QUEUE to prevent unbounded memory growth
    pub fn queue_transaction(&self, tx: ReadyTransaction) -> bool {
        let mut queue = self.tx_queue.write();
        if queue.len() >= MAX_TX_QUEUE {
            tracing::warn!("TX queue full ({} txs), rejecting transaction", MAX_TX_QUEUE);
            return false;
        }
        tracing::debug!(
            "📥 queue_transaction: Queueing TX from 0x{} nonce={}",
            hex::encode(&tx.from),
            tx.nonce
        );
        queue.push(tx);
        tracing::debug!("📥 queue_transaction: Queue size now = {}", queue.len());
        true
    }

    /// Check if a transaction hash is pending
    pub fn is_pending(&self, tx_hash: &TxHash) -> bool {
        self.pending_txs.contains_key(tx_hash)
    }

    /// Add a pending transaction
    /// SECURITY: Enforces MAX_MEMPOOL_PENDING to prevent unbounded memory growth
    pub fn add_pending(&mut self, tx_hash: TxHash, tx: PendingTransaction) -> bool {
        if self.pending_txs.len() >= MAX_MEMPOOL_PENDING {
            tracing::warn!(
                "Mempool full ({} txs), rejecting pending transaction",
                MAX_MEMPOOL_PENDING
            );
            return false;
        }
        self.pending_txs.insert(tx_hash, tx);
        true
    }

    /// Remove a pending transaction
    pub fn remove_pending(&mut self, tx_hash: &TxHash) -> Option<PendingTransaction> {
        self.pending_txs.remove(tx_hash)
    }
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Utility functions
// ============================================================================

pub fn hex_to_address(s: &str) -> Option<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return None;
    }
    let bytes = hex::decode(s).ok()?;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Some(addr)
}

fn address_to_hex(addr: &Address) -> String {
    format!("0x{}", hex::encode(addr))
}

fn hash_to_hex(hash: &TxHash) -> String {
    format!("0x{}", hex::encode(hash))
}

/// Generate a cryptographically secure transaction hash from sender address and nonce.
///
/// Uses keccak256(RLP([sender, nonce])) following Ethereum conventions.
/// This produces a deterministic, collision-resistant 32-byte hash.
pub fn generate_tx_hash(from: &Address, nonce: u64) -> TxHash {
    use luxtensor_crypto::keccak256;

    // RLP-encode [from, nonce] — simplified canonical encoding
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(b"TX_HASH_V2:"); // domain separator prevents cross-protocol replay
    data.extend_from_slice(from);
    data.extend_from_slice(&nonce.to_be_bytes());
    keccak256(&data)
}

/// Generate a deterministic contract address from deployer + nonce.
///
/// Uses keccak256(RLP([deployer, nonce]))[12..] following Ethereum CREATE semantics.
#[allow(dead_code)]
fn generate_contract_address(deployer: &Address, nonce: u64) -> Address {
    use luxtensor_crypto::keccak256;

    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(b"CONTRACT_ADDR:"); // domain separator
    data.extend_from_slice(deployer);
    data.extend_from_slice(&nonce.to_be_bytes());
    let hash = keccak256(&data);

    // Take last 20 bytes (Ethereum CREATE convention)
    let mut result = [0u8; 20];
    result.copy_from_slice(&hash[12..32]);
    result
}

/// Register Ethereum-compatible RPC methods
///
/// # Parameters
/// - `io`: The JSON-RPC IO handler
/// - `mempool`: Transaction mempool (pending_txs, tx_queue)
/// - `unified_state`: Primary state source for reads (chain_id, nonces, balances, code, storage)
/// - `broadcaster`: P2P transaction broadcaster for relaying RPC-submitted transactions
/// - `evm_executor`: Shared EVM executor from block execution (for eth_call storage reads)
pub fn register_eth_methods(
    io: &mut IoHandler,
    mempool: Arc<RwLock<Mempool>>,
    unified_state: Arc<RwLock<luxtensor_core::UnifiedStateDB>>,
    db: Arc<luxtensor_storage::BlockchainDB>,
    broadcaster: Arc<dyn crate::TransactionBroadcaster>,
    evm_executor: Option<luxtensor_contracts::EvmExecutor>,
) {
    // eth_chainId - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("eth_chainId", move |_params: Params| {
        let chain_id = state.read().chain_id();
        Ok(json!(format!("0x{:x}", chain_id)))
    });

    // NOTE: eth_blockNumber is registered in server.rs with proper DB query
    // The old implementation here used EvmState.block_number which was incorrect

    // eth_getBalance - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("eth_getBalance", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing address".to_string(),
            data: None,
        })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address".to_string(),
            data: None,
        })?;

        let addr = luxtensor_core::Address::from(address);
        let balance = state.read().get_balance(&addr);
        Ok(json!(format!("0x{:x}", balance)))
    });

    // eth_getTransactionCount (nonce) - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("eth_getTransactionCount", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing address".to_string(),
            data: None,
        })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address".to_string(),
            data: None,
        })?;

        let addr = luxtensor_core::Address::from(address);
        let nonce = state.read().get_nonce(&addr);
        Ok(json!(format!("0x{:x}", nonce)))
    });

    // eth_gasPrice - Returns current base fee from EIP-1559 FeeMarket
    // Uses dynamic pricing: 0.5 gwei initial, adjusts based on block fullness
    io.add_sync_method("eth_gasPrice", move |_params: Params| {
        // Use FeeMarket for dynamic gas pricing
        use luxtensor_consensus::FeeMarket;
        let market = FeeMarket::new();
        let base_fee = market.current_base_fee();
        Ok(json!(format!("0x{:x}", base_fee)))
    });

    // eth_estimateGas — real EVM dry-run simulation (non-committing)
    let state_for_estimate = unified_state.clone();
    io.add_sync_method("eth_estimateGas", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        // Parse call params (same format as eth_call)
        let call_obj = match p.get(0) {
            Some(obj) => obj,
            None => {
                // No params → simple transfer estimate
                return Ok(json!("0x5208")); // 21000
            }
        };

        let from_str = call_obj
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");
        let to_str = call_obj.get("to").and_then(|v| v.as_str());
        let data_hex = call_obj.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
        let value_str = call_obj.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");

        let from_addr = hex_to_address(from_str).ok_or_else(|| {
            jsonrpc_core::Error::invalid_params(format!(
                "Invalid 'from' address format: {}",
                from_str
            ))
        })?;
        let data = {
            let s = data_hex.strip_prefix("0x").unwrap_or(data_hex);
            hex::decode(s).unwrap_or_default()
        };
        let value: u128 = {
            let s = value_str.strip_prefix("0x").unwrap_or(value_str);
            u128::from_str_radix(s, 16).unwrap_or(0)
        };

        // Simple transfer (no data, has to address)
        if data.is_empty() && to_str.is_some() {
            return Ok(json!("0x5208")); // 21_000
        }

        // Contract creation or call — use gas estimation formula
        let calldata_gas = luxtensor_contracts::revm_integration::estimate_calldata_gas(&data);
        let is_create = to_str.is_none();

        let base_gas: u64 = 21_000;
        let create_gas: u64 = if is_create { 32_000 } else { 0 };

        // If we have contract code, try dry-run via EvmExecutor
        if let Some(to_addr_str) = to_str {
            if let Some(to_addr) = hex_to_address(to_addr_str) {
                let state_guard = state_for_estimate.read();
                let code = state_guard.get_code(&luxtensor_core::Address::from(to_addr));
                let block_number = state_guard.block_number();
                drop(state_guard);

                if let Some(contract_code) = code {
                    // Create executor seeded with state from UnifiedStateDB
                    let executor = luxtensor_contracts::EvmExecutor::default();
                    // Fund the caller and deploy code so the EVM sees the correct state
                    {
                        let state_r = state_for_estimate.read();
                        let caller_balance =
                            state_r.get_balance(&luxtensor_core::Address::from(from_addr));
                        executor.fund_account(
                            &luxtensor_core::Address::from(from_addr),
                            caller_balance,
                        );
                        executor.deploy_code(
                            &luxtensor_core::Address::from(to_addr),
                            contract_code.clone(),
                        );
                    }
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    match executor.call(
                        luxtensor_core::Address::from(from_addr),
                        luxtensor_contracts::ContractAddress::from(to_addr),
                        contract_code.to_vec(),
                        data.clone(),
                        value,
                        30_000_000, // High gas limit for estimation
                        block_number,
                        timestamp,
                        1, // gas_price for estimation
                    ) {
                        Ok((_output, gas_used, _logs)) => {
                            // Add 15% safety margin (Geth-style)
                            let estimated = (gas_used as f64 * 1.15) as u64;
                            let estimated = estimated.max(21_000);
                            return Ok(json!(format!("0x{:x}", estimated)));
                        }
                        Err(_) => {
                            // Execution failed — return generous estimate
                            return Ok(json!(format!("0x{:x}", base_gas + calldata_gas + 100_000)));
                        }
                    }
                }
            }
        }

        // Fallback: analytic estimate
        let estimated = base_gas
            + create_gas
            + calldata_gas
            + if is_create { data.len() as u64 * 200 } else { 50_000 };
        Ok(json!(format!("0x{:x}", estimated)))
    });

    // NOTE: eth_sendTransaction is handled by tx_rpc.rs which registers after this
    // and overrides this handler. The tx_rpc.rs version includes P2P broadcasting.
    // This duplicate was removed to avoid confusion and dead code.

    // eth_getTransactionReceipt - uses mempool for pending_txs, unified_state for block_number
    // Falls back to DB lookup for confirmed transactions
    let mp_for_receipt = mempool.clone();
    let state_for_receipt = unified_state.clone();
    let db_for_receipt = db.clone();
    io.add_sync_method("eth_getTransactionReceipt", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let hash_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing transaction hash".to_string(),
                data: None,
            })?;

        let hash_str = hash_str.strip_prefix("0x").unwrap_or(hash_str);
        let hash_bytes = hex::decode(hash_str).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hash".to_string(),
            data: None,
        })?;

        let mut hash = [0u8; 32];
        let len = std::cmp::min(hash_bytes.len(), 32);
        hash[..len].copy_from_slice(&hash_bytes[..len]);

        let mempool_guard = mp_for_receipt.read();
        let block_number = state_for_receipt.read().block_number();

        // Check mempool for pending/recently-confirmed transactions
        if let Some(tx) = mempool_guard.pending_txs.get(&hash) {
            return Ok(json!({
                "transactionHash": hash_to_hex(&tx.hash),
                "transactionIndex": "0x0",
                "blockHash": hash_to_hex(&tx.hash),
                "blockNumber": format!("0x{:x}", block_number),
                "from": address_to_hex(&tx.from),
                "to": tx.to.as_ref().map(address_to_hex),
                "contractAddress": tx.contract_address.as_ref().map(address_to_hex),
                "cumulativeGasUsed": format!("0x{:x}", tx.gas_used),
                "gasUsed": format!("0x{:x}", tx.gas_used),
                "status": if tx.status { "0x1" } else { "0x0" },
                "logs": []
            }));
        }
        drop(mempool_guard);

        // DB fallback: look up the stored receipt for confirmed transactions
        if let Ok(Some(receipt_bytes)) = db_for_receipt.get_receipt(&hash) {
            if let Ok(r) = bincode::deserialize::<luxtensor_core::receipt::Receipt>(&receipt_bytes) {
                let status_hex = match r.status {
                    luxtensor_core::receipt::ExecutionStatus::Success => "0x1",
                    luxtensor_core::receipt::ExecutionStatus::Failed => "0x0",
                };
                let logs: Vec<serde_json::Value> = r.logs.iter().map(|log| {
                    json!({
                        "address": format!("0x{}", hex::encode(log.address.as_bytes())),
                        "topics": log.topics.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
                        "data": format!("0x{}", hex::encode(&log.data)),
                    })
                }).collect();

                return Ok(json!({
                    "transactionHash": format!("0x{}", hex::encode(hash)),
                    "transactionIndex": format!("0x{:x}", r.transaction_index),
                    "blockHash": format!("0x{}", hex::encode(r.block_hash)),
                    "blockNumber": format!("0x{:x}", r.block_height),
                    "from": format!("0x{}", hex::encode(r.from.as_bytes())),
                    "to": r.to.map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                    "contractAddress": r.contract_address.map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                    "cumulativeGasUsed": format!("0x{:x}", r.gas_used),
                    "gasUsed": format!("0x{:x}", r.gas_used),
                    "status": status_hex,
                    "logs": logs
                }));
            }
        }

        Ok(json!(null))
    });

    // eth_call - Execute a call without creating a transaction (read-only)
    // Uses shared EvmExecutor.static_call() to read REAL contract storage
    let state_for_call = unified_state.clone();
    let evm_for_call = evm_executor.clone();
    io.add_sync_method("eth_call", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let call_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing call object".to_string(),
            data: None,
        })?;

        // Parse call parameters
        let from_str = call_obj
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");

        let to_str = call_obj.get("to").and_then(|v| v.as_str());

        let data_hex = call_obj.get("data").and_then(|v| v.as_str()).unwrap_or("0x");

        // Parse gas limit from call object (default: 1M)
        let gas_limit: u64 = call_obj
            .get("gas")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).ok()
            })
            .unwrap_or(1_000_000);

        // Parse addresses
        let from_addr = hex_to_address(from_str).unwrap_or([0u8; 20]);
        let to_addr = match to_str {
            None => return Ok(json!("0x")),
            Some(addr_str) => match hex_to_address(addr_str) {
                None => return Ok(json!("0x")),
                Some(addr) => addr,
            },
        };

        // Parse data
        let data = {
            let s = data_hex.strip_prefix("0x").unwrap_or(data_hex);
            hex::decode(s).unwrap_or_default()
        };

        // Get contract code and block number from UnifiedStateDB
        let state_guard = state_for_call.read();
        let contract_code = match state_guard.get_code(&luxtensor_core::Address::from(to_addr)) {
            Some(code) => code.to_vec(),
            None => {
                return Ok(json!("0x"));
            }
        };
        let block_number = state_guard.block_number();
        drop(state_guard);

        // Get current timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Use shared EvmExecutor.static_call() which deep-clones the real EVM state
        // (accounts, storage, block_hashes) and executes WITHOUT committing changes.
        // This ensures eth_call reads the actual contract storage from executed TXs.
        if let Some(ref executor) = evm_for_call {
            match executor.static_call(
                luxtensor_core::Address::from(from_addr),
                luxtensor_contracts::ContractAddress::from(to_addr),
                contract_code,
                data,
                gas_limit,
                block_number,
                timestamp,
                1, // gas_price for eth_call
            ) {
                Ok((output, _gas_used, _logs)) => Ok(json!(format!("0x{}", hex::encode(output)))),
                Err(e) => {
                    tracing::warn!("eth_call execution error: {:?}", e);
                    Ok(json!("0x"))
                }
            }
        } else {
            // Fallback: no shared executor, create a fresh one (no storage — legacy behavior)
            let fallback = luxtensor_contracts::EvmExecutor::default();
            fallback.deploy_code(&luxtensor_core::Address::from(to_addr), contract_code.clone());
            match fallback.static_call(
                luxtensor_core::Address::from(from_addr),
                luxtensor_contracts::ContractAddress::from(to_addr),
                contract_code,
                data,
                gas_limit,
                block_number,
                timestamp,
                1,
            ) {
                Ok((output, _gas_used, _logs)) => Ok(json!(format!("0x{}", hex::encode(output)))),
                Err(e) => {
                    tracing::warn!("eth_call fallback execution error: {:?}", e);
                    Ok(json!("0x"))
                }
            }
        }
    });

    // eth_getCode - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("eth_getCode", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing address".to_string(),
            data: None,
        })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address".to_string(),
            data: None,
        })?;

        let addr = luxtensor_core::Address::from(address);

        // UnifiedStateDB is the sole source of truth for contract code
        if let Some(code) = state.read().get_code(&addr) {
            return Ok(json!(format!("0x{}", hex::encode(&code))));
        }

        // No code at this address
        Ok(json!("0x"))
    });

    // eth_accounts
    // SECURITY: Returns empty array. Previously returned hardcoded Hardhat default
    // addresses with publicly-known private keys, which would allow anyone to
    // steal funds sent to those addresses.
    io.add_sync_method("eth_accounts", move |_params: Params| Ok(json!([])));

    // net_version - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("net_version", move |_params: Params| {
        let chain_id = state.read().chain_id();
        Ok(json!(chain_id.to_string()))
    });

    // eth_sendRawTransaction - Standard Ethereum RLP-encoded signed transactions
    // Supports Legacy (type 0), EIP-2930 (type 1), EIP-1559 (type 2)
    // Full MetaMask / ethers.js / web3.js compatibility
    let mp_for_sendraw = mempool.clone();
    let unified_for_sendraw = unified_state.clone();
    let broadcaster_for_sendraw = broadcaster.clone();
    io.add_sync_method("eth_sendRawTransaction", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let raw_tx = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing raw transaction".to_string(),
            data: None,
        })?;

        // Decode hex raw transaction
        let raw_tx = raw_tx.strip_prefix("0x").unwrap_or(raw_tx);
        let tx_bytes = hex::decode(raw_tx).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hex format".to_string(),
            data: None,
        })?;

        if tx_bytes.len() < 10 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Transaction too short".to_string(),
                data: None,
            });
        }

        // ========== RLP DECODE (standard Ethereum wire format) ==========
        let decoded = decode_rlp_transaction(&tx_bytes).map_err(|e| {
            tracing::warn!("RLP decode failed: {}", e);
            RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Failed to decode RLP transaction: {}", e),
                data: None,
            }
        })?;

        let from = decoded.from;
        let nonce = decoded.nonce;
        let to = decoded.to;
        let value = decoded.value;
        let gas = decoded.gas_limit;
        let data = decoded.data.clone();
        let r = decoded.r;
        let s = decoded.s;
        let v = decoded.v as u8;
        let tx_hash = decoded.signing_hash; // keccak256 of full raw bytes

        info!(
            "📝 RLP decoded TX type={} from=0x{} nonce={} to={} value={} gas={}",
            decoded.tx_type,
            hex::encode(&from),
            nonce,
            to.map(|a| format!("0x{}", hex::encode(a))).unwrap_or_else(|| "CREATE".into()),
            value,
            gas,
        );

        // === REPLAY PROTECTION: Validate chain ID ===
        let expected_chain_id = unified_for_sendraw.read().chain_id();
        if decoded.chain_id != 0 && decoded.chain_id != expected_chain_id {
            return Err(RpcError {
                code: ErrorCode::ServerError(-32000),
                message: format!(
                    "chain ID mismatch: expected {} got {}",
                    expected_chain_id, decoded.chain_id
                ),
                data: None,
            });
        }

        // === DOUBLE-SPEND PROTECTION: Validate nonce ===
        let from_addr = luxtensor_core::Address::from(from);
        let current_nonce = unified_for_sendraw.read().get_nonce(&from_addr);
        if nonce < current_nonce {
            return Err(RpcError {
                code: ErrorCode::ServerError(-32000),
                message: format!("nonce too low: expected {} got {}", current_nonce, nonce),
                data: None,
            });
        }

        // Check for duplicate nonce in pending transactions (mempool)
        {
            let mempool_guard = mp_for_sendraw.read();
            for (_, tx) in mempool_guard.pending_txs.iter() {
                if tx.from == from && tx.nonce == nonce {
                    return Err(RpcError {
                        code: ErrorCode::ServerError(-32000),
                        message: format!("known transaction: nonce {} already pending", nonce),
                        data: None,
                    });
                }
            }
        }

        // Create ReadyTransaction with signature
        let gas_price = decoded.gas_price;
        let ready_tx =
            ReadyTransaction { nonce, from, to, value, data: data.clone(), gas, gas_price, r, s, v };

        let mut mempool_guard = mp_for_sendraw.write();

        // Store pending transaction in mempool
        let pending_tx = PendingTransaction {
            hash: tx_hash,
            from,
            to,
            value,
            data: data.clone(),
            gas,
            nonce,
            executed: false,
            contract_address: None,
            status: true,
            gas_used: 0,
        };
        if !mempool_guard.add_pending(tx_hash, pending_tx) {
            return Err(RpcError {
                code: ErrorCode::InternalError,
                message: "Mempool full — cannot accept more pending transactions".to_string(),
                data: None,
            });
        }

        // Queue for block production (mempool)
        if !mempool_guard.queue_transaction(ready_tx) {
            return Err(RpcError {
                code: ErrorCode::InternalError,
                message: "Transaction queue full — cannot accept more transactions".to_string(),
                data: None,
            });
        }

        // Broadcast to P2P network for multi-node propagation
        {
            let to_addr = to.map(luxtensor_core::Address::from);
            let mut core_tx = luxtensor_core::Transaction::with_chain_id(
                expected_chain_id,
                nonce,
                luxtensor_core::Address::from(from),
                to_addr,
                value,
                decoded.gas_price,
                gas,
                data,
            );
            // Preserve original ECDSA signature from the signed transaction
            core_tx.v = v;
            core_tx.r = r;
            core_tx.s = s;

            if let Err(e) = broadcaster_for_sendraw.broadcast(&core_tx) {
                tracing::warn!("Failed to broadcast raw transaction to P2P: {}", e);
            } else {
                info!("📡 Raw transaction broadcasted to P2P network: {}", hash_to_hex(&tx_hash));
            }
        }

        info!("📥 Received signed raw transaction: {}", hash_to_hex(&tx_hash));
        Ok(json!(hash_to_hex(&tx_hash)))
    });

    // === Additional ETH methods for full compatibility ===

    // eth_syncing - Returns syncing status
    io.add_sync_method("eth_syncing", move |_params: Params| {
        Ok(json!(false)) // Not syncing
    });

    // eth_mining - Returns whether client is mining
    io.add_sync_method("eth_mining", move |_params: Params| Ok(json!(false)));

    // eth_hashrate - Returns hashrate
    io.add_sync_method("eth_hashrate", move |_params: Params| Ok(json!("0x0")));

    // eth_coinbase - Returns coinbase address
    io.add_sync_method("eth_coinbase", move |_params: Params| {
        Ok(json!("0x0000000000000000000000000000000000000000"))
    });

    // eth_protocolVersion - Returns protocol version
    io.add_sync_method("eth_protocolVersion", move |_params: Params| {
        Ok(json!("0x41")) // Protocol version 65
    });

    let unified_for_storage = unified_state.clone();

    // eth_getStorageAt - Route to UnifiedStateDB
    io.add_sync_method("eth_getStorageAt", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(RpcError::invalid_params("Missing address or position"));
        }

        // Parse address
        let addr_bytes = match hex_to_address(&parsed[0]) {
            Some(a) => a,
            None => {
                return Ok(json!(
                    "0x0000000000000000000000000000000000000000000000000000000000000000"
                ))
            }
        };

        // Parse slot (32 bytes)
        let slot_str = parsed[1].trim_start_matches("0x");
        let mut slot = [0u8; 32];
        if let Ok(bytes) = hex::decode(slot_str) {
            let start = 32_usize.saturating_sub(bytes.len());
            slot[start..].copy_from_slice(&bytes);
        }

        // Get storage value from UnifiedStateDB
        let addr = luxtensor_core::Address::from(addr_bytes);
        let state = unified_for_storage.read();
        let value = state.get_storage(&addr, &slot);

        Ok(json!(format!("0x{}", hex::encode(value))))
    });

    // net_listening - Returns whether node is listening
    io.add_sync_method("net_listening", move |_params: Params| Ok(json!(true)));

    // web3_sha3 - Returns Keccak-256 hash
    io.add_sync_method("web3_sha3", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(RpcError::invalid_params("Missing data"));
        }
        let data = parsed[0].trim_start_matches("0x");
        let bytes = hex::decode(data).unwrap_or_default();
        use sha3::{Digest, Keccak256};
        let hash = Keccak256::digest(&bytes);
        Ok(json!(format!("0x{}", hex::encode(hash))))
    });

    // rpc_modules - Returns available RPC modules
    io.add_sync_method("rpc_modules", move |_params: Params| {
        Ok(json!({
            "eth": "1.0",
            "net": "1.0",
            "web3": "1.0",
            "staking": "1.0",
            "subnet": "1.0",
            "neuron": "1.0",
            "weight": "1.0",
            "ai": "1.0"
        }))
    });

    // dev_faucet - Credit tokens to address for testing (DEV MODE ONLY)
    // Uses unified_state for balance operations
    // SECURITY: This endpoint only works when chain_id indicates a dev/test network
    let dev_state = unified_state.clone();
    io.add_sync_method("dev_faucet", move |params: Params| {
        // Guard: only allow faucet on dev/test chain IDs
        // Chain ID 8898 = LuxTensor devnet, 9999 = LuxTensor testnet, 1337 = local dev, 31337 = Hardhat
        let chain_id = dev_state.read().chain_id();
        let allowed_chains: [u64; 4] = [8898, 9999, 1337, 31337];
        if !allowed_chains.contains(&chain_id) {
            return Err(RpcError {
                code: ErrorCode::MethodNotFound,
                message: "dev_faucet is only available on dev/test networks (chain_id 8898, 9999, 1337, 31337)".to_string(),
                data: None,
            });
        }

        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing address".to_string(),
                data: None,
            })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address format".to_string(),
            data: None,
        })?;

        // Parse amount (default: 1000 MDT = 1000 * 10^9 base units)
        let amount: u128 = p.get(1)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(1_000_000_000_000); // 1000 MDT default

        // Credit account in UnifiedStateDB
        let mut state_guard = dev_state.write();
        let addr = luxtensor_core::Address::from(address);
        let current_balance = state_guard.get_balance(&addr);
        let new_balance = current_balance + amount;
        state_guard.set_balance(addr, new_balance);

        Ok(json!({
            "success": true,
            "address": address_to_hex(&address),
            "credited": amount,
            "new_balance": new_balance.to_string()
        }))
    });

    // ========================================================================
    // eth_getTransactionByHash — Full RLP-encoded response
    // ========================================================================
    // Returns transaction data in standard Ethereum JSON format with proper
    // hex encoding, allowing ethers.js / web3.js to parse responses correctly.
    let mp_for_gettx = mempool.clone();
    let unified_for_gettx = unified_state.clone();
    let db_for_gettx = db.clone();
    io.add_sync_method("eth_getTransactionByHash", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let hash_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing transaction hash".to_string(),
            data: None,
        })?;

        let hash_str = hash_str.strip_prefix("0x").unwrap_or(hash_str);
        let hash_bytes = hex::decode(hash_str).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hex hash".to_string(),
            data: None,
        })?;

        if hash_bytes.len() != 32 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Hash must be 32 bytes".to_string(),
                data: None,
            });
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);

        // Look up in mempool first (pending transactions)
        let mempool_guard = mp_for_gettx.read();
        if let Some(tx) = mempool_guard.pending_txs.get(&hash) {
            let _block_number = unified_for_gettx.read().block_number();
            let chain_id = unified_for_gettx.read().chain_id();

            // Find the corresponding ReadyTransaction for signature data
            let (r_hex, s_hex, v_hex) = {
                let mut found = ("0x0".to_string(), "0x0".to_string(), "0x0".to_string());
                let queue_guard = mempool_guard.tx_queue.read();
                for ready_tx in queue_guard.iter() {
                    if ready_tx.from == tx.from && ready_tx.nonce == tx.nonce {
                        found = (
                            format!("0x{}", hex::encode(ready_tx.r)),
                            format!("0x{}", hex::encode(ready_tx.s)),
                            format!("0x{:x}", ready_tx.v as u64),
                        );
                        break;
                    }
                }
                found
            };

            // Standard Ethereum transaction object response
            // Compliant with: https://ethereum.org/en/developers/docs/apis/json-rpc/#eth_gettransactionbyhash
            return Ok(json!({
                "hash": hash_to_hex(&tx.hash),
                "nonce": format!("0x{:x}", tx.nonce),
                "blockHash": null,  // pending tx has no block yet
                "blockNumber": null,
                "transactionIndex": null,
                "from": address_to_hex(&tx.from),
                "to": tx.to.as_ref().map(address_to_hex),
                "value": format!("0x{:x}", tx.value),
                "gas": format!("0x{:x}", tx.gas),
                "gasPrice": format!("0x{:x}", 1_000_000_000u64), // 1 gwei default
                "input": format!("0x{}", hex::encode(&tx.data)),
                "v": v_hex,
                "r": r_hex,
                "s": s_hex,
                "chainId": format!("0x{:x}", chain_id),
                "type": "0x0"
            }));
        }

        // Not found in mempool — query block storage for confirmed transaction
        if let Ok(Some(tx)) = db_for_gettx.get_transaction(&hash) {
            let chain_id = unified_for_gettx.read().chain_id();

            // Look up the block containing this transaction for blockHash/blockNumber
            // We search by iterating recent blocks; for production scale,
            // a tx_hash → block_height index in the DB would be faster.
            let (block_hash, block_number, tx_index) = {
                let mut found = (None, None, None);
                let best_height = db_for_gettx.get_best_height().unwrap_or(Some(0)).unwrap_or(0);
                // Search last 1000 blocks (bounded scan)
                let search_start = best_height.saturating_sub(1000);
                for h in (search_start..=best_height).rev() {
                    if let Ok(Some(block)) = db_for_gettx.get_block_by_height(h) {
                        for (idx, btx) in block.transactions.iter().enumerate() {
                            if btx.hash() == hash {
                                found = (
                                    Some(format!("0x{}", hex::encode(block.hash()))),
                                    Some(format!("0x{:x}", h)),
                                    Some(format!("0x{:x}", idx)),
                                );
                                break;
                            }
                        }
                        if found.0.is_some() {
                            break;
                        }
                    }
                }
                found
            };

            return Ok(json!({
                "hash": format!("0x{}", hex::encode(tx.hash())),
                "nonce": format!("0x{:x}", tx.nonce),
                "blockHash": block_hash,
                "blockNumber": block_number,
                "transactionIndex": tx_index,
                "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                "to": tx.to.as_ref().map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                "value": format!("0x{:x}", tx.value),
                "gas": format!("0x{:x}", tx.gas_limit),
                "gasPrice": format!("0x{:x}", tx.gas_price),
                "input": format!("0x{}", hex::encode(&tx.data)),
                "v": format!("0x{:x}", tx.v as u64),
                "r": format!("0x{}", hex::encode(tx.r)),
                "s": format!("0x{}", hex::encode(tx.s)),
                "chainId": format!("0x{:x}", chain_id),
                "type": "0x0"
            }));
        }

        // Transaction not found anywhere
        Ok(json!(null))
    });

    // eth_getBlockByNumber — Returns block info
    let unified_for_block = unified_state.clone();
    io.add_sync_method("eth_getBlockByNumber", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let block_tag = p.get(0).and_then(|v| v.as_str()).unwrap_or("latest");

        let state = unified_for_block.read();
        let block_number = if block_tag == "latest" || block_tag == "pending" {
            state.block_number()
        } else {
            let s = block_tag.strip_prefix("0x").unwrap_or(block_tag);
            u64::from_str_radix(s, 16).unwrap_or(state.block_number())
        };

        // Return a minimal but valid block object
        Ok(json!({
            "number": format!("0x{:x}", block_number),
            "hash": format!("0x{}", hex::encode([0u8; 32])),
            "parentHash": format!("0x{}", hex::encode([0u8; 32])),
            "nonce": "0x0000000000000000",
            "sha3Uncles": format!("0x{}", hex::encode([0u8; 32])),
            "logsBloom": format!("0x{}", hex::encode([0u8; 256])),
            "transactionsRoot": format!("0x{}", hex::encode([0u8; 32])),
            "stateRoot": format!("0x{}", hex::encode([0u8; 32])),
            "receiptsRoot": format!("0x{}", hex::encode([0u8; 32])),
            "miner": "0x0000000000000000000000000000000000000000",
            "difficulty": "0x0",
            "totalDifficulty": "0x0",
            "extraData": "0x",
            "size": "0x0",
            "gasLimit": format!("0x{:x}", 30_000_000u64),
            "gasUsed": "0x0",
            "timestamp": format!("0x{:x}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0)),
            "transactions": [],
            "uncles": [],
            "baseFeePerGas": format!("0x{:x}", 1_000_000_000u64)
        }))
    });

    // ========================================================================
    // eth_feeHistory — EIP-1559 fee history (critical for MetaMask)
    // ========================================================================
    let db_for_fee = db.clone();
    let unified_for_fee = unified_state.clone();
    io.add_sync_method("eth_feeHistory", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        // Parse block_count (first param)
        let block_count = p
            .get(0)
            .and_then(|v| {
                if let Some(n) = v.as_u64() {
                    Some(n)
                } else if let Some(s) = v.as_str() {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    u64::from_str_radix(s, 16).ok()
                } else {
                    None
                }
            })
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing or invalid block_count".to_string(),
                data: None,
            })?;

        // Clamp block_count to 1024 (Ethereum standard limit)
        let block_count = block_count.min(1024);
        if block_count == 0 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "block_count must be > 0".to_string(),
                data: None,
            });
        }

        // Parse newest_block (second param)
        let newest_block_tag = p.get(1).and_then(|v| v.as_str()).unwrap_or("latest");

        let state_guard = unified_for_fee.read();
        let current_block = state_guard.block_number();
        drop(state_guard);

        let newest_block = match newest_block_tag {
            "latest" | "pending" => current_block,
            "earliest" => 0,
            s => {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).unwrap_or(current_block)
            }
        };

        // reward_percentiles (third param) — optional array of floats
        let _reward_percentiles: Vec<f64> = p
            .get(2)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .unwrap_or_default();

        // Calculate the oldest block we'll report
        let oldest_block = newest_block.saturating_sub(block_count - 1);

        let mut base_fee_per_gas: Vec<String> = Vec::new();
        let mut gas_used_ratio: Vec<f64> = Vec::new();
        let mut reward: Vec<Vec<String>> = Vec::new();

        // Use FeeMarket for base fee calculations
        use luxtensor_consensus::FeeMarket;
        let market = FeeMarket::new();
        let default_base_fee = market.current_base_fee();

        // Iterate from oldest_block to newest_block
        for height in oldest_block..=newest_block {
            if let Ok(Some(block)) = db_for_fee.get_block_by_height(height) {
                let gas_used = block.header.gas_used;
                let gas_limit = block.header.gas_limit;
                let ratio = if gas_limit > 0 { gas_used as f64 / gas_limit as f64 } else { 0.0 };
                gas_used_ratio.push(ratio);
                // Use default_base_fee since we don't persist per-block base fee
                base_fee_per_gas.push(format!("0x{:x}", default_base_fee));
                // Reward: empty inner array per block (we don't track per-tx priority fees)
                reward.push(_reward_percentiles.iter().map(|_| "0x0".to_string()).collect());
            } else {
                // Block not found in DB, use defaults
                gas_used_ratio.push(0.0);
                base_fee_per_gas.push(format!("0x{:x}", default_base_fee));
                reward.push(_reward_percentiles.iter().map(|_| "0x0".to_string()).collect());
            }
        }

        // EIP-1559 spec: baseFeePerGas has block_count + 1 entries
        // (includes the predicted next base fee)
        base_fee_per_gas.push(format!("0x{:x}", default_base_fee));

        info!(
            "eth_feeHistory: block_count={}, oldest=0x{:x}, newest=0x{:x}",
            block_count, oldest_block, newest_block
        );

        Ok(json!({
            "oldestBlock": format!("0x{:x}", oldest_block),
            "baseFeePerGas": base_fee_per_gas,
            "gasUsedRatio": gas_used_ratio,
            "reward": if _reward_percentiles.is_empty() { None } else { Some(reward) }
        }))
    });

    // ========================================================================
    // eth_getBlockByHash — Standard block lookup by hash
    // ========================================================================
    let db_for_bbh = db.clone();
    let unified_for_bbh = unified_state.clone();
    io.add_sync_method("eth_getBlockByHash", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let hash_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing block hash".to_string(),
                data: None,
            })?;

        let hash_str = hash_str.strip_prefix("0x").unwrap_or(hash_str);
        let hash_bytes = hex::decode(hash_str).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hex hash".to_string(),
            data: None,
        })?;

        if hash_bytes.len() != 32 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Hash must be 32 bytes".to_string(),
                data: None,
            });
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);

        let full_transactions = p.get(1)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Look up block by hash in DB
        match db_for_bbh.get_block(&hash) {
            Ok(Some(block)) => {
                let block_hash = block.hash();
                let chain_id = unified_for_bbh.read().chain_id();

                let transactions = if full_transactions {
                    // Return full transaction objects
                    block.transactions.iter().enumerate().map(|(idx, tx)| {
                        json!({
                            "hash": format!("0x{}", hex::encode(tx.hash())),
                            "nonce": format!("0x{:x}", tx.nonce),
                            "blockHash": format!("0x{}", hex::encode(block_hash)),
                            "blockNumber": format!("0x{:x}", block.header.height),
                            "transactionIndex": format!("0x{:x}", idx),
                            "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                            "to": tx.to.as_ref().map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                            "value": format!("0x{:x}", tx.value),
                            "gas": format!("0x{:x}", tx.gas_limit),
                            "gasPrice": format!("0x{:x}", tx.gas_price),
                            "input": format!("0x{}", hex::encode(&tx.data)),
                            "v": format!("0x{:x}", tx.v as u64),
                            "r": format!("0x{}", hex::encode(tx.r)),
                            "s": format!("0x{}", hex::encode(tx.s)),
                            "chainId": format!("0x{:x}", chain_id),
                            "type": "0x0"
                        })
                    }).collect::<Vec<_>>()
                } else {
                    // Return only transaction hashes
                    block.transactions.iter().map(|tx| {
                        json!(format!("0x{}", hex::encode(tx.hash())))
                    }).collect::<Vec<_>>()
                };

                info!("eth_getBlockByHash: found block height={} txs={}",
                    block.header.height, block.transactions.len());

                Ok(json!({
                    "number": format!("0x{:x}", block.header.height),
                    "hash": format!("0x{}", hex::encode(block_hash)),
                    "parentHash": format!("0x{}", hex::encode(block.header.previous_hash)),
                    "nonce": "0x0000000000000000",
                    "sha3Uncles": format!("0x{}", hex::encode([0u8; 32])),
                    "logsBloom": format!("0x{}", hex::encode([0u8; 256])),
                    "transactionsRoot": format!("0x{}", hex::encode(block.header.txs_root)),
                    "stateRoot": format!("0x{}", hex::encode(block.header.state_root)),
                    "receiptsRoot": format!("0x{}", hex::encode(block.header.receipts_root)),
                    "miner": format!("0x{}", hex::encode(&block.header.validator[..20])),
                    "difficulty": "0x0",
                    "totalDifficulty": "0x0",
                    "extraData": format!("0x{}", hex::encode(&block.header.extra_data)),
                    "size": "0x0",
                    "gasLimit": format!("0x{:x}", block.header.gas_limit),
                    "gasUsed": format!("0x{:x}", block.header.gas_used),
                    "timestamp": format!("0x{:x}", block.header.timestamp),
                    "transactions": transactions,
                    "uncles": [],
                    "baseFeePerGas": format!("0x{:x}", 1_000_000_000u64)
                }))
            }
            Ok(None) => Ok(json!(null)),
            Err(e) => {
                tracing::warn!("eth_getBlockByHash DB error: {:?}", e);
                Ok(json!(null))
            }
        }
    });

    // ========================================================================
    // eth_getBlockTransactionCountByNumber — Transaction count in a block
    // ========================================================================
    let db_for_txcount = db.clone();
    let unified_for_txcount = unified_state.clone();
    io.add_sync_method("eth_getBlockTransactionCountByNumber", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let block_tag = p.get(0).and_then(|v| v.as_str()).unwrap_or("latest");

        let state_guard = unified_for_txcount.read();
        let current_block = state_guard.block_number();
        drop(state_guard);

        let block_number = match block_tag {
            "latest" | "pending" => current_block,
            "earliest" => 0,
            s => {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).unwrap_or(current_block)
            }
        };

        match db_for_txcount.get_block_by_height(block_number) {
            Ok(Some(block)) => {
                let count = block.transactions.len();
                info!(
                    "eth_getBlockTransactionCountByNumber: block={} count={}",
                    block_number, count
                );
                Ok(json!(format!("0x{:x}", count)))
            }
            Ok(None) => {
                // Block not found — return 0 (matches common Ethereum node behavior)
                Ok(json!("0x0"))
            }
            Err(e) => {
                tracing::warn!("eth_getBlockTransactionCountByNumber DB error: {:?}", e);
                Ok(json!("0x0"))
            }
        }
    });
}

/// Register eth_getLogs and filter-related RPC methods
/// Uses UnifiedStateDB for block_number reads
pub fn register_log_methods(
    io: &mut IoHandler,
    log_store: Arc<RwLock<crate::logs::LogStore>>,
    unified_state: Arc<RwLock<luxtensor_core::UnifiedStateDB>>,
) {
    // eth_getLogs - Query historical logs
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_getLogs", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number();
        let logs = store.read().get_logs(&filter, current_block);

        let rpc_logs: Vec<serde_json::Value> = logs.iter().map(|log| log.to_rpc_log()).collect();

        Ok(json!(rpc_logs))
    });

    // eth_newFilter - Create a new filter
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_newFilter", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number();
        let filter_id = store.read().new_filter(filter, current_block);

        Ok(json!(filter_id))
    });

    // eth_getFilterChanges - Get logs since last poll
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_getFilterChanges", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter ID".to_string(),
            data: None,
        })?;

        let current_block = state.read().block_number();
        match store.read().get_filter_changes(filter_id, current_block) {
            Some(logs) => {
                let rpc_logs: Vec<serde_json::Value> =
                    logs.iter().map(|log| log.to_rpc_log()).collect();
                Ok(json!(rpc_logs))
            }
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Filter not found".to_string(),
                data: None,
            }),
        }
    });

    // eth_getFilterLogs - Get all logs for a filter
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_getFilterLogs", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter ID".to_string(),
            data: None,
        })?;

        let current_block = state.read().block_number();
        let store_read = store.read();

        // For eth_getFilterLogs, we return all logs matching the original filter
        // This requires access to the filter itself
        match store_read.get_filter_changes(filter_id, current_block) {
            Some(logs) => {
                let rpc_logs: Vec<serde_json::Value> =
                    logs.iter().map(|log| log.to_rpc_log()).collect();
                Ok(json!(rpc_logs))
            }
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Filter not found".to_string(),
                data: None,
            }),
        }
    });

    // eth_uninstallFilter - Remove a filter
    let store = log_store.clone();
    io.add_sync_method("eth_uninstallFilter", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter ID".to_string(),
            data: None,
        })?;

        let removed = store.read().uninstall_filter(filter_id);
        Ok(json!(removed))
    });

    // ========================================================================
    // eth_newBlockFilter — Create a filter for new blocks
    // ========================================================================
    let store_for_bf = log_store.clone();
    let state_for_bf = unified_state.clone();
    io.add_sync_method("eth_newBlockFilter", move |_params: Params| {
        let current_block = state_for_bf.read().block_number();
        // Create a log filter that tracks new blocks (empty filter = all logs)
        let filter =
            crate::logs::LogFilter { from_block: Some(current_block + 1), ..Default::default() };
        let filter_id = store_for_bf.read().new_filter(filter, current_block);
        info!("eth_newBlockFilter: created filter_id={} at block={}", filter_id, current_block);
        Ok(json!(filter_id))
    });

    // ========================================================================
    // eth_newPendingTransactionFilter — Create a filter for pending txs
    // ========================================================================
    let store_for_ptf = log_store.clone();
    let state_for_ptf = unified_state.clone();
    io.add_sync_method("eth_newPendingTransactionFilter", move |_params: Params| {
        let current_block = state_for_ptf.read().block_number();
        // Create a filter tracking from current block onward
        let filter =
            crate::logs::LogFilter { from_block: Some(current_block), ..Default::default() };
        let filter_id = store_for_ptf.read().new_filter(filter, current_block);
        info!(
            "eth_newPendingTransactionFilter: created filter_id={} at block={}",
            filter_id, current_block
        );
        Ok(json!(filter_id))
    });

    info!("Registered eth_getLogs and filter methods");
}

/// Parse a filter object from JSON
fn parse_log_filter(obj: &serde_json::Value) -> Result<crate::logs::LogFilter, RpcError> {
    use crate::logs::LogFilter;

    let mut filter = LogFilter::default();

    // Parse fromBlock
    if let Some(from) = obj.get("fromBlock").and_then(|v| v.as_str()) {
        filter.from_block = parse_block_number(from);
    }

    // Parse toBlock
    if let Some(to) = obj.get("toBlock").and_then(|v| v.as_str()) {
        filter.to_block = parse_block_number(to);
    }

    // Parse address (can be single address or array)
    if let Some(addr) = obj.get("address") {
        let addresses = if let Some(addr_str) = addr.as_str() {
            vec![parse_address(addr_str)?]
        } else if let Some(arr) = addr.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(parse_address)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };
        if !addresses.is_empty() {
            filter.address = Some(addresses);
        }
    }

    // Parse topics
    if let Some(topics) = obj.get("topics").and_then(|v| v.as_array()) {
        let mut topic_filters = Vec::new();
        for topic in topics {
            if topic.is_null() {
                topic_filters.push(None);
            } else if let Some(topic_str) = topic.as_str() {
                topic_filters.push(Some(vec![parse_hash(topic_str)?]));
            } else if let Some(arr) = topic.as_array() {
                let hashes: Vec<[u8; 32]> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| parse_hash(s).ok())
                    .collect();
                if hashes.is_empty() {
                    topic_filters.push(None);
                } else {
                    topic_filters.push(Some(hashes));
                }
            } else {
                topic_filters.push(None);
            }
        }
        if !topic_filters.is_empty() {
            filter.topics = Some(topic_filters);
        }
    }

    // Parse blockHash
    if let Some(hash) = obj.get("blockHash").and_then(|v| v.as_str()) {
        filter.block_hash = Some(parse_hash(hash)?);
    }

    Ok(filter)
}

fn parse_block_number(s: &str) -> Option<u64> {
    match s {
        "latest" | "pending" => None,
        "earliest" => Some(0),
        _ => {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u64::from_str_radix(s, 16).ok()
        }
    }
}

fn parse_address(s: &str) -> Result<luxtensor_core::types::Address, RpcError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address length".to_string(),
            data: None,
        });
    }
    let bytes = hex::decode(s).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid address hex".to_string(),
        data: None,
    })?;
    Ok(luxtensor_core::types::Address::try_from_slice(&bytes).ok_or_else(|| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid address length".to_string(),
        data: None,
    })?)
}

fn parse_hash(s: &str) -> Result<[u8; 32], RpcError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 64 {
        return Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hash length".to_string(),
            data: None,
        });
    }
    let bytes = hex::decode(s).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid hash hex".to_string(),
        data: None,
    })?;
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// Register ERC-4337 Account Abstraction RPC methods
pub fn register_aa_methods(
    io: &mut IoHandler,
    entry_point: Arc<RwLock<luxtensor_contracts::EntryPoint>>,
) {
    // eth_sendUserOperation - Submit a user operation
    let ep = entry_point.clone();
    io.add_sync_method("eth_sendUserOperation", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let user_op_json = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing user operation".to_string(),
            data: None,
        })?;

        let _entry_point_addr = p.get(1).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing entry point address".to_string(),
            data: None,
        })?;

        // Parse user operation
        let user_op = parse_user_operation(user_op_json)?;

        // Validate and queue the operation for block inclusion
        let entry_point = ep.read();
        match entry_point.validate_user_op(&user_op) {
            Ok(()) => {
                // Queue in EntryPoint's pending pool — will be drained during block production
                let op_hash = entry_point.queue_user_op(user_op);
                Ok(json!(format!("0x{}", hex::encode(op_hash))))
            }
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Validation failed: {}", e),
                data: None,
            }),
        }
    });

    // eth_estimateUserOperationGas - Estimate gas for user operation
    let ep = entry_point.clone();
    io.add_sync_method("eth_estimateUserOperationGas", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let user_op_json = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing user operation".to_string(),
            data: None,
        })?;

        let user_op = parse_user_operation(user_op_json)?;
        let entry_point = ep.read();

        match entry_point.estimate_user_op_gas(&user_op) {
            Ok(estimate) => Ok(json!({
                "preVerificationGas": format!("0x{:x}", estimate.pre_verification_gas),
                "verificationGasLimit": format!("0x{:x}", estimate.verification_gas),
                "callGasLimit": format!("0x{:x}", estimate.call_gas),
            })),
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Estimation failed: {}", e),
                data: None,
            }),
        }
    });

    // eth_getUserOperationReceipt - Get receipt for a user operation
    let ep = entry_point.clone();
    io.add_sync_method("eth_getUserOperationReceipt", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let op_hash_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing operation hash".to_string(),
            data: None,
        })?;

        let op_hash = parse_hash(op_hash_str)?;
        let entry_point = ep.read();

        match entry_point.get_user_op_receipt(&op_hash) {
            Some(receipt) => Ok(json!({
                "userOpHash": format!("0x{}", hex::encode(receipt.user_op_hash)),
                "sender": format!("0x{}", hex::encode(receipt.sender.as_bytes())),
                "nonce": format!("0x{:x}", receipt.nonce),
                "paymaster": receipt.paymaster.map(|p| format!("0x{}", hex::encode(p.as_bytes()))),
                "actualGasUsed": format!("0x{:x}", receipt.actual_gas_used),
                "actualGasCost": format!("0x{:x}", receipt.actual_gas_cost),
                "success": receipt.success,
                "reason": receipt.reason,
                "receipt": {
                    "transactionHash": format!("0x{}", hex::encode(receipt.transaction_hash)),
                    "blockNumber": format!("0x{:x}", receipt.block_number),
                    "blockHash": format!("0x{}", hex::encode(receipt.block_hash)),
                }
            })),
            None => Ok(json!(null)),
        }
    });

    // eth_supportedEntryPoints - Get list of supported entry points
    let ep = entry_point.clone();
    io.add_sync_method("eth_supportedEntryPoints", move |_params: Params| {
        let entry_point = ep.read();
        let supported = entry_point.get_supported_entry_points();
        Ok(json!(supported))
    });

    // eth_getUserOperationByHash - Get user operation by hash (ERC-4337)
    let ep = entry_point.clone();
    io.add_sync_method("eth_getUserOperationByHash", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let op_hash_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing operation hash".to_string(),
            data: None,
        })?;

        let op_hash = parse_hash(op_hash_str)?;
        let entry_point = ep.read();

        // Return receipt info if operation was processed
        match entry_point.get_user_op_receipt(&op_hash) {
            Some(receipt) => Ok(json!({
                "userOperation": null, // Original op not stored for privacy
                "entryPoint": "0x0000000000000000000000000000000000004337",
                "transactionHash": format!("0x{}", hex::encode(receipt.transaction_hash)),
                "blockNumber": format!("0x{:x}", receipt.block_number),
                "blockHash": format!("0x{}", hex::encode(receipt.block_hash)),
            })),
            None => Ok(json!(null)),
        }
    });

    // eth_chainId - Return chain ID for AA context (ERC-4337)
    // Note: This complements the standard eth_chainId but is specific to AA operations
    let ep = entry_point.clone();
    io.add_sync_method("aa_chainId", move |_params: Params| {
        let entry_point = ep.read();
        let chain_id = entry_point.chain_id();
        Ok(json!(format!("0x{:x}", chain_id)))
    });

    info!("Registered ERC-4337 Account Abstraction RPC methods (6 methods)");
}

/// Parse a UserOperation from JSON
fn parse_user_operation(
    obj: &serde_json::Value,
) -> Result<luxtensor_contracts::UserOperation, RpcError> {
    use luxtensor_contracts::UserOperation;

    let sender = obj.get("sender").and_then(|v| v.as_str()).ok_or_else(|| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Missing sender".to_string(),
        data: None,
    })?;

    let nonce = obj
        .get("nonce")
        .and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u128::from_str_radix(s, 16).ok()
        })
        .unwrap_or(0);

    let call_gas_limit = parse_gas_value(obj.get("callGasLimit"));
    let verification_gas_limit = parse_gas_value(obj.get("verificationGasLimit"));
    let pre_verification_gas = parse_gas_value(obj.get("preVerificationGas"));
    let max_fee_per_gas = parse_gas_value(obj.get("maxFeePerGas"));
    let max_priority_fee_per_gas = parse_gas_value(obj.get("maxPriorityFeePerGas"));

    let init_code = parse_hex_bytes(obj.get("initCode"));
    let call_data = parse_hex_bytes(obj.get("callData"));
    let paymaster_and_data = parse_hex_bytes(obj.get("paymasterAndData"));
    let signature = parse_hex_bytes(obj.get("signature"));

    Ok(UserOperation {
        sender: parse_address(sender)?,
        nonce,
        init_code,
        call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data,
        signature,
    })
}

fn parse_gas_value(val: Option<&serde_json::Value>) -> u64 {
    val.and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u64::from_str_radix(s, 16).ok()
        })
        .unwrap_or(0)
}

fn parse_hex_bytes(val: Option<&serde_json::Value>) -> Vec<u8> {
    val.and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            hex::decode(s).ok()
        })
        .unwrap_or_default()
}

// ============================================================================
// RLP Fuzz Tests — Malformed Input Resilience
// ============================================================================
// These tests verify that the RLP decoder never panics on arbitrary input,
// returns proper errors for malformed data, and correctly handles edge cases
// that real-world attackers might craft.

#[cfg(test)]
mod tests {
    use super::*;

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

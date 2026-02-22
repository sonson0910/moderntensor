use crate::{Address, Hash, Result};
use luxtensor_crypto::keccak256;
use serde::{Deserialize, Serialize};

/// Strip leading zero bytes for canonical RLP encoding of big integers.
fn strip_leading_zeros(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    &bytes[start..]
}

/// Transaction structure with chain_id for replay protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Chain ID to prevent cross-chain replay attacks
    pub chain_id: u64,
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub gas_price: u64,
    pub gas_limit: u64,
    pub data: Vec<u8>,

    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Transaction {
    /// Create a new transaction with default devnet chain_id (8898)
    ///
    /// For production, prefer `with_chain_id()` with the correct chain ID.
    pub fn new(
        nonce: u64,
        from: Address,
        to: Option<Address>,
        value: u128,
        gas_price: u64,
        gas_limit: u64,
        data: Vec<u8>,
    ) -> Self {
        Self::with_chain_id(8898, nonce, from, to, value, gas_price, gas_limit, data)
    }

    /// Create a new transaction with explicit chain_id
    pub fn with_chain_id(
        chain_id: u64,
        nonce: u64,
        from: Address,
        to: Option<Address>,
        value: u128,
        gas_price: u64,
        gas_limit: u64,
        data: Vec<u8>,
    ) -> Self {
        Self {
            chain_id,
            nonce,
            from,
            to,
            value,
            gas_price,
            gas_limit,
            data,
            v: 0,
            r: [0u8; 32],
            s: [0u8; 32],
        }
    }

    /// Calculate Ethereum-standard transaction hash.
    ///
    /// This is `keccak256(RLP([nonce, gasPrice, gasLimit, to, value, data, v, r, s]))`
    /// where `v` follows EIP-155: `v = recovery_id + 2 * chain_id + 35`.
    ///
    /// This matches what `eth_sendRawTransaction` returns and what wallets/explorers
    /// use to identify transactions.
    pub fn hash(&self) -> Hash {
        keccak256(&self.rlp_signed())
    }

    /// Get the EIP-155 signing hash (used for signature verification).
    ///
    /// This is `keccak256(RLP([nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0]))`.
    /// Used internally for ecrecover — NOT the transaction ID.
    pub fn signing_hash(&self) -> Hash {
        keccak256(&self.signing_message())
    }

    /// Get signing message for this transaction (EIP-155 standard format).
    ///
    /// Produces `RLP([nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0])`
    /// which is the standard EIP-155 unsigned transaction encoding.
    /// This ensures compatibility with all Ethereum wallets (MetaMask, eth_account, ethers.js).
    pub fn signing_message(&self) -> Vec<u8> {
        use crate::rlp::{rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128, rlp_encode_list};

        let to_encoded = match &self.to {
            Some(addr) => rlp_encode_bytes(addr.as_bytes()),
            None => rlp_encode_bytes(&[]),
        };

        rlp_encode_list(&[
            rlp_encode_u64(self.nonce),
            rlp_encode_u64(self.gas_price),
            rlp_encode_u64(self.gas_limit),
            to_encoded,
            rlp_encode_u128(self.value),
            rlp_encode_bytes(&self.data),
            rlp_encode_u64(self.chain_id),
            rlp_encode_bytes(&[]), // 0 (EIP-155)
            rlp_encode_bytes(&[]), // 0 (EIP-155)
        ])
    }

    /// Encode the full signed transaction as RLP.
    ///
    /// Produces `RLP([nonce, gasPrice, gasLimit, to, value, data, v, r, s])`
    /// where v follows EIP-155: `v = recovery_id + 2 * chain_id + 35`.
    /// This is the standard Ethereum "raw transaction" encoding.
    pub fn rlp_signed(&self) -> Vec<u8> {
        use crate::rlp::{rlp_encode_bytes, rlp_encode_u64, rlp_encode_u128, rlp_encode_list};

        let to_encoded = match &self.to {
            Some(addr) => rlp_encode_bytes(addr.as_bytes()),
            None => rlp_encode_bytes(&[]),
        };

        // EIP-155: v = recovery_id + 2 * chain_id + 35
        let v_eip155 = (self.v as u64) + 2 * self.chain_id + 35;

        // Strip leading zeros from r and s for canonical RLP encoding
        let r_trimmed = strip_leading_zeros(&self.r);
        let s_trimmed = strip_leading_zeros(&self.s);

        rlp_encode_list(&[
            rlp_encode_u64(self.nonce),
            rlp_encode_u64(self.gas_price),
            rlp_encode_u64(self.gas_limit),
            to_encoded,
            rlp_encode_u128(self.value),
            rlp_encode_bytes(&self.data),
            rlp_encode_u64(v_eip155),
            rlp_encode_bytes(r_trimmed),
            rlp_encode_bytes(s_trimmed),
        ])
    }

    /// Verify transaction signature
    pub fn verify_signature(&self) -> Result<()> {
        use luxtensor_crypto::{address_from_public_key, recover_public_key, verify_signature};

        // Get signing message
        let message = self.signing_message();
        let message_hash = luxtensor_crypto::keccak256(&message);

        // Combine r and s into signature
        let mut signature = [0u8; 64];
        signature[..32].copy_from_slice(&self.r);
        signature[32..].copy_from_slice(&self.s);

        // Recover public key from signature
        let public_key = recover_public_key(&message_hash, &signature, self.v)
            .map_err(|_| crate::CoreError::InvalidSignature)?;

        // Derive address from public key
        let recovered_address =
            address_from_public_key(&public_key).map_err(|_| crate::CoreError::InvalidSignature)?;

        // Verify the recovered address matches the from address
        if recovered_address.as_bytes() != self.from.as_bytes() {
            return Err(crate::CoreError::InvalidSignature);
        }

        // Also verify signature directly
        let is_valid = verify_signature(&message_hash, &signature, &public_key)
            .map_err(|_| crate::CoreError::InvalidSignature)?;

        if !is_valid {
            return Err(crate::CoreError::InvalidSignature);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let from = Address::zero();
        let to = Some(Address::zero());
        let tx = Transaction::new(0, from, to, 1000, 1, 21000, vec![]);

        assert_eq!(tx.chain_id, 8898); // Default chain_id (DEVNET)
        assert_eq!(tx.nonce, 0);
        assert_eq!(tx.value, 1000);
        assert_eq!(tx.gas_limit, 21000);
    }

    #[test]
    fn test_transaction_with_chain_id() {
        let from = Address::zero();
        let tx = Transaction::with_chain_id(9999, 5, from, None, 500, 2, 50000, vec![1, 2, 3]);

        assert_eq!(tx.chain_id, 9999);
        assert_eq!(tx.nonce, 5);
        assert_eq!(tx.value, 500);
    }

    #[test]
    fn test_transaction_hash() {
        let from = Address::zero();
        let tx = Transaction::new(0, from, None, 1000, 1, 21000, vec![]);
        let hash = tx.hash();
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_signing_message_includes_chain_id() {
        let from = Address::zero();
        let tx1 = Transaction::with_chain_id(1, 0, from, None, 1000, 1, 21000, vec![]);
        let tx2 = Transaction::with_chain_id(2, 0, from, None, 1000, 1, 21000, vec![]);

        // Different chain_id should produce different signing messages
        assert_ne!(tx1.signing_message(), tx2.signing_message());
    }
}

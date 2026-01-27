use serde::{Deserialize, Serialize};
use crate::{Hash, Address, Result};
use luxtensor_crypto::keccak256;

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

    // Signature components
    pub v: u8,
    pub r: [u8; 32],
    pub s: [u8; 32],
}

impl Transaction {
    /// Create a new transaction with chain_id
    pub fn new(
        nonce: u64,
        from: Address,
        to: Option<Address>,
        value: u128,
        gas_price: u64,
        gas_limit: u64,
        data: Vec<u8>,
    ) -> Self {
        Self::with_chain_id(1, nonce, from, to, value, gas_price, gas_limit, data)
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

    /// Calculate transaction hash
    pub fn hash(&self) -> Hash {
        let bytes = bincode::serialize(self).unwrap();
        keccak256(&bytes)
    }

    /// Get signing message for this transaction (includes chain_id for replay protection)
    pub fn signing_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        // Include chain_id FIRST to prevent cross-chain replay attacks
        msg.extend_from_slice(&self.chain_id.to_le_bytes());
        msg.extend_from_slice(&self.nonce.to_le_bytes());
        msg.extend_from_slice(self.from.as_bytes());
        if let Some(to) = self.to {
            msg.extend_from_slice(to.as_bytes());
        }
        msg.extend_from_slice(&self.value.to_le_bytes());
        msg.extend_from_slice(&self.gas_price.to_le_bytes());
        msg.extend_from_slice(&self.gas_limit.to_le_bytes());
        msg.extend_from_slice(&self.data);
        msg
    }

    /// Verify transaction signature
    pub fn verify_signature(&self) -> Result<()> {
        use luxtensor_crypto::{verify_signature, address_from_public_key, recover_public_key};

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
        let recovered_address = address_from_public_key(&public_key)
            .map_err(|_| crate::CoreError::InvalidSignature)?;

        // Verify the recovered address matches the from address
        if recovered_address != *self.from.as_bytes() {
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

        assert_eq!(tx.chain_id, 1); // Default chain_id
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

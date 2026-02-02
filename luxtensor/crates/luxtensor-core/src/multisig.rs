//! Multisig Transaction Support
//!
//! Provides multi-signature functionality for:
//! - Treasury management (community funds require N-of-M signatures)
//! - Governance proposals (critical changes require multiple validators)
//! - Emergency operations (emergency pause requires multiple signers)

use crate::types::{Address, Hash};
use luxtensor_crypto::keccak256;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Multisig errors
#[derive(Debug, Error)]
pub enum MultisigError {
    #[error("Wallet not found: {0}")]
    WalletNotFound(String),
    #[error("Invalid threshold: {threshold} > {total} signers")]
    InvalidThreshold { threshold: u8, total: u8 },
    #[error("Duplicate signer")]
    DuplicateSigner,
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    #[error("Already signed by this signer")]
    AlreadySigned,
    #[error("Not authorized to sign")]
    NotAuthorized,
    #[error("Transaction already executed")]
    AlreadyExecuted,
    #[error("Transaction expired")]
    Expired,
    #[error("Insufficient signatures: have {have}, need {need}")]
    InsufficientSignatures { have: u8, need: u8 },
}

pub type Result<T> = std::result::Result<T, MultisigError>;

/// Multisig wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigWallet {
    /// Unique wallet ID (derived from signers + threshold)
    pub id: String,
    /// Required number of signatures to execute
    pub threshold: u8,
    /// List of authorized signers
    pub signers: Vec<Address>,
    /// Wallet creation timestamp
    pub created_at: u64,
    /// Optional description/name
    pub name: Option<String>,
}

impl MultisigWallet {
    /// Create a new multisig wallet
    pub fn new(signers: Vec<Address>, threshold: u8, name: Option<String>) -> Result<Self> {
        // Validate threshold
        if threshold == 0 || threshold as usize > signers.len() {
            return Err(MultisigError::InvalidThreshold {
                threshold,
                total: signers.len() as u8,
            });
        }

        // Check for duplicate signers
        let unique_signers: HashSet<_> = signers.iter().collect();
        if unique_signers.len() != signers.len() {
            return Err(MultisigError::DuplicateSigner);
        }

        // Generate wallet ID from signers + threshold
        let mut hasher_input = Vec::new();
        for signer in &signers {
            hasher_input.extend_from_slice(signer.as_ref());
        }
        hasher_input.push(threshold);
        let id = hex::encode(&keccak256(&hasher_input)[..8]);

        Ok(Self {
            id,
            threshold,
            signers,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            name,
        })
    }

    /// Check if address is authorized signer
    pub fn is_signer(&self, address: &Address) -> bool {
        self.signers.contains(address)
    }

    /// Get wallet address (for receiving funds)
    pub fn address(&self) -> Address {
        // Derive wallet address from ID
        let hash = keccak256(self.id.as_bytes());
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash[12..32]);
        Address::new(addr)
    }
}

/// Pending multisig transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMultisigTx {
    /// Unique transaction ID
    pub id: String,
    /// Wallet ID this transaction belongs to
    pub wallet_id: String,
    /// Destination address
    pub to: Address,
    /// Value to transfer (in wei)
    pub value: u128,
    /// Transaction data (for contract calls)
    pub data: Vec<u8>,
    /// Signers who have approved
    pub approvals: Vec<Address>,
    /// When the transaction was proposed
    pub proposed_at: u64,
    /// Expiration time (0 = never expires)
    pub expires_at: u64,
    /// Whether the transaction has been executed
    pub executed: bool,
    /// Execution timestamp (if executed)
    pub executed_at: Option<u64>,
    /// Transaction hash (if executed)
    pub tx_hash: Option<Hash>,
}

impl PendingMultisigTx {
    /// Create a new pending transaction
    pub fn new(
        wallet_id: String,
        to: Address,
        value: u128,
        data: Vec<u8>,
        proposer: Address,
        ttl_seconds: u64,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Generate unique tx ID
        let mut id_input = Vec::new();
        id_input.extend_from_slice(wallet_id.as_bytes());
        id_input.extend_from_slice(to.as_ref());
        id_input.extend_from_slice(&value.to_be_bytes());
        id_input.extend_from_slice(&now.to_be_bytes());
        let id = hex::encode(&keccak256(&id_input)[..12]);

        Self {
            id,
            wallet_id,
            to,
            value,
            data,
            approvals: vec![proposer], // Proposer auto-approves
            proposed_at: now,
            expires_at: if ttl_seconds > 0 { now + ttl_seconds } else { 0 },
            executed: false,
            executed_at: None,
            tx_hash: None,
        }
    }

    /// Check if transaction has expired
    pub fn is_expired(&self) -> bool {
        if self.expires_at == 0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        now > self.expires_at
    }

    /// Get number of approvals
    pub fn approval_count(&self) -> u8 {
        self.approvals.len() as u8
    }

    /// Check if signer has already approved
    pub fn has_approved(&self, signer: &Address) -> bool {
        self.approvals.contains(signer)
    }
}

/// Multisig manager
pub struct MultisigManager {
    /// All registered wallets
    wallets: RwLock<HashMap<String, MultisigWallet>>,
    /// Pending transactions by ID
    pending_txs: RwLock<HashMap<String, PendingMultisigTx>>,
    /// Default TTL for transactions (in seconds)
    default_ttl: u64,
}

impl MultisigManager {
    /// Create new multisig manager
    pub fn new() -> Self {
        Self {
            wallets: RwLock::new(HashMap::new()),
            pending_txs: RwLock::new(HashMap::new()),
            default_ttl: 7 * 24 * 60 * 60, // 7 days
        }
    }

    /// Create a new multisig wallet
    pub fn create_wallet(
        &self,
        signers: Vec<Address>,
        threshold: u8,
        name: Option<String>,
    ) -> Result<MultisigWallet> {
        let wallet = MultisigWallet::new(signers, threshold, name)?;
        let wallet_id = wallet.id.clone();

        let mut wallets = self.wallets.write();
        wallets.insert(wallet_id.clone(), wallet.clone());

        Ok(wallet)
    }

    /// Get wallet by ID
    pub fn get_wallet(&self, wallet_id: &str) -> Option<MultisigWallet> {
        self.wallets.read().get(wallet_id).cloned()
    }

    /// Propose a new transaction
    pub fn propose_transaction(
        &self,
        wallet_id: &str,
        proposer: &Address,
        to: Address,
        value: u128,
        data: Vec<u8>,
    ) -> Result<PendingMultisigTx> {
        // Verify wallet exists
        let wallet = self.get_wallet(wallet_id)
            .ok_or_else(|| MultisigError::WalletNotFound(wallet_id.to_string()))?;

        // Verify proposer is authorized
        if !wallet.is_signer(proposer) {
            return Err(MultisigError::NotAuthorized);
        }

        // Create pending transaction
        let tx = PendingMultisigTx::new(
            wallet_id.to_string(),
            to,
            value,
            data,
            *proposer,
            self.default_ttl,
        );

        let tx_id = tx.id.clone();
        self.pending_txs.write().insert(tx_id.clone(), tx.clone());

        Ok(tx)
    }

    /// Approve a pending transaction
    pub fn approve_transaction(
        &self,
        tx_id: &str,
        signer: &Address,
    ) -> Result<PendingMultisigTx> {
        let mut pending = self.pending_txs.write();
        let tx = pending.get_mut(tx_id)
            .ok_or_else(|| MultisigError::TransactionNotFound(tx_id.to_string()))?;

        // Check if already executed
        if tx.executed {
            return Err(MultisigError::AlreadyExecuted);
        }

        // Check if expired
        if tx.is_expired() {
            return Err(MultisigError::Expired);
        }

        // Verify signer is authorized
        let wallet = self.get_wallet(&tx.wallet_id)
            .ok_or_else(|| MultisigError::WalletNotFound(tx.wallet_id.clone()))?;
        if !wallet.is_signer(signer) {
            return Err(MultisigError::NotAuthorized);
        }

        // Check if already signed
        if tx.has_approved(signer) {
            return Err(MultisigError::AlreadySigned);
        }

        tx.approvals.push(*signer);

        Ok(tx.clone())
    }

    /// Check if transaction can be executed
    pub fn can_execute(&self, tx_id: &str) -> Result<bool> {
        let pending = self.pending_txs.read();
        let tx = pending.get(tx_id)
            .ok_or_else(|| MultisigError::TransactionNotFound(tx_id.to_string()))?;

        if tx.executed {
            return Ok(false);
        }

        if tx.is_expired() {
            return Err(MultisigError::Expired);
        }

        let wallet = self.get_wallet(&tx.wallet_id)
            .ok_or_else(|| MultisigError::WalletNotFound(tx.wallet_id.clone()))?;

        Ok(tx.approval_count() >= wallet.threshold)
    }

    /// Execute a transaction (called by executor after threshold reached)
    pub fn mark_executed(&self, tx_id: &str, tx_hash: Hash) -> Result<()> {
        let mut pending = self.pending_txs.write();
        let tx = pending.get_mut(tx_id)
            .ok_or_else(|| MultisigError::TransactionNotFound(tx_id.to_string()))?;

        let wallet = self.get_wallet(&tx.wallet_id)
            .ok_or_else(|| MultisigError::WalletNotFound(tx.wallet_id.clone()))?;

        if tx.approval_count() < wallet.threshold {
            return Err(MultisigError::InsufficientSignatures {
                have: tx.approval_count(),
                need: wallet.threshold,
            });
        }

        tx.executed = true;
        tx.executed_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        );
        tx.tx_hash = Some(tx_hash);

        Ok(())
    }

    /// Get all pending transactions for a wallet
    pub fn get_pending_for_wallet(&self, wallet_id: &str) -> Vec<PendingMultisigTx> {
        self.pending_txs.read()
            .values()
            .filter(|tx| tx.wallet_id == wallet_id && !tx.executed && !tx.is_expired())
            .cloned()
            .collect()
    }

    /// Get transaction by ID
    pub fn get_transaction(&self, tx_id: &str) -> Option<PendingMultisigTx> {
        self.pending_txs.read().get(tx_id).cloned()
    }

    /// Cleanup expired transactions
    pub fn cleanup_expired(&self) {
        let mut pending = self.pending_txs.write();
        let expired: Vec<String> = pending
            .iter()
            .filter(|(_, tx)| tx.is_expired() && !tx.executed)
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            pending.remove(&id);
        }
    }
}

impl Default for MultisigManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(n: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[19] = n;
        Address::new(addr)
    }

    #[test]
    fn test_create_wallet() {
        let manager = MultisigManager::new();
        let signers = vec![test_address(1), test_address(2), test_address(3)];

        let wallet = manager.create_wallet(signers.clone(), 2, Some("Treasury".to_string())).unwrap();

        assert_eq!(wallet.threshold, 2);
        assert_eq!(wallet.signers.len(), 3);
        assert!(wallet.name.is_some());
    }

    #[test]
    fn test_invalid_threshold() {
        let manager = MultisigManager::new();
        let signers = vec![test_address(1), test_address(2)];

        let result = manager.create_wallet(signers, 3, None); // 3-of-2 invalid
        assert!(result.is_err());
    }

    #[test]
    fn test_propose_and_approve() {
        let manager = MultisigManager::new();
        let signers = vec![test_address(1), test_address(2), test_address(3)];

        let wallet = manager.create_wallet(signers.clone(), 2, None).unwrap();

        // Propose transaction
        let tx = manager.propose_transaction(
            &wallet.id,
            &test_address(1),
            test_address(99),
            1000,
            vec![],
        ).unwrap();

        assert_eq!(tx.approval_count(), 1); // Proposer auto-approves
        assert!(!manager.can_execute(&tx.id).unwrap());

        // Second signer approves
        let tx = manager.approve_transaction(&tx.id, &test_address(2)).unwrap();
        assert_eq!(tx.approval_count(), 2);
        assert!(manager.can_execute(&tx.id).unwrap());
    }

    #[test]
    fn test_unauthorized_signer() {
        let manager = MultisigManager::new();
        let signers = vec![test_address(1), test_address(2)];

        let wallet = manager.create_wallet(signers, 2, None).unwrap();

        // Non-signer tries to propose
        let result = manager.propose_transaction(
            &wallet.id,
            &test_address(99), // Not a signer
            test_address(50),
            1000,
            vec![],
        );

        assert!(matches!(result, Err(MultisigError::NotAuthorized)));
    }
}

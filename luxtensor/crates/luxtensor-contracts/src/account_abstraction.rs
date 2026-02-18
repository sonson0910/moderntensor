// ERC-4337 Account Abstraction Implementation
// Phase 3: Gasless transactions and smart contract wallets

use luxtensor_core::types::{Address, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info};

/// Maximum gas for user operation validation
pub const MAX_VERIFICATION_GAS: u64 = 500_000;
/// Maximum gas for user operation execution
pub const MAX_CALL_GAS: u64 = 3_000_000;
/// Minimum stake required for paymaster
pub const MIN_PAYMASTER_STAKE: u128 = 1_000_000_000_000_000_000; // 1 ETH

/// User Operation for Account Abstraction (ERC-4337)
///
/// This struct represents a pseudo-transaction that can be submitted
/// on behalf of a smart contract wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    /// The account making the operation
    pub sender: Address,
    /// Anti-replay nonce
    pub nonce: u128,
    /// Init code for deploying the sender if not yet deployed
    pub init_code: Vec<u8>,
    /// Data to pass to the sender for execution
    pub call_data: Vec<u8>,
    /// Gas limit for the main execution call
    pub call_gas_limit: u64,
    /// Gas limit for verification step
    pub verification_gas_limit: u64,
    /// Gas to compensate bundler for pre-verification
    pub pre_verification_gas: u64,
    /// Maximum fee per gas (EIP-1559)
    pub max_fee_per_gas: u64,
    /// Maximum priority fee per gas (EIP-1559)
    pub max_priority_fee_per_gas: u64,
    /// Paymaster address and data (empty if self-paying)
    pub paymaster_and_data: Vec<u8>,
    /// Signature over the user operation
    pub signature: Vec<u8>,
}

impl UserOperation {
    /// Calculate the hash of the user operation
    pub fn hash(&self, entry_point: &Address, chain_id: u64) -> Hash {
        use luxtensor_crypto::keccak256;

        let mut data = Vec::new();
        data.extend_from_slice(self.sender.as_bytes());
        data.extend_from_slice(&self.nonce.to_le_bytes());
        data.extend_from_slice(&keccak256(&self.init_code));
        data.extend_from_slice(&keccak256(&self.call_data));
        data.extend_from_slice(&self.call_gas_limit.to_le_bytes());
        data.extend_from_slice(&self.verification_gas_limit.to_le_bytes());
        data.extend_from_slice(&self.pre_verification_gas.to_le_bytes());
        data.extend_from_slice(&self.max_fee_per_gas.to_le_bytes());
        data.extend_from_slice(&self.max_priority_fee_per_gas.to_le_bytes());
        data.extend_from_slice(&keccak256(&self.paymaster_and_data));
        data.extend_from_slice(entry_point.as_bytes());
        data.extend_from_slice(&chain_id.to_le_bytes());

        keccak256(&data)
    }

    /// Get gas required for this operation
    pub fn required_gas(&self) -> u64 {
        self.call_gas_limit + self.verification_gas_limit + self.pre_verification_gas
    }

    /// Check if operation uses a paymaster
    pub fn has_paymaster(&self) -> bool {
        self.paymaster_and_data.len() >= 20
    }

    /// Get paymaster address if present
    pub fn paymaster(&self) -> Option<Address> {
        Address::try_from_slice(&self.paymaster_and_data)
    }

    /// Validate basic constraints
    pub fn validate_basic(&self) -> Result<(), AccountAbstractionError> {
        // Check gas limits
        if self.verification_gas_limit > MAX_VERIFICATION_GAS {
            return Err(AccountAbstractionError::VerificationGasExceeded);
        }
        if self.call_gas_limit > MAX_CALL_GAS {
            return Err(AccountAbstractionError::CallGasExceeded);
        }
        if self.signature.is_empty() {
            return Err(AccountAbstractionError::InvalidSignature);
        }
        Ok(())
    }
}

/// Result of user operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperationReceipt {
    /// Hash of the user operation
    pub user_op_hash: Hash,
    /// Address of the sender
    pub sender: Address,
    /// Nonce of the operation
    pub nonce: u128,
    /// Paymaster used (if any)
    pub paymaster: Option<Address>,
    /// Actual gas used
    pub actual_gas_used: u64,
    /// Actual gas cost
    pub actual_gas_cost: u128,
    /// Whether the operation succeeded
    pub success: bool,
    /// Revert reason if failed
    pub reason: Option<String>,
    /// Transaction hash that included this operation
    pub transaction_hash: Hash,
    /// Block number
    pub block_number: u64,
    /// Block hash
    pub block_hash: Hash,
}

/// Paymaster stake info
#[derive(Debug, Clone)]
pub struct PaymasterInfo {
    pub address: Address,
    pub stake: u128,
    pub unstake_delay_sec: u64,
    pub deposit: u128,
}

/// Account Abstraction Error
#[derive(Debug, Clone, thiserror::Error)]
pub enum AccountAbstractionError {
    #[error("Verification gas limit exceeded")]
    VerificationGasExceeded,
    #[error("Call gas limit exceeded")]
    CallGasExceeded,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid nonce")]
    InvalidNonce,
    #[error("Sender not deployed and no init code")]
    SenderNotDeployed,
    #[error("Paymaster not staked")]
    PaymasterNotStaked,
    #[error("Invalid paymaster address")]
    InvalidPaymaster,
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("User operation expired")]
    Expired,
}

/// EntryPoint contract implementation (ERC-4337)
///
/// This is the singleton contract that handles user operations,
/// validates them, and executes them on the sender's behalf.
pub struct EntryPoint {
    /// Supported entry point addresses
    pub supported_entry_points: Vec<Address>,
    /// User operation nonces per sender
    nonces: Arc<RwLock<HashMap<Address, u128>>>,
    /// Paymaster stakes
    paymasters: Arc<RwLock<HashMap<Address, PaymasterInfo>>>,
    /// Pending user operations (by hash)
    #[allow(dead_code)] // Reserved for operation bundling
    pending_ops: Arc<RwLock<HashMap<Hash, UserOperation>>>,
    /// Executed receipts (by hash)
    receipts: Arc<RwLock<HashMap<Hash, UserOperationReceipt>>>,
    /// Chain ID
    chain_id: u64,
}

impl EntryPoint {
    /// Create a new EntryPoint
    pub fn new(chain_id: u64) -> Self {
        // Default entry point address (standard ERC-4337)
        let entry_point_addr = Address::from([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x43, 0x37, // 0x4337
        ]);

        Self {
            supported_entry_points: vec![entry_point_addr],
            nonces: Arc::new(RwLock::new(HashMap::new())),
            paymasters: Arc::new(RwLock::new(HashMap::new())),
            pending_ops: Arc::new(RwLock::new(HashMap::new())),
            receipts: Arc::new(RwLock::new(HashMap::new())),
            chain_id,
        }
    }

    /// Get supported entry points
    pub fn get_supported_entry_points(&self) -> Vec<String> {
        self.supported_entry_points
            .iter()
            .map(|a| format!("0x{}", hex::encode(a.as_bytes())))
            .collect()
    }

    /// Get chain ID for this entry point
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get nonce for a sender
    pub fn get_nonce(&self, sender: &Address) -> u128 {
        *self.nonces.read().get(sender).unwrap_or(&0)
    }

    /// Validate a user operation
    pub fn validate_user_op(
        &self,
        user_op: &UserOperation,
    ) -> Result<(), AccountAbstractionError> {
        // Basic validation (includes signature non-empty check)
        user_op.validate_basic()?;

        // Verify the signature against the user operation hash
        // The signature must be valid for the sender's address
        let entry_point = self.supported_entry_points.first()
            .ok_or(AccountAbstractionError::InvalidSignature)?;
        let op_hash = user_op.hash(entry_point, self.chain_id);
        if !self.verify_user_op_signature(user_op, &op_hash) {
            return Err(AccountAbstractionError::InvalidSignature);
        }

        // Check nonce
        let expected_nonce = self.get_nonce(&user_op.sender);
        if user_op.nonce != expected_nonce {
            return Err(AccountAbstractionError::InvalidNonce);
        }

        // Check paymaster stake if used
        if user_op.has_paymaster() {
            let paymaster = match user_op.paymaster() {
                Some(p) => p,
                None => return Err(AccountAbstractionError::InvalidPaymaster),
            };
            let paymasters = self.paymasters.read();
            match paymasters.get(&paymaster) {
                Some(info) if info.stake >= MIN_PAYMASTER_STAKE => {}
                _ => return Err(AccountAbstractionError::PaymasterNotStaked),
            }
        }

        debug!("User operation validated: {:?}", user_op.sender);
        Ok(())
    }

    /// Verify the signature on a UserOperation
    /// Returns true if the signature is valid for the sender address
    fn verify_user_op_signature(&self, user_op: &UserOperation, op_hash: &Hash) -> bool {
        // Signature must be at least 64 bytes (compact ECDSA) + 1 byte recovery id
        if user_op.signature.len() < 64 {
            return false;
        }

        let sig_bytes: [u8; 64] = match user_op.signature[..64].try_into() {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Recovery ID is the 65th byte (if present), default to 0
        let recovery_id = if user_op.signature.len() > 64 {
            user_op.signature[64]
        } else {
            0
        };

        // Recover public key from signature
        match luxtensor_crypto::recover_public_key(op_hash, &sig_bytes, recovery_id) {
            Ok(recovered_pubkey) => {
                // Derive address from recovered public key
                match luxtensor_crypto::address_from_public_key(&recovered_pubkey) {
                    Ok(recovered_addr) => {
                        // Compare recovered address with sender
                        recovered_addr.as_bytes() == user_op.sender.as_bytes()
                    }
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    /// Simulate validation of a user operation
    pub fn simulate_validation(
        &self,
        user_op: &UserOperation,
    ) -> Result<SimulationResult, AccountAbstractionError> {
        // Validate
        self.validate_user_op(user_op)?;

        // Estimate gas
        let pre_op_gas = user_op.verification_gas_limit + user_op.pre_verification_gas;

        Ok(SimulationResult {
            pre_op_gas,
            prefund: (user_op.required_gas() as u128).saturating_mul(user_op.max_fee_per_gas as u128),
            valid_after: 0,
            valid_until: u64::MAX,
        })
    }

    /// Queue a validated user operation for inclusion in the next block.
    /// Returns the operation hash.
    pub fn queue_user_op(&self, user_op: UserOperation) -> Hash {
        let ep_addr = &self.supported_entry_points[0];
        let op_hash = user_op.hash(ep_addr, self.chain_id);
        self.pending_ops.write().insert(op_hash, user_op);
        op_hash
    }

    /// Drain all pending user operations for block inclusion.
    /// Returns the ops removed from the pending pool.
    pub fn drain_pending_ops(&self) -> Vec<UserOperation> {
        let mut pending = self.pending_ops.write();
        let ops: Vec<UserOperation> = pending.values().cloned().collect();
        pending.clear();
        ops
    }

    /// Get number of pending user operations
    pub fn pending_count(&self) -> usize {
        self.pending_ops.read().len()
    }

    /// Handle a batch of user operations
    ///
    /// `block_number` and `block_hash` are provided by the block producer and
    /// attached to every receipt so that downstream consumers can locate the
    /// inclusion proof.
    pub fn handle_ops(
        &self,
        ops: Vec<UserOperation>,
        beneficiary: Address,
        block_number: u64,
        block_hash: Hash,
    ) -> Vec<Result<UserOperationReceipt, AccountAbstractionError>> {
        let mut results = Vec::new();

        for op in ops {
            let result = self.handle_single_op(op, &beneficiary, block_number, block_hash);
            results.push(result);
        }

        results
    }

    /// Handle a single user operation
    ///
    /// Executes the user operation: validates, estimates gas, updates nonce,
    /// and records the receipt. Gas cost is computed from the conservative
    /// estimate (verification + pre-verification + calldata cost).
    ///
    /// Full EVM execution of `call_data` on `sender` will be wired once
    /// the block producer passes the shared EvmExecutor into handle_ops.
    fn handle_single_op(
        &self,
        user_op: UserOperation,
        beneficiary: &Address,
        block_number: u64,
        block_hash: Hash,
    ) -> Result<UserOperationReceipt, AccountAbstractionError> {
        let entry_point = &self.supported_entry_points[0];
        let op_hash = user_op.hash(entry_point, self.chain_id);

        // Validate
        self.validate_user_op(&user_op)?;

        // Conservative gas estimate:
        // verification_gas + pre_verification_gas + base call cost + calldata cost
        let verification_gas = user_op.verification_gas_limit.min(MAX_VERIFICATION_GAS);
        let calldata_gas = (user_op.call_data.len() as u64) * 16; // 16 gas per non-zero byte (worst case)
        let base_execution_gas = 21_000u64; // Base transaction cost
        let gas_used = verification_gas
            .saturating_add(user_op.pre_verification_gas)
            .saturating_add(base_execution_gas)
            .saturating_add(calldata_gas)
            .min(user_op.call_gas_limit + user_op.verification_gas_limit + user_op.pre_verification_gas);
        let gas_cost = (gas_used as u128).saturating_mul(user_op.max_fee_per_gas as u128);

        // Update nonce
        {
            let mut nonces = self.nonces.write();
            let nonce = nonces.entry(user_op.sender).or_insert(0);
            *nonce += 1;
        }

        let receipt = UserOperationReceipt {
            user_op_hash: op_hash,
            sender: user_op.sender,
            nonce: user_op.nonce,
            paymaster: user_op.paymaster(),
            actual_gas_used: gas_used,
            actual_gas_cost: gas_cost,
            success: true,
            reason: None,
            transaction_hash: op_hash, // Bundler sets final tx hash when included
            block_number,
            block_hash,
        };

        // Store receipt
        self.receipts.write().insert(op_hash, receipt.clone());

        info!(
            "Executed user operation: sender={:?}, nonce={}, gas_used={}, gas_cost={}, beneficiary={:?}",
            user_op.sender, user_op.nonce, gas_used, gas_cost, beneficiary
        );

        Ok(receipt)
    }

    /// Get receipt for a user operation
    pub fn get_user_op_receipt(&self, op_hash: &Hash) -> Option<UserOperationReceipt> {
        self.receipts.read().get(op_hash).cloned()
    }

    /// Estimate gas for a user operation
    pub fn estimate_user_op_gas(
        &self,
        user_op: &UserOperation,
    ) -> Result<GasEstimate, AccountAbstractionError> {
        // Validate first
        user_op.validate_basic()?;

        // Estimate verification gas
        let verification_gas = if user_op.init_code.is_empty() {
            100_000 // Existing account
        } else {
            200_000 // Account creation
        };

        // Estimate call gas based on call data
        let call_gas = if user_op.call_data.is_empty() {
            21_000 // Base transfer
        } else {
            100_000 + (user_op.call_data.len() as u64 * 16) // Data cost
        };

        // Pre-verification gas
        let pre_verification_gas = 21_000 + (user_op.call_data.len() as u64 * 4);

        Ok(GasEstimate {
            pre_verification_gas,
            verification_gas,
            call_gas,
        })
    }

    /// Add stake for a paymaster
    pub fn add_paymaster_stake(
        &self,
        paymaster: Address,
        stake: u128,
        unstake_delay_sec: u64,
    ) {
        let mut paymasters = self.paymasters.write();
        let info = paymasters.entry(paymaster).or_insert(PaymasterInfo {
            address: paymaster,
            stake: 0,
            unstake_delay_sec: 0,
            deposit: 0,
        });
        info.stake = info.stake.saturating_add(stake);
        info.unstake_delay_sec = unstake_delay_sec;

        info!("Paymaster staked: {:?}, stake={}", paymaster, info.stake);
    }

    /// Deposit for a paymaster
    pub fn deposit_to(&self, paymaster: Address, amount: u128) {
        let mut paymasters = self.paymasters.write();
        let info = paymasters.entry(paymaster).or_insert(PaymasterInfo {
            address: paymaster,
            stake: 0,
            unstake_delay_sec: 0,
            deposit: 0,
        });
        info.deposit = info.deposit.saturating_add(amount);
    }

    /// Get paymaster deposit
    pub fn get_deposit(&self, paymaster: &Address) -> u128 {
        self.paymasters
            .read()
            .get(paymaster)
            .map(|i| i.deposit)
            .unwrap_or(0)
    }
}

/// Simulation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub pre_op_gas: u64,
    pub prefund: u128,
    pub valid_after: u64,
    pub valid_until: u64,
}

/// Gas estimate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEstimate {
    pub pre_verification_gas: u64,
    pub verification_gas: u64,
    pub call_gas: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_crypto::KeyPair;

    /// Well-known test secret key (deterministic).
    const TEST_SECRET: [u8; 32] = [1u8; 32];
    /// Chain ID used by tests.
    const TEST_CHAIN_ID: u64 = 8898;

    /// Create a keypair and derive the sender address from it.
    fn test_keypair() -> KeyPair {
        KeyPair::from_secret(&TEST_SECRET).unwrap()
    }

    /// Sign a UserOperation in-place with the given keypair.
    /// Determines the correct ECDSA recovery id (0 or 1) so that
    /// `recover_public_key` reproduces the signer's address.
    fn sign_user_op(op: &mut UserOperation, keypair: &KeyPair, entry_point: &Address, chain_id: u64) {
        let op_hash = op.hash(entry_point, chain_id);
        let sig = keypair.sign(&op_hash).unwrap();
        let sender_bytes = keypair.address();

        for rid in 0u8..=1 {
            if let Ok(pubkey) = luxtensor_crypto::recover_public_key(&op_hash, &sig, rid) {
                if let Ok(addr) = luxtensor_crypto::address_from_public_key(&pubkey) {
                    if addr == sender_bytes {
                        let mut signature = sig.to_vec();
                        signature.push(rid);
                        op.signature = signature;
                        return;
                    }
                }
            }
        }
        panic!("Could not find valid recovery ID for test signature");
    }

    /// Create a properly-signed test UserOperation.
    fn create_test_user_op() -> UserOperation {
        let keypair = test_keypair();
        let sender = Address::from(keypair.address());

        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let ep_addr = entry_point.supported_entry_points[0];

        let mut op = UserOperation {
            sender,
            nonce: 0,
            init_code: vec![],
            call_data: vec![0x12, 0x34, 0x56, 0x78],
            call_gas_limit: 100_000,
            verification_gas_limit: 100_000,
            pre_verification_gas: 21_000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000,
            paymaster_and_data: vec![],
            signature: vec![0x00], // placeholder — will be overwritten
        };
        sign_user_op(&mut op, &keypair, &ep_addr, TEST_CHAIN_ID);
        op
    }

    #[test]
    fn test_entry_point_creation() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let supported = entry_point.get_supported_entry_points();
        assert_eq!(supported.len(), 1);
    }

    #[test]
    fn test_user_op_hash() {
        let op = create_test_user_op();
        let entry_point = Address::from([0u8; 20]);
        let hash1 = op.hash(&entry_point, 1);
        let hash2 = op.hash(&entry_point, 1);
        assert_eq!(hash1, hash2);

        // Different chain ID should give different hash
        let hash3 = op.hash(&entry_point, 2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_validate_user_op() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let op = create_test_user_op();

        let result = entry_point.validate_user_op(&op);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_nonce() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let keypair = test_keypair();
        let sender = Address::from(keypair.address());
        let ep_addr = entry_point.supported_entry_points[0];

        let mut op = UserOperation {
            sender,
            nonce: 5, // Wrong nonce — expected 0
            init_code: vec![],
            call_data: vec![0x12, 0x34, 0x56, 0x78],
            call_gas_limit: 100_000,
            verification_gas_limit: 100_000,
            pre_verification_gas: 21_000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000,
            paymaster_and_data: vec![],
            signature: vec![0x00],
        };
        // Sign with the wrong nonce so the op passes signature check
        // but fails the nonce check
        sign_user_op(&mut op, &keypair, &ep_addr, TEST_CHAIN_ID);

        let result = entry_point.validate_user_op(&op);
        assert!(matches!(result, Err(AccountAbstractionError::InvalidNonce)));
    }

    #[test]
    fn test_handle_ops() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let op = create_test_user_op();
        let beneficiary = Address::from([2u8; 20]);

        let results = entry_point.handle_ops(vec![op], beneficiary, 42, [0xBBu8; 32]);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());

        let receipt = results[0].as_ref().unwrap();
        assert!(receipt.success);
        assert_eq!(receipt.block_number, 42);
        assert_eq!(receipt.block_hash, [0xBBu8; 32]);
    }

    #[test]
    fn test_nonce_increment() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let keypair = test_keypair();
        let sender = Address::from(keypair.address());

        assert_eq!(entry_point.get_nonce(&sender), 0);

        let op = create_test_user_op();
        let beneficiary = Address::from([2u8; 20]);
        let _ = entry_point.handle_ops(vec![op], beneficiary, 1, [0u8; 32]);

        assert_eq!(entry_point.get_nonce(&sender), 1);
    }

    #[test]
    fn test_estimate_gas() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let op = create_test_user_op();

        let estimate = entry_point.estimate_user_op_gas(&op).unwrap();
        assert!(estimate.verification_gas > 0);
        assert!(estimate.call_gas > 0);
        assert!(estimate.pre_verification_gas > 0);
    }

    #[test]
    fn test_paymaster_stake() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let paymaster = Address::from([3u8; 20]);

        entry_point.add_paymaster_stake(paymaster, MIN_PAYMASTER_STAKE, 86400);

        let deposit = entry_point.get_deposit(&paymaster);
        assert_eq!(deposit, 0);

        entry_point.deposit_to(paymaster, 100);
        assert_eq!(entry_point.get_deposit(&paymaster), 100);
    }

    #[test]
    fn test_user_op_with_paymaster() {
        let entry_point = EntryPoint::new(TEST_CHAIN_ID);
        let keypair = test_keypair();
        let sender = Address::from(keypair.address());
        let paymaster = Address::from([3u8; 20]);
        let ep_addr = entry_point.supported_entry_points[0];

        // Add paymaster stake
        entry_point.add_paymaster_stake(paymaster, MIN_PAYMASTER_STAKE, 86400);

        let mut op = UserOperation {
            sender,
            nonce: 0,
            init_code: vec![],
            call_data: vec![0x12, 0x34, 0x56, 0x78],
            call_gas_limit: 100_000,
            verification_gas_limit: 100_000,
            pre_verification_gas: 21_000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 1_000_000,
            paymaster_and_data: paymaster.as_bytes().to_vec(),
            signature: vec![0x00],
        };
        // Sign AFTER setting paymaster_and_data since it's part of the hash
        sign_user_op(&mut op, &keypair, &ep_addr, TEST_CHAIN_ID);

        let result = entry_point.validate_user_op(&op);
        assert!(result.is_ok());
    }
}

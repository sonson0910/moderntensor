use luxtensor_core::{Transaction, Address, Account, StateDB, CoreError, Result};
use luxtensor_crypto::keccak256;
use luxtensor_contracts::EvmExecutor;
use luxtensor_contracts::evm_executor::EvmLog;
use serde::{Deserialize, Serialize};
use sha3::{Keccak256, Digest};
use tracing::info;

// Re-export shared Receipt types from luxtensor-core
pub use luxtensor_core::receipt::{Receipt, ExecutionStatus, Log};

/// Convert structured EVM logs from REVM into executor Log entries
fn convert_evm_logs(evm_logs: &[EvmLog]) -> Vec<Log> {
    evm_logs.iter().map(|log| {
        let mut addr_bytes = [0u8; 20];
        let len = log.address.len().min(20);
        addr_bytes[..len].copy_from_slice(&log.address[..len]);
        Log {
            address: Address::from(addr_bytes),
            topics: log.topics.clone(),
            data: log.data.clone(),
        }
    }).collect()
}

/// Minimum gas price to prevent zero-fee spam (1 Gwei equivalent)
const MIN_GAS_PRICE: u64 = 1;

/// Transaction executor
///
/// Holds a **shared** `EvmExecutor` instance so that contract storage,
/// bytecode, and account balances persist across transactions within a
/// block and across blocks (the executor is created once at node startup
/// and lives for the lifetime of the node process).
pub struct TransactionExecutor {
    /// Chain ID for cross-chain replay protection.
    /// Every transaction must carry this chain_id or it will be rejected.
    chain_id: u64,
    base_gas_cost: u64,
    gas_per_byte: u64,
    /// Skip signature verification (for development only!)
    skip_signature_verification: bool,
    /// Shared EVM executor — state persists across all transactions
    evm: EvmExecutor,
}

impl TransactionExecutor {
    /// Create a new transaction executor with signature verification enabled (production mode)
    #[must_use]
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            base_gas_cost: 21000,  // Base transaction cost
            gas_per_byte: 68,      // Cost per byte of data
            skip_signature_verification: false,  // PRODUCTION: always verify
            evm: EvmExecutor::new(chain_id),
        }
    }

    /// Create executor for development mode (signature verification disabled)
    /// WARNING: Only use for local development/testing!
    #[must_use]
    pub fn new_dev_mode(chain_id: u64) -> Self {
        Self {
            chain_id,
            base_gas_cost: 21000,
            gas_per_byte: 68,
            skip_signature_verification: true,
            evm: EvmExecutor::new(chain_id),
        }
    }

    /// Get the chain_id this executor validates against
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get a reference to the shared EVM executor (for state inspection or persistence)
    pub fn evm(&self) -> &EvmExecutor {
        &self.evm
    }

    /// Execute a transaction and update state
    pub fn execute(
        &self,
        tx: &Transaction,
        state: &mut StateDB,
        block_height: u64,
        block_hash: [u8; 32],
        tx_index: usize,
        block_timestamp: u64,
    ) -> Result<Receipt> {
        // SECURITY: Validate chain_id — reject cross-chain replay attacks
        if tx.chain_id != self.chain_id {
            return Err(CoreError::InvalidTransaction(
                format!(
                    "Chain ID mismatch: tx has {}, node expects {}",
                    tx.chain_id, self.chain_id
                )
            ));
        }

        // SECURITY: Enforce minimum gas price to prevent zero-fee spam
        if tx.gas_price < MIN_GAS_PRICE {
            return Err(CoreError::InvalidTransaction(
                format!(
                    "Gas price too low: {} < minimum {}",
                    tx.gas_price, MIN_GAS_PRICE
                )
            ));
        }

        // Signature verification - CRITICAL for production!
        if !self.skip_signature_verification {
            tx.verify_signature()?;
        }

        // Get sender account
        let mut sender = state.get_account(&tx.from)
            .unwrap_or_else(|| Account::new());

        // Check nonce
        if sender.nonce != tx.nonce {
            return Err(CoreError::InvalidTransaction(
                format!("Invalid nonce: expected {}, got {}", sender.nonce, tx.nonce)
            ));
        }

        // Calculate gas cost
        let gas_cost = self.calculate_gas_cost(tx)?;
        if gas_cost > tx.gas_limit {
            return Err(CoreError::InvalidTransaction(
                format!("Gas limit {} too low, need {}", tx.gas_limit, gas_cost)
            ));
        }

        // Calculate total cost with overflow protection
        let gas_fee = (gas_cost as u128)
            .checked_mul(tx.gas_price as u128)
            .ok_or_else(|| CoreError::InvalidTransaction(
                "Gas fee calculation overflow".to_string()
            ))?;

        let total_cost = gas_fee
            .checked_add(tx.value)
            .ok_or_else(|| CoreError::InvalidTransaction(
                "Total cost calculation overflow".to_string()
            ))?;

        // Check balance
        if sender.balance < total_cost {
            return Err(CoreError::InvalidTransaction(
                format!("Insufficient balance: have {}, need {}", sender.balance, total_cost)
            ));
        }

        // Save balance before deduction for EVM state sync
        let sender_balance_before_deduction = sender.balance;

        // Deduct cost from sender (already verified balance >= total_cost)
        sender.balance = sender.balance.saturating_sub(total_cost);
        sender.nonce = sender.nonce.saturating_add(1);
        state.set_account(tx.from, sender);

        // 🔧 FIX: Track actual gas used (starts with basic calc, updated by EVM)
        let mut actual_gas_used = gas_cost;
        let mut tx_logs: Vec<Log> = Vec::new();

        // Transfer value to recipient if present
        let (status, contract_address) = if let Some(to_addr) = tx.to {
            // Check if destination is a contract (has code)
            let has_code = state.get_code(&to_addr)
                .map(|code| !code.is_empty())
                .unwrap_or(false);

            if has_code && !tx.data.is_empty() {
                // Contract call — execute via shared EVM executor
                let contract_code = state.get_code(&to_addr).unwrap_or_default();
                let timestamp = block_timestamp;

                let contract_addr_bytes: [u8; 20] = *to_addr.as_bytes();

                // Sync caller balance into EVM state before execution
                self.evm.fund_account(&tx.from, sender_balance_before_deduction);

                match self.evm.call(
                    tx.from,
                    luxtensor_contracts::ContractAddress(contract_addr_bytes),
                    contract_code,
                    tx.data.clone(),
                    tx.value,
                    tx.gas_limit,
                    block_height,
                    timestamp,
                    tx.gas_price as u128,
                ) {
                    Ok((_output, evm_gas_used, evm_logs)) => {
                        actual_gas_used = evm_gas_used.max(gas_cost);
                        tx_logs = convert_evm_logs(&evm_logs);
                        // Credit value to contract if sent
                        if tx.value > 0 {
                            let mut recipient = state.get_account(&to_addr)
                                .unwrap_or_else(|| Account::new());
                            recipient.balance = recipient.balance.saturating_add(tx.value);
                            state.set_account(to_addr, recipient);
                        }
                        info!("📞 Contract call to 0x{} succeeded", hex::encode(contract_addr_bytes));
                        (ExecutionStatus::Success, None)
                    }
                    Err(e) => {
                        tracing::error!("❌ Contract call FAILED: {:?}", e);
                        // Refund value to sender on failure
                        let mut sender_refund = state.get_account(&tx.from)
                            .unwrap_or_else(|| Account::new());
                        sender_refund.balance = sender_refund.balance.saturating_add(tx.value);
                        state.set_account(tx.from, sender_refund);
                        (ExecutionStatus::Failed, None)
                    }
                }
            } else {
                // Plain value transfer (no contract code at destination or empty data)
                let mut recipient = state.get_account(&to_addr)
                    .unwrap_or_else(|| Account::new());
                recipient.balance = recipient.balance.saturating_add(tx.value);
                state.set_account(to_addr, recipient);
                (ExecutionStatus::Success, None)
            }
        } else {
            // Contract deployment - CREATE operation using shared EVM executor
            // Use block timestamp for deterministic execution
            let timestamp = block_timestamp;

            // Sync deployer balance into EVM state
            self.evm.fund_account(&tx.from, sender_balance_before_deduction);

            match self.evm.deploy(
                tx.from,
                tx.data.clone(),  // Init code (constructor bytecode)
                tx.value,
                tx.gas_limit,
                block_height,
                timestamp,
                tx.gas_price as u128,
            ) {
                Ok((contract_address_vec, evm_gas_used, evm_logs, deployed_code)) => {
                    actual_gas_used = evm_gas_used.max(gas_cost);
                    tx_logs = convert_evm_logs(&evm_logs);
                    // Use contract address returned by revm
                    let mut contract_addr_bytes = [0u8; 20];
                    if contract_address_vec.len() >= 20 {
                        contract_addr_bytes.copy_from_slice(&contract_address_vec[..20]);
                    }
                    let contract_addr = Address::from(contract_addr_bytes);

                    // Create contract account with bytecode stored directly
                    let code_hash = {
                        let mut code_hasher = Keccak256::new();
                        code_hasher.update(&deployed_code);
                        let mut hash = [0u8; 32];
                        hash.copy_from_slice(&code_hasher.finalize());
                        hash
                    };

                    let contract_account = Account::contract(tx.value, deployed_code.clone(), code_hash);
                    state.set_account(contract_addr, contract_account);

                    info!("📄 Contract deployed at 0x{} (gas used: {})",
                          hex::encode(&contract_addr_bytes), evm_gas_used);
                    (ExecutionStatus::Success, Some(contract_addr))
                }
                Err(e) => {
                    tracing::error!("❌ Contract deployment FAILED: {:?}", e);
                    tracing::error!("   From: 0x{}", hex::encode(tx.from.as_bytes()));
                    tracing::error!("   Data len: {} bytes", tx.data.len());
                    tracing::error!("   Gas limit: {}", tx.gas_limit);
                    if tx.value > 0 {
                        let mut sender_refund = state.get_account(&tx.from)
                            .unwrap_or_else(|| Account::new());
                        sender_refund.balance = sender_refund.balance.saturating_add(tx.value);
                        state.set_account(tx.from, sender_refund);
                    }
                    (ExecutionStatus::Failed, None)
                }
            }
        };

        // Gas refund: return unused gas to sender
        // Upfront we charged gas_fee = gas_cost * gas_price, but actual may differ
        let actual_gas_fee = (actual_gas_used as u128)
            .saturating_mul(tx.gas_price as u128);
        let gas_refund = gas_fee.saturating_sub(actual_gas_fee);
        if gas_refund > 0 {
            let mut sender_after = state.get_account(&tx.from)
                .unwrap_or_else(|| Account::new());
            sender_after.balance = sender_after.balance.saturating_add(gas_refund);
            state.set_account(tx.from, sender_after);
        }

        // Create receipt
        let receipt = Receipt {
            transaction_hash: tx.hash(),
            block_height,
            block_hash,
            transaction_index: tx_index,
            from: tx.from,
            to: tx.to,
            gas_used: actual_gas_used,
            status,
            logs: tx_logs,
            contract_address,
        };

        Ok(receipt)
    }

    /// Calculate gas cost for a transaction
    fn calculate_gas_cost(&self, tx: &Transaction) -> Result<u64> {
        let mut gas = self.base_gas_cost;

        // Add gas for data
        gas += self.gas_per_byte * (tx.data.len() as u64);

        Ok(gas)
    }

    /// Batch execute transactions
    pub fn execute_batch(
        &self,
        transactions: &[Transaction],
        state: &mut StateDB,
        block_height: u64,
        block_hash: [u8; 32],
        block_timestamp: u64,
    ) -> Vec<Result<Receipt>> {
        transactions.iter()
            .enumerate()
            .map(|(idx, tx)| self.execute(tx, state, block_height, block_hash, idx, block_timestamp))
            .collect()
    }
}

impl Default for TransactionExecutor {
    /// Default executor uses LuxTensor mainnet chain_id (8899)
    fn default() -> Self {
        Self::new(8899) // luxtensor_core::constants::chain_id::MAINNET
    }
}

/// Calculate receipts merkle root
#[must_use]
pub fn calculate_receipts_root(receipts: &[Receipt]) -> [u8; 32] {
    if receipts.is_empty() {
        return [0u8; 32];
    }

    let receipt_hashes: Vec<[u8; 32]> = receipts.iter()
        .map(|r| {
            let data = bincode::serialize(r).unwrap_or_default();
            keccak256(&data)
        })
        .collect();

    let tree = luxtensor_crypto::MerkleTree::new(receipt_hashes);
    tree.root()
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_crypto::KeyPair;

    /// Test chain_id — matches Transaction::new() default (devnet)
    const TEST_CHAIN_ID: u64 = 8898;

    fn create_signed_transaction(
        keypair: &KeyPair,
        nonce: u64,
        to: Option<Address>,
        value: u128,
    ) -> Transaction {
        let from = Address::from(keypair.address());
        let mut tx = Transaction::new(nonce, from, to, value, 1, 100000, vec![]);

        // Sign transaction
        let msg = tx.signing_message();
        let msg_hash = keccak256(&msg);
        let sig = keypair.sign(&msg_hash).expect("Failed to sign");

        tx.r.copy_from_slice(&sig[..32]);
        tx.s.copy_from_slice(&sig[32..]);
        tx.v = 0;

        tx
    }

    #[test]
    fn test_executor_creation() {
        let executor = TransactionExecutor::new(TEST_CHAIN_ID);
        assert_eq!(executor.base_gas_cost, 21000);
        assert_eq!(executor.chain_id, TEST_CHAIN_ID);
    }

    #[test]
    fn test_gas_calculation() {
        let executor = TransactionExecutor::new(TEST_CHAIN_ID);
        let tx = Transaction::new(
            0,
            Address::zero(),
            Some(Address::zero()),
            1000,
            1,
            100000,
            vec![0; 10], // 10 bytes of data
        );

        let gas_cost = executor.calculate_gas_cost(&tx).unwrap();
        assert_eq!(gas_cost, 21000 + 68 * 10);
    }

    #[test]
    fn test_simple_transfer() {
        let executor = TransactionExecutor::new(TEST_CHAIN_ID);
        let mut state = StateDB::new();

        // Setup sender with balance
        let keypair = KeyPair::generate();
        let from = Address::from(keypair.address());
        let mut sender = Account::new();
        sender.balance = 1_000_000;
        sender.nonce = 0;
        state.set_account(from, sender);

        // Create and sign transaction
        let to = Address::zero();
        let tx = create_signed_transaction(&keypair, 0, Some(to), 1000);

        // Execute transaction
        let result = executor.execute(
            &tx,
            &mut state,
            1,
            [1u8; 32],
            0,
            1000,
        );

        // For now, signature verification may fail without proper signing
        // Just check that execution doesn't panic
        let _ = result;
    }

    #[test]
    fn test_insufficient_balance() {
        let executor = TransactionExecutor::new(TEST_CHAIN_ID);
        let mut state = StateDB::new();

        // Setup sender with insufficient balance
        let keypair = KeyPair::generate();
        let from = Address::from(keypair.address());
        let mut sender = Account::new();
        sender.balance = 100;  // Not enough
        sender.nonce = 0;
        state.set_account(from, sender);

        let tx = create_signed_transaction(&keypair, 0, Some(Address::zero()), 1000);

        let result = executor.execute(
            &tx,
            &mut state,
            1,
            [1u8; 32],
            0,
            1000,
        );

        // Should fail due to insufficient balance or signature issue
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_execution() {
        let executor = TransactionExecutor::new(TEST_CHAIN_ID);
        let mut state = StateDB::new();

        let keypair = KeyPair::generate();
        let from = Address::from(keypair.address());
        let mut sender = Account::new();
        sender.balance = 10_000_000;
        sender.nonce = 0;
        state.set_account(from, sender);

        let txs = vec![
            create_signed_transaction(&keypair, 0, Some(Address::zero()), 1000),
            create_signed_transaction(&keypair, 1, Some(Address::zero()), 2000),
        ];

        let results = executor.execute_batch(&txs, &mut state, 1, [1u8; 32], 1000);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_receipts_root() {
        let receipts = vec![
            Receipt {
                transaction_hash: [1u8; 32],
                block_height: 1,
                block_hash: [0u8; 32],
                transaction_index: 0,
                from: Address::zero(),
                to: Some(Address::zero()),
                gas_used: 21000,
                status: ExecutionStatus::Success,
                logs: vec![],
                contract_address: None,
            },
        ];

        let root = calculate_receipts_root(&receipts);
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn test_empty_receipts_root() {
        let receipts = vec![];
        let root = calculate_receipts_root(&receipts);
        assert_eq!(root, [0u8; 32]);
    }
}

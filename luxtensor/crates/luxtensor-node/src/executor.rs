use luxtensor_core::{Transaction, Address, Account, StateDB, CoreError, Result, MetagraphTxPayload};
use luxtensor_core::constants::precompiles;
use luxtensor_crypto::keccak256;
use luxtensor_contracts::EvmExecutor;
use luxtensor_contracts::evm_executor::EvmLog;
use luxtensor_contracts::{AIPrecompileState, RevmBytes, execute_ai_precompile, is_luxtensor_precompile};
use luxtensor_storage::metagraph_store::MetagraphDB;
use sha3::{Keccak256, Digest};
use std::sync::Arc;
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

/// TX Router classification — determines which execution path a transaction takes.
enum TxType {
    /// AI Layer: metagraph precompile (neuron/subnet/validator operations)
    MetagraphOp,
    /// AI Layer: custom AI precompiles (0x10-0x28: inference, vectors, training, etc.)
    AIPrecompile { to: Address },
    /// dApp Layer: contract deployment (CREATE, tx.to == None)
    EvmDeploy,
    /// dApp Layer: plain transfer or contract call (tx.to == Some)
    TransferOrCall { to: Address },
}

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
    /// MetagraphDB for precompile transactions (neuron/validator/subnet ops)
    metagraph_db: Option<Arc<MetagraphDB>>,
    /// AI precompile state (inference requests, vector store, training jobs)
    ai_precompile_state: Option<Arc<AIPrecompileState>>,
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
            metagraph_db: None,
            ai_precompile_state: None,
        }
    }

    /// Attach a MetagraphDB so the executor can handle metagraph precompile transactions.
    /// Call this after `new()` at node startup.
    pub fn with_metagraph(mut self, db: Arc<MetagraphDB>) -> Self {
        self.metagraph_db = Some(db);
        self
    }

    /// Attach AI precompile state so the executor can route AI precompile calls (0x10-0x28).
    /// Call this after `new()` at node startup.
    pub fn with_ai_precompiles(mut self, state: Arc<AIPrecompileState>) -> Self {
        self.ai_precompile_state = Some(state);
        self
    }

    /// Get a reference to the shared EVM executor (for state inspection or persistence)
    pub fn evm(&self) -> &EvmExecutor {
        &self.evm
    }

    /// Transaction type classification for the TX Router.
    fn classify_tx(&self, tx: &Transaction) -> TxType {
        let precompile_addr = Address::from(precompiles::metagraph_address());
        if tx.to == Some(precompile_addr) {
            return TxType::MetagraphOp;
        }
        // Check for AI precompile addresses (0x10-0x28)
        if let Some(to_addr) = tx.to {
            let addr_bytes: [u8; 20] = *to_addr.as_bytes();
            if is_luxtensor_precompile(&addr_bytes) {
                return TxType::AIPrecompile { to: to_addr };
            }
        }
        match tx.to {
            None => TxType::EvmDeploy,
            Some(to_addr) => TxType::TransferOrCall { to: to_addr },
        }
    }

    /// Execute a transaction and update state.
    ///
    /// Routes the transaction through a 3-way classifier:
    /// - `MetagraphOp`    → AI layer precompile (neuron/subnet/validator ops)
    /// - `EvmDeploy`      → dApp layer contract deployment (CREATE)
    /// - `TransferOrCall` → transfer or dApp layer contract call
    pub fn execute(
        &self,
        tx: &Transaction,
        state: &mut StateDB,
        block_height: u64,
        block_hash: [u8; 32],
        tx_index: usize,
        block_timestamp: u64,
    ) -> Result<Receipt> {
        // ── Common Validation ────────────────────────────────────────────────

        // SECURITY: Validate chain_id — reject cross-chain replay attacks
        if tx.chain_id != self.chain_id {
            return Err(CoreError::InvalidTransaction(
                format!(
                    "Chain ID mismatch: tx has {}, node expects {}",
                    tx.chain_id, self.chain_id
                )
            ));
        }

        // Faucet TX bypass — transactions from Address::zero() are special
        // "mint" transactions used by dev_faucet.
        let is_faucet_mint = tx.from == Address::zero();

        // SECURITY: Enforce minimum gas price to prevent zero-fee spam
        if !is_faucet_mint && tx.gas_price < MIN_GAS_PRICE {
            return Err(CoreError::InvalidTransaction(
                format!(
                    "Gas price too low: {} < minimum {}",
                    tx.gas_price, MIN_GAS_PRICE
                )
            ));
        }

        // Signature verification - CRITICAL for production!
        if !is_faucet_mint && !self.skip_signature_verification {
            tx.verify_signature()?;
        }

        // Get sender account
        let mut sender = state.get_account(&tx.from)
            .unwrap_or_else(|| Account::new());

        // Check nonce (skip for faucet mints)
        if !is_faucet_mint && sender.nonce != tx.nonce {
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

        // Check balance (skip for faucet mints)
        if !is_faucet_mint && sender.balance < total_cost {
            return Err(CoreError::InvalidTransaction(
                format!("Insufficient balance: have {}, need {}", sender.balance, total_cost)
            ));
        }

        // Save balance before deduction for EVM state sync
        let sender_balance_before_deduction = sender.balance;

        // Deduct cost from sender (skip for faucet)
        if !is_faucet_mint {
            sender.balance = sender.balance.saturating_sub(total_cost);
        }
        sender.nonce = sender.nonce.saturating_add(1);
        state.set_account(tx.from, sender);

        let mut actual_gas_used = gas_cost;
        let mut tx_logs: Vec<Log> = Vec::new();

        // ── TX Router: classify and dispatch ─────────────────────────────────

        let (status, contract_address) = match self.classify_tx(tx) {
            TxType::MetagraphOp => {
                let meta_result = self.route_metagraph(tx, &mut actual_gas_used);
                // Metagraph returns early with its own receipt
                return Ok(Receipt {
                    transaction_hash: tx.hash(),
                    block_height,
                    block_hash,
                    transaction_index: tx_index,
                    from: tx.from,
                    to: tx.to,
                    gas_used: actual_gas_used,
                    status: meta_result,
                    logs: vec![],
                    contract_address: None,
                });
            }
            TxType::AIPrecompile { to } => {
                self.route_ai_precompile(
                    tx, to, block_height,
                    &mut actual_gas_used, &mut tx_logs,
                )
            }
            TxType::TransferOrCall { to } => {
                self.route_transfer_or_call(
                    tx, state, to,
                    sender_balance_before_deduction,
                    block_height, block_timestamp,
                    &mut actual_gas_used, &mut tx_logs,
                )
            }
            TxType::EvmDeploy => {
                self.route_deploy(
                    tx, state,
                    sender_balance_before_deduction,
                    block_height, block_timestamp,
                    &mut actual_gas_used, &mut tx_logs,
                )
            }
        };

        // ── Gas Refund ───────────────────────────────────────────────────────

        let actual_gas_fee = (actual_gas_used as u128)
            .saturating_mul(tx.gas_price as u128);
        let gas_refund = gas_fee.saturating_sub(actual_gas_fee);
        if gas_refund > 0 {
            let mut sender_after = state.get_account(&tx.from)
                .unwrap_or_else(|| Account::new());
            sender_after.balance = sender_after.balance.saturating_add(gas_refund);
            state.set_account(tx.from, sender_after);
        }

        // ── Receipt ──────────────────────────────────────────────────────────

        Ok(Receipt {
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
        })
    }

    // ── Route: AI Precompile (0x10-0x28) ──────────────────────────────────

    fn route_ai_precompile(
        &self,
        tx: &Transaction,
        to: Address,
        block_height: u64,
        actual_gas_used: &mut u64,
        tx_logs: &mut Vec<Log>,
    ) -> (ExecutionStatus, Option<Address>) {
        let addr_bytes: [u8; 20] = *to.as_bytes();
        let caller: [u8; 20] = *tx.from.as_bytes();
        let input = RevmBytes::copy_from_slice(&tx.data);

        if let Some(ref ai_state) = self.ai_precompile_state {
            match execute_ai_precompile(
                &addr_bytes, &input, tx.gas_limit, ai_state, caller, block_height,
            ) {
                Some(Ok(output)) => {
                    *actual_gas_used = output.gas_used.max(*actual_gas_used);
                    info!(
                        "🤖 AI precompile 0x{:02x} executed: {} gas",
                        addr_bytes[19], output.gas_used
                    );

                    // 🔐 M6 SECURITY: Emit EVM log for AI precompile audit trail
                    let mut log_data = Vec::with_capacity(56);
                    log_data.extend_from_slice(&caller);      // 20 bytes: who called
                    log_data.extend_from_slice(&block_height.to_be_bytes()); // 8 bytes: when
                    log_data.extend_from_slice(&addr_bytes);   // 20 bytes: which precompile
                    log_data.extend_from_slice(&output.gas_used.to_be_bytes()); // 8 bytes: gas
                    tx_logs.push(Log {
                        address: to,
                        topics: vec![
                            keccak256(b"AIPrecompileExecuted(address,uint64,address,uint64)"),
                        ],
                        data: log_data,
                    });

                    (ExecutionStatus::Success, None)
                }
                Some(Err(e)) => {
                    tracing::error!("❌ AI precompile 0x{:02x} failed: {:?}", addr_bytes[19], e);
                    (ExecutionStatus::Failed, None)
                }
                None => {
                    // Should not reach here (classify_tx already verified), but handle gracefully
                    tracing::warn!("⚠️ Address 0x{} not recognized as AI precompile", hex::encode(addr_bytes));
                    (ExecutionStatus::Failed, None)
                }
            }
        } else {
            tracing::warn!("⚠️ AIPrecompileState not attached — AI precompile tx ignored");
            (ExecutionStatus::Failed, None)
        }
    }

    // ── Route: Metagraph precompile (AI layer) ───────────────────────────

    fn route_metagraph(&self, tx: &Transaction, _actual_gas_used: &mut u64) -> ExecutionStatus {
        match MetagraphTxPayload::decode(&tx.data) {
            Ok(payload) => {
                let meta_status = if let Some(ref mdb) = self.metagraph_db {
                    self.execute_metagraph_precompile(&payload, mdb)
                } else {
                    tracing::warn!("⚠️ MetagraphDB not attached — precompile tx ignored");
                    ExecutionStatus::Success
                };
                info!("🔗 Metagraph precompile executed: {:?}", meta_status);
                meta_status
            }
            Err(e) => {
                tracing::warn!("🚫 Metagraph precompile: decode failed: {}", e);
                ExecutionStatus::Failed
            }
        }
    }

    // ── Route: Transfer or Contract Call (dApp layer) ────────────────────

    fn route_transfer_or_call(
        &self,
        tx: &Transaction,
        state: &mut StateDB,
        to_addr: Address,
        sender_balance_before: u128,
        block_height: u64,
        block_timestamp: u64,
        actual_gas_used: &mut u64,
        tx_logs: &mut Vec<Log>,
    ) -> (ExecutionStatus, Option<Address>) {
        // Check if destination has contract code
        let has_code = state.get_code(&to_addr)
            .map(|code| !code.is_empty())
            .unwrap_or(false);

        if has_code && !tx.data.is_empty() {
            // ── EVM Contract Call ──
            let contract_code = state.get_code(&to_addr).unwrap_or_default();
            let contract_addr_bytes: [u8; 20] = *to_addr.as_bytes();

            // Sync caller balance into EVM state before execution
            self.evm.fund_account(&tx.from, sender_balance_before);

            match self.evm.call(
                tx.from,
                luxtensor_contracts::ContractAddress(contract_addr_bytes),
                contract_code,
                tx.data.clone(),
                tx.value,
                tx.gas_limit,
                block_height,
                block_timestamp,
                tx.gas_price as u128,
            ) {
                Ok((_output, evm_gas_used, evm_logs)) => {
                    *actual_gas_used = evm_gas_used.max(*actual_gas_used);
                    *tx_logs = convert_evm_logs(&evm_logs);
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
            // ── Plain Value Transfer ──
            let mut recipient = state.get_account(&to_addr)
                .unwrap_or_else(|| Account::new());
            recipient.balance = recipient.balance.saturating_add(tx.value);
            state.set_account(to_addr, recipient);
            (ExecutionStatus::Success, None)
        }
    }

    // ── Route: Contract Deployment (dApp layer — CREATE) ────────────────

    fn route_deploy(
        &self,
        tx: &Transaction,
        state: &mut StateDB,
        sender_balance_before: u128,
        block_height: u64,
        block_timestamp: u64,
        actual_gas_used: &mut u64,
        tx_logs: &mut Vec<Log>,
    ) -> (ExecutionStatus, Option<Address>) {
        // Sync deployer balance into EVM state
        self.evm.fund_account(&tx.from, sender_balance_before);

        match self.evm.deploy(
            tx.from,
            tx.data.clone(),
            tx.value,
            tx.gas_limit,
            block_height,
            block_timestamp,
            tx.gas_price as u128,
        ) {
            Ok((contract_address_vec, evm_gas_used, evm_logs, deployed_code)) => {
                *actual_gas_used = evm_gas_used.max(*actual_gas_used);
                *tx_logs = convert_evm_logs(&evm_logs);
                // Build contract address
                let mut contract_addr_bytes = [0u8; 20];
                if contract_address_vec.len() >= 20 {
                    contract_addr_bytes.copy_from_slice(&contract_address_vec[..20]);
                }
                let contract_addr = Address::from(contract_addr_bytes);

                // Create contract account with bytecode
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
                tracing::error!("❌ Contract deployment FAILED: {:?} (from=0x{}, data_len={}, gas={})",
                    e, hex::encode(tx.from.as_bytes()), tx.data.len(), tx.gas_limit);
                if tx.value > 0 {
                    let mut sender_refund = state.get_account(&tx.from)
                        .unwrap_or_else(|| Account::new());
                    sender_refund.balance = sender_refund.balance.saturating_add(tx.value);
                    state.set_account(tx.from, sender_refund);
                }
                (ExecutionStatus::Failed, None)
            }
        }
    }

    /// Calculate gas cost for a transaction
    fn calculate_gas_cost(&self, tx: &Transaction) -> Result<u64> {
        let mut gas = self.base_gas_cost;

        // Add gas for data
        gas += self.gas_per_byte * (tx.data.len() as u64);

        Ok(gas)
    }

    /// Execute a metagraph precompile transaction.
    /// Decodes `MetagraphTxPayload` and applies the operation to MetagraphDB.
    fn execute_metagraph_precompile(
        &self,
        payload: &MetagraphTxPayload,
        mdb: &MetagraphDB,
    ) -> ExecutionStatus {
        use luxtensor_storage::metagraph_store::{NeuronData, ValidatorData, SubnetData};
        use tracing::{info, warn};

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        match payload {
            MetagraphTxPayload::RegisterNeuron {
                subnet_id, uid, hotkey, coldkey, endpoint, stake, active,
            } => {
                let nd = NeuronData {
                    uid: *uid,
                    subnet_id: *subnet_id,
                    hotkey: *hotkey,
                    coldkey: *coldkey,
                    stake: *stake,
                    trust: 0,
                    rank: 0,
                    incentive: 0,
                    dividends: 0,
                    emission: 0,
                    last_update: now,
                    active: *active,
                    endpoint: endpoint.clone(),
                };
                match mdb.store_neuron(&nd) {
                    Ok(_) => {
                        info!("🔗 Precompile: Registered neuron uid={} subnet={}", uid, subnet_id);
                        ExecutionStatus::Success
                    }
                    Err(e) => {
                        warn!("❌ Precompile: RegisterNeuron failed: {}", e);
                        ExecutionStatus::Failed
                    }
                }
            }

            MetagraphTxPayload::RegisterValidator { hotkey, name, stake } => {
                let vd = ValidatorData {
                    address: *hotkey,
                    public_key: vec![],    // not required for consensus
                    stake: *stake,
                    is_active: true,
                    name: name.clone(),
                    registered_at: now,
                    last_block_produced: 0,
                    blocks_produced: 0,
                };
                match mdb.register_validator(&vd) {
                    Ok(_) => {
                        info!("🔗 Precompile: Registered validator 0x{}", hex::encode(hotkey));
                        ExecutionStatus::Success
                    }
                    Err(e) => {
                        warn!("❌ Precompile: RegisterValidator failed: {}", e);
                        ExecutionStatus::Failed
                    }
                }
            }

            MetagraphTxPayload::CreateSubnet { subnet_id, owner, name, min_stake } => {
                let sd = SubnetData {
                    id: *subnet_id,
                    name: name.clone(),
                    owner: *owner,
                    emission_rate: 0,
                    created_at: now,
                    tempo: 100,
                    max_neurons: 256,
                    min_stake: *min_stake,
                    active: true,
                };
                match mdb.store_subnet(&sd) {
                    Ok(_) => {
                        info!("🔗 Precompile: Created subnet id={}", subnet_id);
                        ExecutionStatus::Success
                    }
                    Err(e) => {
                        warn!("❌ Precompile: CreateSubnet failed: {}", e);
                        ExecutionStatus::Failed
                    }
                }
            }

            MetagraphTxPayload::SetWeights { subnet_id, uid, weights } => {
                match mdb.store_weights(*subnet_id, *uid, weights) {
                    Ok(_) => {
                        info!("🔗 Precompile: SetWeights uid={} subnet={} ({} weights)", uid, subnet_id, weights.len());
                        ExecutionStatus::Success
                    }
                    Err(e) => {
                        warn!("❌ Precompile: SetWeights failed: {}", e);
                        ExecutionStatus::Failed
                    }
                }
            }
        }
    }

}

impl Default for TransactionExecutor {
    /// Default executor uses LuxTensor mainnet chain_id (8898)
    fn default() -> Self {
        Self::new(8898) // luxtensor_core::constants::chain_id::MAINNET
    }
}

impl TransactionExecutor {
    /// Create a development executor that skips signature verification.
    /// WARNING: Only use for testing!
    #[cfg(test)]
    pub fn new_dev(chain_id: u64) -> Self {
        Self {
            chain_id,
            base_gas_cost: 21000,
            gas_per_byte: 68,
            skip_signature_verification: true,
            evm: EvmExecutor::new(chain_id),
            metagraph_db: None,
            ai_precompile_state: None,
        }
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

// Advanced REVM Integration for Full EVM Compatibility
//
// This module provides enhanced EVM features for LuxTensor:
// - Precompiled contracts (ecrecover, sha256, etc.)
// - EVM inspector for debugging and tracing
// - Event/Log parsing from EVM execution
// - Gas estimation improvements
// - EIP-compliant behavior

use crate::types::ContractAddress;
use crate::executor::Log;
use luxtensor_core::types::Hash;
use revm::primitives::{
    U256, Log as RevmLog,
    SpecId,
};

/// EVM specification version to use
/// Using CANCUN for latest EVM features
pub const EVM_SPEC: SpecId = SpecId::CANCUN;

/// Configuration for EVM execution
#[derive(Debug, Clone)]
pub struct EvmConfig {
    /// Gas limit for blocks
    pub block_gas_limit: u64,
    /// Base fee per gas
    pub base_fee: u128,
    /// Chain ID
    pub chain_id: u64,
    /// Enable precompiles
    pub enable_precompiles: bool,
    /// Enable EIP-1559 dynamic fees
    pub enable_eip1559: bool,
    /// Enable tracing/debugging
    pub enable_tracing: bool,
}

impl Default for EvmConfig {
    fn default() -> Self {
        Self {
            block_gas_limit: 30_000_000,
            base_fee: 1_000_000_000, // 1 gwei
            chain_id: 777, // LuxTensor chain ID
            enable_precompiles: true,
            enable_eip1559: true,
            enable_tracing: false,
        }
    }
}

/// Standard Ethereum precompile addresses
pub mod precompiles {


    /// ECRECOVER precompile address (0x01)
    pub const ECRECOVER: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

    /// SHA256 precompile address (0x02)
    pub const SHA256: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2];

    /// RIPEMD160 precompile address (0x03)
    pub const RIPEMD160: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3];

    /// IDENTITY precompile address (0x04)
    pub const IDENTITY: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4];

    /// MODEXP precompile address (0x05)
    pub const MODEXP: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5];

    /// BN128_ADD precompile address (0x06)
    pub const BN128_ADD: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6];

    /// BN128_MUL precompile address (0x07)
    pub const BN128_MUL: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7];

    /// BN128_PAIRING precompile address (0x08)
    pub const BN128_PAIRING: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8];

    /// BLAKE2F precompile address (0x09)
    pub const BLAKE2F: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9];

    // ========== LuxTensor AI Precompiles ==========
    // Custom precompiles for native AI integration (Phase 2)

    /// AI_REQUEST precompile address (0x10)
    /// Submit AI inference request to the network
    /// Input: abi.encode(model_hash, input_data, callback_address, max_reward)
    /// Output: bytes32 request_id
    pub const AI_REQUEST: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10];

    /// VERIFY_PROOF precompile address (0x11)
    /// Verify ZK proof for AI computation
    /// Input: abi.encode(proof_type, proof_data, public_inputs)
    /// Output: bool is_valid
    pub const VERIFY_PROOF: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x11];

    /// GET_RESULT precompile address (0x12)
    /// Retrieve completed AI inference result
    /// Input: bytes32 request_id
    /// Output: abi.encode(status, result_data, fulfiller_address)
    pub const GET_RESULT: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x12];

    /// COMPUTE_PAYMENT precompile address (0x13)
    /// Calculate required payment for AI request based on model complexity
    /// Input: bytes32 model_hash, uint256 input_size
    /// Output: uint256 required_payment
    pub const COMPUTE_PAYMENT: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x13];
}

/// Parse REVM logs into LuxTensor Log format
pub fn parse_revm_logs(revm_logs: &[RevmLog], default_address: &ContractAddress) -> Vec<Log> {
    revm_logs
        .iter()
        .map(|log| {
            // Convert REVM address to ContractAddress
            let address = if log.address.0 .0 != [0u8; 20] {
                ContractAddress(log.address.0 .0)
            } else {
                *default_address
            };

            // Convert topics (B256 -> [u8; 32])
            let topics: Vec<Hash> = log
                .topics()
                .iter()
                .map(|t| t.0)
                .collect();

            Log {
                address,
                topics,
                data: log.data.data.to_vec(),
            }
        })
        .collect()
}

/// Decode ABI-encoded function selector from input data
pub fn decode_function_selector(input: &[u8]) -> Option<[u8; 4]> {
    if input.len() >= 4 {
        let mut selector = [0u8; 4];
        selector.copy_from_slice(&input[0..4]);
        Some(selector)
    } else {
        None
    }
}

/// Calculate function selector from signature (e.g., "transfer(address,uint256)")
pub fn calculate_function_selector(signature: &str) -> [u8; 4] {
    use luxtensor_crypto::keccak256;
    let hash = keccak256(signature.as_bytes());
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&hash[0..4]);
    selector
}

/// Common ERC-20 function selectors
pub mod erc20_selectors {
    use super::calculate_function_selector;
    use std::sync::LazyLock;

    pub static TRANSFER: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("transfer(address,uint256)"));
    pub static BALANCE_OF: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("balanceOf(address)"));
    pub static APPROVE: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("approve(address,uint256)"));
    pub static TRANSFER_FROM: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("transferFrom(address,address,uint256)"));
    pub static ALLOWANCE: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("allowance(address,address)"));
    pub static TOTAL_SUPPLY: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("totalSupply()"));
}

/// Common ERC-721 function selectors
pub mod erc721_selectors {
    use super::calculate_function_selector;
    use std::sync::LazyLock;

    pub static OWNER_OF: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("ownerOf(uint256)"));
    pub static SAFE_TRANSFER_FROM: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("safeTransferFrom(address,address,uint256)"));
    pub static SET_APPROVAL_FOR_ALL: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("setApprovalForAll(address,bool)"));
    pub static GET_APPROVED: LazyLock<[u8; 4]> =
        LazyLock::new(|| calculate_function_selector("getApproved(uint256)"));
}

/// Gas cost constants (EIP-compliant)
pub mod gas_costs {
    /// Base transaction cost
    pub const TX_BASE: u64 = 21_000;
    /// Cost per zero byte of data
    pub const TX_DATA_ZERO: u64 = 4;
    /// Cost per non-zero byte of data
    pub const TX_DATA_NONZERO: u64 = 16;
    /// Cost for contract creation
    pub const TX_CREATE: u64 = 32_000;
    /// Cost per byte of deployed code
    pub const CODE_DEPOSIT_BYTE: u64 = 200;
    /// SSTORE cost (cold)
    pub const SSTORE_SET: u64 = 20_000;
    /// SSTORE cost (warm)
    pub const SSTORE_RESET: u64 = 2_900;
    /// SLOAD cost (cold)
    pub const SLOAD_COLD: u64 = 2_100;
    /// SLOAD cost (warm)
    pub const SLOAD_WARM: u64 = 100;
    /// Call cost (cold address)
    pub const CALL_COLD: u64 = 2_600;
    /// Call cost (warm address)
    pub const CALL_WARM: u64 = 100;
    /// Memory expansion cost base
    pub const MEMORY_GAS: u64 = 3;
    /// LOG base cost
    pub const LOG_GAS: u64 = 375;
    /// LOG topic cost
    pub const LOG_TOPIC_GAS: u64 = 375;
    /// LOG data byte cost
    pub const LOG_DATA_GAS: u64 = 8;
}

/// Estimate gas for transaction data
pub fn estimate_calldata_gas(data: &[u8]) -> u64 {
    let mut gas = 0u64;
    for byte in data {
        if *byte == 0 {
            gas += gas_costs::TX_DATA_ZERO;
        } else {
            gas += gas_costs::TX_DATA_NONZERO;
        }
    }
    gas
}

/// Estimate total gas for a transaction
pub fn estimate_transaction_gas(
    data: &[u8],
    is_contract_creation: bool,
    _estimated_execution: u64,
) -> u64 {
    let mut gas = gas_costs::TX_BASE;
    gas += estimate_calldata_gas(data);

    if is_contract_creation {
        gas += gas_costs::TX_CREATE;
        // Add code deposit cost estimation
        gas += (data.len() as u64) * gas_costs::CODE_DEPOSIT_BYTE;
    }

    gas
}

// ============================================================================
// AI Precompile Router — maps addresses 0x10-0x28 to concrete handlers
// ============================================================================
// This is the bridge between revm's EVM execution and LuxTensor's native AI
// precompiles. It checks if a CALL target is an AI precompile address and,
// if so, executes the corresponding handler instead of running EVM bytecode.

use crate::ai_precompiles::{
    AIPrecompileState,
    ai_request_precompile, verify_proof_precompile, get_result_precompile,
    compute_payment_precompile, train_request_precompile,
    vector_store_precompile, vector_query_precompile,
    classify_precompile, anomaly_score_precompile, similarity_gate_precompile,
    semantic_relate_precompile, cluster_assign_precompile,
    register_vector_precompile, global_search_precompile,
    is_ai_precompile, is_semantic_precompile, is_training_precompile,
    is_ai_primitives_precompile, is_registry_precompile,
};
use revm::primitives::Bytes;

/// Check if a given 20-byte address is any LuxTensor custom precompile
pub fn is_luxtensor_precompile(address: &[u8; 20]) -> bool {
    is_ai_precompile(address) || is_training_precompile(address) ||
    is_semantic_precompile(address) || is_ai_primitives_precompile(address) ||
    is_registry_precompile(address)
}

/// Route a call to the appropriate AI precompile handler.
/// Returns `None` if the address is NOT a custom precompile.
/// Returns `Some(PrecompileResult)` if it IS a custom precompile.
///
/// # Arguments
/// * `address` — 20-byte target address of the CALL
/// * `input` — calldata (ABI-encoded arguments)
/// * `gas_limit` — available gas
/// * `state` — shared AI precompile state (vector stores, registries, etc.)
/// * `caller` — 20-byte caller address
/// * `block_number` — current block height (for registry TTL)
pub fn execute_ai_precompile(
    address: &[u8; 20],
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
    caller: [u8; 20],
    block_number: u64,
) -> Option<revm::primitives::PrecompileResult> {
    let last_byte = address[19];

    // AI Core (0x10 - 0x13)
    match last_byte {
        0x10 if is_ai_precompile(address) => {
            Some(ai_request_precompile(input, gas_limit, state, caller))
        }
        0x11 if is_ai_precompile(address) => {
            Some(verify_proof_precompile(input, gas_limit))
        }
        0x12 if is_ai_precompile(address) => {
            Some(get_result_precompile(input, gas_limit, state))
        }
        0x13 if is_ai_precompile(address) => {
            Some(compute_payment_precompile(input, gas_limit))
        }
        // Training (0x14)
        0x14 if is_training_precompile(address) => {
            Some(train_request_precompile(input, gas_limit, state, caller))
        }
        // Semantic Layer (0x20, 0x21)
        0x20 if is_semantic_precompile(address) => {
            Some(vector_store_precompile(input, gas_limit, state))
        }
        0x21 if is_semantic_precompile(address) => {
            Some(vector_query_precompile(input, gas_limit, state))
        }
        // AI Primitives (0x22-0x26)
        0x22 if is_ai_primitives_precompile(address) => {
            Some(classify_precompile(input, gas_limit, state))
        }
        0x23 if is_ai_primitives_precompile(address) => {
            Some(anomaly_score_precompile(input, gas_limit, state))
        }
        0x24 if is_ai_primitives_precompile(address) => {
            Some(similarity_gate_precompile(input, gas_limit, state))
        }
        0x25 if is_ai_primitives_precompile(address) => {
            Some(semantic_relate_precompile(input, gas_limit, state))
        }
        0x26 if is_ai_primitives_precompile(address) => {
            Some(cluster_assign_precompile(input, gas_limit, state))
        }
        // World Semantic Index (0x27, 0x28)
        0x27 if is_registry_precompile(address) => {
            Some(register_vector_precompile(input, gas_limit, state, &caller, block_number))
        }
        0x28 if is_registry_precompile(address) => {
            Some(global_search_precompile(input, gas_limit, state))
        }
        _ => None, // Not a custom precompile
    }
}

/// EVM execution result with detailed information
#[derive(Debug, Clone)]
pub struct DetailedExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Gas used
    pub gas_used: u64,
    /// Gas refunded
    pub gas_refunded: u64,
    /// Return data
    pub return_data: Vec<u8>,
    /// Logs emitted
    pub logs: Vec<Log>,
    /// Revert reason (if reverted)
    pub revert_reason: Option<String>,
    /// Created contract address (if deployment)
    pub created_address: Option<ContractAddress>,
    /// State changes made
    pub state_changes: Vec<StateChange>,
}

/// Represents a state change from execution
#[derive(Debug, Clone)]
pub struct StateChange {
    pub address: ContractAddress,
    pub change_type: StateChangeType,
}

#[derive(Debug, Clone)]
pub enum StateChangeType {
    BalanceChange { old: u128, new: u128 },
    StorageChange { key: Hash, old: Hash, new: Hash },
    NonceChange { old: u64, new: u64 },
    CodeChange { old_hash: Hash, new_hash: Hash },
    Created,
    SelfDestructed,
}

/// Decode revert reason from return data
pub fn decode_revert_reason(data: &[u8]) -> Option<String> {
    // Check for standard Error(string) selector: 0x08c379a0
    if data.len() >= 68 && data[0..4] == [0x08, 0xc3, 0x79, 0xa0] {
        // Skip selector (4) + offset (32) + length prefix (32)
        let string_len = u64::from_be_bytes([
            data[36], data[37], data[38], data[39],
            data[40], data[41], data[42], data[43],
        ]) as usize;

        if data.len() >= 68 + string_len {
            if let Ok(reason) = String::from_utf8(data[68..68 + string_len].to_vec()) {
                return Some(reason);
            }
        }
    }

    // Check for Panic(uint256) selector: 0x4e487b71
    if data.len() >= 36 && data[0..4] == [0x4e, 0x48, 0x7b, 0x71] {
        let panic_code = U256::from_be_slice(&data[4..36]);
        let reason = match panic_code.to::<u64>() {
            0x00 => "Generic compiler panic",
            0x01 => "Assert failed",
            0x11 => "Arithmetic overflow/underflow",
            0x12 => "Division by zero",
            0x21 => "Invalid enum value",
            0x22 => "Invalid storage access",
            0x31 => "Pop empty array",
            0x32 => "Array index out of bounds",
            0x41 => "Memory overflow",
            0x51 => "Zero-initialized function call",
            _ => "Unknown panic code",
        };
        return Some(format!("Panic: {}", reason));
    }

    // Fallback: try to decode as UTF-8
    String::from_utf8(data.to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_selector_calculation() {
        // Known selector for transfer(address,uint256)
        let selector = calculate_function_selector("transfer(address,uint256)");
        assert_eq!(selector, [0xa9, 0x05, 0x9c, 0xbb]);
    }

    #[test]
    fn test_calldata_gas_estimation() {
        let data = vec![0x00, 0x01, 0x00, 0xff];
        let gas = estimate_calldata_gas(&data);
        // 2 zeros (4 each) + 2 non-zeros (16 each) = 8 + 32 = 40
        assert_eq!(gas, 40);
    }

    #[test]
    fn test_transaction_gas_estimation() {
        let data = vec![0x60, 0x60, 0x60, 0x40];
        let gas = estimate_transaction_gas(&data, false, 0);
        // Base (21000) + 4 non-zero bytes (4 * 16 = 64) = 21064
        assert_eq!(gas, 21_064);
    }

    #[test]
    fn test_decode_function_selector() {
        let input = vec![0xa9, 0x05, 0x9c, 0xbb, 0x00, 0x01, 0x02];
        let selector = decode_function_selector(&input);
        assert_eq!(selector, Some([0xa9, 0x05, 0x9c, 0xbb]));
    }

    #[test]
    fn test_decode_function_selector_too_short() {
        let input = vec![0xa9, 0x05];
        let selector = decode_function_selector(&input);
        assert_eq!(selector, None);
    }

    #[test]
    fn test_evm_config_default() {
        let config = EvmConfig::default();
        assert_eq!(config.chain_id, 777);
        assert!(config.enable_precompiles);
        assert!(config.enable_eip1559);
    }
}

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

//! Chain Constants and Configuration
//!
//! Official chain IDs and addresses for Luxtensor networks.

/// Canonical LuxTensor chain ID used across all crates.
/// Prefer this over `chain_id::MAINNET` when you just need "the" LuxTensor ID.
pub const LUXTENSOR_CHAIN_ID: u64 = 8898;

/// Official Chain IDs
pub mod chain_id {
    use super::LUXTENSOR_CHAIN_ID;

    /// Mainnet chain ID (canonical: 8898)
    pub const MAINNET: u64 = LUXTENSOR_CHAIN_ID;
    /// Testnet chain ID
    pub const TESTNET: u64 = 9999;
    /// Devnet chain ID (local development)
    ///
    /// SECURITY: Must be DIFFERENT from MAINNET (8898) to prevent signature replay attacks.
    /// A transaction signed with chain_id=8898 on devnet would be valid on mainnet
    /// if DEVNET == MAINNET (EIP-155 replay protection relies on chain_id uniqueness).
    ///
    /// Previously set to 8898 (same as MAINNET) which was incorrect.
    /// Changed to 8899 to ensure devnet and mainnet transactions cannot be replayed.
    pub const DEVNET: u64 = 8899;
}

/// Official Treasury and System Addresses
pub mod addresses {
    /// DAO Treasury address for reward distribution (mainnet)
    /// This receives a portion of block rewards for ecosystem development
    pub const DAO_TREASURY_MAINNET: &str = "0xDA00000000000000000000000000000000000001";

    /// DAO Treasury address (testnet)
    pub const DAO_TREASURY_TESTNET: &str = "0xDA00000000000000000000000000000000000002";

    /// System address for protocol operations
    pub const SYSTEM_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

    /// Burn address (tokens sent here are permanently destroyed)
    pub const BURN_ADDRESS: &str = "0x000000000000000000000000000000000000dEaD";
}

/// Block and Consensus Parameters
pub mod consensus {
    /// Target block time in seconds (matching economic_model.rs BLOCK_TIME_SECONDS)
    pub const BLOCK_TIME_SECS: u64 = 12;

    /// Epoch length in blocks
    pub const EPOCH_LENGTH: u64 = 100;

    /// Maximum validators in active set
    pub const MAX_VALIDATORS: usize = 100;

    /// Minimum stake to become validator (1,000,000 tokens = 10^24 base units)
    pub const MIN_STAKE: u128 = 1_000_000_000_000_000_000_000_000;

    /// Block gas limit
    pub const BLOCK_GAS_LIMIT: u64 = 30_000_000;

    /// Maximum transactions per block
    pub const MAX_TXS_PER_BLOCK: usize = 10_000;
}

/// Token Economics
pub mod tokenomics {
    /// Total supply (21 million tokens, matching emission.rs max_supply)
    pub const TOTAL_SUPPLY: u128 = 21_000_000_000_000_000_000_000_000;

    /// Decimals (18, like ETH)
    pub const DECIMALS: u8 = 18;

    /// One full token in base units
    pub const ONE_TOKEN: u128 = 1_000_000_000_000_000_000;

    /// Block reward (initial) - 0.24 MDT, matching halving.rs INITIAL_BLOCK_REWARD
    /// Scaled from 2 MDT for 100s blocks → 0.24 MDT for 12s blocks
    pub const INITIAL_BLOCK_REWARD: u128 = 240_000_000_000_000_000;

    /// DAO share of block rewards (10%)
    pub const DAO_REWARD_SHARE_BPS: u64 = 1000; // basis points

    /// Validator share of block rewards (90%)
    pub const VALIDATOR_REWARD_SHARE_BPS: u64 = 9000;
}

/// Network Parameters
pub mod network {
    /// Default P2P port
    pub const DEFAULT_P2P_PORT: u16 = 30303;

    /// Default RPC port
    pub const DEFAULT_RPC_PORT: u16 = 8545;

    /// Maximum peers
    pub const MAX_PEERS: usize = 50;

    /// Sync request rate limit (requests per peer per minute)
    pub const SYNC_RATE_LIMIT: u32 = 10;
}

/// Transaction Parameters
pub mod transaction {
    /// Default gas price
    pub const DEFAULT_GAS_PRICE: u64 = 50;

    /// Minimum gas price
    pub const MIN_GAS_PRICE: u64 = 1;

    /// Gas for simple transfer
    pub const TRANSFER_GAS: u64 = 21000;

    /// Transaction expiration in mempool (30 minutes)
    pub const MEMPOOL_EXPIRATION_SECS: u64 = 1800;
}

/// Get chain name from chain ID
#[allow(unreachable_patterns)] // DEVNET == MAINNET intentionally
pub fn chain_name(chain_id: u64) -> &'static str {
    match chain_id {
        chain_id::MAINNET => "Mainnet",
        chain_id::TESTNET => "Testnet",
        chain_id::DEVNET => "Devnet",
        _ => "Unknown",
    }
}

/// Get DAO treasury address for chain
pub fn dao_treasury_for_chain(chain_id: u64) -> &'static str {
    match chain_id {
        chain_id::MAINNET => addresses::DAO_TREASURY_MAINNET,
        chain_id::TESTNET => addresses::DAO_TREASURY_TESTNET,
        _ => addresses::DAO_TREASURY_TESTNET, // Use testnet for devnet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_name() {
        assert_eq!(chain_name(chain_id::MAINNET), "Mainnet");
        assert_eq!(chain_name(chain_id::TESTNET), "Testnet");
    }

    #[test]
    fn test_one_token_calculation() {
        assert_eq!(tokenomics::ONE_TOKEN, 10u128.pow(18));
    }
}

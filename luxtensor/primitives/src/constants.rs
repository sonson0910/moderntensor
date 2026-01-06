// Blockchain constants

/// Genesis block timestamp
pub const GENESIS_TIMESTAMP: u64 = 1704067200; // 2024-01-01 00:00:00 UTC

/// Target block time in seconds
pub const BLOCK_TIME: u64 = 6;

/// Blocks per epoch
pub const BLOCKS_PER_EPOCH: u64 = 100;

/// Maximum block size in bytes
pub const MAX_BLOCK_SIZE: usize = 1_000_000;

/// Maximum transaction size in bytes
pub const MAX_TX_SIZE: usize = 128_000;

/// Chain ID
pub const CHAIN_ID: u64 = 1337;

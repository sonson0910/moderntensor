use serde::{Deserialize, Serialize};

/// Block hash type (32 bytes)
pub type Hash = [u8; 32];

/// Address type (20 bytes, Ethereum-compatible)
pub type Address = [u8; 20];

/// Signature type (65 bytes: r[32] + s[32] + v[1])
pub type Signature = [u8; 65];

/// Block number type
pub type BlockNumber = u64;

/// Gas amount type
pub type Gas = u64;

/// Token amount type (in smallest unit)
pub type Balance = u128;

/// Timestamp type (Unix timestamp in seconds)
pub type Timestamp = u64;

/// Nonce type for transactions
pub type Nonce = u64;

/// Chain ID for transaction signing
pub type ChainId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wei(pub u128);

impl Wei {
    pub const ZERO: Wei = Wei(0);
    
    pub fn from_ether(ether: u64) -> Self {
        Wei(ether as u128 * 1_000_000_000_000_000_000)
    }
    
    pub fn to_ether(&self) -> f64 {
        self.0 as f64 / 1_000_000_000_000_000_000.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GasPrice(pub u64);

impl GasPrice {
    pub const DEFAULT: GasPrice = GasPrice(1_000_000_000); // 1 Gwei
}

// Genesis configuration for LuxTensor blockchain
// Configure initial accounts, balances, and validators via config file

use luxtensor_core::types::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

/// Pre-deployed MDTStaking contract address (deterministic for all networks)
pub const STAKING_CONTRACT_ADDRESS: &str = "0x5FbDB2315678afecb367f032d93F642f64180aa3";

/// MDT Native Token address (0x...01 is standard precompile convention for wrapped native token)
/// Similar to WETH address convention. This IS the production address.
pub const MDT_TOKEN_ADDRESS: &str = "0x0000000000000000000000000000000000000001";


/// Genesis account configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisAccount {
    /// Account address
    pub address: String,
    /// Initial balance in wei
    pub balance: u128,
    /// Optional: code for pre-deployed contracts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Genesis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisConfig {
    /// Chain ID
    pub chain_id: u64,
    /// Genesis timestamp (unix seconds)
    pub timestamp: u64,
    /// Initial accounts with balances
    pub accounts: Vec<GenesisAccount>,
    /// Validator addresses (if empty, defaults to first account)
    #[serde(default)]
    pub validators: Vec<String>,
    /// Extra data
    #[serde(default)]
    pub extra_data: String,
}

impl GenesisConfig {
    /// Create default mainnet genesis config
    pub fn mainnet() -> Self {
        Self {
            chain_id: 777,  // LuxTensor mainnet chain ID
            timestamp: 0,
            accounts: vec![],  // No pre-funded accounts in mainnet
            validators: vec![],
            extra_data: "LuxTensor Mainnet Genesis".to_string(),
        }
    }

    /// Create default testnet genesis config
    pub fn testnet() -> Self {
        Self {
            chain_id: 7777,  // LuxTensor testnet chain ID
            timestamp: 0,
            accounts: vec![
                GenesisAccount {
                    address: "0x0000000000000000000000000000000000000001".to_string(),
                    balance: 1_000_000_000_000_000_000_000u128, // 1000 LUX
                    code: None,
                },
            ],
            validators: vec![],
            extra_data: "LuxTensor Testnet Genesis".to_string(),
        }
    }

    /// Create development genesis config with Hardhat-style test accounts
    /// WARNING: Only for local development! Private keys are public.
    pub fn development() -> Self {
        Self {
            chain_id: 31337,  // Hardhat chain ID
            timestamp: 0,
            accounts: vec![
                // Hardhat default accounts - NEVER USE IN PRODUCTION
                GenesisAccount {
                    address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".to_string(),
                    balance: 10_000_000_000_000_000_000_000u128, // 10000 ETH
                    code: None,
                },
                GenesisAccount {
                    address: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".to_string(),
                    balance: 10_000_000_000_000_000_000_000u128,
                    code: None,
                },
                GenesisAccount {
                    address: "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC".to_string(),
                    balance: 10_000_000_000_000_000_000_000u128,
                    code: None,
                },
            ],
            validators: vec![],
            extra_data: "LuxTensor Development Genesis - DO NOT USE IN PRODUCTION".to_string(),
        }
    }

    /// Load genesis config from JSON file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, GenesisError> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| GenesisError::IoError(e.to_string()))?;

        let config: GenesisConfig = serde_json::from_str(&content)
            .map_err(|e| GenesisError::ParseError(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    /// Save genesis config to JSON file
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), GenesisError> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| GenesisError::ParseError(e.to_string()))?;

        std::fs::write(&path, content)
            .map_err(|e| GenesisError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Validate genesis configuration
    pub fn validate(&self) -> Result<(), GenesisError> {
        if self.chain_id == 0 {
            return Err(GenesisError::InvalidConfig("chain_id cannot be 0".to_string()));
        }

        // Safety: reject development genesis on production chain IDs
        if self.is_development() && (self.chain_id == 8899 || self.chain_id == 9999) {
            return Err(GenesisError::InvalidConfig(
                format!(
                    "Development genesis (known test accounts) cannot be used on chain {} ({}). \
                     Use a production genesis config for mainnet/testnet.",
                    self.chain_id,
                    if self.chain_id == 8899 { "Mainnet" } else { "Testnet" }
                )
            ));
        }

        for account in &self.accounts {
            if !account.address.starts_with("0x") || account.address.len() != 42 {
                return Err(GenesisError::InvalidConfig(
                    format!("Invalid address format: {}", account.address)
                ));
            }
        }

        Ok(())
    }

    /// Get accounts as Address -> Balance map
    pub fn get_account_balances(&self) -> HashMap<Address, u128> {
        let mut balances = HashMap::new();

        for account in &self.accounts {
            if let Some(addr) = parse_address(&account.address) {
                balances.insert(addr, account.balance);
            } else {
                warn!("Failed to parse genesis address: {}", account.address);
            }
        }

        balances
    }

    /// Check if this is a development config (has known test accounts)
    pub fn is_development(&self) -> bool {
        self.chain_id == 31337 ||
        self.extra_data.contains("Development") ||
        self.accounts.iter().any(|a|
            a.address.to_lowercase() == "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        )
    }
}

/// Genesis errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum GenesisError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
}

/// Parse address from hex string
fn parse_address(s: &str) -> Option<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return None;
    }
    let bytes = hex::decode(s).ok()?;
    Some(Address::from_slice(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mainnet_config() {
        let config = GenesisConfig::mainnet();
        assert_eq!(config.chain_id, 777);
        assert!(config.accounts.is_empty());
        assert!(!config.is_development());
    }

    #[test]
    fn test_dev_config() {
        let config = GenesisConfig::development();
        assert_eq!(config.chain_id, 31337);
        assert_eq!(config.accounts.len(), 3);
        assert!(config.is_development());
    }

    #[test]
    fn test_validation() {
        let mut config = GenesisConfig::development();
        assert!(config.validate().is_ok());

        config.chain_id = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_balances() {
        let config = GenesisConfig::development();
        let balances = config.get_account_balances();
        assert_eq!(balances.len(), 3);
    }
}

use ethers::types::Address;
use serde::Deserialize;
use std::env;

/// Configuration for the AI Oracle Node.
///
/// # Required Environment Variables
///
/// | Variable | Description |
/// |----------|-------------|
/// | `ORACLE_CONTRACT_ADDRESS` | Ethereum address of the oracle smart contract |
/// | `ORACLE_PRIVATE_KEY` | Hex-encoded private key for signing transactions |
///
/// # Optional Environment Variables
///
/// | Variable | Default | Description |
/// |----------|---------|-------------|
/// | `NODE_WS_URL` | `ws://127.0.0.1:8546` | WebSocket endpoint of the LuxTensor node |
/// | `DATABASE_URL` | _(none)_ | PostgreSQL connection URL for persistent storage |
#[derive(Debug, Deserialize, Clone)]
pub struct OracleConfig {
    pub node_ws_url: String,
    pub oracle_contract_address: Address,
    pub private_key: String,
    pub database_url: Option<String>,
}

impl OracleConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Self {
            node_ws_url: env::var("NODE_WS_URL")
                .unwrap_or_else(|_| "ws://127.0.0.1:8546".to_string()),
            oracle_contract_address: env::var("ORACLE_CONTRACT_ADDRESS")
                .map_err(|_| anyhow::anyhow!("ORACLE_CONTRACT_ADDRESS environment variable is required"))?
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ORACLE_CONTRACT_ADDRESS: {}", e))?,
            private_key: env::var("ORACLE_PRIVATE_KEY")
                .map_err(|_| anyhow::anyhow!("ORACLE_PRIVATE_KEY environment variable must be set"))?,
            database_url: env::var("DATABASE_URL").ok(),
        };

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values beyond basic parsing.
    ///
    /// Checks:
    /// - `node_ws_url` uses a WebSocket scheme (`ws://` or `wss://`)
    /// - `private_key` is 64 hex characters (32 bytes)
    /// - `oracle_contract_address` is not the zero address
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate WebSocket URL scheme
        if !self.node_ws_url.starts_with("ws://") && !self.node_ws_url.starts_with("wss://") {
            anyhow::bail!(
                "NODE_WS_URL must use ws:// or wss:// scheme, got: {}",
                self.node_ws_url
            );
        }

        // Validate private key format (64 hex chars = 32 bytes)
        let key = self.private_key.strip_prefix("0x").unwrap_or(&self.private_key);
        if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
            anyhow::bail!(
                "ORACLE_PRIVATE_KEY must be 64 hex characters (32 bytes), got {} chars",
                key.len()
            );
        }

        // Validate contract address is not zero
        if self.oracle_contract_address == Address::zero() {
            anyhow::bail!("ORACLE_CONTRACT_ADDRESS must not be the zero address");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_good_config() {
        let config = OracleConfig {
            node_ws_url: "ws://127.0.0.1:8546".to_string(),
            oracle_contract_address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
                .parse()
                .unwrap(),
            private_key: "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .to_string(),
            database_url: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_bad_ws_url() {
        let config = OracleConfig {
            node_ws_url: "http://127.0.0.1:8545".to_string(),
            oracle_contract_address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
                .parse()
                .unwrap(),
            private_key: "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .to_string(),
            database_url: None,
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("ws://"));
    }

    #[test]
    fn test_validate_bad_private_key() {
        let config = OracleConfig {
            node_ws_url: "ws://127.0.0.1:8546".to_string(),
            oracle_contract_address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
                .parse()
                .unwrap(),
            private_key: "too_short".to_string(),
            database_url: None,
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("64 hex"));
    }

    #[test]
    fn test_validate_zero_address() {
        let config = OracleConfig {
            node_ws_url: "ws://127.0.0.1:8546".to_string(),
            oracle_contract_address: Address::zero(),
            private_key: "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .to_string(),
            database_url: None,
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("zero address"));
    }

    #[test]
    fn test_validate_with_0x_prefix() {
        let config = OracleConfig {
            node_ws_url: "wss://mainnet.luxtensor.io/ws".to_string(),
            oracle_contract_address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
                .parse()
                .unwrap(),
            private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .to_string(),
            database_url: Some("postgres://localhost/oracle".to_string()),
        };
        assert!(config.validate().is_ok());
    }
}

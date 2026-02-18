use ethers::types::Address;
use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct OracleConfig {
    pub node_ws_url: String,
    pub oracle_contract_address: Address,
    pub private_key: String,
    pub database_url: Option<String>,
}

impl OracleConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            node_ws_url: env::var("NODE_WS_URL")
                .unwrap_or_else(|_| "ws://127.0.0.1:8546".to_string()),
            oracle_contract_address: env::var("ORACLE_CONTRACT_ADDRESS")
                .map_err(|_| anyhow::anyhow!("ORACLE_CONTRACT_ADDRESS environment variable is required"))?
                .parse()
                .map_err(|e| anyhow::anyhow!("Invalid ORACLE_CONTRACT_ADDRESS: {}", e))?,
            private_key: env::var("ORACLE_PRIVATE_KEY")
                .map_err(|_| anyhow::anyhow!("ORACLE_PRIVATE_KEY environment variable must be set"))?,
            database_url: env::var("DATABASE_URL").ok(),
        })
    }
}

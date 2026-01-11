use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,

    /// Consensus configuration
    pub consensus: ConsensusConfig,

    /// Network configuration
    pub network: NetworkConfig,

    /// Storage configuration
    pub storage: StorageConfig,

    /// RPC configuration
    pub rpc: RpcConfig,

    /// Logging configuration
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node name/identifier
    pub name: String,

    /// Chain ID
    pub chain_id: u64,

    /// Data directory
    pub data_dir: PathBuf,

    /// Whether this is a validator node
    pub is_validator: bool,

    /// Validator private key path (if validator)
    pub validator_key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Block time in seconds
    pub block_time: u64,

    /// Epoch length in blocks
    pub epoch_length: u64,

    /// Minimum stake required to be a validator (as string for TOML compatibility)
    pub min_stake: String,

    /// Maximum number of validators
    pub max_validators: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// P2P listening address
    pub listen_addr: String,

    /// P2P listening port
    pub listen_port: u16,

    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,

    /// Maximum number of peers
    pub max_peers: usize,

    /// Enable mDNS discovery
    pub enable_mdns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Database path
    pub db_path: PathBuf,

    /// Enable database compression
    pub enable_compression: bool,

    /// Max open files
    pub max_open_files: i32,

    /// Cache size in MB
    pub cache_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    /// Enable RPC server
    pub enabled: bool,

    /// RPC listening address
    pub listen_addr: String,

    /// RPC listening port
    pub listen_port: u16,

    /// Number of worker threads
    pub threads: usize,

    /// CORS allowed origins
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log to file
    pub log_to_file: bool,

    /// Log file path
    pub log_file: Option<PathBuf>,

    /// JSON formatted logs
    pub json_format: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                name: "luxtensor-node".to_string(),
                chain_id: 1,
                data_dir: PathBuf::from("./data"),
                is_validator: false,
                validator_key_path: None,
            },
            consensus: ConsensusConfig {
                block_time: 3,
                epoch_length: 100,
                min_stake: "1000000000000000000".to_string(), // 1 token (10^18)
                max_validators: 100,
            },
            network: NetworkConfig {
                listen_addr: "0.0.0.0".to_string(),
                listen_port: 30303,
                bootstrap_nodes: vec![],
                max_peers: 50,
                enable_mdns: true,
            },
            storage: StorageConfig {
                db_path: PathBuf::from("./data/db"),
                enable_compression: true,
                max_open_files: 1000,
                cache_size: 256, // 256 MB
            },
            rpc: RpcConfig {
                enabled: true,
                listen_addr: "127.0.0.1".to_string(),
                listen_port: 8545,
                threads: 4,
                cors_origins: vec!["*".to_string()],
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                log_to_file: false,
                log_file: None,
                json_format: false,
            },
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn to_file(&self, path: &str) -> anyhow::Result<()> {
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate network config
        if self.network.listen_port == 0 {
            anyhow::bail!("Invalid network port: 0");
        }

        if self.network.max_peers == 0 {
            anyhow::bail!("Max peers must be greater than 0");
        }

        // Validate RPC config
        if self.rpc.enabled && self.rpc.listen_port == 0 {
            anyhow::bail!("Invalid RPC port: 0");
        }

        // Validate consensus config
        if self.consensus.block_time == 0 {
            anyhow::bail!("Block time must be greater than 0");
        }

        if self.consensus.epoch_length == 0 {
            anyhow::bail!("Epoch length must be greater than 0");
        }

        // Validate storage config
        if self.storage.cache_size == 0 {
            anyhow::bail!("Cache size must be greater than 0");
        }

        // Validate logging config
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            anyhow::bail!("Invalid log level: {}", self.logging.level);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.node.name, "luxtensor-node");
        assert_eq!(config.network.listen_port, 30303);
        assert_eq!(config.rpc.listen_port, 8545);
    }

    #[test]
    fn test_validate_valid_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_port() {
        let mut config = Config::default();
        config.network.listen_port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = Config::default();
        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());
    }
}

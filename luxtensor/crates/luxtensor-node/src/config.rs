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

    /// Mempool configuration
    #[serde(default)]
    pub mempool: MempoolConfig,

    /// Faucet configuration (devnet only)
    #[serde(default)]
    pub faucet: FaucetConfig,
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

    /// Unique validator ID for leader election (e.g., "validator-1")
    #[serde(default)]
    pub validator_id: Option<String>,

    /// DAO treasury address for rewards distribution (hex with 0x prefix)
    #[serde(default = "default_dao_address")]
    pub dao_address: String,

    /// Development mode: enables pre-funded test accounts (NEVER use in production)
    #[serde(default)]
    pub dev_mode: bool,
}

/// Default DAO treasury address (ModernTensor Foundation)
///
/// IMPORTANT: For production deployments, configure this in your config.toml:
/// ```toml
/// [node]
/// dao_address = "0xYOUR_ACTUAL_DAO_ADDRESS_HERE"
/// ```
///
/// Official addresses:
/// - Mainnet: Will be announced before mainnet launch
/// - Testnet: 0xDAO0000000000000000000000000000000000002
fn default_dao_address() -> String {
    // Use the canonical mainnet DAO treasury address by default.
    // For custom deployments, override this in config.toml.
    luxtensor_core::constants::addresses::DAO_TREASURY_MAINNET.to_string()
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

    /// Block gas limit (default: 30_000_000)
    #[serde(default = "default_gas_limit")]
    pub gas_limit: u64,

    /// List of known validators for leader election (in order)
    #[serde(default)]
    pub validators: Vec<String>,
}

/// Default block gas limit: 30 million
fn default_gas_limit() -> u64 {
    30_000_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// P2P listening address
    pub listen_addr: String,

    /// P2P listening port
    pub listen_port: u16,

    /// Bootstrap nodes (multiaddr format, e.g., "/ip4/1.2.3.4/tcp/30303/p2p/12D3KooW...")
    pub bootstrap_nodes: Vec<String>,

    /// Maximum number of peers
    pub max_peers: usize,

    /// Enable mDNS discovery (local network only)
    pub enable_mdns: bool,

    /// Path to node identity key file (for persistent Peer ID)
    /// If not set, uses "./node.key" in data_dir
    #[serde(default)]
    pub node_key_path: Option<PathBuf>,
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

    /// WebSocket listening port (default: 8546)
    pub ws_port: u16,

    /// Enable WebSocket server
    pub ws_enabled: bool,

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolConfig {
    /// Maximum number of transactions in the mempool
    pub max_size: usize,
    /// Maximum transactions per sender
    pub max_per_sender: usize,
    /// Minimum gas price (in base units)
    pub min_gas_price: u128,
    /// Maximum transaction size in bytes
    pub max_tx_size: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: 10_000,
            max_per_sender: 16,
            min_gas_price: 1_000_000_000,
            max_tx_size: 131_072, // 128 KB
        }
    }
}

/// Faucet configuration for development/test networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaucetConfig {
    /// Enable faucet endpoint
    pub enabled: bool,
    /// Amount to drip per request (in base units / wei)
    /// Default: 1000 LUX = 1000 * 10^18 wei
    pub drip_amount: String,
    /// Cooldown period per address in seconds
    pub cooldown_secs: u64,
    /// Maximum drip requests per address per day
    pub max_daily_drips: u32,
}

impl Default for FaucetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            drip_amount: "1000000000000000000000".to_string(), // 1000 LUX (10^21 wei)
            cooldown_secs: 60,
            max_daily_drips: 10,
        }
    }
}

impl FaucetConfig {
    /// Parse drip_amount string to u128
    pub fn drip_amount_u128(&self) -> u128 {
        self.drip_amount.parse::<u128>().unwrap_or(1_000_000_000_000_000_000_000)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                name: "luxtensor-node".to_string(),
                chain_id: 8898, // LuxTensor mainnet (canonical chain ID)
                data_dir: PathBuf::from("./data"),
                is_validator: false,
                validator_key_path: None,
                validator_id: None,
                dao_address: default_dao_address(),
                dev_mode: false, // Production mode by default
            },
            consensus: ConsensusConfig {
                block_time: 12,
                epoch_length: 100,
                min_stake: "1000000000000000000000000".to_string(), // 1M tokens (10^24)
                max_validators: 100,
                gas_limit: default_gas_limit(),
                validators: vec![], // Must be configured explicitly via config.toml
            },
            network: NetworkConfig {
                listen_addr: "0.0.0.0".to_string(),
                listen_port: 30303,
                bootstrap_nodes: vec![],
                max_peers: 50,
                enable_mdns: true,
                node_key_path: None, // Will use ./node.key in data_dir
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
                ws_port: 8546,
                ws_enabled: true,
                threads: 4,
                cors_origins: vec!["http://localhost:*".to_string()], // SECURITY: restricted from wildcard "*"
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                log_to_file: false,
                log_file: None,
                json_format: false,
            },
            mempool: MempoolConfig::default(),
            faucet: FaucetConfig::default(),
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
        // Validate DAO address format
        {
            let dao = &self.node.dao_address;
            if !dao.starts_with("0x") || dao.len() != 42 {
                anyhow::bail!(
                    "Invalid dao_address format: '{}'. Must be 0x-prefixed 20-byte hex address.",
                    dao
                );
            }
            // Verify it's valid hex
            if hex::decode(dao.trim_start_matches("0x")).is_err() {
                anyhow::bail!("Invalid dao_address: '{}' contains non-hex characters.", dao);
            }
            // Warn if using the zero address on non-dev chains
            let is_zero = dao == "0x0000000000000000000000000000000000000000";
            let is_production_chain = self.node.chain_id == 8898 || self.node.chain_id == 9999;
            if is_zero && is_production_chain && !self.node.dev_mode {
                anyhow::bail!(
                    "dao_address is the zero address on chain {}. \
                     Configure a real DAO treasury address in config.toml for production.",
                    self.node.chain_id
                );
            }
        }

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

        if self.rpc.enabled && self.rpc.threads == 0 {
            anyhow::bail!("RPC threads must be greater than 0");
        }

        // Validate consensus config
        if self.consensus.block_time == 0 {
            anyhow::bail!("Block time must be greater than 0");
        }

        if self.consensus.epoch_length == 0 {
            anyhow::bail!("Epoch length must be greater than 0");
        }

        if self.consensus.max_validators == 0 {
            anyhow::bail!("max_validators must be greater than 0");
        }

        // Validate min_stake is a valid u128 numeric value
        if self.consensus.min_stake.parse::<u128>().is_err() {
            anyhow::bail!(
                "Invalid min_stake: '{}'. Must be a numeric value (u128) representing base units.",
                self.consensus.min_stake
            );
        }

        // Validate storage config
        if self.storage.cache_size == 0 {
            anyhow::bail!("Cache size must be greater than 0");
        }

        if self.storage.max_open_files <= 0 {
            anyhow::bail!("max_open_files must be greater than 0");
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

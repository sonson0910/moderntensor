use clap::{Parser, Subcommand, Args};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "luxtensor")]
#[command(about = "LuxTensor CLI - High-performance Layer 1 blockchain for AI", long_about = None)]
#[command(version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version information
    Version,

    /// Generate a new keypair
    #[command(name = "key-gen")]
    GenerateKey,

    /// Show node status (connects to RPC)
    Status {
        /// RPC endpoint URL
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },

    /// Admin commands for node management
    #[command(subcommand)]
    Admin(AdminCommands),

    /// Generate node configuration from template
    #[command(name = "config-gen")]
    ConfigGen(ConfigGenArgs),

    /// Query blockchain state
    #[command(subcommand)]
    Query(QueryCommands),
}

#[derive(Subcommand)]
enum AdminCommands {
    /// Get peer count
    Peers {
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
    /// Get validator list
    Validators {
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
    /// Get sync status
    Sync {
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
    /// Get block height
    Height {
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
}

#[derive(Args)]
struct ConfigGenArgs {
    /// Node name (e.g., node-1, node-2)
    #[arg(short, long)]
    name: String,

    /// Node number (used for port offset, 1-based)
    #[arg(short = 'i', long, default_value = "1")]
    node_id: u16,

    /// Is this a validator node
    #[arg(short, long, default_value = "true")]
    validator: bool,

    /// Output config file path
    #[arg(short, long, default_value = "config.toml")]
    output: PathBuf,

    /// Bootstrap peer (multiaddr format)
    #[arg(short, long)]
    bootstrap: Option<String>,
}

#[derive(Subcommand)]
enum QueryCommands {
    /// Get account balance
    Balance {
        /// Account address (0x...)
        address: String,
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
    /// Get block by number
    Block {
        /// Block number or "latest"
        number: String,
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
    /// Get transaction by hash
    Tx {
        /// Transaction hash (0x...)
        hash: String,
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Version => {
            println!("LuxTensor CLI v0.2.0");
            println!("High-performance Layer 1 blockchain for AI workloads");
            println!("Built with Rust + revm EVM");
        }

        Commands::GenerateKey => {
            use luxtensor_crypto::KeyPair;

            let keypair = KeyPair::generate();
            let address = keypair.address();

            println!("🔑 Generated new keypair:");
            println!("   Address: 0x{}", hex::encode(address));
            println!("\n⚠️  IMPORTANT: Save your private key securely!");
        }

        Commands::Status { rpc } => {
            match rpc_call(&rpc, "system_syncState", vec![]).await {
                Ok(result) => {
                    println!("📊 Node Status:");
                    if let Some(height) = result.get("current_height") {
                        println!("   Height: {}", height);
                    }
                    println!("   Status: Connected");
                }
                Err(e) => {
                    println!("❌ Failed to connect to {}: {}", rpc, e);
                }
            }
        }

        Commands::Admin(admin) => handle_admin(admin).await?,

        Commands::ConfigGen(args) => generate_config(args)?,

        Commands::Query(query) => handle_query(query).await?,
    }

    Ok(())
}

async fn handle_admin(cmd: AdminCommands) -> Result<()> {
    match cmd {
        AdminCommands::Peers { rpc } => {
            let result = rpc_call(&rpc, "system_peerCount", vec![]).await?;
            println!("👥 Peer Count: {}", result);
        }
        AdminCommands::Validators { rpc } => {
            let result = rpc_call(&rpc, "staking_getActiveValidators", vec![]).await?;
            println!("🔒 Active Validators:");
            if let Some(arr) = result.as_array() {
                for v in arr {
                    if let Some(addr) = v.get("address") {
                        println!("   {}", addr);
                    }
                }
            }
        }
        AdminCommands::Sync { rpc } => {
            let result = rpc_call(&rpc, "system_syncState", vec![]).await?;
            println!("🔄 Sync State: {}", serde_json::to_string_pretty(&result)?);
        }
        AdminCommands::Height { rpc } => {
            let result = rpc_call(&rpc, "chain_getHeight", vec![]).await?;
            println!("📦 Block Height: {}", result);
        }
    }
    Ok(())
}

async fn handle_query(cmd: QueryCommands) -> Result<()> {
    match cmd {
        QueryCommands::Balance { address, rpc } => {
            let result = rpc_call(&rpc, "eth_getBalance", vec![
                serde_json::json!(address),
                serde_json::json!("latest"),
            ]).await?;
            println!("💰 Balance: {} wei", result);
        }
        QueryCommands::Block { number, rpc } => {
            let block_num = if number == "latest" {
                serde_json::json!("latest")
            } else {
                serde_json::json!(format!("0x{:x}", number.parse::<u64>().unwrap_or(0)))
            };
            let result = rpc_call(&rpc, "eth_getBlockByNumber", vec![block_num, serde_json::json!(false)]).await?;
            println!("📦 Block: {}", serde_json::to_string_pretty(&result)?);
        }
        QueryCommands::Tx { hash, rpc } => {
            let result = rpc_call(&rpc, "eth_getTransactionByHash", vec![serde_json::json!(hash)]).await?;
            println!("📄 Transaction: {}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

async fn rpc_call(rpc: &str, method: &str, params: Vec<serde_json::Value>) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });

    let resp: serde_json::Value = client
        .post(rpc)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    if let Some(error) = resp.get("error") {
        anyhow::bail!("RPC error: {}", error);
    }

    Ok(resp.get("result").cloned().unwrap_or(serde_json::Value::Null))
}

fn generate_config(args: ConfigGenArgs) -> Result<()> {
    let port_offset = (args.node_id - 1) as u16;
    let p2p_port = 30303 + port_offset;
    let rpc_port = 8545 + port_offset;

    let bootstrap = args.bootstrap.map(|b| format!("bootstrap_nodes = [\"{}\"]", b))
        .unwrap_or_else(|| "bootstrap_nodes = []".to_string());

    let config = format!(r#"# LuxTensor Node Configuration
# Generated by: luxtensor config-gen

[node]
name = "{name}"
chain_id = 777
data_dir = "./data"
is_validator = {validator}
validator_key_path = "./validator.key"
validator_id = "{name}"

[consensus]
block_time = 3
epoch_length = 100
min_stake = "1000000000000000000000"
max_validators = 100
gas_limit = 30000000

[network]
listen_addr = "0.0.0.0"
listen_port = {p2p_port}
{bootstrap}
max_peers = 50
enable_mdns = true

[storage]
db_path = "./data/db"
enable_compression = true
max_open_files = 1000
cache_size = 512

[rpc]
enabled = true
listen_addr = "0.0.0.0"
listen_port = {rpc_port}
threads = 4
cors_origins = ["*"]

[logging]
level = "info"
log_to_file = true
log_file = "./node.log"
"#,
        name = args.name,
        validator = args.validator,
        p2p_port = p2p_port,
        rpc_port = rpc_port,
        bootstrap = bootstrap,
    );

    std::fs::write(&args.output, config)?;
    println!("✅ Config generated: {}", args.output.display());
    println!("   P2P Port: {}", p2p_port);
    println!("   RPC Port: {}", rpc_port);

    Ok(())
}

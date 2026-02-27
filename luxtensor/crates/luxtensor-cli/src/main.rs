//! LuxTensor CLI — command-line interface for the LuxTensor blockchain.
//!
//! # Modules
//!
//! - [`helpers`] — RLP encoding, parsing, cryptographic utilities, RPC client

mod helpers;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use rand::RngCore;
use std::path::PathBuf;

use helpers::*;

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

    /// Send a transaction
    #[command(name = "send-tx")]
    SendTx(SendTxArgs),

    /// Stake tokens to become a validator
    Stake(StakeArgs),

    /// Unstake tokens
    Unstake(UnstakeArgs),

    /// Delegate stake to a validator
    Delegate(DelegateArgs),

    /// Import private key to a keystore file
    #[command(name = "import-key")]
    ImportKey(ImportKeyArgs),

    /// Export private key from keystore
    #[command(name = "export-key")]
    ExportKey(ExportKeyArgs),
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

#[derive(Args)]
struct SendTxArgs {
    /// RPC endpoint URL
    #[arg(long)]
    rpc_url: String,

    /// Sender's private key (hex). Omit to use LUXTENSOR_PRIVATE_KEY env var or interactive prompt
    #[arg(long)]
    from: Option<String>,

    /// Destination address (0x...)
    #[arg(long)]
    to: String,

    /// Amount in wei
    #[arg(long)]
    value: String,

    /// Gas price in wei (optional, fetched from node if omitted)
    #[arg(long)]
    gas_price: Option<String>,

    /// Gas limit (default: 21000)
    #[arg(long)]
    gas_limit: Option<u64>,

    /// Chain ID (default: 8898)
    #[arg(long)]
    chain_id: Option<u64>,

    /// Hex-encoded calldata (optional)
    #[arg(long)]
    data: Option<String>,
}

#[derive(Args)]
struct StakeArgs {
    /// RPC endpoint URL
    #[arg(long)]
    rpc_url: String,

    /// Staker's private key (hex). Omit to use LUXTENSOR_PRIVATE_KEY env var or interactive prompt
    #[arg(long)]
    from: Option<String>,

    /// Amount of MDT to stake
    #[arg(long)]
    amount: String,
}

#[derive(Args)]
struct UnstakeArgs {
    /// RPC endpoint URL
    #[arg(long)]
    rpc_url: String,

    /// Staker's private key (hex). Omit to use LUXTENSOR_PRIVATE_KEY env var or interactive prompt
    #[arg(long)]
    from: Option<String>,

    /// Amount of MDT to unstake
    #[arg(long)]
    amount: String,
}

#[derive(Args)]
struct DelegateArgs {
    /// RPC endpoint URL
    #[arg(long)]
    rpc_url: String,

    /// Delegator's private key (hex). Omit to use LUXTENSOR_PRIVATE_KEY env var or interactive prompt
    #[arg(long)]
    from: Option<String>,

    /// Validator address to delegate to (0x...)
    #[arg(long)]
    validator: String,

    /// Amount of MDT to delegate
    #[arg(long)]
    amount: String,
}

#[derive(Args)]
struct ImportKeyArgs {
    /// Private key hex to import. Omit to use LUXTENSOR_PRIVATE_KEY env var or interactive prompt
    #[arg(long)]
    private_key: Option<String>,

    /// Output keystore file path
    #[arg(long)]
    output: PathBuf,

    /// Encryption password (will prompt if omitted)
    #[arg(long)]
    password: Option<String>,
}

#[derive(Args)]
struct ExportKeyArgs {
    /// Path to the keystore JSON file
    #[arg(long)]
    keystore: PathBuf,

    /// Password to decrypt the keystore (will prompt interactively if omitted)
    #[arg(long)]
    password: Option<String>,
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

// ============================================================
// Main + dispatch
// ============================================================

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

        Commands::Status { rpc } => match rpc_call(&rpc, "system_syncState", vec![]).await {
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
        },

        Commands::Admin(admin) => handle_admin(admin).await?,
        Commands::ConfigGen(args) => generate_config(args)?,
        Commands::Query(query) => handle_query(query).await?,
        Commands::SendTx(args) => handle_send_tx(args).await?,
        Commands::Stake(args) => handle_stake(args).await?,
        Commands::Unstake(args) => handle_unstake(args).await?,
        Commands::Delegate(args) => handle_delegate(args).await?,
        Commands::ImportKey(args) => handle_import_key(args)?,
        Commands::ExportKey(args) => handle_export_key(args)?,
    }

    Ok(())
}

// ============================================================
// Admin commands
// ============================================================

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

// ============================================================
// Query commands
// ============================================================

async fn handle_query(cmd: QueryCommands) -> Result<()> {
    match cmd {
        QueryCommands::Balance { address, rpc } => {
            let result = rpc_call(
                &rpc,
                "eth_getBalance",
                vec![serde_json::json!(address), serde_json::json!("latest")],
            )
            .await?;
            println!("💰 Balance: {} wei", result);
        }
        QueryCommands::Block { number, rpc } => {
            let block_num = if number == "latest" {
                serde_json::json!("latest")
            } else {
                serde_json::json!(format!("0x{:x}", number.parse::<u64>().unwrap_or(0)))
            };
            let result =
                rpc_call(&rpc, "eth_getBlockByNumber", vec![block_num, serde_json::json!(false)])
                    .await?;
            println!("📦 Block: {}", serde_json::to_string_pretty(&result)?);
        }
        QueryCommands::Tx { hash, rpc } => {
            let result =
                rpc_call(&rpc, "eth_getTransactionByHash", vec![serde_json::json!(hash)]).await?;
            println!("📄 Transaction: {}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

// ============================================================
// Config generation
// ============================================================

fn generate_config(args: ConfigGenArgs) -> Result<()> {
    // SECURITY: Validate node_id >= 1 to prevent u32 underflow wrapping
    if args.node_id == 0 {
        anyhow::bail!("node_id must be >= 1");
    }
    let port_offset = (args.node_id - 1) as u16;
    let p2p_port = 30303 + port_offset;
    let rpc_port = 8545 + port_offset;

    let bootstrap = args
        .bootstrap
        .map(|b| format!("bootstrap_nodes = [\"{}\"]", b))
        .unwrap_or_else(|| "bootstrap_nodes = []".to_string());

    let config = format!(
        r#"# LuxTensor Node Configuration
# Generated by: luxtensor config-gen

[node]
name = "{name}"
chain_id = 8898
data_dir = "./data"
is_validator = {validator}
validator_key_path = "./validator.key"
validator_id = "{name}"

[consensus]
block_time = 12
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

// ============================================================
// Transaction command
// ============================================================

async fn handle_send_tx(args: SendTxArgs) -> Result<()> {
    // Read private key securely (env var or interactive prompt if not on CLI)
    let from_key = read_private_key(args.from)?;
    let secret = parse_private_key(&from_key)?;
    let keypair = luxtensor_crypto::KeyPair::from_secret(&secret)
        .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
    let from_address = keypair.address();
    println!("📤 Sending transaction from: {}", from_address);

    // Parse destination address
    let to_hex = args.to.strip_prefix("0x").unwrap_or(&args.to);
    let to_bytes =
        hex::decode(to_hex).map_err(|_| anyhow::anyhow!("Invalid destination address hex"))?;
    if to_bytes.len() != 20 {
        anyhow::bail!("Destination address must be 20 bytes, got {}", to_bytes.len());
    }

    // Parse value
    let value_bytes = parse_wei_amount(&args.value)?;

    // Chain ID (default: LuxTensor = 8898)
    let chain_id = args.chain_id.unwrap_or(8898);

    // Fetch nonce from the node
    let nonce_result = rpc_call(
        &args.rpc_url,
        "eth_getTransactionCount",
        vec![serde_json::json!(format!("{}", from_address)), serde_json::json!("latest")],
    )
    .await?;
    let nonce = parse_hex_u64(nonce_result.as_str().unwrap_or(&nonce_result.to_string()))?;

    // Get gas price (from arg or from node)
    let gas_price_bytes = if let Some(ref gp) = args.gas_price {
        parse_wei_amount(gp)?
    } else {
        let gp_result = rpc_call(&args.rpc_url, "eth_gasPrice", vec![]).await?;
        let gp = parse_hex_u64(gp_result.as_str().unwrap_or(&gp_result.to_string()))?;
        u64_to_be_trimmed(gp)
    };

    // Gas limit
    let gas_limit = args.gas_limit.unwrap_or(21000);

    // Calldata
    let data_bytes = if let Some(ref data) = args.data {
        let d = data.strip_prefix("0x").unwrap_or(data);
        hex::decode(d).map_err(|_| anyhow::anyhow!("Invalid hex calldata"))?
    } else {
        vec![]
    };

    println!("   Nonce:     {}", nonce);
    println!("   To:        0x{}", hex::encode(&to_bytes));
    println!("   Value:     {} wei", args.value);
    println!("   Gas Limit: {}", gas_limit);
    println!("   Chain ID:  {}", chain_id);

    // Build unsigned tx for EIP-155 signing:
    // [nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0]
    let unsigned_tx = rlp_encode_list(&[
        rlp_encode_u64(nonce),
        rlp_encode_bytes(&gas_price_bytes),
        rlp_encode_u64(gas_limit),
        rlp_encode_bytes(&to_bytes),
        rlp_encode_bytes(&value_bytes),
        rlp_encode_bytes(&data_bytes),
        rlp_encode_u64(chain_id),
        rlp_encode_u64(0),
        rlp_encode_u64(0),
    ]);

    // Hash the unsigned transaction
    let tx_hash = luxtensor_crypto::keccak256(&unsigned_tx);

    // Sign the hash
    let signature = keypair.sign(&tx_hash).map_err(|e| anyhow::anyhow!("Signing failed: {}", e))?;

    let r = &signature[..32];
    let s = &signature[32..];

    // Determine recovery ID by trying both (0, 1) and matching the recovered address
    let mut recovery_id = 0u8;
    for rid in 0..2u8 {
        if let Ok(pubkey) = luxtensor_crypto::recover_public_key(&tx_hash, &signature, rid) {
            if let Ok(addr) = luxtensor_crypto::address_from_public_key(&pubkey) {
                if addr == from_address {
                    recovery_id = rid;
                    break;
                }
            }
        }
    }

    // EIP-155: v = chain_id * 2 + 35 + recovery_id
    let v = chain_id * 2 + 35 + recovery_id as u64;

    // Build signed tx: [nonce, gasPrice, gasLimit, to, value, data, v, r, s]
    let signed_tx = rlp_encode_list(&[
        rlp_encode_u64(nonce),
        rlp_encode_bytes(&gas_price_bytes),
        rlp_encode_u64(gas_limit),
        rlp_encode_bytes(&to_bytes),
        rlp_encode_bytes(&value_bytes),
        rlp_encode_bytes(&data_bytes),
        rlp_encode_u64(v),
        rlp_encode_bytes(trim_leading_zeros(r)),
        rlp_encode_bytes(trim_leading_zeros(s)),
    ]);

    let raw_tx_hex = format!("0x{}", hex::encode(&signed_tx));

    // Send the raw transaction
    let result =
        rpc_call(&args.rpc_url, "eth_sendRawTransaction", vec![serde_json::json!(raw_tx_hex)])
            .await?;

    println!("✅ Transaction sent!");
    println!("   Tx Hash: {}", result);

    Ok(())
}

// ============================================================
// Staking commands
// ============================================================

async fn handle_stake(args: StakeArgs) -> Result<()> {
    let from_key = read_private_key(args.from)?;
    let secret = parse_private_key(&from_key)?;
    let keypair = luxtensor_crypto::KeyPair::from_secret(&secret)
        .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
    let address = keypair.address();

    println!("🔒 Staking {} MDT from {}", args.amount, address);

    let result = rpc_call(
        &args.rpc_url,
        "staking_stake",
        vec![serde_json::json!(format!("{}", address)), serde_json::json!(args.amount)],
    )
    .await?;

    println!("✅ Stake submitted!");
    println!("   Result: {}", result);

    Ok(())
}

async fn handle_unstake(args: UnstakeArgs) -> Result<()> {
    let from_key = read_private_key(args.from)?;
    let secret = parse_private_key(&from_key)?;
    let keypair = luxtensor_crypto::KeyPair::from_secret(&secret)
        .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
    let address = keypair.address();

    println!("🔓 Unstaking {} MDT from {}", args.amount, address);

    let result = rpc_call(
        &args.rpc_url,
        "staking_unstake",
        vec![serde_json::json!(format!("{}", address)), serde_json::json!(args.amount)],
    )
    .await?;

    println!("✅ Unstake submitted!");
    println!("   Result: {}", result);

    Ok(())
}

async fn handle_delegate(args: DelegateArgs) -> Result<()> {
    let from_key = read_private_key(args.from)?;
    let secret = parse_private_key(&from_key)?;
    let keypair = luxtensor_crypto::KeyPair::from_secret(&secret)
        .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
    let address = keypair.address();

    println!("🤝 Delegating {} MDT from {} to validator {}", args.amount, address, args.validator);

    let result = rpc_call(
        &args.rpc_url,
        "staking_delegate",
        vec![
            serde_json::json!(format!("{}", address)),
            serde_json::json!(args.validator),
            serde_json::json!(args.amount),
        ],
    )
    .await?;

    println!("✅ Delegation submitted!");
    println!("   Result: {}", result);

    Ok(())
}

// ============================================================
// Key management commands
// ============================================================

fn handle_import_key(args: ImportKeyArgs) -> Result<()> {
    println!("⚠️  WARNING: Handle private keys with extreme care.");

    let pk = read_private_key(args.private_key)?;
    let secret = parse_private_key(&pk)?;
    let keypair = luxtensor_crypto::KeyPair::from_secret(&secret)
        .map_err(|e| anyhow::anyhow!("Invalid private key: {}", e))?;
    let address = keypair.address();

    // Get password (prompt interactively if not provided via flag)
    let password = match args.password {
        Some(p) => p,
        None => {
            let pass = rpassword::prompt_password("Enter password to encrypt keystore: ")
                .map_err(|e| anyhow::anyhow!("Failed to read password: {}", e))?;
            pass.trim().to_string()
        }
    };

    if password.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }

    // Generate random 32-byte salt and 16-byte IV using OS CSPRNG
    let mut salt = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let mut iv = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut iv);

    // Derive 32-byte key using scrypt KDF
    let derived_key = derive_key_scrypt(password.as_bytes(), &salt)?;

    // Encrypt the private key with AES-128-CTR (first 16 bytes of derived key)
    let ciphertext = aes128_ctr_apply(&derived_key[..16], &iv, &secret)?;

    // MAC for integrity verification: keccak256(derived_key[16..32] || ciphertext)
    let mut mac_input = Vec::with_capacity(48);
    mac_input.extend_from_slice(&derived_key[16..32]);
    mac_input.extend_from_slice(&ciphertext);
    let mac = luxtensor_crypto::keccak256(&mac_input);

    let keystore = serde_json::json!({
        "version": 2,
        "address": format!("{}", address),
        "crypto": {
            "cipher": "aes-128-ctr",
            "cipherparams": {
                "iv": hex::encode(iv)
            },
            "ciphertext": hex::encode(ciphertext),
            "kdf": "scrypt",
            "kdfparams": {
                "n": 16384,
                "r": 8,
                "p": 1,
                "dklen": 32,
                "salt": hex::encode(salt)
            },
            "mac": hex::encode(mac)
        }
    });

    std::fs::write(&args.output, serde_json::to_string_pretty(&keystore)?)?;

    println!("✅ Keystore saved: {}", args.output.display());
    println!("   Address: {}", address);
    println!("⚠️  Remember your password — it cannot be recovered!");

    Ok(())
}

fn handle_export_key(args: ExportKeyArgs) -> Result<()> {
    // Read password securely: prompt interactively if not provided via CLI arg
    let password = match args.password {
        Some(p) => {
            eprintln!("\u{26a0}\u{fe0f}  Warning: Passing password via CLI arg exposes it in shell history.");
            eprintln!("   Consider omitting --password to use the interactive prompt instead.");
            p
        }
        None => {
            rpassword::prompt_password("\u{1f511} Enter keystore password: ")
                .map_err(|e| anyhow::anyhow!("Failed to read password: {}", e))?
                .trim()
                .to_string()
        }
    };

    if password.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }

    let data = std::fs::read_to_string(&args.keystore).map_err(|e| {
        anyhow::anyhow!("Failed to read keystore '{}': {}", args.keystore.display(), e)
    })?;
    let keystore: serde_json::Value =
        serde_json::from_str(&data).map_err(|e| anyhow::anyhow!("Invalid keystore JSON: {}", e))?;

    let crypto = keystore
        .get("crypto")
        .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing 'crypto' field"))?;

    let cipher = crypto.get("cipher").and_then(|v| v.as_str()).unwrap_or("unknown");

    let kdf = crypto.get("kdf").and_then(|v| v.as_str()).unwrap_or("unknown");

    let ciphertext_hex = crypto
        .get("ciphertext")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing ciphertext"))?;
    let ciphertext = hex::decode(ciphertext_hex)?;
    if ciphertext.len() != 32 {
        anyhow::bail!("Invalid keystore: ciphertext must be 32 bytes");
    }

    let kdfparams = crypto
        .get("kdfparams")
        .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing kdfparams"))?;
    let salt_hex = kdfparams
        .get("salt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing salt"))?;
    let salt = hex::decode(salt_hex)?;

    let stored_mac_hex = crypto
        .get("mac")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing mac"))?;
    let stored_mac = hex::decode(stored_mac_hex)?;

    // Derive key based on KDF type
    let derived_key = match kdf {
        "scrypt" => {
            // v2 keystore: scrypt KDF
            derive_key_scrypt(password.as_bytes(), &salt)?
        }
        "keccak256-iter" => {
            // v1 legacy keystore: iterated keccak256
            eprintln!("⚠️  WARNING: This keystore uses a legacy encryption format (keccak256-iter + XOR).");
            eprintln!("   Please re-import your key to upgrade to the secure format (scrypt + AES-128-CTR).");
            let iterations = kdfparams
                .get("iterations")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing iterations"))?
                as u32;
            derive_key_legacy(password.as_bytes(), &salt, iterations)
        }
        _ => {
            anyhow::bail!("Unsupported KDF: {}", kdf);
        }
    };

    // Verify MAC
    let mut mac_input = Vec::with_capacity(48);
    mac_input.extend_from_slice(&derived_key[16..32]);
    mac_input.extend_from_slice(&ciphertext);
    let computed_mac = luxtensor_crypto::keccak256(&mac_input);

    if !constant_time_eq(&computed_mac[..], &stored_mac[..]) {
        anyhow::bail!("❌ Incorrect password or corrupted keystore");
    }

    // Decrypt private key based on cipher type
    let private_key: [u8; 32] = match cipher {
        "aes-128-ctr" => {
            // v2 keystore: AES-128-CTR
            let iv_hex = crypto
                .get("cipherparams")
                .and_then(|v| v.get("iv"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid keystore: missing IV for AES-128-CTR"))?;
            let iv = hex::decode(iv_hex)?;
            let decrypted = aes128_ctr_apply(&derived_key[..16], &iv, &ciphertext)?;
            let mut key = [0u8; 32];
            key.copy_from_slice(&decrypted);
            key
        }
        "xor-keccak256" => {
            // v1 legacy keystore: XOR decryption
            let mut key = [0u8; 32];
            for i in 0..32 {
                key[i] = ciphertext[i] ^ derived_key[i];
            }
            key
        }
        _ => {
            anyhow::bail!("Unsupported cipher: {}", cipher);
        }
    };

    let address = keystore.get("address").and_then(|v| v.as_str()).unwrap_or("unknown");

    println!("⚠️  WARNING: Your private key is displayed below. Keep it secure!");
    println!("🔑 Address: {}", address);
    println!("   Private Key: 0x{}", hex::encode(private_key));
    println!("⚠️  Never share your private key with anyone!");

    Ok(())
}

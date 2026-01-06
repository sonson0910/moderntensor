mod config;
mod service;
mod mempool;
mod executor;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;
use service::NodeService;
use tracing::info;
use tracing_subscriber;

#[derive(Parser)]
#[clap(name = "luxtensor-node")]
#[clap(author, version, about = "LuxTensor - High-performance Layer 1 blockchain", long_about = None)]
struct Cli {
    /// Configuration file path
    #[clap(short, long, value_name = "FILE", default_value = "config.toml")]
    config: String,
    
    /// Subcommand
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the node
    Start,
    
    /// Initialize a new node configuration
    Init {
        /// Output configuration file path
        #[clap(short, long, default_value = "config.toml")]
        output: String,
    },
    
    /// Show node version
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Init { output }) => {
            init_config(&output)?;
        }
        Some(Commands::Version) => {
            show_version();
        }
        Some(Commands::Start) | None => {
            start_node(&cli.config).await?;
        }
    }
    
    Ok(())
}

/// Initialize a new configuration file
fn init_config(output: &str) -> Result<()> {
    let config = Config::default();
    config.to_file(output)?;
    println!("✅ Configuration file created: {}", output);
    println!("Edit the configuration and run: luxtensor-node start");
    Ok(())
}

/// Show version information
fn show_version() {
    println!("🦀 LuxTensor Node");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Build: {}", env!("CARGO_PKG_NAME"));
    println!("Rust: {}", env!("CARGO_PKG_RUST_VERSION"));
}

/// Start the node
async fn start_node(config_path: &str) -> Result<()> {
    // Load configuration
    let config = if std::path::Path::new(config_path).exists() {
        Config::from_file(config_path)?
    } else {
        info!("Configuration file not found, using defaults");
        Config::default()
    };
    
    // Initialize logging
    init_logging(&config)?;
    
    // Print banner
    print_banner();
    
    // Create and start node service
    let mut service = NodeService::new(config).await?;
    service.start().await?;
    
    // Wait for shutdown
    service.wait_for_shutdown().await?;
    
    Ok(())
}

/// Initialize logging
fn init_logging(config: &Config) -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.logging.level));
    
    // Note: JSON formatting requires additional features
    // For now, use standard formatting
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();
    
    Ok(())
}

/// Print startup banner
fn print_banner() {
    println!();
    println!("╔═══════════════════════════════════════════════╗");
    println!("║                                               ║");
    println!("║          🦀 LuxTensor Node v{}           ║", env!("CARGO_PKG_VERSION"));
    println!("║                                               ║");
    println!("║      High-Performance Layer 1 Blockchain     ║");
    println!("║                                               ║");
    println!("╚═══════════════════════════════════════════════╝");
    println!();
}

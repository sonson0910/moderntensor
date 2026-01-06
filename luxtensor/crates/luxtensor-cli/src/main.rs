use clap::{Parser, Subcommand};
use anyhow::Result;

#[derive(Parser)]
#[command(name = "luxtensor")]
#[command(about = "LuxTensor CLI - High-performance Layer 1 blockchain", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version information
    Version,
    
    /// Generate a new keypair
    GenerateKey,
    
    /// Show node status
    Status,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Version => {
            println!("LuxTensor CLI v0.1.0");
            println!("Rust blockchain implementation");
        }
        Commands::GenerateKey => {
            use luxtensor_crypto::KeyPair;
            
            let keypair = KeyPair::generate();
            let address = keypair.address();
            
            println!("Generated new keypair:");
            println!("Address: 0x{}", hex::encode(address));
            println!("\n⚠️  IMPORTANT: Save your private key securely!");
        }
        Commands::Status => {
            println!("LuxTensor Node Status:");
            println!("  Phase: 1 - Foundation");
            println!("  Status: Development");
            println!("  Components: Core + Crypto");
        }
    }
    
    Ok(())
}

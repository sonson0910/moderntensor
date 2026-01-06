//! # LuxTensor Node
//!
//! Main node binary for running LuxTensor blockchain.

use clap::Parser;

#[derive(Parser)]
#[clap(name = "luxtensor-node")]
#[clap(about = "LuxTensor blockchain node", version)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Parser)]
enum Commands {
    /// Start the node
    Start {
        #[clap(long, default_value = "30303")]
        port: u16,
        
        #[clap(long, default_value = "8545")]
        rpc_port: u16,
    },
    
    /// Initialize genesis
    Init {
        #[clap(long)]
        genesis: String,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start { port, rpc_port } => {
            println!("Starting LuxTensor node...");
            println!("P2P Port: {}", port);
            println!("RPC Port: {}", rpc_port);
            println!("Implementation coming in Phase 6!");
        }
        Commands::Init { genesis } => {
            println!("Initializing genesis from: {}", genesis);
            println!("Implementation coming in Phase 6!");
        }
    }
}

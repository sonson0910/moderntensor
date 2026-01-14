//! Luxtensor Indexer service entry point

use luxtensor_indexer::{Config, Indexer, Backfill, Storage};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "luxtensor_indexer=info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║           Luxtensor Indexer v0.1.0                       ║");
    info!("╚══════════════════════════════════════════════════════════╝");

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let mode = if args.len() > 1 { &args[1] } else { "live" };

    // Load configuration
    let config = Config::from_env();

    info!("Configuration:");
    info!("  Database: {}", config.database_url.split('@').last().unwrap_or("***"));
    info!("  Node RPC: {}", config.node_ws_url.replace("ws", "http"));
    info!("  GraphQL:  {}", config.graphql_bind);
    info!("  Mode:     {}", mode);

    match mode {
        "backfill" => {
            // Backfill mode: fetch historical blocks via RPC
            let start_block: u64 = args.get(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            let end_block: Option<u64> = args.get(3)
                .and_then(|s| s.parse().ok());

            let batch_size: u64 = std::env::var("BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100);

            info!("Backfill mode: block {} to {:?}", start_block, end_block);

            // Connect to storage
            let storage = Arc::new(Storage::connect(&config.database_url).await?);
            storage.run_migrations().await?;

            // Convert WS URL to HTTP
            let rpc_url = config.node_ws_url
                .replace("ws://", "http://")
                .replace("wss://", "https://");

            // Run backfill
            let backfill = Backfill::new(&rpc_url, storage, batch_size);
            backfill.run(start_block, end_block).await?;
        }
        _ => {
            // Live mode: WebSocket subscription + GraphQL API
            let indexer = Indexer::new(config).await?;
            info!("Starting indexer in live mode...");
            indexer.run().await?;
        }
    }

    Ok(())
}

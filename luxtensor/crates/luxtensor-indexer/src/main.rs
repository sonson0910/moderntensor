//! Luxtensor Indexer service entry point

use luxtensor_indexer::{Config, Indexer};
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

    // Load configuration
    let config = Config::from_env();

    info!("Configuration:");
    info!("  Database: {}", config.database_url.split('@').last().unwrap_or("***"));
    info!("  Node WS:  {}", config.node_ws_url);
    info!("  GraphQL:  {}", config.graphql_bind);

    // Create and run indexer
    let indexer = Indexer::new(config).await?;

    info!("Starting indexer...");
    indexer.run().await?;

    Ok(())
}

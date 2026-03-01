use luxtensor_oracle::{run, OracleConfig};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load config from environment variables directly.
    // NOTE: dotenv removed (banned in deny.toml). In production, set env vars
    // via systemd, Docker, or your orchestrator — never auto-load .env files.
    let config = OracleConfig::from_env()?;

    // Run Oracle
    run(config).await?;

    Ok(())
}

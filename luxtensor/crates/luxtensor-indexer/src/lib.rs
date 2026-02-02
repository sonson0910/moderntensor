//! Luxtensor Indexer
//!
//! Blockchain indexer for Luxtensor with PostgreSQL storage and GraphQL API.
//!
//! # Architecture
//!
//! ```text
//! Luxtensor Node (WebSocket)
//!         ↓
//! BlockListener → EventDecoder → StorageWriter
//!         ↓
//! PostgreSQL ← GraphQL API
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use luxtensor_indexer::{Indexer, Config};
//!
//! let config = Config::from_env();
//! let indexer = Indexer::new(config).await?;
//! indexer.run().await?;
//! ```

pub mod config;
pub mod models;
pub mod listener;
pub mod decoder;
pub mod storage;
pub mod graphql;
pub mod error;
pub mod backfill;
pub mod metrics;

pub use config::Config;
pub use error::{IndexerError, Result};
pub use listener::BlockListener;
pub use decoder::EventDecoder;
pub use storage::Storage;
pub use graphql::GraphQLServer;
pub use backfill::Backfill;
pub use metrics::IndexerMetrics;

use std::sync::Arc;
use tracing::{info, error};

/// Main indexer service
pub struct Indexer {
    config: Config,
    storage: Arc<Storage>,
    listener: BlockListener,
    graphql: GraphQLServer,
}

impl Indexer {
    /// Create new indexer instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing Luxtensor Indexer...");

        // Connect to PostgreSQL
        let storage = Arc::new(Storage::connect(&config.database_url).await?);
        info!("Connected to PostgreSQL");

        // Run migrations
        storage.run_migrations().await?;
        info!("Database migrations complete");

        // Create block listener
        let listener = BlockListener::new(
            &config.node_ws_url,
            storage.clone(),
        );

        // Create GraphQL server
        let graphql = GraphQLServer::new(
            storage.clone(),
            &config.graphql_bind,
        );

        Ok(Self {
            config,
            storage,
            listener,
            graphql,
        })
    }

    /// Run the indexer
    pub async fn run(self) -> Result<()> {
        info!("Starting Luxtensor Indexer...");

        // Spawn GraphQL server
        let graphql_handle = tokio::spawn(async move {
            if let Err(e) = self.graphql.run().await {
                error!("GraphQL server error: {}", e);
            }
        });

        // Run block listener (main loop)
        let listener_handle = tokio::spawn(async move {
            if let Err(e) = self.listener.run().await {
                error!("Block listener error: {}", e);
            }
        });

        // Wait for both
        tokio::try_join!(graphql_handle, listener_handle)?;

        Ok(())
    }
}

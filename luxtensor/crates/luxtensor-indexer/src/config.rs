//! Indexer configuration

use std::env;

/// Indexer configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection URL
    pub database_url: String,

    /// Luxtensor node WebSocket URL
    pub node_ws_url: String,

    /// Luxtensor node RPC URL
    pub node_rpc_url: String,

    /// GraphQL server bind address
    pub graphql_bind: String,

    /// Number of blocks to index per batch
    pub batch_size: u32,

    /// Starting block number (0 = genesis)
    pub start_block: u64,
}

impl Config {
    /// Create config from environment variables.
    ///
    /// **DATABASE_URL is required** — if not set, the indexer will panic at startup
    /// rather than silently connecting with default credentials.
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                eprintln!(
                    "FATAL: DATABASE_URL environment variable is not set.\n\
                     The indexer requires an explicit database connection string.\n\
                     Example: DATABASE_URL=postgres://user:pass@host/luxtensor_indexer"
                );
                std::process::exit(1);
            }),
            node_ws_url: env::var("NODE_WS_URL")
                .unwrap_or_else(|_| "ws://localhost:8546".to_string()),
            node_rpc_url: env::var("NODE_RPC_URL")
                .unwrap_or_else(|_| "http://localhost:8545".to_string()),
            graphql_bind: env::var("GRAPHQL_BIND")
                .unwrap_or_else(|_| "0.0.0.0:4000".to_string()),
            batch_size: env::var("BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            start_block: env::var("START_BLOCK")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
        }
    }

    /// Create config for testing
    pub fn for_testing() -> Self {
        Self {
            database_url: "postgres://postgres:postgres@localhost/luxtensor_indexer_test".to_string(),
            node_ws_url: "ws://localhost:8546".to_string(),
            node_rpc_url: "http://localhost:8545".to_string(),
            graphql_bind: "127.0.0.1:4001".to_string(),
            batch_size: 10,
            start_block: 0,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_env()
    }
}

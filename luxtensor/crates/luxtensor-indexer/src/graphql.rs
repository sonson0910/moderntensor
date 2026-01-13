//! GraphQL API server
//!
//! Provides GraphQL queries for indexed blockchain data.

use crate::error::Result;
use crate::storage::Storage;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

/// GraphQL API server
pub struct GraphQLServer {
    storage: Arc<Storage>,
    bind_address: String,
}

impl GraphQLServer {
    /// Create new GraphQL server
    pub fn new(storage: Arc<Storage>, bind_address: &str) -> Self {
        Self {
            storage,
            bind_address: bind_address.to_string(),
        }
    }

    /// Run the GraphQL server
    ///
    /// For now, this is a simple JSON-RPC style endpoint.
    /// Full GraphQL integration will be added in a future version.
    pub async fn run(self) -> Result<()> {
        info!("GraphQL server starting on {}", self.bind_address);

        let listener = TcpListener::bind(&self.bind_address).await
            .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

        info!("Indexer API listening on http://{}", self.bind_address);
        info!("Available endpoints:");
        info!("  GET /health - Health check");
        info!("  POST /query - Query indexed data");

        // Simple HTTP server loop
        loop {
            let (socket, addr) = listener.accept().await
                .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

            let storage = self.storage.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, storage).await {
                    tracing::error!("Connection error from {}: {}", addr, e);
                }
            });
        }
    }
}

use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn handle_connection(
    mut socket: tokio::net::TcpStream,
    storage: Arc<Storage>,
) -> Result<()> {
    let mut buffer = [0; 4096];
    let n = socket.read(&mut buffer).await
        .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

    let request = String::from_utf8_lossy(&buffer[..n]);

    // Parse simple HTTP request
    let response = if request.starts_with("GET /health") {
        // Health check
        let status = storage.get_sync_status().await?;
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"status\":\"ok\",\"last_block\":{},\"syncing\":{}}}",
            status.last_indexed_block,
            status.is_syncing
        )
    } else if request.starts_with("POST /query") {
        // Query endpoint - parse JSON body and handle queries
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"message\":\"Query endpoint ready. Use GraphQL client or REST queries.\"}}"
        )
    } else if request.starts_with("GET /blocks") {
        // Get latest blocks
        let latest = storage.get_latest_block().await?;
        match latest {
            Some(block) => format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"number\":{},\"hash\":\"{}\",\"tx_count\":{}}}",
                block.number, block.hash, block.tx_count
            ),
            None => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"message\":\"No blocks indexed yet\"}".to_string(),
        }
    } else {
        // Default response
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"endpoints\":[\"/health\",\"/blocks\",\"/query\"]}}"
        )
    };

    socket.write_all(response.as_bytes()).await
        .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

    Ok(())
}

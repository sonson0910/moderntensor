//! Block listener - subscribes to node WebSocket for new blocks

use crate::decoder::EventDecoder;
use crate::error::{IndexerError, Result};
use crate::models::Block;
use crate::storage::Storage;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{info, warn, error, debug};

/// Block listener that subscribes to new blocks via WebSocket
pub struct BlockListener {
    ws_url: String,
    storage: Arc<Storage>,
    decoder: EventDecoder,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<serde_json::Value>,
    id: u64,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RpcResponse {
    jsonrpc: String,
    result: Option<serde_json::Value>,
    error: Option<RpcError>,
    id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SubscriptionEvent {
    jsonrpc: String,
    method: Option<String>,
    params: Option<SubscriptionParams>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SubscriptionParams {
    subscription: String,
    result: serde_json::Value,
}

impl BlockListener {
    /// Create new block listener
    pub fn new(ws_url: &str, storage: Arc<Storage>) -> Self {
        Self {
            ws_url: ws_url.to_string(),
            storage: storage.clone(),
            decoder: EventDecoder::new(storage),
        }
    }

    /// Run the block listener
    pub async fn run(&self) -> Result<()> {
        info!("Starting block listener on {}", self.ws_url);

        loop {
            match self.listen_loop().await {
                Ok(_) => {
                    info!("Listener loop ended normally");
                }
                Err(e) => {
                    error!("Listener error: {}. Reconnecting in 5s...", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn listen_loop(&self) -> Result<()> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        info!("Connected to node WebSocket");

        // Subscribe to new blocks
        let subscribe_request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "luxtensor_subscribe".to_string(),
            params: vec![serde_json::json!("newBlocks")],
            id: 1,
        };

        let request_json = serde_json::to_string(&subscribe_request)?;
        write.send(Message::Text(request_json)).await
            .map_err(|e| IndexerError::WebSocket(e))?;

        info!("Subscribed to newBlocks");

        // Mark as syncing
        self.storage.update_sync_status(0, true).await?;

        // Listen for messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    self.handle_message(&text).await?;
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await
                        .map_err(|e| IndexerError::WebSocket(e))?;
                }
                Ok(Message::Close(_)) => {
                    warn!("WebSocket closed by server");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    return Err(IndexerError::WebSocket(e));
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        // Try to parse as subscription event
        if let Ok(event) = serde_json::from_str::<SubscriptionEvent>(text) {
            if let Some(params) = event.params {
                self.handle_block_event(&params.result).await?;
            }
            return Ok(());
        }

        // Try to parse as RPC response (subscription acknowledgment)
        if let Ok(response) = serde_json::from_str::<RpcResponse>(text) {
            if let Some(result) = response.result {
                debug!("Subscription confirmed: {:?}", result);
            }
            if let Some(error) = response.error {
                error!("RPC error: {} - {}", error.code, error.message);
            }
        }

        Ok(())
    }

    async fn handle_block_event(&self, data: &serde_json::Value) -> Result<()> {
        let block_number = data.get("number")
            .and_then(|n| n.as_str())
            .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        let block_hash = data.get("hash")
            .and_then(|h| h.as_str())
            .unwrap_or("")
            .to_string();

        let parent_hash = data.get("parentHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string());

        let timestamp = data.get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        let tx_count = data.get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len() as i32)
            .unwrap_or(0);

        info!("New block: {} (txs: {})", block_number, tx_count);

        // Create block record
        let block = Block {
            number: block_number,
            hash: block_hash,
            parent_hash,
            timestamp,
            tx_count,
            indexed_at: Some(Utc::now()),
        };

        // Store block
        self.storage.upsert_block(&block).await?;

        // Decode and store transactions
        if let Some(txs) = data.get("transactions").and_then(|t| t.as_array()) {
            for tx_data in txs {
                self.decoder.decode_transaction(block_number, timestamp, tx_data).await?;
            }
        }

        // Update sync status
        self.storage.update_sync_status(block_number, true).await?;

        Ok(())
    }
}

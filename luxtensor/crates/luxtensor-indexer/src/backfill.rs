//! Backfill module - fetches historical blocks via RPC

use crate::decoder::EventDecoder;
use crate::error::{IndexerError, Result};
use crate::models::Block;
use crate::storage::Storage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, warn, error};

/// Backfill service to index historical blocks
pub struct Backfill {
    rpc_url: String,
    storage: Arc<Storage>,
    decoder: EventDecoder,
    batch_size: u64,
    /// SECURITY (M-2): Reuse a single HTTP client to avoid per-call TLS handshake overhead.
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: Vec<serde_json::Value>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<serde_json::Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl Backfill {
    /// Create new backfill service
    pub fn new(rpc_url: &str, storage: Arc<Storage>, batch_size: u64) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            storage: storage.clone(),
            decoder: EventDecoder::new(storage),
            batch_size,
            client: reqwest::Client::new(),
        }
    }

    /// Run backfill from start_block to end_block
    pub async fn run(&self, start_block: u64, end_block: Option<u64>) -> Result<()> {
        info!("Starting backfill from block {}", start_block);

        // Get current block number if end not specified
        let target_block = match end_block {
            Some(n) => n,
            None => self.get_block_number().await?,
        };

        info!("Backfill target: block {} to {}", start_block, target_block);

        let mut current = start_block;

        while current <= target_block {
            let batch_end = std::cmp::min(
                current.saturating_add(self.batch_size.max(1).saturating_sub(1)),
                target_block,
            );

            info!("Indexing blocks {} - {}", current, batch_end);

            for block_num in current..=batch_end {
                // SECURITY (IDX-M6): Retry with exponential backoff instead of
                // single retry-then-drop which silently loses blocks.
                const MAX_BLOCK_RETRIES: u32 = 5;
                let mut retry_delay = Duration::from_millis(500);
                let mut indexed = false;

                for attempt in 1..=MAX_BLOCK_RETRIES {
                    match self.index_block(block_num).await {
                        Ok(_) => {
                            indexed = true;
                            break;
                        }
                        Err(e) => {
                            if attempt == MAX_BLOCK_RETRIES {
                                error!(
                                    block = block_num,
                                    attempts = MAX_BLOCK_RETRIES,
                                    error = %e,
                                    "Failed to index block after all retries — block SKIPPED"
                                );
                            } else {
                                warn!(
                                    block = block_num,
                                    attempt = attempt,
                                    delay_ms = retry_delay.as_millis() as u64,
                                    error = %e,
                                    "Failed to index block, retrying"
                                );
                                sleep(retry_delay).await;
                                retry_delay = retry_delay.saturating_mul(2);
                            }
                        }
                    }
                }

                if !indexed {
                    warn!(block = block_num, "Block was skipped after {} retries", MAX_BLOCK_RETRIES);
                }
            }

            // Update sync status
            self.storage.update_sync_status(batch_end as i64, true).await?;

            current = batch_end + 1;

            // Small delay to avoid overwhelming the node
            sleep(Duration::from_millis(100)).await;
        }

        self.storage.update_sync_status(target_block as i64, false).await?;
        info!("Backfill complete! Indexed blocks {} to {}", start_block, target_block);

        Ok(())
    }

    /// Get current block number from node
    async fn get_block_number(&self) -> Result<u64> {
        let response = self.rpc_call("eth_blockNumber", vec![]).await?;

        let hex_str = response
            .as_str()
            .ok_or_else(|| IndexerError::Parse("Invalid block number".into()))?;

        let block_num = u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
            .map_err(|e| IndexerError::Parse(e.to_string()))?;

        Ok(block_num)
    }

    /// Index a single block
    async fn index_block(&self, block_number: u64) -> Result<()> {
        let hex_block = format!("0x{:x}", block_number);

        // Get block with transactions
        let block_data = self.rpc_call(
            "eth_getBlockByNumber",
            vec![serde_json::json!(hex_block), serde_json::json!(true)],
        ).await?;

        if block_data.is_null() {
            return Err(IndexerError::Parse(format!("Block {} not found", block_number)));
        }

        // Parse block
        // SECURITY (IDX-M2): Validate block hash is present instead of silently
        // storing an empty string, which could corrupt the index.
        let block_hash = block_data.get("hash")
            .and_then(|h| h.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| IndexerError::Parse(
                format!("Block {} has no 'hash' field", block_number)
            ))?
            .to_string();

        let parent_hash = block_data.get("parentHash")
            .and_then(|h| h.as_str())
            .map(|s| s.to_string());

        let timestamp = block_data.get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        let tx_count = block_data.get("transactions")
            .and_then(|t| t.as_array())
            .map(|a| a.len() as i32)
            .unwrap_or(0);

        // Create block record
        let block = Block {
            number: block_number as i64,
            hash: block_hash,
            parent_hash,
            timestamp,
            tx_count,
            indexed_at: None,
        };

        // Store block
        self.storage.upsert_block(&block).await?;

        // Decode and store transactions
        if let Some(txs) = block_data.get("transactions").and_then(|t| t.as_array()) {
            for tx_data in txs {
                self.decoder.decode_transaction(block_number as i64, timestamp, tx_data).await?;
            }
        }

        Ok(())
    }

    /// Make RPC call
    async fn rpc_call(&self, method: &str, params: Vec<serde_json::Value>) -> Result<serde_json::Value> {
        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: 1,
        };

        let response = self.client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| IndexerError::Connection(e.to_string()))?;

        let rpc_response: RpcResponse = response
            .json()
            .await
            .map_err(|e| IndexerError::Parse(e.to_string()))?;

        if let Some(error) = rpc_response.error {
            return Err(IndexerError::Rpc(format!("{}: {}", error.code, error.message)));
        }

        rpc_response.result.ok_or_else(|| IndexerError::Parse("No result".into()))
    }
}

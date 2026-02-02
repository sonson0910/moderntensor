//! HTTP REST API server for querying indexed data

use crate::error::Result;
use crate::storage::Storage;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::info;

/// HTTP API server
pub struct GraphQLServer {
    storage: Arc<Storage>,
    bind_address: String,
}

#[derive(Debug, Deserialize)]
struct QueryRequest {
    #[serde(rename = "type")]
    query_type: String,
    #[serde(default)]
    hash: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    hotkey: Option<String>,
    #[serde(default)]
    number: Option<i64>,
    #[serde(default)]
    from: Option<i64>,
    #[serde(default)]
    to: Option<i64>,
    #[serde(default)]
    limit: Option<i32>,
    #[serde(default)]
    offset: Option<i32>,
}

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl GraphQLServer {
    /// Create new HTTP API server
    pub fn new(storage: Arc<Storage>, bind_address: &str) -> Self {
        Self {
            storage,
            bind_address: bind_address.to_string(),
        }
    }

    /// Run the HTTP API server
    pub async fn run(self) -> Result<()> {
        info!("HTTP API server starting on {}", self.bind_address);

        let listener = TcpListener::bind(&self.bind_address).await
            .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

        info!("Indexer API listening on http://{}", self.bind_address);
        info!("Available endpoints:");
        info!("  GET  /health              - Health check");
        info!("  GET  /blocks              - Latest block");
        info!("  GET  /blocks/:number      - Block by number");
        info!("  GET  /tx/:hash            - Transaction by hash");
        info!("  GET  /address/:addr/txs   - Transactions by address");
        info!("  GET  /address/:addr/transfers - Transfers by address");
        info!("  GET  /stakes/:hotkey      - Stake history");
        info!("  POST /query               - Query indexed data");

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

async fn handle_connection(
    mut socket: tokio::net::TcpStream,
    storage: Arc<Storage>,
) -> Result<()> {
    let mut buffer = [0; 8192];
    let n = socket.read(&mut buffer).await
        .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

    let request = String::from_utf8_lossy(&buffer[..n]);
    let lines: Vec<&str> = request.lines().collect();
    let first_line = lines.first().unwrap_or(&"");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    let method = parts.first().unwrap_or(&"GET");
    let path = parts.get(1).unwrap_or(&"/");

    let response = match (*method, *path) {
        // Health check
        ("GET", "/health") => {
            let status = storage.get_sync_status().await?;
            json_response(200, &serde_json::json!({
                "status": "ok",
                "last_block": status.last_indexed_block,
                "syncing": status.is_syncing
            }))
        }

        // Latest block
        ("GET", "/blocks") => {
            match storage.get_latest_block().await? {
                Some(block) => json_response(200, &serde_json::json!({
                    "number": block.number,
                    "hash": block.hash,
                    "parent_hash": block.parent_hash,
                    "timestamp": block.timestamp,
                    "tx_count": block.tx_count
                })),
                None => json_response(404, &serde_json::json!({
                    "error": "No blocks indexed yet"
                })),
            }
        }

        // Block by number
        (_, p) if p.starts_with("/blocks/") => {
            let num_str = p.trim_start_matches("/blocks/");
            match num_str.parse::<i64>() {
                Ok(num) => {
                    match storage.get_block(num).await? {
                        Some(block) => json_response(200, &serde_json::json!({
                            "number": block.number,
                            "hash": block.hash,
                            "parent_hash": block.parent_hash,
                            "timestamp": block.timestamp,
                            "tx_count": block.tx_count
                        })),
                        None => json_response(404, &serde_json::json!({
                            "error": format!("Block {} not found", num)
                        })),
                    }
                }
                Err(_) => json_response(400, &serde_json::json!({
                    "error": "Invalid block number"
                })),
            }
        }

        // Transaction by hash
        (_, p) if p.starts_with("/tx/") => {
            let hash = p.trim_start_matches("/tx/");
            match storage.get_transaction_by_hash(hash).await? {
                Some(tx) => json_response(200, &serde_json::json!({
                    "hash": tx.hash,
                    "block_number": tx.block_number,
                    "from": tx.from_address,
                    "to": tx.to_address,
                    "value": tx.value,
                    "gas_used": tx.gas_used,
                    "status": tx.status,
                    "type": tx.tx_type
                })),
                None => json_response(404, &serde_json::json!({
                    "error": format!("Transaction {} not found", hash)
                })),
            }
        }

        // Transactions by address
        (_, p) if p.contains("/txs") => {
            let addr = p.split('/').nth(2).unwrap_or("");
            let txs = storage.get_transactions_by_address(addr, 50, 0).await?;
            let tx_list: Vec<_> = txs.iter().map(|tx| serde_json::json!({
                "hash": tx.hash,
                "block_number": tx.block_number,
                "from": tx.from_address,
                "to": tx.to_address,
                "value": tx.value,
                "type": tx.tx_type
            })).collect();
            json_response(200, &serde_json::json!({
                "address": addr,
                "count": tx_list.len(),
                "transactions": tx_list
            }))
        }

        // Transfers by address
        (_, p) if p.contains("/transfers") => {
            let addr = p.split('/').nth(2).unwrap_or("");
            let transfers = storage.get_transfers_by_address(addr, 50, 0).await?;
            let transfer_list: Vec<_> = transfers.iter().map(|t| serde_json::json!({
                "tx_hash": t.tx_hash,
                "block_number": t.block_number,
                "from": t.from_address,
                "to": t.to_address,
                "amount": t.amount,
                "timestamp": t.timestamp
            })).collect();
            json_response(200, &serde_json::json!({
                "address": addr,
                "count": transfer_list.len(),
                "transfers": transfer_list
            }))
        }

        // Stake history
        (_, p) if p.starts_with("/stakes/") => {
            let hotkey = p.trim_start_matches("/stakes/");
            let stakes = storage.get_stake_history(hotkey, 100).await?;
            let stake_list: Vec<_> = stakes.iter().map(|s| serde_json::json!({
                "block_number": s.block_number,
                "coldkey": s.coldkey,
                "hotkey": s.hotkey,
                "amount": s.amount,
                "action": s.action,
                "timestamp": s.timestamp
            })).collect();
            json_response(200, &serde_json::json!({
                "hotkey": hotkey,
                "count": stake_list.len(),
                "stakes": stake_list
            }))
        }

        // POST /query - Generic query endpoint
        ("POST", "/query") => {
            // Find body after empty line
            let body_start = request.find("\r\n\r\n")
                .or_else(|| request.find("\n\n"))
                .map(|i| i + 4)
                .unwrap_or(0);
            let body = &request[body_start..];

            match serde_json::from_str::<QueryRequest>(body) {
                Ok(query) => handle_query(&storage, query).await?,
                Err(e) => json_response(400, &serde_json::json!({
                    "error": format!("Invalid JSON: {}", e)
                })),
            }
        }

        // Stats
        ("GET", "/stats") => {
            let status = storage.get_sync_status().await?;
            let latest = storage.get_latest_block().await?;
            json_response(200, &serde_json::json!({
                "last_indexed_block": status.last_indexed_block,
                "is_syncing": status.is_syncing,
                "latest_block_hash": latest.map(|b| b.hash)
            }))
        }

        // Default - API info
        _ => {
            json_response(200, &serde_json::json!({
                "name": "Luxtensor Indexer API",
                "version": "0.1.0",
                "endpoints": {
                    "health": "GET /health",
                    "blocks": "GET /blocks, GET /blocks/:number",
                    "transactions": "GET /tx/:hash, GET /address/:addr/txs",
                    "transfers": "GET /address/:addr/transfers",
                    "stakes": "GET /stakes/:hotkey",
                    "stats": "GET /stats",
                    "query": "POST /query"
                }
            }))
        }
    };

    socket.write_all(response.as_bytes()).await
        .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

    Ok(())
}

async fn handle_query(storage: &Storage, query: QueryRequest) -> Result<String> {
    match query.query_type.as_str() {
        "transaction" => {
            if let Some(hash) = query.hash {
                match storage.get_transaction_by_hash(&hash).await? {
                    Some(tx) => Ok(json_response(200, &serde_json::json!({
                        "hash": tx.hash,
                        "block_number": tx.block_number,
                        "from": tx.from_address,
                        "to": tx.to_address,
                        "value": tx.value
                    }))),
                    None => Ok(json_response(404, &serde_json::json!({
                        "error": "Transaction not found"
                    }))),
                }
            } else {
                Ok(json_response(400, &serde_json::json!({
                    "error": "Missing 'hash' parameter"
                })))
            }
        }
        "transactions" => {
            if let Some(addr) = query.address {
                let limit = query.limit.unwrap_or(50);
                let offset = query.offset.unwrap_or(0);
                let txs = storage.get_transactions_by_address(&addr, limit, offset).await?;
                Ok(json_response(200, &serde_json::json!({
                    "count": txs.len(),
                    "transactions": txs.iter().map(|tx| serde_json::json!({
                        "hash": tx.hash,
                        "block_number": tx.block_number,
                        "from": tx.from_address,
                        "to": tx.to_address,
                        "value": tx.value
                    })).collect::<Vec<_>>()
                })))
            } else {
                Ok(json_response(400, &serde_json::json!({
                    "error": "Missing 'address' parameter"
                })))
            }
        }
        "block" => {
            if let Some(num) = query.number {
                match storage.get_block(num).await? {
                    Some(block) => Ok(json_response(200, &serde_json::json!({
                        "number": block.number,
                        "hash": block.hash,
                        "tx_count": block.tx_count
                    }))),
                    None => Ok(json_response(404, &serde_json::json!({
                        "error": "Block not found"
                    }))),
                }
            } else {
                Ok(json_response(400, &serde_json::json!({
                    "error": "Missing 'number' parameter"
                })))
            }
        }
        "blocks" => {
            let from = query.from.unwrap_or(0);
            let to = query.to.unwrap_or(from + 10);
            let blocks = storage.get_blocks(from, to).await?;
            Ok(json_response(200, &serde_json::json!({
                "count": blocks.len(),
                "blocks": blocks.iter().map(|b| serde_json::json!({
                    "number": b.number,
                    "hash": b.hash,
                    "tx_count": b.tx_count
                })).collect::<Vec<_>>()
            })))
        }
        "transfers" => {
            if let Some(addr) = query.address {
                let limit = query.limit.unwrap_or(50);
                let offset = query.offset.unwrap_or(0);
                let transfers = storage.get_transfers_by_address(&addr, limit, offset).await?;
                Ok(json_response(200, &serde_json::json!({
                    "count": transfers.len(),
                    "transfers": transfers
                })))
            } else {
                Ok(json_response(400, &serde_json::json!({
                    "error": "Missing 'address' parameter"
                })))
            }
        }
        "stakes" => {
            if let Some(hotkey) = query.hotkey {
                let limit = query.limit.unwrap_or(100);
                let stakes = storage.get_stake_history(&hotkey, limit).await?;
                Ok(json_response(200, &serde_json::json!({
                    "count": stakes.len(),
                    "stakes": stakes
                })))
            } else {
                Ok(json_response(400, &serde_json::json!({
                    "error": "Missing 'hotkey' parameter"
                })))
            }
        }
        "stats" => {
            let status = storage.get_sync_status().await?;
            Ok(json_response(200, &serde_json::json!({
                "last_indexed_block": status.last_indexed_block,
                "is_syncing": status.is_syncing
            })))
        }
        _ => {
            Ok(json_response(400, &serde_json::json!({
                "error": format!("Unknown query type: {}", query.query_type),
                "supported": ["transaction", "transactions", "block", "blocks", "transfers", "stakes", "stats"]
            })))
        }
    }
}

fn json_response(status: u16, data: &serde_json::Value) -> String {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let body = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());

    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
        status, status_text, body.len(), body
    )
}

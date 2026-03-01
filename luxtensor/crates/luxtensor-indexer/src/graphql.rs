//! HTTP REST API server for querying indexed data
//!
//! # TODO(#IDX-MIGRATE): Migrate to axum/actix-web
//!
//! This module implements a hand-rolled HTTP/1.1 server using raw TCP sockets.
//! While functional and hardened with rate limiting, timeouts, and body size
//! limits, it should be migrated to a production HTTP framework (e.g. `axum`)
//! for proper HTTP parsing, middleware, keep-alive, chunked encoding, TLS,
//! and compliance with RFC 7230+.

use crate::error::Result;
use crate::storage::Storage;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tracing::info;

/// Constant-time comparison to prevent timing attacks on API key validation.
/// Compares all bytes regardless of where the first difference is.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Maximum number of concurrent HTTP connections to prevent resource exhaustion.
const MAX_CONCURRENT_CONNECTIONS: usize = 1024;

/// Maximum requests per IP within the rate-limit window before throttling.
const RATE_LIMIT_MAX_REQUESTS: u32 = 100;

/// Rate-limit window duration in seconds. Counters reset after this period.
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Interval (in accepted connections) between rate-limiter cleanup sweeps.
/// Stale entries older than `RATE_LIMIT_WINDOW_SECS` are pruned every N accepts.
const RATE_LIMIT_CLEANUP_INTERVAL: u64 = 256;

/// SECURITY (H-1): Maximum request body size in bytes.
/// The read buffer is sized to this limit; requests with Content-Length
/// exceeding this value are rejected before parsing.
const MAX_REQUEST_BODY: usize = 64 * 1024; // 64KB limit

/// Read timeout for incoming HTTP requests (seconds).
const READ_TIMEOUT_SECS: u64 = 10;

/// Per-IP sliding-window rate limiter.
///
/// Each IP is tracked with a request count and the window start time.
/// Once the count exceeds `RATE_LIMIT_MAX_REQUESTS` within the current
/// window, further requests from that IP are rejected with HTTP 429.
/// The window resets automatically once `RATE_LIMIT_WINDOW_SECS` elapse.
struct IpRateLimiter {
    /// Map from IP to (request_count, window_start).
    state: parking_lot::Mutex<HashMap<IpAddr, (u32, Instant)>>,
}

impl IpRateLimiter {
    fn new() -> Self {
        Self { state: parking_lot::Mutex::new(HashMap::new()) }
    }

    /// Check whether `ip` is allowed to proceed.
    /// Returns `true` if under the limit, `false` if rate-limited.
    fn check_rate_limit(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS);
        let mut map = self.state.lock();
        let entry = map.entry(ip).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1) >= window {
            entry.0 = 0;
            entry.1 = now;
        }

        entry.0 += 1;
        entry.0 <= RATE_LIMIT_MAX_REQUESTS
    }

    /// Remove entries whose window has expired to prevent unbounded growth.
    fn cleanup(&self) {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS);
        let mut map = self.state.lock();
        map.retain(|_, (_, start)| now.duration_since(*start) < window);
    }
}

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

impl GraphQLServer {
    /// Create new HTTP API server
    pub fn new(storage: Arc<Storage>, bind_address: &str) -> Self {
        Self { storage, bind_address: bind_address.to_string() }
    }

    /// Run the HTTP API server
    pub async fn run(self) -> Result<()> {
        info!("HTTP API server starting on {}", self.bind_address);

        let listener = TcpListener::bind(&self.bind_address)
            .await
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

        // SECURITY: Limit concurrent connections to prevent resource exhaustion
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
        // SECURITY (M-4): Per-IP rate limiter — sliding window, 100 req/min default
        let rate_limiter = Arc::new(IpRateLimiter::new());
        let mut accept_count: u64 = 0;

        loop {
            let (socket, addr) = listener
                .accept()
                .await
                .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

            // Periodically prune expired entries from the rate limiter
            accept_count = accept_count.wrapping_add(1);
            if accept_count % RATE_LIMIT_CLEANUP_INTERVAL == 0 {
                rate_limiter.cleanup();
            }

            // Per-IP rate limit check (before spawning a task)
            if !rate_limiter.check_rate_limit(addr.ip()) {
                tracing::warn!("Rate limit exceeded for {}, rejecting", addr.ip());
                // Best-effort 429 response; ignore write errors on rejected sockets
                let mut sock = socket;
                let _ = tokio::io::AsyncWriteExt::write_all(
                    &mut sock,
                    b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nRetry-After: 60\r\n\r\n",
                ).await;
                continue;
            }

            let storage = self.storage.clone();
            let api_key = std::env::var("INDEXER_API_KEY").ok();
            let sem = semaphore.clone();

            tokio::spawn(async move {
                // Acquire permit before processing; drop will release it
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::warn!("Semaphore closed, dropping connection from {}", addr);
                        return;
                    }
                };
                if let Err(e) = handle_connection(socket, storage, api_key.as_deref()).await {
                    tracing::error!("Connection error from {}: {}", addr, e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: tokio::net::TcpStream,
    storage: Arc<Storage>,
    api_key: Option<&str>,
) -> Result<()> {
    // SECURITY (H-1): Read buffer capped at MAX_REQUEST_BODY to limit POST payloads.
    let mut buffer = vec![0u8; MAX_REQUEST_BODY];
    // SECURITY: Enforce a read timeout to prevent Slowloris-style DoS attacks
    let n = match tokio::time::timeout(
        std::time::Duration::from_secs(READ_TIMEOUT_SECS),
        socket.read(&mut buffer),
    )
    .await
    {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => {
            return Err(crate::error::IndexerError::Connection(e.to_string()));
        }
        Err(_) => {
            return Err(crate::error::IndexerError::Connection("Read timeout".to_string()));
        }
    };

    let request = String::from_utf8_lossy(&buffer[..n]);
    let lines: Vec<&str> = request.lines().collect();
    let first_line = lines.first().unwrap_or(&"");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    let method = parts.first().unwrap_or(&"GET");
    let path = parts.get(1).unwrap_or(&"/");

    // SECURITY (H-1): Reject early if Content-Length exceeds our buffer limit.
    if let Some(cl) = lines
        .iter()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split_once(':'))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
    {
        if cl > MAX_REQUEST_BODY {
            let response = json_response(
                413,
                &serde_json::json!({
                    "error": "Request body too large"
                }),
            );
            socket
                .write_all(response.as_bytes())
                .await
                .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;
            return Ok(());
        }
    }

    // SECURITY (H-3): Never log API keys. Use constant-time comparison only.
    // API key authentication (if INDEXER_API_KEY env var is set)
    // Health endpoint is always public
    if let Some(expected_key) = api_key {
        if *path != "/health" {
            let auth_header = lines
                .iter()
                .find(|l| l.to_lowercase().starts_with("authorization:"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim());
            let bearer = auth_header.and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
            if !constant_time_eq(bearer.as_bytes(), expected_key.as_bytes()) {
                let response = json_response(
                    401,
                    &serde_json::json!({
                        "error": "Unauthorized: invalid or missing API key"
                    }),
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;
                return Ok(());
            }
        }
    }

    // SECURITY (IDX-H4): Enforce HTTP method on all routes.
    // Only GET and POST are supported; reject everything else early.
    if !matches!(*method, "GET" | "POST" | "OPTIONS") {
        let response = json_response(
            405,
            &serde_json::json!({ "error": "Method not allowed" }),
        );
        socket
            .write_all(response.as_bytes())
            .await
            .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;
        return Ok(());
    }

    let response = match (*method, *path) {
        // Health check
        ("GET", "/health") => {
            let status = storage.get_sync_status().await?;
            json_response(
                200,
                &serde_json::json!({
                    "status": "ok",
                    "last_block": status.last_indexed_block,
                    "syncing": status.is_syncing
                }),
            )
        }

        // Latest block
        ("GET", "/blocks") => match storage.get_latest_block().await? {
            Some(block) => json_response(
                200,
                &serde_json::json!({
                    "number": block.number,
                    "hash": block.hash,
                    "parent_hash": block.parent_hash,
                    "timestamp": block.timestamp,
                    "tx_count": block.tx_count
                }),
            ),
            None => json_response(
                404,
                &serde_json::json!({
                    "error": "No blocks indexed yet"
                }),
            ),
        },

        // Block by number (GET only)
        ("GET", p) if p.starts_with("/blocks/") => {
            let num_str = p.trim_start_matches("/blocks/");
            match num_str.parse::<i64>() {
                Ok(num) => match storage.get_block(num).await? {
                    Some(block) => json_response(
                        200,
                        &serde_json::json!({
                            "number": block.number,
                            "hash": block.hash,
                            "parent_hash": block.parent_hash,
                            "timestamp": block.timestamp,
                            "tx_count": block.tx_count
                        }),
                    ),
                    None => json_response(
                        404,
                        &serde_json::json!({
                            "error": format!("Block {} not found", num)
                        }),
                    ),
                },
                Err(_) => json_response(
                    400,
                    &serde_json::json!({
                        "error": "Invalid block number"
                    }),
                ),
            }
        }

        // Transaction by hash (GET only)
        ("GET", p) if p.starts_with("/tx/") => {
            let hash = p.trim_start_matches("/tx/");
            match storage.get_transaction_by_hash(hash).await? {
                Some(tx) => json_response(
                    200,
                    &serde_json::json!({
                        "hash": tx.hash,
                        "block_number": tx.block_number,
                        "from": tx.from_address,
                        "to": tx.to_address,
                        "value": tx.value,
                        "gas_used": tx.gas_used,
                        "status": tx.status,
                        "type": tx.tx_type
                    }),
                ),
                None => json_response(
                    404,
                    &serde_json::json!({
                        "error": format!("Transaction {} not found", hash)
                    }),
                ),
            }
        }

        // Transactions by address (GET only)
        // SECURITY (IDX-M1): Use starts_with + exact segment matching instead of
        // greedy `contains("/txs")` which could match unintended paths.
        ("GET", p) if p.starts_with("/address/") && p.ends_with("/txs") => {
            let addr = p.strip_prefix("/address/")
                .and_then(|s| s.strip_suffix("/txs"))
                .unwrap_or("");
            let txs = storage.get_transactions_by_address(addr, 50, 0).await?;
            let tx_list: Vec<_> = txs
                .iter()
                .map(|tx| {
                    serde_json::json!({
                        "hash": tx.hash,
                        "block_number": tx.block_number,
                        "from": tx.from_address,
                        "to": tx.to_address,
                        "value": tx.value,
                        "type": tx.tx_type
                    })
                })
                .collect();
            json_response(
                200,
                &serde_json::json!({
                    "address": addr,
                    "count": tx_list.len(),
                    "transactions": tx_list
                }),
            )
        }

        // Transfers by address (GET only)
        // SECURITY (IDX-M1): Use starts_with + exact segment matching.
        ("GET", p) if p.starts_with("/address/") && p.ends_with("/transfers") => {
            let addr = p.strip_prefix("/address/")
                .and_then(|s| s.strip_suffix("/transfers"))
                .unwrap_or("");
            let transfers = storage.get_transfers_by_address(addr, 50, 0).await?;
            let transfer_list: Vec<_> = transfers
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "tx_hash": t.tx_hash,
                        "block_number": t.block_number,
                        "from": t.from_address,
                        "to": t.to_address,
                        "amount": t.amount,
                        "timestamp": t.timestamp
                    })
                })
                .collect();
            json_response(
                200,
                &serde_json::json!({
                    "address": addr,
                    "count": transfer_list.len(),
                    "transfers": transfer_list
                }),
            )
        }

        // Stake history (GET only)
        ("GET", p) if p.starts_with("/stakes/") => {
            let hotkey = p.trim_start_matches("/stakes/");
            let stakes = storage.get_stake_history(hotkey, 100).await?;
            let stake_list: Vec<_> = stakes
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "block_number": s.block_number,
                        "coldkey": s.coldkey,
                        "hotkey": s.hotkey,
                        "amount": s.amount,
                        "action": s.action,
                        "timestamp": s.timestamp
                    })
                })
                .collect();
            json_response(
                200,
                &serde_json::json!({
                    "hotkey": hotkey,
                    "count": stake_list.len(),
                    "stakes": stake_list
                }),
            )
        }

        // POST /query - Generic query endpoint
        ("POST", "/query") => {
            // Find body after empty line
            // SECURITY: Handle both \r\n\r\n (4 bytes) and \n\n (2 bytes) correctly
            let body_start = request
                .find("\r\n\r\n")
                .map(|i| i + 4)
                .or_else(|| request.find("\n\n").map(|i| i + 2))
                .unwrap_or(0);
            let body = &request[body_start..];

            match serde_json::from_str::<QueryRequest>(body) {
                Ok(query) => handle_query(&storage, query).await?,
                Err(e) => json_response(
                    400,
                    &serde_json::json!({
                        "error": format!("Invalid JSON: {}", e)
                    }),
                ),
            }
        }

        // Stats (GET only)
        ("GET", "/stats") => {
            let status = storage.get_sync_status().await?;
            let latest = storage.get_latest_block().await?;
            json_response(
                200,
                &serde_json::json!({
                    "last_indexed_block": status.last_indexed_block,
                    "is_syncing": status.is_syncing,
                    "latest_block_hash": latest.map(|b| b.hash)
                }),
            )
        }

        // Default - API info
        _ => json_response(
            200,
            &serde_json::json!({
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
            }),
        ),
    };

    socket
        .write_all(response.as_bytes())
        .await
        .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

    Ok(())
}

async fn handle_query(storage: &Storage, query: QueryRequest) -> Result<String> {
    match query.query_type.as_str() {
        "transaction" => {
            if let Some(hash) = query.hash {
                match storage.get_transaction_by_hash(&hash).await? {
                    Some(tx) => Ok(json_response(
                        200,
                        &serde_json::json!({
                            "hash": tx.hash,
                            "block_number": tx.block_number,
                            "from": tx.from_address,
                            "to": tx.to_address,
                            "value": tx.value
                        }),
                    )),
                    None => Ok(json_response(
                        404,
                        &serde_json::json!({
                            "error": "Transaction not found"
                        }),
                    )),
                }
            } else {
                Ok(json_response(
                    400,
                    &serde_json::json!({
                        "error": "Missing 'hash' parameter"
                    }),
                ))
            }
        }
        "transactions" => {
            if let Some(addr) = query.address {
                // SECURITY (IDX-H2): Cap limit/offset to prevent excessive DB load
                let limit = query.limit.unwrap_or(50).min(1000).max(1);
                let offset = query.offset.unwrap_or(0).max(0);
                let txs = storage.get_transactions_by_address(&addr, limit, offset).await?;
                Ok(json_response(
                    200,
                    &serde_json::json!({
                        "count": txs.len(),
                        "transactions": txs.iter().map(|tx| serde_json::json!({
                            "hash": tx.hash,
                            "block_number": tx.block_number,
                            "from": tx.from_address,
                            "to": tx.to_address,
                            "value": tx.value
                        })).collect::<Vec<_>>()
                    }),
                ))
            } else {
                Ok(json_response(
                    400,
                    &serde_json::json!({
                        "error": "Missing 'address' parameter"
                    }),
                ))
            }
        }
        "block" => {
            if let Some(num) = query.number {
                match storage.get_block(num).await? {
                    Some(block) => Ok(json_response(
                        200,
                        &serde_json::json!({
                            "number": block.number,
                            "hash": block.hash,
                            "tx_count": block.tx_count
                        }),
                    )),
                    None => Ok(json_response(
                        404,
                        &serde_json::json!({
                            "error": "Block not found"
                        }),
                    )),
                }
            } else {
                Ok(json_response(
                    400,
                    &serde_json::json!({
                        "error": "Missing 'number' parameter"
                    }),
                ))
            }
        }
        "blocks" => {
            let from = query.from.unwrap_or(0);
            // SECURITY (IDX-H2): Cap block range to prevent excessive DB load
            let to = query.to.unwrap_or(from + 10).min(from.saturating_add(1000));
            let blocks = storage.get_blocks(from, to).await?;
            Ok(json_response(
                200,
                &serde_json::json!({
                    "count": blocks.len(),
                    "blocks": blocks.iter().map(|b| serde_json::json!({
                        "number": b.number,
                        "hash": b.hash,
                        "tx_count": b.tx_count
                    })).collect::<Vec<_>>()
                }),
            ))
        }
        "transfers" => {
            if let Some(addr) = query.address {
                // SECURITY (IDX-H2): Cap limit/offset to prevent excessive DB load
                let limit = query.limit.unwrap_or(50).min(1000).max(1);
                let offset = query.offset.unwrap_or(0).max(0);
                let transfers = storage.get_transfers_by_address(&addr, limit, offset).await?;
                Ok(json_response(
                    200,
                    &serde_json::json!({
                        "count": transfers.len(),
                        "transfers": transfers
                    }),
                ))
            } else {
                Ok(json_response(
                    400,
                    &serde_json::json!({
                        "error": "Missing 'address' parameter"
                    }),
                ))
            }
        }
        "stakes" => {
            if let Some(hotkey) = query.hotkey {
                // SECURITY (IDX-H2): Cap limit to prevent excessive DB load
                let limit = query.limit.unwrap_or(100).min(1000).max(1);
                let stakes = storage.get_stake_history(&hotkey, limit).await?;
                Ok(json_response(
                    200,
                    &serde_json::json!({
                        "count": stakes.len(),
                        "stakes": stakes
                    }),
                ))
            } else {
                Ok(json_response(
                    400,
                    &serde_json::json!({
                        "error": "Missing 'hotkey' parameter"
                    }),
                ))
            }
        }
        "stats" => {
            let status = storage.get_sync_status().await?;
            Ok(json_response(
                200,
                &serde_json::json!({
                    "last_indexed_block": status.last_indexed_block,
                    "is_syncing": status.is_syncing
                }),
            ))
        }
        _ => Ok(json_response(
            400,
            &serde_json::json!({
                "error": format!("Unknown query type: {}", query.query_type),
                "supported": ["transaction", "transactions", "block", "blocks", "transfers", "stakes", "stats"]
            }),
        )),
    }
}

// SECURITY (H-2): Restrict CORS origin. In production, set this to the specific
// frontend domain instead of a wildcard. Default to 'null' (no cross-origin access).
fn json_response(status: u16, data: &serde_json::Value) -> String {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let body = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());

    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: null\r\n\r\n{}",
        status, status_text, body.len(), body
    )
}

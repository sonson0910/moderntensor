//! HTTP REST API server for querying indexed data
use crate::error::Result;
use crate::storage::Storage;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// HTTP API server
pub struct GraphQLServer {
    storage: Arc<Storage>,
    bind_address: String,
}

#[derive(Clone)]
struct AppState {
    storage: Arc<Storage>,
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
struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

// Helper to sanitize limit
fn sanitize_limit(limit: Option<i32>) -> i32 {
    limit.unwrap_or(50).clamp(1, 100)
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

        let state = AppState {
            storage: self.storage,
        };

        let app = Router::new()
            .route("/", get(api_info))
            .route("/health", get(health_check))
            .route("/blocks", get(get_latest_block))
            .route("/blocks/:number", get(get_block_by_number))
            .route("/tx/:hash", get(get_transaction))
            .route("/address/:addr/txs", get(get_transactions_by_address))
            .route("/address/:addr/transfers", get(get_transfers_by_address))
            .route("/stakes/:hotkey", get(get_stake_history))
            .route("/query", post(handle_query))
            .route("/stats", get(get_stats))
            .with_state(state);

        info!("Indexer API listening on http://{}", self.bind_address);
        let listener = tokio::net::TcpListener::bind(&self.bind_address).await
            .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

        axum::serve(listener, app).await
            .map_err(|e| crate::error::IndexerError::Connection(e.to_string()))?;

        Ok(())
    }
}

// Wraps JSON response with CORS headers
fn json_response<T: Serialize>(status: StatusCode, body: T) -> Response {
    (
        status,
        [("Access-Control-Allow-Origin", "*")],
        Json(body),
    ).into_response()
}

// Handlers

async fn api_info() -> Response {
    json_response(StatusCode::OK, serde_json::json!({
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

async fn health_check(State(state): State<AppState>) -> Response {
    match state.storage.get_sync_status().await {
        Ok(status) => json_response(StatusCode::OK, serde_json::json!({
            "status": "ok",
            "last_block": status.last_indexed_block,
            "syncing": status.is_syncing
        })),
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_latest_block(State(state): State<AppState>) -> Response {
    match state.storage.get_latest_block().await {
        Ok(Some(block)) => json_response(StatusCode::OK, block),
        Ok(None) => json_response(StatusCode::NOT_FOUND, ApiResponse::<()>::error("No blocks indexed yet")),
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_block_by_number(
    State(state): State<AppState>,
    Path(number): Path<i64>,
) -> Response {
    match state.storage.get_block(number).await {
        Ok(Some(block)) => json_response(StatusCode::OK, block),
        Ok(None) => json_response(StatusCode::NOT_FOUND, ApiResponse::<()>::error(format!("Block {} not found", number))),
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_transaction(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Response {
    match state.storage.get_transaction_by_hash(&hash).await {
        Ok(Some(tx)) => json_response(StatusCode::OK, tx),
        Ok(None) => json_response(StatusCode::NOT_FOUND, ApiResponse::<()>::error(format!("Transaction {} not found", hash))),
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_transactions_by_address(
    State(state): State<AppState>,
    Path(addr): Path<String>,
) -> Response {
    match state.storage.get_transactions_by_address(&addr, 50, 0).await {
        Ok(txs) => {
             json_response(StatusCode::OK, serde_json::json!({
                "address": addr,
                "count": txs.len(),
                "transactions": txs
            }))
        },
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_transfers_by_address(
    State(state): State<AppState>,
    Path(addr): Path<String>,
) -> Response {
    match state.storage.get_transfers_by_address(&addr, 50, 0).await {
        Ok(transfers) => {
            json_response(StatusCode::OK, serde_json::json!({
                "address": addr,
                "count": transfers.len(),
                "transfers": transfers
            }))
        },
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_stake_history(
    State(state): State<AppState>,
    Path(hotkey): Path<String>,
) -> Response {
    match state.storage.get_stake_history(&hotkey, 100).await {
        Ok(stakes) => {
            json_response(StatusCode::OK, serde_json::json!({
                "hotkey": hotkey,
                "count": stakes.len(),
                "stakes": stakes
            }))
        },
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn get_stats(State(state): State<AppState>) -> Response {
     match tokio::try_join!(
        state.storage.get_sync_status(),
        state.storage.get_latest_block()
    ) {
        Ok((status, latest)) => {
             json_response(StatusCode::OK, serde_json::json!({
                "last_indexed_block": status.last_indexed_block,
                "is_syncing": status.is_syncing,
                "latest_block_hash": latest.map(|b| b.hash)
            }))
        },
        Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
    }
}

async fn handle_query(
    State(state): State<AppState>,
    Json(query): Json<QueryRequest>,
) -> Response {
    match query.query_type.as_str() {
        "transaction" => {
            if let Some(hash) = query.hash {
                match state.storage.get_transaction_by_hash(&hash).await {
                    Ok(Some(tx)) => json_response(StatusCode::OK, tx),
                    Ok(None) => json_response(StatusCode::NOT_FOUND, ApiResponse::<()>::error("Transaction not found")),
                    Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
                }
            } else {
                json_response(StatusCode::BAD_REQUEST, ApiResponse::<()>::error("Missing 'hash' parameter"))
            }
        }
        "transactions" => {
            if let Some(addr) = query.address {
                let limit = sanitize_limit(query.limit);
                let offset = query.offset.unwrap_or(0);
                match state.storage.get_transactions_by_address(&addr, limit, offset).await {
                    Ok(txs) => json_response(StatusCode::OK, serde_json::json!({
                        "count": txs.len(),
                        "transactions": txs
                    })),
                    Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
                }
            } else {
                json_response(StatusCode::BAD_REQUEST, ApiResponse::<()>::error("Missing 'address' parameter"))
            }
        }
        "block" => {
            if let Some(num) = query.number {
                match state.storage.get_block(num).await {
                    Ok(Some(block)) => json_response(StatusCode::OK, block),
                    Ok(None) => json_response(StatusCode::NOT_FOUND, ApiResponse::<()>::error("Block not found")),
                    Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
                }
            } else {
                json_response(StatusCode::BAD_REQUEST, ApiResponse::<()>::error("Missing 'number' parameter"))
            }
        }
        "blocks" => {
            let from = query.from.unwrap_or(0);
            let to = query.to.unwrap_or(from + 10);
            match state.storage.get_blocks(from, to).await {
                 Ok(blocks) => json_response(StatusCode::OK, serde_json::json!({
                    "count": blocks.len(),
                    "blocks": blocks
                })),
                Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
            }
        }
        "transfers" => {
             if let Some(addr) = query.address {
                let limit = sanitize_limit(query.limit);
                let offset = query.offset.unwrap_or(0);
                match state.storage.get_transfers_by_address(&addr, limit, offset).await {
                     Ok(transfers) => json_response(StatusCode::OK, serde_json::json!({
                        "count": transfers.len(),
                        "transfers": transfers
                    })),
                    Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
                }
            } else {
                json_response(StatusCode::BAD_REQUEST, ApiResponse::<()>::error("Missing 'address' parameter"))
            }
        }
        "stakes" => {
             if let Some(hotkey) = query.hotkey {
                let limit = sanitize_limit(query.limit);
                match state.storage.get_stake_history(&hotkey, limit).await {
                     Ok(stakes) => json_response(StatusCode::OK, serde_json::json!({
                        "count": stakes.len(),
                        "stakes": stakes
                    })),
                    Err(e) => json_response(StatusCode::INTERNAL_SERVER_ERROR, ApiResponse::<()>::error(e.to_string())),
                }
            } else {
               json_response(StatusCode::BAD_REQUEST, ApiResponse::<()>::error("Missing 'hotkey' parameter"))
            }
        }
        "stats" => get_stats(State(state)).await,
        _ => json_response(StatusCode::BAD_REQUEST, ApiResponse::<()>::error(format!("Unknown query type: {}", query.query_type))),
    }
}

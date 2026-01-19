// RPC Client for testing LuxTensor nodes
// Provides HTTP client for JSON-RPC calls

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

/// RPC Client for LuxTensor node
pub struct RpcClient {
    url: String,
    client: reqwest::blocking::Client,
    id: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    method: String,
    params: Value,
    id: u64,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
    pub id: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcClient {
    /// Create new RPC client for given endpoint
    pub fn new(url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            url: url.to_string(),
            client,
            id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create client for Node 1 (default port 8545)
    pub fn node1() -> Self {
        Self::new("http://localhost:8545")
    }

    /// Create client for Node 2 (port 8555)
    pub fn node2() -> Self {
        Self::new("http://localhost:8555")
    }

    /// Create client for Node 3 (port 8565)
    pub fn node3() -> Self {
        Self::new("http://localhost:8565")
    }

    /// Send raw JSON-RPC request
    pub fn call(&self, method: &str, params: Value) -> Result<JsonRpcResponse, String> {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
            id,
        };

        let response = self.client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        response.json::<JsonRpcResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    // ============================================================
    // Ethereum JSON-RPC Methods
    // ============================================================

    /// Get current block number
    pub fn eth_block_number(&self) -> Result<u64, String> {
        let response = self.call("eth_blockNumber", json!([]))?;

        if let Some(error) = response.error {
            return Err(format!("RPC error: {}", error.message));
        }

        response.result
            .and_then(|v| v.as_str().map(String::from))
            .and_then(|s| {
                let s = s.strip_prefix("0x").unwrap_or(&s);
                u64::from_str_radix(s, 16).ok()
            })
            .ok_or_else(|| "Invalid block number response".to_string())
    }

    /// Send transaction
    pub fn eth_send_transaction(
        &self,
        from: &str,
        to: &str,
        value: &str,
        gas: Option<&str>,
    ) -> Result<String, String> {
        let mut tx_obj = json!({
            "from": from,
            "to": to,
            "value": value,
        });

        if let Some(g) = gas {
            tx_obj["gas"] = json!(g);
        }

        let response = self.call("eth_sendTransaction", json!([tx_obj]))?;

        if let Some(error) = response.error {
            return Err(format!("RPC error: {}", error.message));
        }

        response.result
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| "Invalid transaction hash response".to_string())
    }

    /// Get transaction by hash
    pub fn eth_get_transaction_by_hash(&self, hash: &str) -> Result<Option<Value>, String> {
        let response = self.call("eth_getTransactionByHash", json!([hash]))?;

        if let Some(error) = response.error {
            return Err(format!("RPC error: {}", error.message));
        }

        Ok(response.result.filter(|v| !v.is_null()))
    }

    /// Get block by number
    pub fn eth_get_block_by_number(&self, number: &str, full_txs: bool) -> Result<Option<Value>, String> {
        let response = self.call("eth_getBlockByNumber", json!([number, full_txs]))?;

        if let Some(error) = response.error {
            return Err(format!("RPC error: {}", error.message));
        }

        Ok(response.result.filter(|v| !v.is_null()))
    }

    /// Get balance
    pub fn eth_get_balance(&self, address: &str, block: &str) -> Result<String, String> {
        let response = self.call("eth_getBalance", json!([address, block]))?;

        if let Some(error) = response.error {
            return Err(format!("RPC error: {}", error.message));
        }

        response.result
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| "Invalid balance response".to_string())
    }

    /// Check if node is reachable
    pub fn is_reachable(&self) -> bool {
        self.eth_block_number().is_ok()
    }

    /// Wait for node to be ready
    pub fn wait_for_ready(&self, timeout_secs: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            if self.is_reachable() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        false
    }

    /// Wait for transaction to be found
    pub fn wait_for_transaction(&self, hash: &str, timeout_secs: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            if let Ok(Some(_)) = self.eth_get_transaction_by_hash(hash) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        false
    }
}

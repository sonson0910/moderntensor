// WebSocket RPC server for real-time blockchain events
// Provides subscriptions for new blocks, transactions, and logs

use crate::types::{RpcBlock, RpcTransaction};
use crate::RpcError;
use futures::{stream::StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

/// WebSocket subscription types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionType {
    /// Subscribe to new blocks
    NewHeads,
    /// Subscribe to new pending transactions
    NewPendingTransactions,
    /// Subscribe to logs matching a filter
    Logs,
    /// Subscribe to sync status updates
    Syncing,
}

/// WebSocket request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum WsRequest {
    /// Subscribe to events
    #[serde(rename = "eth_subscribe")]
    Subscribe { params: Vec<String> },
    /// Unsubscribe from events
    #[serde(rename = "eth_unsubscribe")]
    Unsubscribe { params: Vec<String> },
}

/// WebSocket response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<WsError>,
}

/// WebSocket error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsError {
    pub code: i32,
    pub message: String,
}

/// Subscription notification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: SubscriptionParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionParams {
    pub subscription: String,
    pub result: serde_json::Value,
}

/// WebSocket RPC server
pub struct WebSocketServer {
    subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    broadcast_tx: mpsc::UnboundedSender<BroadcastEvent>,
    broadcast_rx: Arc<RwLock<mpsc::UnboundedReceiver<BroadcastEvent>>>,
}

/// A subscription
#[derive(Debug, Clone)]
struct Subscription {
    id: String,
    sub_type: SubscriptionType,
    tx: mpsc::UnboundedSender<Message>,
}

/// Event to broadcast to subscribers
#[derive(Debug, Clone)]
pub enum BroadcastEvent {
    NewBlock(RpcBlock),
    NewTransaction(RpcTransaction),
    SyncStatus { syncing: bool },
}

impl WebSocketServer {
    /// Create a new WebSocket server
    pub fn new() -> Self {
        let (broadcast_tx, broadcast_rx) = mpsc::unbounded_channel();

        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
            broadcast_rx: Arc::new(RwLock::new(broadcast_rx)),
        }
    }

    /// Get a handle to send broadcast events
    pub fn get_broadcast_sender(&self) -> mpsc::UnboundedSender<BroadcastEvent> {
        self.broadcast_tx.clone()
    }

    /// Start the WebSocket server
    pub async fn start(self, addr: &str) -> Result<(), RpcError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| RpcError::ServerError(e.to_string()))?;

        info!("WebSocket RPC server listening on {}", addr);

        // Spawn broadcast handler
        let subscriptions = self.subscriptions.clone();
        let mut broadcast_rx = self.broadcast_rx.write().await;
        let broadcast_rx = std::mem::replace(
            &mut *broadcast_rx,
            mpsc::unbounded_channel().1, // dummy receiver
        );

        tokio::spawn(async move {
            Self::handle_broadcasts(subscriptions, broadcast_rx).await;
        });

        // Accept connections
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New WebSocket connection from {}", addr);
                    let subscriptions = self.subscriptions.clone();

                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, subscriptions).await {
                            error!("WebSocket connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle a WebSocket connection
    async fn handle_connection(
        stream: TcpStream,
        subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
    ) -> Result<(), RpcError> {
        let ws_stream = accept_async(stream)
            .await
            .map_err(|e| RpcError::ServerError(e.to_string()))?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Spawn a task to send messages from the channel to the WebSocket
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_sender.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Handle incoming messages
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    debug!("Received message: {}", text);

                    if let Err(e) = Self::handle_message(&text, &subscriptions, &tx).await {
                        warn!("Error handling message: {}", e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle a WebSocket message
    async fn handle_message(
        text: &str,
        subscriptions: &Arc<RwLock<HashMap<String, Subscription>>>,
        tx: &mpsc::UnboundedSender<Message>,
    ) -> Result<(), RpcError> {
        let request: serde_json::Value = serde_json::from_str(text)
            .map_err(|e| RpcError::InvalidRequest(e.to_string()))?;

        let method = request["method"]
            .as_str()
            .ok_or_else(|| RpcError::InvalidRequest("Missing method".to_string()))?;

        let id = request["id"].as_u64();

        match method {
            "eth_subscribe" => {
                let params = request["params"]
                    .as_array()
                    .ok_or_else(|| RpcError::InvalidRequest("Invalid params".to_string()))?;

                if params.is_empty() {
                    return Err(RpcError::InvalidRequest(
                        "Missing subscription type".to_string(),
                    ));
                }

                let sub_type_str = params[0]
                    .as_str()
                    .ok_or_else(|| RpcError::InvalidRequest("Invalid subscription type".to_string()))?;

                let sub_type = match sub_type_str {
                    "newHeads" => SubscriptionType::NewHeads,
                    "newPendingTransactions" => SubscriptionType::NewPendingTransactions,
                    "logs" => SubscriptionType::Logs,
                    "syncing" => SubscriptionType::Syncing,
                    _ => {
                        return Err(RpcError::InvalidRequest(format!(
                            "Unknown subscription type: {}",
                            sub_type_str
                        )))
                    }
                };

                // Generate subscription ID (using timestamp + counter for uniqueness)
                use std::sync::atomic::{AtomicU64, Ordering};
                static COUNTER: AtomicU64 = AtomicU64::new(0);
                let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let sub_id = format!("0x{:016x}{:016x}", timestamp, counter);

                // Store subscription
                let subscription = Subscription {
                    id: sub_id.clone(),
                    sub_type: sub_type.clone(),
                    tx: tx.clone(),
                };

                subscriptions.write().await.insert(sub_id.clone(), subscription);

                // Send response
                let response = WsResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!(sub_id)),
                    error: None,
                };

                let response_text = serde_json::to_string(&response)
                    .map_err(|e| RpcError::SerializationError(e.to_string()))?;

                tx.send(Message::Text(response_text))
                    .map_err(|e| RpcError::ServerError(e.to_string()))?;

                info!("Created subscription {} for {:?}", sub_id, sub_type);
            }
            "eth_unsubscribe" => {
                let params = request["params"]
                    .as_array()
                    .ok_or_else(|| RpcError::InvalidRequest("Invalid params".to_string()))?;

                if params.is_empty() {
                    return Err(RpcError::InvalidRequest(
                        "Missing subscription ID".to_string(),
                    ));
                }

                let sub_id = params[0]
                    .as_str()
                    .ok_or_else(|| RpcError::InvalidRequest("Invalid subscription ID".to_string()))?;

                let removed = subscriptions.write().await.remove(sub_id).is_some();

                let response = WsResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!(removed)),
                    error: None,
                };

                let response_text = serde_json::to_string(&response)
                    .map_err(|e| RpcError::SerializationError(e.to_string()))?;

                tx.send(Message::Text(response_text))
                    .map_err(|e| RpcError::ServerError(e.to_string()))?;

                info!("Removed subscription {}", sub_id);
            }
            _ => {
                return Err(RpcError::MethodNotFound(method.to_string()));
            }
        }

        Ok(())
    }

    /// Handle broadcast events
    async fn handle_broadcasts(
        subscriptions: Arc<RwLock<HashMap<String, Subscription>>>,
        mut broadcast_rx: mpsc::UnboundedReceiver<BroadcastEvent>,
    ) {
        while let Some(event) = broadcast_rx.recv().await {
            let subscriptions = subscriptions.read().await;

            match event {
                BroadcastEvent::NewBlock(block) => {
                    for (sub_id, sub) in subscriptions.iter() {
                        if sub.sub_type == SubscriptionType::NewHeads {
                            let notification = SubscriptionNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "eth_subscription".to_string(),
                                params: SubscriptionParams {
                                    subscription: sub_id.clone(),
                                    result: serde_json::to_value(&block).unwrap(),
                                },
                            };

                            if let Ok(text) = serde_json::to_string(&notification) {
                                let _ = sub.tx.send(Message::Text(text));
                            }
                        }
                    }
                }
                BroadcastEvent::NewTransaction(tx) => {
                    for (sub_id, sub) in subscriptions.iter() {
                        if sub.sub_type == SubscriptionType::NewPendingTransactions {
                            let notification = SubscriptionNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "eth_subscription".to_string(),
                                params: SubscriptionParams {
                                    subscription: sub_id.clone(),
                                    result: serde_json::to_value(&tx.hash).unwrap(),
                                },
                            };

                            if let Ok(text) = serde_json::to_string(&notification) {
                                let _ = sub.tx.send(Message::Text(text));
                            }
                        }
                    }
                }
                BroadcastEvent::SyncStatus { syncing } => {
                    for (sub_id, sub) in subscriptions.iter() {
                        if sub.sub_type == SubscriptionType::Syncing {
                            let notification = SubscriptionNotification {
                                jsonrpc: "2.0".to_string(),
                                method: "eth_subscription".to_string(),
                                params: SubscriptionParams {
                                    subscription: sub_id.clone(),
                                    result: serde_json::json!({ "syncing": syncing }),
                                },
                            };

                            if let Ok(text) = serde_json::to_string(&notification) {
                                let _ = sub.tx.send(Message::Text(text));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for WebSocketServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_type_serialization() {
        let sub_type = SubscriptionType::NewHeads;
        let json = serde_json::to_string(&sub_type).unwrap();
        assert_eq!(json, r#""NewHeads""#);

        let deserialized: SubscriptionType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SubscriptionType::NewHeads);
    }

    #[test]
    fn test_ws_response_serialization() {
        let response = WsResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            result: Some(serde_json::json!("0x123")),
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"result\":\"0x123\""));
    }

    #[tokio::test]
    async fn test_websocket_server_creation() {
        let server = WebSocketServer::new();
        let _sender = server.get_broadcast_sender();
        // Basic creation test
        assert!(true);
    }
}

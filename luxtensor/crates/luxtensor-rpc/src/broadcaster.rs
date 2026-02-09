// Transaction broadcasting module for production use
// Provides trait-based abstraction for P2P and WebSocket broadcasting

use luxtensor_core::Transaction;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Errors that can occur during transaction broadcasting
#[derive(Debug, Error)]
pub enum BroadcastError {
    #[error("Channel closed: {0}")]
    ChannelClosed(String),

    #[error("Broadcast timeout")]
    Timeout,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("All broadcasters failed")]
    AllFailed,
}

/// Transaction broadcaster trait - required for RpcServer
///
/// This trait abstracts the broadcasting mechanism, allowing different
/// implementations for P2P network, WebSocket subscribers, or both.
///
/// # Thread Safety
/// All implementations must be Send + Sync to support async operations.
pub trait TransactionBroadcaster: Send + Sync {
    /// Broadcast a transaction to the network/subscribers
    ///
    /// Returns Ok(()) if broadcast succeeded to at least one peer/subscriber.
    /// Returns Err if broadcast completely failed.
    fn broadcast(&self, tx: &Transaction) -> Result<(), BroadcastError>;

    /// Get the name of this broadcaster for logging
    fn name(&self) -> &'static str;
}

/// No-op broadcaster for testing or when broadcasting is disabled
pub struct NoOpBroadcaster;

impl TransactionBroadcaster for NoOpBroadcaster {
    fn broadcast(&self, tx: &Transaction) -> Result<(), BroadcastError> {
        debug!("NoOpBroadcaster: Skipping broadcast for tx 0x{}", hex::encode(tx.hash()));
        Ok(())
    }

    fn name(&self) -> &'static str {
        "NoOp"
    }
}

/// Default broadcaster channel capacity — bounded to prevent OOM
const BROADCASTER_CHANNEL_CAPACITY: usize = 4096;

/// Channel-based broadcaster that sends to a receiver (P2P layer or other)
///
/// SECURITY: Uses bounded channel to prevent unbounded memory growth
/// from slow consumers or malicious peers.
pub struct ChannelBroadcaster {
    sender: mpsc::Sender<Transaction>,
    name: &'static str,
}

impl ChannelBroadcaster {
    /// Create a new channel broadcaster with a bounded sender
    pub fn new(sender: mpsc::Sender<Transaction>, name: &'static str) -> Self {
        Self { sender, name }
    }

    /// Create broadcaster for P2P network
    pub fn for_p2p(sender: mpsc::Sender<Transaction>) -> Self {
        Self::new(sender, "P2P")
    }

    /// Create broadcaster for WebSocket subscribers
    pub fn for_websocket(sender: mpsc::Sender<Transaction>) -> Self {
        Self::new(sender, "WebSocket")
    }

    /// Create a bounded channel pair for P2P broadcasting
    pub fn new_p2p_pair() -> (Self, mpsc::Receiver<Transaction>) {
        let (tx, rx) = mpsc::channel(BROADCASTER_CHANNEL_CAPACITY);
        (Self::new(tx, "P2P"), rx)
    }

    /// Create a bounded channel pair for WebSocket broadcasting
    pub fn new_websocket_pair() -> (Self, mpsc::Receiver<Transaction>) {
        let (tx, rx) = mpsc::channel(BROADCASTER_CHANNEL_CAPACITY);
        (Self::new(tx, "WebSocket"), rx)
    }
}

impl TransactionBroadcaster for ChannelBroadcaster {
    fn broadcast(&self, tx: &Transaction) -> Result<(), BroadcastError> {
        // SECURITY: try_send so broadcast() stays sync and non-blocking.
        // If channel is full, the oldest consumer hasn't kept up.
        self.sender.try_send(tx.clone()).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                warn!("📤 {} broadcaster: Channel full, dropping tx 0x{}", self.name, hex::encode(tx.hash()));
                BroadcastError::NetworkError("Broadcast channel full".to_string())
            }
            mpsc::error::TrySendError::Closed(_) => {
                BroadcastError::ChannelClosed(format!("{} channel closed", self.name))
            }
        })?;

        info!("📤 {} broadcaster: Sent tx 0x{} to channel", self.name, hex::encode(tx.hash()));
        Ok(())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

/// Composite broadcaster that broadcasts to multiple channels
///
/// Use this when you need to broadcast to both P2P and WebSocket.
/// Success if at least one broadcaster succeeds.
pub struct CompositeBroadcaster {
    broadcasters: Vec<Arc<dyn TransactionBroadcaster>>,
}

impl CompositeBroadcaster {
    /// Create a new composite broadcaster
    pub fn new(broadcasters: Vec<Arc<dyn TransactionBroadcaster>>) -> Self {
        Self { broadcasters }
    }

    /// Create with two broadcasters (common case: P2P + WebSocket)
    pub fn dual(
        first: Arc<dyn TransactionBroadcaster>,
        second: Arc<dyn TransactionBroadcaster>,
    ) -> Self {
        Self::new(vec![first, second])
    }
}

impl TransactionBroadcaster for CompositeBroadcaster {
    fn broadcast(&self, tx: &Transaction) -> Result<(), BroadcastError> {
        let tx_hash = hex::encode(tx.hash());
        let mut success_count = 0;
        let mut last_error = None;

        for broadcaster in &self.broadcasters {
            match broadcaster.broadcast(tx) {
                Ok(()) => {
                    success_count += 1;
                    debug!("Composite: {} succeeded for tx 0x{}", broadcaster.name(), tx_hash);
                }
                Err(e) => {
                    warn!("Composite: {} failed for tx 0x{}: {}", broadcaster.name(), tx_hash, e);
                    last_error = Some(e);
                }
            }
        }

        if success_count > 0 {
            debug!(
                "Composite: {}/{} broadcasters succeeded for tx 0x{}",
                success_count,
                self.broadcasters.len(),
                tx_hash
            );
            Ok(())
        } else {
            last_error.map_or(Err(BroadcastError::AllFailed), Err)
        }
    }

    fn name(&self) -> &'static str {
        "Composite"
    }
}

/// Builder for creating broadcasters with fluent API
pub struct BroadcasterBuilder {
    broadcasters: Vec<Arc<dyn TransactionBroadcaster>>,
}

impl BroadcasterBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self { broadcasters: Vec::new() }
    }

    /// Add P2P broadcaster (bounded channel)
    pub fn with_p2p(mut self, sender: mpsc::Sender<Transaction>) -> Self {
        self.broadcasters.push(Arc::new(ChannelBroadcaster::for_p2p(sender)));
        self
    }

    /// Add WebSocket broadcaster (bounded channel)
    pub fn with_websocket(mut self, sender: mpsc::Sender<Transaction>) -> Self {
        self.broadcasters.push(Arc::new(ChannelBroadcaster::for_websocket(sender)));
        self
    }

    /// Add a custom broadcaster
    pub fn with_custom(mut self, broadcaster: Arc<dyn TransactionBroadcaster>) -> Self {
        self.broadcasters.push(broadcaster);
        self
    }

    /// Build the final broadcaster
    pub fn build(self) -> Arc<dyn TransactionBroadcaster> {
        match self.broadcasters.len() {
            0 => Arc::new(NoOpBroadcaster),
            1 => self.broadcasters.into_iter().next().unwrap_or_else(|| Arc::new(NoOpBroadcaster)),
            _ => Arc::new(CompositeBroadcaster::new(self.broadcasters)),
        }
    }
}

impl Default for BroadcasterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::Address;

    fn create_test_tx() -> Transaction {
        Transaction {
            chain_id: 8898, // LuxTensor devnet chain ID for tests
            nonce: 1,
            from: Address::zero(),
            to: Some(Address::zero()),
            value: 1000,
            gas_price: 1,
            gas_limit: 21000,
            data: vec![],
            v: 0,
            r: [0u8; 32],
            s: [0u8; 32],
        }
    }

    #[test]
    fn test_noop_broadcaster() {
        let broadcaster = NoOpBroadcaster;
        let tx = create_test_tx();

        assert!(broadcaster.broadcast(&tx).is_ok());
        assert_eq!(broadcaster.name(), "NoOp");
    }

    #[test]
    fn test_channel_broadcaster() {
        let (sender, mut receiver) = mpsc::channel(64);
        let broadcaster = ChannelBroadcaster::for_p2p(sender);
        let tx = create_test_tx();

        assert!(broadcaster.broadcast(&tx).is_ok());
        assert_eq!(broadcaster.name(), "P2P");

        // Verify tx was received
        let received = receiver.try_recv().unwrap();
        assert_eq!(received.nonce, tx.nonce);
    }

    #[test]
    fn test_composite_broadcaster() {
        let (sender1, _rx1) = mpsc::channel(64);
        let (sender2, _rx2) = mpsc::channel(64);

        let broadcaster = CompositeBroadcaster::dual(
            Arc::new(ChannelBroadcaster::for_p2p(sender1)),
            Arc::new(ChannelBroadcaster::for_websocket(sender2)),
        );

        let tx = create_test_tx();
        assert!(broadcaster.broadcast(&tx).is_ok());
    }

    #[test]
    fn test_builder() {
        let (sender, _rx) = mpsc::channel(64);

        let broadcaster = BroadcasterBuilder::new().with_p2p(sender).build();

        let tx = create_test_tx();
        assert!(broadcaster.broadcast(&tx).is_ok());
    }

    #[test]
    fn test_builder_empty() {
        let broadcaster = BroadcasterBuilder::new().build();

        let tx = create_test_tx();
        assert!(broadcaster.broadcast(&tx).is_ok()); // NoOp
    }
}

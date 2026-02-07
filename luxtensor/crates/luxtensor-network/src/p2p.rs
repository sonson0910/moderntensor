use crate::error::NetworkError;
use crate::messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_TRANSACTIONS};
use crate::peer::PeerManager;
use libp2p::gossipsub::{
    Behaviour as Gossipsub, Config as GossipsubConfig, ConfigBuilder, IdentTopic, Message, MessageAuthenticity,
    MessageId, ValidationMode,
};
use libp2p::identity::Keypair;
use libp2p::{Multiaddr, PeerId};
use luxtensor_core::block::Block;
use luxtensor_core::transaction::Transaction;
use luxtensor_core::types::Hash;
use parking_lot::RwLock;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as StdHash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// P2P network configuration
#[derive(Debug, Clone)]
pub struct P2PConfig {
    /// Listen address
    pub listen_addr: Multiaddr,

    /// Maximum number of peers
    pub max_peers: usize,

    /// Genesis hash for compatibility check
    pub genesis_hash: Hash,

    /// Enable mDNS discovery
    pub enable_mdns: bool,

    /// Heartbeat interval for gossipsub (seconds)
    pub heartbeat_interval: u64,

    /// Message TTL (seconds)
    pub message_ttl: u64,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/30303".parse().unwrap(),
            max_peers: 50,
            genesis_hash: [0u8; 32],
            enable_mdns: true,
            heartbeat_interval: 1,
            message_ttl: 60,
        }
    }
}

/// Gossipsub topics for the network
#[derive(Clone)]
pub struct GossipTopics {
    pub blocks: IdentTopic,
    pub transactions: IdentTopic,
}

impl Default for GossipTopics {
    fn default() -> Self {
        Self {
            blocks: IdentTopic::new(TOPIC_BLOCKS),
            transactions: IdentTopic::new(TOPIC_TRANSACTIONS),
        }
    }
}

/// Event from the P2P network
#[derive(Debug)]
pub enum P2PEvent {
    /// New transaction received
    NewTransaction(Transaction),

    /// New block received
    NewBlock(Block),

    /// Message received from peer
    Message {
        peer_id: PeerId,
        message: NetworkMessage,
    },

    /// Peer connected
    PeerConnected(PeerId),

    /// Peer disconnected
    PeerDisconnected(PeerId),

    /// Gossipsub event
    GossipMessage {
        source: Option<PeerId>,
        data: Vec<u8>,
        topic: String,
    },
}

/// Statistics for gossipsub
#[derive(Debug, Clone, Default)]
pub struct GossipStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub blocks_broadcast: u64,
    pub transactions_broadcast: u64,
}

/// P2P node for blockchain networking with gossipsub
pub struct P2PNode {
    peer_manager: PeerManager,
    config: P2PConfig,
    event_sender: mpsc::UnboundedSender<P2PEvent>,
    local_peer_id: PeerId,
    keypair: Keypair,
    topics: GossipTopics,
    gossipsub: Arc<RwLock<Option<Gossipsub>>>,
    stats: Arc<RwLock<GossipStats>>,
}

impl P2PNode {
    /// Create a new P2P node with gossipsub
    pub async fn new(
        config: P2PConfig,
        event_sender: mpsc::UnboundedSender<P2PEvent>,
    ) -> Result<Self, NetworkError> {
        // Generate keypair for identity
        let keypair = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(keypair.public());

        info!("Local peer ID: {}", local_peer_id);

        let peer_manager = PeerManager::new(config.max_peers);
        let topics = GossipTopics::default();

        // Build gossipsub config
        let gossipsub_config = Self::build_gossipsub_config(&config)?;

        // Create gossipsub behaviour
        let gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        ).map_err(|e| NetworkError::GossipsubInit(e.to_string()))?;

        Ok(Self {
            peer_manager,
            config,
            event_sender,
            local_peer_id,
            keypair,
            topics,
            gossipsub: Arc::new(RwLock::new(Some(gossipsub))),
            stats: Arc::new(RwLock::new(GossipStats::default())),
        })
    }

    /// Build gossipsub configuration
    fn build_gossipsub_config(config: &P2PConfig) -> Result<GossipsubConfig, NetworkError> {
        // Custom message ID function to deduplicate messages
        let message_id_fn = |message: &Message| {
            let mut hasher = DefaultHasher::new();
            message.data.hash(&mut hasher);
            if let Some(source) = &message.source {
                source.hash(&mut hasher);
            }
            MessageId::from(hasher.finish().to_be_bytes().to_vec())
        };

        ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(config.heartbeat_interval))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .max_transmit_size(2 * 1024 * 1024) // 2 MB max message size
            .build()
            .map_err(|e| NetworkError::GossipsubInit(e.to_string()))
    }

    /// Subscribe to network topics
    pub fn subscribe_topics(&mut self) -> Result<(), NetworkError> {
        let mut gossipsub_guard = self.gossipsub.write();
        if let Some(gossipsub) = gossipsub_guard.as_mut() {
            // Subscribe to blocks topic
            gossipsub.subscribe(&self.topics.blocks)
                .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;
            info!("Subscribed to topic: {}", TOPIC_BLOCKS);

            // Subscribe to transactions topic
            gossipsub.subscribe(&self.topics.transactions)
                .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;
            info!("Subscribed to topic: {}", TOPIC_TRANSACTIONS);
        }
        Ok(())
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(&mut self, tx: Transaction) -> Result<(), NetworkError> {
        let message = NetworkMessage::NewTransaction(tx);
        let data = bincode::serialize(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        self.publish_to_topic(&self.topics.transactions.clone(), data)?;

        // Update stats
        let mut stats = self.stats.write();
        stats.transactions_broadcast += 1;
        stats.messages_sent += 1;

        debug!("Broadcast transaction to network");
        Ok(())
    }

    /// Broadcast a block to the network
    pub fn broadcast_block(&mut self, block: Block) -> Result<(), NetworkError> {
        let block_hash = block.hash();
        let block_height = block.header.height;

        let message = NetworkMessage::NewBlock(block);
        let data = bincode::serialize(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        self.publish_to_topic(&self.topics.blocks.clone(), data)?;

        // Update stats
        let mut stats = self.stats.write();
        stats.blocks_broadcast += 1;
        stats.messages_sent += 1;

        info!(
            "Broadcast block #{} ({}) to network",
            block_height,
            hex::encode(&block_hash[..8])
        );
        Ok(())
    }

    /// Publish data to a topic
    fn publish_to_topic(&self, topic: &IdentTopic, data: Vec<u8>) -> Result<(), NetworkError> {
        // Validate message size
        if data.len() > 2 * 1024 * 1024 {
            return Err(NetworkError::MessageTooLarge(data.len()));
        }

        let mut gossipsub_guard = self.gossipsub.write();
        if let Some(gossipsub) = gossipsub_guard.as_mut() {
            match gossipsub.publish(topic.clone(), data) {
                Ok(_) => {
                    debug!("Published message to topic {}", topic);
                }
                Err(e) => {
                    // InsufficientPeers is expected when no peers are subscribed
                    // Log warning but don't fail - message will be available when peers join
                    warn!("Gossipsub publish warning: {} (this is normal if no peers connected)", e);
                }
            }
        } else {
            return Err(NetworkError::GossipsubNotInitialized);
        }
        Ok(())
    }

    /// Handle incoming gossipsub message
    pub fn handle_gossip_message(
        &mut self,
        source: Option<PeerId>,
        topic: String,
        data: Vec<u8>,
    ) -> Result<(), NetworkError> {
        // Update stats
        {
            let mut stats = self.stats.write();
            stats.messages_received += 1;
        }

        // Parse message with size limit to prevent DoS
        let message: NetworkMessage = crate::messages::deserialize_message(&data)
            .map_err(|e| NetworkError::DeserializationFailed(e.to_string()))?;

        debug!(
            "Received {} from {:?} on topic {}",
            message.message_type(),
            source,
            topic
        );

        // Validate and process
        match &message {
            NetworkMessage::NewTransaction(tx) => {
                // Validate transaction size
                if tx.data.len() > 128 * 1024 {
                    warn!("Received oversized transaction, ignoring");
                    return Err(NetworkError::MessageTooLarge(tx.data.len()));
                }

                // Send event
                let _ = self.event_sender.send(P2PEvent::NewTransaction(tx.clone()));
            }
            NetworkMessage::NewBlock(block) => {
                // Validate block
                if block.transactions.len() > 10000 {
                    warn!("Received block with too many transactions, ignoring");
                    return Err(NetworkError::InvalidMessage("Block too large".to_string()));
                }

                // Send event
                let _ = self.event_sender.send(P2PEvent::NewBlock(block.clone()));
            }
            _ => {
                // Send generic message event
                if let Some(peer_id) = source {
                    let _ = self.event_sender.send(P2PEvent::Message {
                        peer_id,
                        message: message.clone(),
                    });
                }
            }
        }

        // Update peer reputation for successful message
        if let Some(peer_id) = source {
            if let Some(peer) = self.peer_manager.get_peer_mut(&peer_id) {
                peer.record_success();
            }
        }

        Ok(())
    }

    /// Get peer manager reference
    pub fn peer_manager(&self) -> &PeerManager {
        &self.peer_manager
    }

    /// Get mutable peer manager reference
    pub fn peer_manager_mut(&mut self) -> &mut PeerManager {
        &mut self.peer_manager
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.peer_manager.peer_count()
    }

    /// Get gossip statistics
    pub fn stats(&self) -> GossipStats {
        self.stats.read().clone()
    }

    /// Get keypair reference
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }

    /// Get topics
    pub fn topics(&self) -> &GossipTopics {
        &self.topics
    }

    /// Check if gossipsub is initialized
    pub fn is_gossipsub_ready(&self) -> bool {
        self.gossipsub.read().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_p2p_node_creation() {
        let config = P2PConfig::default();
        let (tx, _rx) = mpsc::unbounded_channel();

        let result = P2PNode::new(config, tx).await;
        assert!(result.is_ok());

        let node = result.unwrap();
        assert!(node.is_gossipsub_ready());
    }

    #[test]
    fn test_p2p_config_default() {
        let config = P2PConfig::default();
        assert_eq!(config.max_peers, 50);
        assert!(config.enable_mdns);
        assert_eq!(config.heartbeat_interval, 1);
        assert_eq!(config.message_ttl, 60);
    }

    #[test]
    fn test_gossip_topics_default() {
        let topics = GossipTopics::default();
        assert_eq!(topics.blocks.hash(), IdentTopic::new(TOPIC_BLOCKS).hash());
        assert_eq!(topics.transactions.hash(), IdentTopic::new(TOPIC_TRANSACTIONS).hash());
    }

    #[tokio::test]
    async fn test_subscribe_topics() {
        let config = P2PConfig::default();
        let (tx, _rx) = mpsc::unbounded_channel();

        let mut node = P2PNode::new(config, tx).await.unwrap();
        let result = node.subscribe_topics();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_broadcast_transaction() {
        let config = P2PConfig::default();
        let (tx, _rx) = mpsc::unbounded_channel();

        let mut node = P2PNode::new(config, tx).await.unwrap();
        node.subscribe_topics().unwrap();

        // Create a test transaction
        // Transaction::new(nonce, from, to, value, gas_price, gas_limit, data)
        let test_tx = Transaction::new(
            0,                          // nonce
            [1u8; 20].into(),           // from
            Some([2u8; 20].into()),     // to
            1000,                       // value
            1,                          // gas_price
            21000,                      // gas_limit
            vec![],                     // data
        );

        // Should succeed (no peers to actually send to in test)
        let result = node.broadcast_transaction(test_tx);
        assert!(result.is_ok());

        // Check stats
        let stats = node.stats();
        assert_eq!(stats.transactions_broadcast, 1);
        assert_eq!(stats.messages_sent, 1);
    }

    #[tokio::test]
    async fn test_stats() {
        let config = P2PConfig::default();
        let (tx, _rx) = mpsc::unbounded_channel();

        let node = P2PNode::new(config, tx).await.unwrap();
        let stats = node.stats();

        assert_eq!(stats.messages_sent, 0);
        assert_eq!(stats.messages_received, 0);
        assert_eq!(stats.blocks_broadcast, 0);
        assert_eq!(stats.transactions_broadcast, 0);
    }
}

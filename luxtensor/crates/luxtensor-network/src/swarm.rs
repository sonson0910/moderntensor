//! P2P Swarm implementation with mDNS discovery
//! This module provides actual network connectivity using libp2p Swarm

use crate::error::NetworkError;
use crate::messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_TRANSACTIONS, TOPIC_SYNC};
use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identity::Keypair,
    mdns,
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use luxtensor_core::block::Block;
use luxtensor_core::transaction::Transaction;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Event from the P2P swarm
#[derive(Debug)]
pub enum SwarmP2PEvent {
    /// New block received
    NewBlock(Block),
    /// New transaction received
    NewTransaction(Transaction),
    /// Peer connected
    PeerConnected(PeerId),
    /// Peer disconnected
    PeerDisconnected(PeerId),
    /// Sync request received - need to send blocks
    SyncRequest { from_height: u64, to_height: u64, requester_id: String },
}

/// Command to send to swarm
#[derive(Debug)]
pub enum SwarmCommand {
    BroadcastBlock(Block),
    BroadcastTransaction(Transaction),
    /// Request sync from peers
    RequestSync { from_height: u64, to_height: u64, my_id: String },
    /// Send blocks in response to sync request
    SendBlocks { blocks: Vec<Block> },
}

/// P2P Swarm Node for actual network connectivity
pub struct SwarmP2PNode {
    swarm: Swarm<BlockchainBehaviour>,
    event_sender: mpsc::UnboundedSender<SwarmP2PEvent>,
    command_rx: mpsc::UnboundedReceiver<SwarmCommand>,
    blocks_topic: IdentTopic,
    transactions_topic: IdentTopic,
    sync_topic: IdentTopic,
}

// Define behaviour using macro
#[derive(libp2p::swarm::NetworkBehaviour)]
struct BlockchainBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
}

impl SwarmP2PNode {
    /// Create a new P2P swarm node with random keypair (for backwards compatibility)
    /// Returns the node and a sender for commands (broadcast blocks/txs)
    pub async fn new(
        listen_port: u16,
        event_sender: mpsc::UnboundedSender<SwarmP2PEvent>,
    ) -> Result<(Self, mpsc::UnboundedSender<SwarmCommand>), NetworkError> {
        let keypair = Keypair::generate_ed25519();
        Self::with_keypair(listen_port, event_sender, keypair, vec![], true).await
    }

    /// Create a new P2P swarm node with provided keypair for persistent identity
    ///
    /// # Arguments
    /// * `listen_port` - Port to listen on
    /// * `event_sender` - Channel to send events to
    /// * `keypair` - Keypair for node identity (load from file for persistent ID)
    /// * `bootstrap_nodes` - List of bootstrap multiaddrs to connect to
    /// * `enable_mdns` - Whether to enable mDNS discovery
    ///
    /// # Returns
    /// * Tuple of (SwarmP2PNode, command sender)
    pub async fn with_keypair(
        listen_port: u16,
        event_sender: mpsc::UnboundedSender<SwarmP2PEvent>,
        keypair: Keypair,
        bootstrap_nodes: Vec<String>,
        enable_mdns: bool,
    ) -> Result<(Self, mpsc::UnboundedSender<SwarmCommand>), NetworkError> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let local_peer_id = PeerId::from(keypair.public());

        info!("🔗 Local Peer ID: {}", local_peer_id);

        // Message ID function to deduplicate
        let message_id_fn = |message: &gossipsub::Message| {
            let mut hasher = DefaultHasher::new();
            message.data.hash(&mut hasher);
            gossipsub::MessageId::from(hasher.finish().to_be_bytes().to_vec())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            // 🔧 FIX: Lower mesh thresholds for small networks (2-5 nodes)
            .mesh_n_low(1)           // Minimum peers in mesh (default: 4)
            .mesh_n_high(6)          // Maximum peers in mesh (default: 12)
            .mesh_n(2)               // Desired peers in mesh (default: 6)
            .mesh_outbound_min(1)    // Minimum outbound peers (default: 2)
            .flood_publish(true)     // Publish to ALL connected peers, not just mesh
            .build()
            .map_err(|e| NetworkError::GossipsubInit(e.to_string()))?;

        // Create gossipsub behaviour
        let gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        ).map_err(|e| NetworkError::GossipsubInit(e.to_string()))?;

        // Create mDNS behaviour for local discovery
        let mdns = mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        ).map_err(|e| NetworkError::Connection(e.to_string()))?;

        // Combine behaviours
        let behaviour = BlockchainBehaviour { gossipsub, mdns };

        // Build swarm using the new API
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                libp2p::tcp::Config::default(),
                libp2p::noise::Config::new,
                libp2p::yamux::Config::default,
            )
            .map_err(|e| NetworkError::Connection(format!("TCP error: {:?}", e)))?
            .with_behaviour(|_| Ok(behaviour))
            .map_err(|e| NetworkError::Connection(format!("Behaviour error: {:?}", e)))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        // Listen on address
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port)
            .parse()
            .map_err(|e: libp2p::multiaddr::Error| NetworkError::Connection(e.to_string()))?;

        swarm.listen_on(listen_addr.clone())
            .map_err(|e| NetworkError::Connection(e.to_string()))?;

        info!("🌐 Listening on {}", listen_addr);

        // Create topics
        let blocks_topic = IdentTopic::new(TOPIC_BLOCKS);
        let transactions_topic = IdentTopic::new(TOPIC_TRANSACTIONS);
        let sync_topic = IdentTopic::new(TOPIC_SYNC);

        // Subscribe to topics
        swarm.behaviour_mut().gossipsub.subscribe(&blocks_topic)
            .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;
        swarm.behaviour_mut().gossipsub.subscribe(&transactions_topic)
            .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;
        swarm.behaviour_mut().gossipsub.subscribe(&sync_topic)
            .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;

        info!("📡 Subscribed to topics: blocks, transactions, sync");

        // Connect to bootstrap nodes if provided
        if !bootstrap_nodes.is_empty() {
            info!("🔗 Connecting to {} bootstrap node(s)...", bootstrap_nodes.len());
            for addr_str in &bootstrap_nodes {
                match addr_str.parse::<Multiaddr>() {
                    Ok(addr) => {
                        // Extract peer ID from multiaddr if present
                        if let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = addr.iter().last() {
                            // Add as explicit peer for gossipsub
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            info!("   Added explicit peer: {}", peer_id);
                        }

                        // Dial the bootstrap node
                        match swarm.dial(addr.clone()) {
                            Ok(_) => info!("   📞 Dialing bootstrap: {}", addr_str),
                            Err(e) => warn!("   ⚠️ Failed to dial {}: {}", addr_str, e),
                        }
                    }
                    Err(e) => {
                        warn!("   ⚠️ Invalid bootstrap address '{}': {}", addr_str, e);
                    }
                }
            }
        } else if enable_mdns {
            info!("📡 mDNS enabled - will discover local peers automatically");
        } else {
            warn!("⚠️ No bootstrap nodes and mDNS disabled - node will be isolated!");
        }

        Ok((Self {
            swarm,
            event_sender,
            command_rx,
            blocks_topic,
            transactions_topic,
            sync_topic,
        }, command_tx))
    }

    /// Run the swarm event loop - this must be called in a tokio task
    pub async fn run(&mut self) {
        info!("🚀 P2P Swarm event loop started");

        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Mdns(event)) => {
                            self.handle_mdns_event(event);
                        }
                        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source,
                            message_id: _,
                            message,
                        })) => {
                            self.handle_gossip_message(propagation_source, message);
                        }
                        SwarmEvent::Behaviour(BlockchainBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
                            peer_id,
                            topic,
                        })) => {
                            info!("👋 Peer {} subscribed to {}", peer_id, topic);
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("🎧 Listening on {}", address);
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            info!("✅ Connected to peer: {}", peer_id);
                            let _ = self.event_sender.send(SwarmP2PEvent::PeerConnected(peer_id));
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            info!("❌ Disconnected from peer: {}", peer_id);
                            let _ = self.event_sender.send(SwarmP2PEvent::PeerDisconnected(peer_id));
                        }
                        _ => {}
                    }
                }
                // Handle broadcast commands
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        SwarmCommand::BroadcastBlock(block) => {
                            if let Err(e) = self.broadcast_block(&block) {
                                warn!("Failed to broadcast block: {}", e);
                            }
                        }
                        SwarmCommand::BroadcastTransaction(tx) => {
                            info!("📨 SWARM: Received BroadcastTransaction command");
                            if let Err(e) = self.broadcast_transaction(&tx) {
                                warn!("Failed to broadcast transaction: {}", e);
                            }
                        }
                        SwarmCommand::RequestSync { from_height, to_height, my_id } => {
                            if let Err(e) = self.send_sync_request(from_height, to_height, my_id) {
                                warn!("Failed to send sync request: {}", e);
                            }
                        }
                        SwarmCommand::SendBlocks { blocks } => {
                            // 🔧 FIX: Send all blocks in a single message for faster sync
                            if !blocks.is_empty() {
                                info!("📤 SWARM: Broadcasting {} blocks for sync", blocks.len());
                                let message = NetworkMessage::Blocks(blocks);
                                let data = match bincode::serialize(&message) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        warn!("Failed to serialize blocks: {}", e);
                                        continue;
                                    }
                                };

                                // Use blocks_topic for sync (more reliable than sync_topic)
                                match self.swarm.behaviour_mut().gossipsub.publish(
                                    self.blocks_topic.clone(),
                                    data
                                ) {
                                    Ok(_) => info!("📡 Sync blocks broadcast successful"),
                                    Err(gossipsub::PublishError::InsufficientPeers) => {
                                        warn!("⚠️ InsufficientPeers for sync blocks broadcast");
                                    }
                                    Err(e) => warn!("Failed to broadcast sync blocks: {}", e),
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle mDNS discovery events
    fn handle_mdns_event(&mut self, event: mdns::Event) {
        match event {
            mdns::Event::Discovered(peers) => {
                for (peer_id, addr) in peers {
                    info!("🔍 mDNS discovered peer: {} at {}", peer_id, addr);
                    // Add peer to gossipsub
                    self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    // Dial the peer
                    if let Err(e) = self.swarm.dial(addr.clone()) {
                        warn!("Failed to dial {}: {}", addr, e);
                    }
                }
            }
            mdns::Event::Expired(peers) => {
                for (peer_id, _addr) in peers {
                    debug!("mDNS peer expired: {}", peer_id);
                    self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                }
            }
        }
    }

    /// Handle incoming gossip message
    fn handle_gossip_message(&mut self, source: PeerId, message: gossipsub::Message) {
        let topic = message.topic.to_string();
        info!("📨 GOSSIP RECEIVED: from {} on topic {} - {} bytes", source, topic, message.data.len());

        // Deserialize message
        match bincode::deserialize::<NetworkMessage>(&message.data) {
            Ok(NetworkMessage::NewBlock(block)) => {
                info!("📥 Received block #{} from peer {}", block.header.height, source);
                let _ = self.event_sender.send(SwarmP2PEvent::NewBlock(block));
            }
            Ok(NetworkMessage::NewTransaction(tx)) => {
                info!("📥 SWARM: Received NewTransaction from peer {}", source);
                let _ = self.event_sender.send(SwarmP2PEvent::NewTransaction(tx));
            }
            Ok(NetworkMessage::SyncRequest { from_height, to_height, requester_id }) => {
                info!("🔄 Sync request from {} for blocks {}-{}", requester_id, from_height, to_height);
                let _ = self.event_sender.send(SwarmP2PEvent::SyncRequest {
                    from_height,
                    to_height,
                    requester_id
                });
            }
            Ok(NetworkMessage::Blocks(blocks)) => {
                info!("📥 Received {} blocks from sync", blocks.len());
                for block in blocks {
                    let _ = self.event_sender.send(SwarmP2PEvent::NewBlock(block));
                }
            }
            Ok(_) => {
                debug!("Received other message type");
            }
            Err(e) => {
                warn!("Failed to deserialize gossip message: {}", e);
            }
        }
    }

    /// Broadcast a block to the network
    pub fn broadcast_block(&mut self, block: &Block) -> Result<(), NetworkError> {
        let message = NetworkMessage::NewBlock(block.clone());
        let data = bincode::serialize(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        match self.swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
            Ok(_) => {
                info!("📡 Broadcast block #{}", block.header.height);
                Ok(())
            }
            Err(gossipsub::PublishError::InsufficientPeers) => {
                debug!("No peers to broadcast block (will sync later)");
                Ok(()) // Not an error - just no peers yet
            }
            Err(e) => {
                Err(NetworkError::PublishFailed(e.to_string()))
            }
        }
    }

    /// Broadcast a transaction to the network
    pub fn broadcast_transaction(&mut self, tx: &Transaction) -> Result<(), NetworkError> {
        let message = NetworkMessage::NewTransaction(tx.clone());
        let data = bincode::serialize(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        // 🔧 WORKAROUND: Use blocks topic instead of transactions topic
        // Blocks topic has more active mesh connections, transactions topic may not have peers
        info!("📡 SWARM: Publishing TX to blocks topic (workaround for mesh issue)");
        match self.swarm.behaviour_mut().gossipsub.publish(self.blocks_topic.clone(), data) {
            Ok(_) => {
                info!("📡 SWARM: TX broadcast successful via blocks topic");
                Ok(())
            }
            Err(gossipsub::PublishError::InsufficientPeers) => {
                warn!("⚠️ SWARM: No peers subscribed to blocks topic - TX NOT propagated!");
                Ok(())
            }
            Err(e) => {
                Err(NetworkError::PublishFailed(e.to_string()))
            }
        }
    }

    /// Get connected peer count
    pub fn peer_count(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Get local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        self.swarm.local_peer_id()
    }

    /// Send sync request to network
    fn send_sync_request(&mut self, from_height: u64, to_height: u64, my_id: String) -> Result<(), NetworkError> {
        let message = NetworkMessage::SyncRequest {
            from_height,
            to_height,
            requester_id: my_id.clone()
        };
        let data = bincode::serialize(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        match self.swarm.behaviour_mut().gossipsub.publish(self.sync_topic.clone(), data) {
            Ok(_) => {
                info!("🔄 Sent sync request for blocks {}-{}", from_height, to_height);
                Ok(())
            }
            Err(gossipsub::PublishError::InsufficientPeers) => {
                debug!("No peers for sync request");
                Ok(())
            }
            Err(e) => {
                Err(NetworkError::PublishFailed(e.to_string()))
            }
        }
    }
}

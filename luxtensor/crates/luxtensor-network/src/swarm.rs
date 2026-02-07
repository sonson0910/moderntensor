//! P2P Swarm implementation with mDNS discovery
//! This module provides actual network connectivity using libp2p Swarm

use crate::error::NetworkError;
use crate::messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_TRANSACTIONS, TOPIC_SYNC};
use crate::peer_discovery::{PeerDiscovery, DiscoveryConfig};
use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identity::Keypair,
    kad::{self, store::MemoryStore, Mode as KadMode},
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
use tracing::{debug, info, warn};

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
    /// Disconnect a peer (e.g. blocked by eclipse protection)
    DisconnectPeer { peer_id: String },
    /// Broadcast AI task to miners for dispatch
    BroadcastTaskDispatch {
        task_id: [u8; 32],
        model_hash: String,
        input_hash: [u8; 32],
        reward: u128,
        deadline: u64,
    },
}

/// P2P Swarm Node for actual network connectivity
/// Default capacity for bounded P2P channels
const P2P_CHANNEL_CAPACITY: usize = 4096;

/// P2P Swarm Node for actual network connectivity
pub struct SwarmP2PNode {
    swarm: Swarm<BlockchainBehaviour>,
    event_sender: mpsc::Sender<SwarmP2PEvent>,
    command_rx: mpsc::Receiver<SwarmCommand>,
    blocks_topic: IdentTopic,
    #[allow(dead_code)] // Reserved for future direct TX topic usage
    transactions_topic: IdentTopic,
    sync_topic: IdentTopic,
    /// Track sync requests per peer for flood protection
    sync_requests: std::collections::HashMap<PeerId, (u32, std::time::Instant)>,
    /// Peer discovery with latency-based selection
    peer_discovery: std::sync::Arc<PeerDiscovery>,
}

/// Maximum sync requests per peer per minute
const MAX_SYNC_REQUESTS_PER_PEER: u32 = 10;
/// Sync rate limit window
const SYNC_RATE_LIMIT_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

// Define behaviour using macro
#[derive(libp2p::swarm::NetworkBehaviour)]
struct BlockchainBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    /// Kademlia DHT for distributed peer discovery
    kademlia: kad::Behaviour<MemoryStore>,
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
        event_sender: mpsc::Sender<SwarmP2PEvent>,
        keypair: Keypair,
        bootstrap_nodes: Vec<String>,
        enable_mdns: bool,
    ) -> Result<(Self, mpsc::Sender<SwarmCommand>), NetworkError> {
        let (command_tx, command_rx) = mpsc::channel(P2P_CHANNEL_CAPACITY);
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
            // 🔧 FIX: Disable flood_publish — use mesh routing for O(log N) instead of O(N)
            .flood_publish(false)
            // 🔧 FIX: cap max message size to 4MB to prevent memory exhaustion
            .max_transmit_size(4 * 1024 * 1024)
            .build()
            .map_err(|e| NetworkError::GossipsubInit(e.to_string()))?;

        // Create gossipsub behaviour with peer scoring
        let mut gossipsub = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone()),
            gossipsub_config,
        ).map_err(|e| NetworkError::GossipsubInit(e.to_string()))?;

        // 🔧 FIX: Enable gossipsub peer scoring to penalise bad actors
        let peer_score_params = gossipsub::PeerScoreParams {
            // Decay scores over 10 minutes
            decay_interval: Duration::from_secs(10),
            decay_to_zero: 0.01,
            retain_score: Duration::from_secs(3600), // remember scores for 1 hour
            // Application-specific: start with 0, each app-level misbehaviour decreases
            app_specific_weight: 1.0,
            // IP colocation: penalise peers sharing IPs
            ip_colocation_factor_weight: -10.0,
            ip_colocation_factor_threshold: 3.0,
            ..Default::default()
        };
        let peer_score_thresholds = gossipsub::PeerScoreThresholds {
            gossip_threshold: -100.0,       // don't gossip to peers below this
            publish_threshold: -200.0,      // don't publish to peers below this
            graylist_threshold: -400.0,     // ignore all messages from peers below this
            opportunistic_graft_threshold: 5.0,
            ..Default::default()
        };
        if let Err(e) = gossipsub.with_peer_score(peer_score_params, peer_score_thresholds) {
            warn!("⚠️ Failed to enable gossipsub peer scoring: {}", e);
        } else {
            info!("  ✓ Gossipsub peer scoring enabled");
        }

        // Create mDNS behaviour for local discovery
        let mdns = mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        ).map_err(|e| NetworkError::Connection(e.to_string()))?;

        // Create Kademlia DHT for distributed peer discovery
        let store = MemoryStore::new(local_peer_id);
        let mut kademlia_config = kad::Config::default();
        kademlia_config.set_protocol_names(vec![
            libp2p::StreamProtocol::new("/luxtensor/kad/1.0.0")
        ]);
        let mut kademlia = kad::Behaviour::with_config(local_peer_id, store, kademlia_config);

        // Set Kademlia mode to Server (full DHT participant)
        kademlia.set_mode(Some(KadMode::Server));

        // Add bootstrap nodes to Kademlia
        for addr_str in &bootstrap_nodes {
            if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                // Extract peer ID from multiaddr if present
                if let Some(peer_id) = extract_peer_id_from_multiaddr(&addr) {
                    kademlia.add_address(&peer_id, addr.clone());
                    info!("📍 Added peer to Kademlia DHT: {}", peer_id);
                }
            }
        }

        // Combine behaviours
        let behaviour = BlockchainBehaviour { gossipsub, mdns, kademlia };

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

            // 🔧 FIX: Trigger Kademlia bootstrap to discover peers through DHT
            if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                warn!("⚠️ Kademlia bootstrap failed (need at least one peer): {}", e);
            } else {
                info!("🔍 Kademlia DHT bootstrap initiated");
            }
        } else if enable_mdns {
            info!("📡 mDNS enabled - will discover local peers automatically");
        } else {
            warn!("⚠️ No bootstrap nodes and mDNS disabled - node will be isolated!");
        }

        // Initialize peer discovery for auto-selecting nearest peers
        let peer_discovery = std::sync::Arc::new(PeerDiscovery::new(DiscoveryConfig::default()));

        Ok((Self {
            swarm,
            event_sender,
            command_rx,
            blocks_topic,
            transactions_topic,
            sync_topic,
            sync_requests: std::collections::HashMap::new(),
            peer_discovery,
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
                            if self.event_sender.try_send(SwarmP2PEvent::PeerConnected(peer_id)).is_err() {
                                warn!("⚠️ Event channel full, dropping PeerConnected event");
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            info!("❌ Disconnected from peer: {}", peer_id);
                            if self.event_sender.try_send(SwarmP2PEvent::PeerDisconnected(peer_id)).is_err() {
                                warn!("⚠️ Event channel full, dropping PeerDisconnected event");
                            }
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
                            // SECURITY: Cap batch size to prevent oversized gossip messages
                            const MAX_SYNC_BATCH: usize = 100;
                            let blocks = if blocks.len() > MAX_SYNC_BATCH {
                                warn!("⚠️ Truncating sync batch from {} to {} blocks", blocks.len(), MAX_SYNC_BATCH);
                                blocks[..MAX_SYNC_BATCH].to_vec()
                            } else {
                                blocks
                            };
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
                                        warn!("⚠️ No peers subscribed to topic for sync blocks broadcast");
                                    }
                                    Err(e) => warn!("Failed to broadcast sync blocks: {}", e),
                                }
                            }
                        }
                        SwarmCommand::DisconnectPeer { peer_id } => {
                            // Disconnect a specific peer (e.g., blocked by eclipse protection)
                            match peer_id.parse::<PeerId>() {
                                Ok(pid) => {
                                    // Remove from gossipsub explicit peers
                                    self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&pid);
                                    // Disconnect all connections to this peer
                                    let _ = self.swarm.disconnect_peer_id(pid);
                                    info!("🚫 Disconnected peer by command: {}", peer_id);
                                }
                                Err(e) => {
                                    warn!("Invalid peer ID format for disconnect: {}: {}", peer_id, e);
                                }
                            }
                        }
                        SwarmCommand::BroadcastTaskDispatch { task_id, model_hash, input_hash, reward, deadline } => {
                            // Broadcast AI task to miners via the sync topic
                            info!("📡 Broadcasting AI task 0x{} to miners", hex::encode(&task_id[..8]));
                            let message = NetworkMessage::AITaskDispatch {
                                task_id,
                                model_hash,
                                input_hash,
                                reward,
                                deadline,
                            };
                            let data = match bincode::serialize(&message) {
                                Ok(d) => d,
                                Err(e) => {
                                    warn!("Failed to serialize AI task dispatch: {}", e);
                                    continue;
                                }
                            };

                            match self.swarm.behaviour_mut().gossipsub.publish(
                                self.sync_topic.clone(),
                                data
                            ) {
                                Ok(_) => info!("📡 AI task dispatch broadcast successful"),
                                Err(gossipsub::PublishError::InsufficientPeers) => {
                                    warn!("⚠️ No miners subscribed to task dispatch topic");
                                }
                                Err(e) => warn!("Failed to broadcast AI task: {}", e),
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

                    // Register with peer discovery for latency tracking
                    let peer_id_str = peer_id.to_string();
                    self.peer_discovery.on_peer_discovered(
                        peer_id_str.clone(),
                        vec![addr.to_string()],
                    );

                    // Add peer to gossipsub
                    self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);

                    // Dial the peer and measure connection time as initial latency
                    let start = std::time::Instant::now();
                    if let Err(e) = self.swarm.dial(addr.clone()) {
                        warn!("Failed to dial {}: {}", addr, e);
                    } else {
                        // Approximate latency from dial time
                        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
                        self.peer_discovery.update_latency(&peer_id_str, latency_ms);
                        self.peer_discovery.on_peer_connected(&peer_id_str);
                    }
                }
            }
            mdns::Event::Expired(peers) => {
                for (peer_id, _addr) in peers {
                    debug!("mDNS peer expired: {}", peer_id);
                    self.peer_discovery.on_peer_disconnected(&peer_id.to_string());
                    self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                }
            }
        }
    }

    /// Handle incoming gossip message
    fn handle_gossip_message(&mut self, source: PeerId, message: gossipsub::Message) {
        let topic = message.topic.to_string();
        let msg_len = message.data.len();
        info!("📨 GOSSIP RECEIVED: from {} on topic {} - {} bytes", source, topic, msg_len);

        // SECURITY: Reject oversized messages before deserialization
        if msg_len > crate::messages::MAX_MESSAGE_SIZE as usize {
            warn!("🚫 Dropping oversized gossip message from {}: {} bytes (max {})",
                  source, msg_len, crate::messages::MAX_MESSAGE_SIZE);
            return;
        }

        // Deserialize message with size limit to prevent DoS
        match crate::messages::deserialize_message(&message.data) {
            Ok(NetworkMessage::NewBlock(block)) => {
                info!("📥 Received block #{} from peer {}", block.header.height, source);
                if self.event_sender.try_send(SwarmP2PEvent::NewBlock(block)).is_err() {
                    warn!("⚠️ Event channel full, dropping NewBlock — node is falling behind");
                }
            }
            Ok(NetworkMessage::NewTransaction(tx)) => {
                info!("📥 SWARM: Received NewTransaction from peer {}", source);
                if self.event_sender.try_send(SwarmP2PEvent::NewTransaction(tx)).is_err() {
                    warn!("⚠️ Event channel full, dropping NewTransaction");
                }
            }
            Ok(NetworkMessage::SyncRequest { from_height, to_height, requester_id }) => {
                // SECURITY: Rate limit sync requests per peer
                let now = std::time::Instant::now();
                let entry = self.sync_requests.entry(source).or_insert((0, now));

                // Reset counter if window expired
                if now.duration_since(entry.1) >= SYNC_RATE_LIMIT_WINDOW {
                    entry.0 = 0;
                    entry.1 = now;
                }

                // Check rate limit
                if entry.0 >= MAX_SYNC_REQUESTS_PER_PEER {
                    warn!("🚫 Rate limiting sync requests from peer {} ({} requests/min)",
                          source, entry.0);
                    return;
                }

                entry.0 += 1;
                info!("🔄 Sync request from {} for blocks {}-{}", requester_id, from_height, to_height);
                if self.event_sender.try_send(SwarmP2PEvent::SyncRequest {
                    from_height,
                    to_height,
                    requester_id
                }).is_err() {
                    warn!("⚠️ Event channel full, dropping SyncRequest");
                }
            }
            Ok(NetworkMessage::Blocks(blocks)) => {
                info!("📥 Received {} blocks from sync", blocks.len());
                for block in blocks {
                    if self.event_sender.try_send(SwarmP2PEvent::NewBlock(block)).is_err() {
                        warn!("⚠️ Event channel full, dropping sync block");
                        break;
                    }
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
                debug!("No peers subscribed to blocks topic (will sync later)");
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

    /// Get the best (lowest latency) discovered peer
    pub fn get_best_peer(&self) -> Option<crate::peer_discovery::DiscoveredPeer> {
        self.peer_discovery.get_best_peer()
    }

    /// Get peer discovery statistics
    pub fn get_peer_discovery_stats(&self) -> crate::peer_discovery::DiscoveryStats {
        self.peer_discovery.get_stats()
    }

    /// Get reference to peer discovery for advanced usage
    pub fn peer_discovery(&self) -> &std::sync::Arc<PeerDiscovery> {
        &self.peer_discovery
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
                debug!("No peers subscribed to sync topic");
                Ok(())
            }
            Err(e) => {
                Err(NetworkError::PublishFailed(e.to_string()))
            }
        }
    }
}

/// Extract PeerId from a multiaddr that ends with /p2p/<peer_id>
fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|p| {
        if let libp2p::multiaddr::Protocol::P2p(peer_id) = p {
            Some(peer_id)
        } else {
            None
        }
    })
}

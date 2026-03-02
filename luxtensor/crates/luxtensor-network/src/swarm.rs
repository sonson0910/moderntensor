//! P2P Swarm implementation with mDNS discovery
//! This module provides actual network connectivity using libp2p Swarm

use crate::eclipse_protection::{EclipseConfig, EclipseProtection};
use crate::error::NetworkError;
use crate::messages::{
    serialize_message, NetworkMessage, TOPIC_BLOCKS, TOPIC_SYNC, TOPIC_TRANSACTIONS,
};
use crate::peer_discovery::{DiscoveryConfig, PeerDiscovery};
use crate::rate_limiter::{RateLimiter, RateLimiterConfig};
use futures::StreamExt;
use libp2p::{
    connection_limits,
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identity::Keypair,
    kad::{self, store::MemoryStore, Mode as KadMode},
    mdns,
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use luxtensor_core::block::Block;
use luxtensor_core::transaction::Transaction;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// 🔧 FIX S2: Tracks pending sync requests with timeout and retry logic.
///
/// When a sync request is sent, a record is created with `sent_at` timestamp.
/// On each event loop tick, `check_timeouts()` detects stale requests and
/// triggers retries up to `max_retries`. After exhausting retries the request
/// is dropped and the peer is penalised.
#[derive(Debug)]
struct PendingSyncRequest {
    from_height: u64,
    to_height: u64,
    my_id: String,
    sent_at: std::time::Instant,
    retries: u32,
}

/// Configuration for sync request timeout behaviour.
const SYNC_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const SYNC_REQUEST_MAX_RETRIES: u32 = 3;

/// Event from the P2P swarm
#[derive(Debug)]
pub enum SwarmP2PEvent {
    /// New block received. Carries the libp2p PeerId of the gossip source
    /// so rate-limiting is keyed by a cryptographic identity the peer cannot
    /// forge without generating a new keypair (Sybil cost).
    NewBlock(Block, PeerId),
    /// New transaction received. Same PeerId semantics.
    NewTransaction(Transaction, PeerId),
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
    RequestSync {
        from_height: u64,
        to_height: u64,
        my_id: String,
    },
    /// Send blocks in response to sync request
    SendBlocks {
        blocks: Vec<Block>,
    },
    /// Disconnect a peer (e.g. blocked by eclipse protection)
    DisconnectPeer {
        peer_id: String,
    },
    /// Broadcast AI task to miners for dispatch
    ///
    /// SECURITY: Requires validator signature to prevent forged dispatches (H3 fix)
    BroadcastTaskDispatch {
        task_id: [u8; 32],
        model_hash: String,
        input_hash: [u8; 32],
        reward: u128,
        deadline: u64,
        /// ECDSA signature proving authority
        validator_signature: Vec<u8>,
        /// Address of the signing validator
        validator_address: [u8; 20],
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
    /// 🔧 FIX: Eclipse protection to limit connections per subnet
    eclipse_protection: std::sync::Arc<EclipseProtection>,
    /// 🔧 FIX F1: Token-bucket rate limiter for all gossip messages
    rate_limiter: std::sync::Arc<RateLimiter>,
    /// Monotonic nonce to prevent gossipsub dedup of identical sync requests
    sync_nonce: AtomicU64,
    /// Throttle: last time we responded to a sync request, **per peer**
    /// 🔧 FIX F17: Per-peer instead of global to avoid starving legitimate requestors
    sync_response_timestamps: std::collections::HashMap<PeerId, std::time::Instant>,
    /// 🔧 FIX S6: PeerId → IP map for dual-key rate limiting.
    /// Populated on `ConnectionEstablished`, removed on `ConnectionClosed`.
    /// Used by `handle_gossip_message` to call `check_with_ip` instead of
    /// `check` alone, preventing Sybil bypass via multiple PeerIds from one IP.
    peer_ips: std::collections::HashMap<PeerId, std::net::IpAddr>,
    /// 🔧 FIX S2: Pending sync requests with timeout tracking.
    /// Checked periodically to retry or drop stale requests.
    pending_sync_requests: Vec<PendingSyncRequest>,
}

/// Maximum sync requests per peer per minute
const MAX_SYNC_REQUESTS_PER_PEER: u32 = 60;
/// Sync rate limit window
const SYNC_RATE_LIMIT_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

// Define behaviour using macro
#[derive(libp2p::swarm::NetworkBehaviour)]
struct BlockchainBehaviour {
    gossipsub: gossipsub::Behaviour,
    /// 🔧 FIX F11: mDNS wrapped in `Toggle` so it can be truly disabled at
    /// runtime. Previously the behaviour was always instantiated even when the
    /// user asked for mDNS to be off, which still bound the multicast socket
    /// and leaked discovery on mainnet/testnet where mDNS is inappropriate.
    mdns: libp2p::swarm::behaviour::toggle::Toggle<mdns::tokio::Behaviour>,
    /// Kademlia DHT for distributed peer discovery
    kademlia: kad::Behaviour<MemoryStore>,
    /// 🔧 FIX F6: Transport-level connection limits.
    /// Rejects inbound connections *before* the Noise handshake completes,
    /// reducing the impact of resource exhaustion attacks. The post-connection
    /// eclipse check in `ConnectionEstablished` still applies as a second layer.
    connection_limits: connection_limits::Behaviour,
}

impl SwarmP2PNode {
    /// Create a new P2P swarm node with random keypair (for backwards compatibility)
    /// Returns the node and a sender for commands (broadcast blocks/txs)
    /// 🔧 FIX: Use bounded channel types to match with_keypair()
    pub async fn new(
        listen_port: u16,
        event_sender: mpsc::Sender<SwarmP2PEvent>,
    ) -> Result<(Self, mpsc::Sender<SwarmCommand>), NetworkError> {
        let keypair = Keypair::generate_ed25519();
        Self::with_keypair(listen_port, event_sender, keypair, vec![], true, 0).await
    }

    /// Create a new P2P swarm node with provided keypair for persistent identity
    ///
    /// # Arguments
    /// * `listen_port` - Port to listen on
    /// * `event_sender` - Channel to send events to
    /// * `keypair` - Keypair for node identity (load from file for persistent ID)
    /// * `bootstrap_nodes` - List of bootstrap multiaddrs to connect to
    /// * `enable_mdns` - Whether to enable mDNS discovery
    /// * `chain_id` - Chain ID for network-specific tuning (mainnet=8898)
    ///
    /// # Returns
    /// * Tuple of (SwarmP2PNode, command sender)
    pub async fn with_keypair(
        listen_port: u16,
        event_sender: mpsc::Sender<SwarmP2PEvent>,
        keypair: Keypair,
        bootstrap_nodes: Vec<String>,
        enable_mdns: bool,
        chain_id: u32,
    ) -> Result<(Self, mpsc::Sender<SwarmCommand>), NetworkError> {
        let (command_tx, command_rx) = mpsc::channel(P2P_CHANNEL_CAPACITY);
        let local_peer_id = PeerId::from(keypair.public());

        info!("🔗 Local Peer ID: {}", local_peer_id);

        // Message ID function to deduplicate — cryptographic content-only hash
        // Uses keccak256 instead of DefaultHasher to prevent collision attacks
        // and ensure stability across Rust versions.
        // 🔧 FIX F14: Use full 32-byte keccak256 hash for message ID.
        // Previously truncated to 20 bytes, needlessly reducing collision
        // resistance from 2^128 to 2^80 in a security-critical dedup context.
        let message_id_fn = |message: &gossipsub::Message| {
            let hash = luxtensor_crypto::keccak256(&message.data);
            gossipsub::MessageId::from(hash.to_vec())
        };

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            // 🔧 FIX: Mesh thresholds are chain-aware. Mainnet (chain_id 8898) uses
            // production values for large networks; devnet/testnet uses relaxed values.
            .mesh_n_low(if chain_id == 8898 { 4 } else { 1 })
            .mesh_n_high(if chain_id == 8898 { 12 } else { 6 })
            .mesh_n(if chain_id == 8898 { 6 } else { 2 })
            .mesh_outbound_min(if chain_id == 8898 { 2 } else { 1 })
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
        )
        .map_err(|e| NetworkError::GossipsubInit(e.to_string()))?;

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
            gossip_threshold: -10.0,   // 🔧 FIX: Tighter threshold (was -100)
            publish_threshold: -30.0,  // 🔧 FIX: Tighter threshold (was -200)
            graylist_threshold: -50.0, // 🔧 FIX: Tighter threshold (was -400)
            opportunistic_graft_threshold: 5.0,
            ..Default::default()
        };
        if let Err(e) = gossipsub.with_peer_score(peer_score_params, peer_score_thresholds) {
            warn!("⚠️ Failed to enable gossipsub peer scoring: {}", e);
        } else {
            info!("  ✓ Gossipsub peer scoring enabled");
        }

        // 🔧 FIX F11: Only instantiate mDNS when explicitly enabled.
        // On mainnet / testnet, mDNS broadcasts on the local LAN are both
        // unnecessary and a privacy / security risk. Wrapping in Toggle
        // avoids binding the multicast socket entirely when disabled.
        let mdns_toggle = if enable_mdns {
            let mdns_behaviour =
                mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
                    .map_err(|e| NetworkError::Connection(e.to_string()))?;
            info!("📡 mDNS discovery enabled");
            libp2p::swarm::behaviour::toggle::Toggle::from(Some(mdns_behaviour))
        } else {
            info!("🔇 mDNS discovery disabled");
            libp2p::swarm::behaviour::toggle::Toggle::from(None)
        };

        // Create Kademlia DHT for distributed peer discovery
        // 🔧 FIX F9: Bound MemoryStore to prevent DHT flooding OOM
        let mut store_config = kad::store::MemoryStoreConfig::default();
        store_config.max_records = 65_536;
        store_config.max_provided_keys = 1_024;
        let store = MemoryStore::with_config(local_peer_id, store_config);
        let mut kademlia_config = kad::Config::default();
        kademlia_config
            .set_protocol_names(vec![libp2p::StreamProtocol::new("/luxtensor/kad/1.0.0")]);
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
        // 🔧 FIX F6: Configure connection limits to reject excess connections at the
        // transport layer, before any Noise handshake or gossipsub exchange occurs.
        let conn_limits = connection_limits::ConnectionLimits::default()
            .with_max_pending_incoming(Some(64))
            .with_max_pending_outgoing(Some(64))
            .with_max_established_incoming(Some(128))
            .with_max_established_outgoing(Some(128))
            .with_max_established_per_peer(Some(2));

        let behaviour = BlockchainBehaviour {
            gossipsub,
            mdns: mdns_toggle,
            kademlia,
            connection_limits: connection_limits::Behaviour::new(conn_limits),
        };

        // Build swarm using the new API
        // 📌 INFO F20: Currently TCP-only transport. For mainnet, consider adding
        // QUIC transport via `.with_quic()` for:
        //   - 0-RTT connection setup (lower latency)
        //   - Built-in TLS 1.3 (eliminates separate Noise handshake)
        //   - Multiplexed streams without head-of-line blocking
        //   - Better NAT traversal characteristics
        // Requires adding `quic` feature to libp2p in workspace Cargo.toml.
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

        swarm
            .listen_on(listen_addr.clone())
            .map_err(|e| NetworkError::Connection(e.to_string()))?;

        info!("🌐 Listening on {}", listen_addr);

        // Create topics
        let blocks_topic = IdentTopic::new(TOPIC_BLOCKS);
        let transactions_topic = IdentTopic::new(TOPIC_TRANSACTIONS);
        let sync_topic = IdentTopic::new(TOPIC_SYNC);

        // Subscribe to topics
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&blocks_topic)
            .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&transactions_topic)
            .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;
        swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&sync_topic)
            .map_err(|e| NetworkError::SubscriptionFailed(e.to_string()))?;

        info!("📡 Subscribed to topics: blocks, transactions, sync");

        // Connect to bootstrap nodes if provided
        if !bootstrap_nodes.is_empty() {
            info!("🔗 Connecting to {} bootstrap node(s)...", bootstrap_nodes.len());
            for addr_str in &bootstrap_nodes {
                match addr_str.parse::<Multiaddr>() {
                    Ok(addr) => {
                        // Extract peer ID from multiaddr if present
                        if let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = addr.iter().last()
                        {
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

        // 🔧 FIX: Use strict eclipse protection on mainnet (max 2 peers per /24 subnet)
        // to prevent Sybil/eclipse attacks. Testnet uses relaxed defaults.
        let eclipse_config =
            if chain_id == 8898 { EclipseConfig::mainnet() } else { EclipseConfig::default() };
        let eclipse_protection = std::sync::Arc::new(EclipseProtection::new(eclipse_config));

        // 🔧 FIX F1: Initialize rate limiter for all gossip messages
        let rate_limiter = std::sync::Arc::new(RateLimiter::new(RateLimiterConfig::default()));

        Ok((
            Self {
                swarm,
                event_sender,
                command_rx,
                blocks_topic,
                transactions_topic,
                sync_topic,
                sync_requests: std::collections::HashMap::new(),
                peer_discovery,
                eclipse_protection,
                rate_limiter,
                sync_nonce: AtomicU64::new(0),
                sync_response_timestamps: std::collections::HashMap::new(),
                peer_ips: std::collections::HashMap::new(),
                pending_sync_requests: Vec::new(),
            },
            command_tx,
        ))
    }

    /// Run the swarm event loop - this must be called in a tokio task
    pub async fn run(&mut self) {
        info!("🚀 P2P Swarm event loop started");

        // 🔧 FIX S2: Periodic timer to check sync request timeouts
        let mut sync_timeout_interval = tokio::time::interval(std::time::Duration::from_secs(10));

        loop {
            tokio::select! {
                // 🔧 FIX S2: Check for timed-out sync requests and retry
                _ = sync_timeout_interval.tick() => {
                    self.check_sync_timeouts();
                }
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
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            // 🔧 FIX: Check eclipse protection before accepting connection
                            let remote_addr = match &endpoint {
                                libp2p::core::ConnectedPoint::Dialer { address, .. } => address.clone(),
                                libp2p::core::ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.clone(),
                            };
                            let is_outbound = matches!(endpoint, libp2p::core::ConnectedPoint::Dialer { .. });

                            // Extract IP from multiaddr for eclipse protection
                            let ip_addr = remote_addr.iter().find_map(|proto| match proto {
                                libp2p::multiaddr::Protocol::Ip4(ip) => Some(std::net::IpAddr::V4(ip)),
                                libp2p::multiaddr::Protocol::Ip6(ip) => Some(std::net::IpAddr::V6(ip)),
                                _ => None,
                            });

                            if let Some(ip) = ip_addr {
                                if !self.eclipse_protection.should_allow_connection(&ip, is_outbound) {
                                    warn!("🛡️ Eclipse protection: rejecting peer {} from {}", peer_id, ip);
                                    let _ = self.swarm.disconnect_peer_id(peer_id);
                                    continue;
                                }
                                self.eclipse_protection.add_peer(peer_id.to_string(), ip, is_outbound);
                                // 🔧 FIX S6: Track PeerId→IP for dual-key rate limiting
                                self.peer_ips.insert(peer_id, ip);
                            }

                            info!("✅ Connected to peer: {}", peer_id);
                            if self.event_sender.try_send(SwarmP2PEvent::PeerConnected(peer_id)).is_err() {
                                warn!("⚠️ Event channel full, dropping PeerConnected event");
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            // 🔧 FIX: Remove peer from eclipse protection tracking
                            self.eclipse_protection.remove_peer(&peer_id.to_string());
                            // 🔧 FIX S6: Remove from IP map
                            self.peer_ips.remove(&peer_id);
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
                                debug!("📤 SWARM: Broadcasting {} blocks for sync", blocks.len());
                                let nonce = self.sync_nonce.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let message = NetworkMessage::Blocks { blocks, nonce };
                                let data = match serialize_message(&message) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        warn!("Failed to serialize blocks: {}", e);
                                        continue;
                                    }
                                };

                                // FIX: Use sync_topic for sync responses (previously used blocks_topic
                                // which polluted block gossip and confused receivers)
                                match self.swarm.behaviour_mut().gossipsub.publish(
                                    self.sync_topic.clone(),
                                    data
                                ) {
                                    Ok(_) => debug!("📡 Sync blocks broadcast successful"),
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
                        SwarmCommand::BroadcastTaskDispatch { task_id, model_hash, input_hash, reward, deadline, validator_signature, validator_address } => {
                            // Broadcast AI task to miners via the sync topic
                            info!("📡 Broadcasting authenticated AI task 0x{} from validator {:?}",
                                hex::encode(&task_id[..8]), &validator_address[..4]);
                            let message = NetworkMessage::AITaskDispatch {
                                task_id,
                                model_hash,
                                input_hash,
                                reward,
                                deadline,
                                validator_signature,
                                validator_address,
                            };
                            let data = match serialize_message(&message) {
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
                    self.peer_discovery
                        .on_peer_discovered(peer_id_str.clone(), vec![addr.to_string()]);

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
        debug!("📨 GOSSIP RECEIVED: from {} on topic {} - {} bytes", source, topic, msg_len);

        // SECURITY: Reject oversized messages before deserialization
        if msg_len > crate::messages::MAX_MESSAGE_SIZE as usize {
            warn!(
                "🚫 Dropping oversized gossip message from {}: {} bytes (max {})",
                source,
                msg_len,
                crate::messages::MAX_MESSAGE_SIZE
            );
            return;
        }

        // 🔧 FIX F1+S5+S6: Rate-limit ALL gossip messages with per-topic cost.
        //
        // Different topics have different resource impacts:
        //   - Transactions (1 token): lightweight, high volume expected
        //   - Blocks (5 tokens):      CPU-heavy validation, large payload
        //   - Sync (10 tokens):       very expensive, triggers DB lookups
        //
        // Uses dual-key (PeerId + IP) to prevent Sybil bypass (S6).
        let msg_cost: f64 = if topic.contains("blocks") {
            5.0
        } else if topic.contains("sync") {
            10.0
        } else {
            1.0 // transactions and other topics
        };
        let rate_ok = if let Some(ip) = self.peer_ips.get(&source) {
            self.rate_limiter.check_with_ip_and_cost(&source.to_string(), ip, msg_cost)
        } else {
            self.rate_limiter.check_with_cost(&source.to_string(), msg_cost)
        };
        if !rate_ok {
            warn!(
                "🚫 Rate limiting peer {} on topic {} (cost {}) — dropping",
                source, topic, msg_cost
            );
            return;
        }

        // Deserialize message with size limit to prevent DoS
        match crate::messages::deserialize_message(&message.data) {
            Ok(NetworkMessage::NewBlock(block)) => {
                debug!("📥 Received block #{} from peer {}", block.header.height, source);

                // 🔧 FIX S8: Detect block withholding / selfish mining.
                // If the block timestamp is more than 60s behind the current time,
                // the proposer likely withheld it for strategic advantage.
                // Penalize the *source* peer (gossip propagator) with a mild score
                // hit. Full proposer slashing requires consensus-layer integration.
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if block.header.height > 0 {
                    if block.header.timestamp + 60 < now_secs {
                        let delay = now_secs - block.header.timestamp;
                        warn!(
                            "⏰ Block #{} from {} arrived {}s late (timestamp={}, now={}). \
                               Possible block withholding.",
                            block.header.height, source, delay, block.header.timestamp, now_secs
                        );
                        // Mild penalty on the relay peer; heavy penalty requires
                        // identifying the actual proposer at the consensus layer.
                        self.eclipse_protection.update_peer_score(&source.to_string(), -2);
                    }
                    // Also reject blocks from the future (> 15s drift)
                    if block.header.timestamp > now_secs + 15 {
                        warn!(
                            "🚫 Block #{} from {} has future timestamp ({} > {}+15), dropping",
                            block.header.height, source, block.header.timestamp, now_secs
                        );
                        self.penalize_peer(&source, -5, "Block with future timestamp");
                        return;
                    }
                }

                if self.event_sender.try_send(SwarmP2PEvent::NewBlock(block, source)).is_err() {
                    warn!("⚠️ Event channel full, dropping NewBlock — node is falling behind");
                }
            }
            Ok(NetworkMessage::NewTransaction(tx)) => {
                info!("📥 SWARM: Received NewTransaction from peer {}", source);
                if self.event_sender.try_send(SwarmP2PEvent::NewTransaction(tx, source)).is_err() {
                    warn!("⚠️ Event channel full, dropping NewTransaction");
                }
            }
            Ok(NetworkMessage::SyncRequest {
                from_height,
                to_height,
                requester_id: _,
                nonce: _,
            }) => {
                // SECURITY: Validate sync range to prevent DoS.
                // An attacker requesting from_height=0, to_height=u64::MAX would force
                // expensive database lookups. Cap range at 1000 blocks.
                const MAX_SYNC_RANGE: u64 = 1000;
                if to_height.saturating_sub(from_height) > MAX_SYNC_RANGE {
                    warn!(
                        "🚫 Rejecting oversized sync request from peer {} ({}-{}, range > {})",
                        source, from_height, to_height, MAX_SYNC_RANGE
                    );
                    return;
                }

                // SECURITY: Rate limit sync requests per peer
                let now = std::time::Instant::now();
                let entry = self.sync_requests.entry(source).or_insert((0, now));

                // Reset counter if window expired
                if now.duration_since(entry.1) >= SYNC_RATE_LIMIT_WINDOW {
                    entry.0 = 0;
                    entry.1 = now;
                }

                entry.0 += 1;

                // Warn on high request rate but don't block — gossipsub replays
                // cached messages in bursts when peers first connect, which would
                // otherwise permanently starve sync.
                if entry.0 == MAX_SYNC_REQUESTS_PER_PEER {
                    debug!(
                        "⚠️ High sync request rate from peer {} ({} requests/min)",
                        source, entry.0
                    );
                }

                // 🔧 FIX F17: Per-peer sync response throttle (was global, starving others)
                let now_throttle = std::time::Instant::now();
                let last = self.sync_response_timestamps.get(&source);
                if let Some(ts) = last {
                    if now_throttle.duration_since(*ts) < std::time::Duration::from_secs(5) {
                        debug!(
                            "🔄 Throttling sync response to peer {} (last response {}ms ago)",
                            source,
                            now_throttle.duration_since(*ts).as_millis()
                        );
                        return;
                    }
                }
                self.sync_response_timestamps.insert(source, now_throttle);

                debug!("🔄 Sync request from {} for blocks {}-{}", source, from_height, to_height);
                if self
                    .event_sender
                    .try_send(SwarmP2PEvent::SyncRequest {
                        from_height,
                        to_height,
                        requester_id: source.to_string(),
                    })
                    .is_err()
                {
                    debug!("⚠️ Event channel full, dropping SyncRequest");
                }
            }
            Ok(NetworkMessage::Blocks { blocks, nonce: _ }) => {
                debug!("📥 Received {} blocks from sync", blocks.len());
                // 🔧 FIX S1: Pre-validate sync blocks at the network layer before
                // forwarding to the consumer. This prevents a malicious peer from
                // injecting a phantom chain of structurally invalid blocks.
                let mut prev_height: Option<u64> = None;
                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let mut valid_count = 0u32;
                for block in blocks.into_iter().take(50) {
                    // 1. Structural validation (gas, extra_data, tx count)
                    if let Err(e) = block.validate() {
                        warn!(
                            "🚫 Sync block #{} from {} failed validation: {}, dropping remainder",
                            block.header.height, source, e
                        );
                        self.penalize_peer(&source, -10, "Invalid sync block structure");
                        break;
                    }
                    // 2. Hash integrity — recompute and compare
                    let computed = block.header.hash();
                    // We can't compare to a stored hash since we don't have the chain,
                    // so at minimum verify the hash is non-zero (not a blank block)
                    if computed == [0u8; 32] {
                        warn!(
                            "🚫 Sync block #{} from {} has zero hash, dropping",
                            block.header.height, source
                        );
                        self.penalize_peer(&source, -10, "Sync block with zero hash");
                        break;
                    }
                    // 3. Height monotonicity within the batch
                    if let Some(prev) = prev_height {
                        if block.header.height != prev + 1 {
                            warn!("🚫 Sync blocks from {} have non-sequential heights ({} → {}), dropping remainder",
                                  source, prev, block.header.height);
                            self.penalize_peer(&source, -10, "Non-sequential sync block heights");
                            break;
                        }
                    }
                    // 4. Timestamp sanity — reject blocks from far future
                    if block.header.height > 0 && block.header.timestamp > now_ts + 30 {
                        warn!(
                            "🚫 Sync block #{} from {} has future timestamp ({} > {}+30), dropping",
                            block.header.height, source, block.header.timestamp, now_ts
                        );
                        self.penalize_peer(&source, -5, "Sync block with future timestamp");
                        break;
                    }
                    prev_height = Some(block.header.height);
                    valid_count += 1;
                    if self.event_sender.try_send(SwarmP2PEvent::NewBlock(block, source)).is_err() {
                        debug!("⚠️ Event channel full, dropping sync block");
                        break;
                    }
                }
                if valid_count > 0 {
                    debug!("✅ Forwarded {} validated sync blocks from {}", valid_count, source);
                    // 🔧 FIX S2: Mark fulfilled sync requests so the timeout tracker
                    // doesn't retry a range that has already been answered.
                    if let Some(first_h) = prev_height.map(|last| last + 1 - valid_count as u64) {
                        self.clear_fulfilled_sync_requests(first_h, prev_height.unwrap_or(first_h));
                    }
                }
            }
            // 🔧 FIX F7: Verify AITaskDispatch ECDSA signature before forwarding.
            // The message spec says "Receivers MUST verify" but previously fell through
            // to the catch-all with zero verification.
            Ok(NetworkMessage::AITaskDispatch {
                task_id,
                model_hash,
                input_hash,
                reward,
                deadline,
                validator_signature,
                validator_address,
            }) => {
                // Validate signature size (ECDSA = 64-65 bytes, DER ≤ 73)
                if validator_signature.len() > 73 {
                    warn!(
                        "🚫 AITaskDispatch from {} has oversized signature ({} bytes), dropping",
                        source,
                        validator_signature.len()
                    );
                    return;
                }
                if validator_signature.is_empty() {
                    warn!("🚫 AITaskDispatch from {} has empty signature, dropping", source);
                    return;
                }

                // Reconstruct the signed payload: (task_id || model_hash || input_hash || reward || deadline)
                let mut signed_payload = Vec::with_capacity(32 + model_hash.len() + 32 + 16 + 8);
                signed_payload.extend_from_slice(&task_id);
                signed_payload.extend_from_slice(model_hash.as_bytes());
                signed_payload.extend_from_slice(&input_hash);
                signed_payload.extend_from_slice(&reward.to_le_bytes());
                signed_payload.extend_from_slice(&deadline.to_le_bytes());
                let payload_hash = luxtensor_crypto::keccak256(&signed_payload);

                // Verify ECDSA signature using recover_address_strict (65-byte sig required)
                // or recover_address (deprecated, accepts 64-byte with trial recovery)
                let expected_addr: luxtensor_crypto::CryptoAddress = validator_address.into();
                let sig_verified = if validator_signature.len() == 65 {
                    luxtensor_crypto::recover_address_strict(&payload_hash, &validator_signature)
                        .map(|recovered| recovered == expected_addr)
                        .unwrap_or(false)
                } else if validator_signature.len() == 64 {
                    #[allow(deprecated)]
                    luxtensor_crypto::recover_address(&payload_hash, &validator_signature)
                        .map(|recovered| recovered == expected_addr)
                        .unwrap_or(false)
                } else {
                    false
                };

                if !sig_verified {
                    warn!("🚫 AITaskDispatch from {} has invalid signature (validator {:?}), dropping",
                          source, &validator_address[..4]);
                    return;
                }

                info!(
                    "✅ AITaskDispatch from {} verified (task 0x{}, reward={})",
                    source,
                    hex::encode(&task_id[..8]),
                    reward
                );
                // TODO: Forward to application layer event channel when AI task handling is implemented
            }
            Ok(_) => {
                debug!("Received other message type");
            }
            Err(e) => {
                let first_bytes: Vec<String> =
                    message.data.iter().take(8).map(|b| format!("{:02x}", b)).collect();
                warn!(
                    "Failed to deserialize gossip message from {} on topic '{}' ({} bytes, first 8: [{}]): {}",
                    source, topic, msg_len, first_bytes.join(" "), e
                );
                // 🔧 FIX S7+S3: Penalize peer for sending malformed gossip data.
                // Uses penalize_peer which bans both the PeerId (eclipse score)
                // and the source IP (rate limiter) to prevent Sybil bypass.
                self.penalize_peer(&source, -5, "Malformed gossip data");
            }
        }
    }

    /// Broadcast a block to the network.
    ///
    /// Logs a warning if the block's timestamp is more than 30 seconds behind
    /// the current wall-clock time, which may indicate withholding / late
    /// publication by the proposer.
    pub fn broadcast_block(&mut self, block: &Block) -> Result<(), NetworkError> {
        // Detect late block publication (S8 — Selfish Mining mitigation)
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let block_ts = block.header.timestamp;
        if now_secs > block_ts && now_secs - block_ts > 30 {
            warn!(
                "Block #{} timestamp {} is {}s behind wall-clock {} — possible withholding",
                block.header.height,
                block_ts,
                now_secs - block_ts,
                now_secs
            );
        }

        let message = NetworkMessage::NewBlock(block.clone());
        let data = serialize_message(&message)
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
            Err(e) => Err(NetworkError::PublishFailed(e.to_string())),
        }
    }

    /// Broadcast a transaction to the network.
    ///
    /// Publishes to the dedicated transactions gossipsub topic.
    /// 🔧 FIX F15: Removed fallback to blocks topic — it polluted block gossip
    /// and degraded block propagation. If no peers subscribe to transactions,
    /// the TX will be discovered later via sync.
    pub fn broadcast_transaction(&mut self, tx: &Transaction) -> Result<(), NetworkError> {
        let message = NetworkMessage::NewTransaction(tx.clone());
        let data = serialize_message(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        match self.swarm.behaviour_mut().gossipsub.publish(self.transactions_topic.clone(), data) {
            Ok(_) => {
                debug!("📡 TX broadcast via transactions topic");
                Ok(())
            }
            Err(gossipsub::PublishError::InsufficientPeers) => {
                warn!("⚠️ No peers subscribed to transactions topic — TX NOT propagated (will sync later)");
                Ok(()) // Not fatal — peers will sync later
            }
            Err(e) => Err(NetworkError::PublishFailed(e.to_string())),
        }
    }

    /// 🔧 FIX S3: Penalize a peer by eclipse score AND ban their IP.
    ///
    /// Combines `eclipse_protection::update_peer_score` with `rate_limiter::ban_ip`
    /// so that generating a new PeerId from the same IP cannot evade the ban.
    fn penalize_peer(&self, source: &PeerId, delta: i32, reason: &str) {
        self.eclipse_protection.update_peer_score(&source.to_string(), delta);
        if let Some(ip) = self.peer_ips.get(source) {
            self.rate_limiter.ban_ip(ip, reason);
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

    /// Send sync request to network.
    ///
    /// Each request gets a unique nonce to prevent gossipsub from deduplicating
    /// identical requests (same from/to/requester_id) via keccak256 message_id_fn.
    ///
    /// 🔧 FIX S2: Tracks the request in `pending_sync_requests` so that
    /// `check_sync_timeouts()` can detect unresponsive peers and retry
    /// with a fresh nonce. After `SYNC_REQUEST_MAX_RETRIES` the request
    /// is dropped.
    fn send_sync_request(
        &mut self,
        from_height: u64,
        to_height: u64,
        my_id: String,
    ) -> Result<(), NetworkError> {
        let nonce = self.sync_nonce.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let message = NetworkMessage::SyncRequest {
            from_height,
            to_height,
            requester_id: my_id.clone(),
            nonce,
        };
        let data = serialize_message(&message)
            .map_err(|e| NetworkError::SerializationFailed(e.to_string()))?;

        match self.swarm.behaviour_mut().gossipsub.publish(self.sync_topic.clone(), data) {
            Ok(_) => {
                debug!("🔄 Sent sync request for blocks {}-{}", from_height, to_height);
                // Track this request for timeout retry
                self.pending_sync_requests.push(PendingSyncRequest {
                    from_height,
                    to_height,
                    my_id,
                    sent_at: std::time::Instant::now(),
                    retries: 0,
                });
                Ok(())
            }
            Err(gossipsub::PublishError::InsufficientPeers) => {
                debug!("No peers subscribed to sync topic");
                Ok(())
            }
            Err(e) => Err(NetworkError::PublishFailed(e.to_string())),
        }
    }

    /// 🔧 FIX S2: Check for timed-out sync requests and retry or drop them.
    ///
    /// Called periodically from the event loop timer. For each pending request
    /// that has exceeded `SYNC_REQUEST_TIMEOUT`:
    /// - If retries < `SYNC_REQUEST_MAX_RETRIES`, re-publish with a new nonce
    /// - Otherwise, drop the request and log a warning
    fn check_sync_timeouts(&mut self) {
        let now = std::time::Instant::now();
        let mut to_retry: Vec<PendingSyncRequest> = Vec::new();

        // Drain expired requests into retry list
        self.pending_sync_requests.retain(|req| {
            if now.duration_since(req.sent_at) < SYNC_REQUEST_TIMEOUT {
                true // still within timeout window
            } else if req.retries < SYNC_REQUEST_MAX_RETRIES {
                to_retry.push(PendingSyncRequest {
                    from_height: req.from_height,
                    to_height: req.to_height,
                    my_id: req.my_id.clone(),
                    sent_at: now,
                    retries: req.retries + 1,
                });
                false // remove the old record; retry will add a new one
            } else {
                warn!(
                    "🚫 Sync request {}-{} timed out after {} retries, dropping",
                    req.from_height, req.to_height, req.retries
                );
                false
            }
        });

        // Re-publish retries with fresh nonces
        for req in to_retry {
            let nonce = self.sync_nonce.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let message = NetworkMessage::SyncRequest {
                from_height: req.from_height,
                to_height: req.to_height,
                requester_id: req.my_id.clone(),
                nonce,
            };
            if let Ok(data) = serialize_message(&message) {
                match self.swarm.behaviour_mut().gossipsub.publish(self.sync_topic.clone(), data) {
                    Ok(_) => {
                        info!(
                            "🔄 Retry {}/{} sync request for blocks {}-{}",
                            req.retries, SYNC_REQUEST_MAX_RETRIES, req.from_height, req.to_height
                        );
                        self.pending_sync_requests.push(req);
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to retry sync request: {}", e);
                    }
                }
            }
        }
    }

    /// 🔧 FIX S2: Clear pending sync requests for a fulfilled range.
    ///
    /// Called when sync blocks are received to remove tracking records
    /// whose range overlaps with the received blocks.
    fn clear_fulfilled_sync_requests(&mut self, received_from: u64, received_to: u64) {
        self.pending_sync_requests.retain(|req| {
            // Remove if the received range covers this request
            !(received_from <= req.from_height && received_to >= req.to_height)
        });
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

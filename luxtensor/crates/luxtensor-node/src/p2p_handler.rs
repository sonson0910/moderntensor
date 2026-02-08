//! P2P Event Handler
//! Handles incoming P2P events like new blocks and transactions

use anyhow::Result;
use luxtensor_core::{Block, Transaction, StateDB};
use luxtensor_network::{P2PEvent, SwarmCommand};
use luxtensor_network::eclipse_protection::EclipseProtection;
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;
use std::net::IpAddr;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use crate::mempool::Mempool;

/// Sync state tracker
pub struct SyncState {
    /// Whether we are currently syncing
    pub is_syncing: bool,
    /// Height we are syncing to
    pub target_height: u64,
    /// Node ID for sync requests
    pub node_id: String,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            is_syncing: false,
            target_height: 0,
            node_id: "node".to_string(),
        }
    }
}

/// Handle P2P events
pub async fn p2p_event_loop(
    mut event_rx: mpsc::UnboundedReceiver<P2PEvent>,
    storage: Arc<BlockchainDB>,
    state_db: Arc<RwLock<StateDB>>,
    mempool: Arc<Mempool>,
    sync_command_tx: Option<mpsc::UnboundedSender<SwarmCommand>>,
    node_id: String,
    eclipse_protection: Arc<EclipseProtection>,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
) -> Result<()> {
    info!("🌐 P2P event loop started");

    let sync_state = Arc::new(RwLock::new(SyncState {
        is_syncing: false,
        target_height: 0,
        node_id: node_id.clone(),
    }));

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                if let Err(e) = handle_p2p_event(
                    event,
                    &storage,
                    &state_db,
                    &mempool,
                    sync_command_tx.as_ref(),
                    &sync_state,
                    &eclipse_protection,
                ).await {
                    error!("Error handling P2P event: {}", e);
                }
            }
            _ = shutdown.recv() => {
                info!("P2P event loop shutting down");
                break;
            }
        }
    }

    Ok(())
}

/// Handle a single P2P event
async fn handle_p2p_event(
    event: P2PEvent,
    storage: &Arc<BlockchainDB>,
    _state_db: &Arc<RwLock<StateDB>>,
    mempool: &Arc<Mempool>,
    sync_command_tx: Option<&mpsc::UnboundedSender<SwarmCommand>>,
    sync_state: &Arc<RwLock<SyncState>>,
    eclipse_protection: &Arc<EclipseProtection>,
) -> Result<()> {
    match event {
        P2PEvent::NewBlock(block) => {
            // Update peer score positively for valid block contribution
            // (We don't have source peer here, but in handle_new_block we could track it)
            handle_new_block(block, storage, sync_command_tx, sync_state, eclipse_protection).await?;
        }
        P2PEvent::NewTransaction(tx) => {
            handle_new_transaction(tx, mempool).await?;
        }
        P2PEvent::PeerConnected(peer_id) => {
            // Eclipse protection (IP check + add_peer) is handled at the swarm layer
            // in ConnectionEstablished, where we have access to the real remote IP
            // and connection direction. Here we just log the event.
            info!("👋 Peer connected: {} (diversity: {}%)",
                peer_id, eclipse_protection.calculate_diversity_score());
        }
        P2PEvent::PeerDisconnected(peer_id) => {
            // Remove peer from Eclipse Protection tracking
            eclipse_protection.remove_peer(&peer_id.to_string());
            info!("👋 Peer disconnected and unregistered: {}", peer_id);
        }
        P2PEvent::GossipMessage { source, data, topic } => {
            debug!("📨 Gossip message from {:?} on topic {}: {} bytes",
                   source, topic, data.len());
            // Could update peer score here based on message validity
        }
        _ => {
            debug!("Unhandled P2P event");
        }
    }

    Ok(())
}

/// Convert PeerId to synthetic IP for subnet diversity tracking
/// This is a hash-based approach since libp2p PeerIds don't directly contain IPs
fn peer_id_to_synthetic_ip(peer_id: &str) -> IpAddr {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    peer_id.hash(&mut hasher);
    let hash = hasher.finish();

    // Create a synthetic IPv4 from the hash for subnet diversity calculation
    let bytes = hash.to_be_bytes();
    IpAddr::V4(std::net::Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))
}

/// Handle incoming block from P2P network
async fn handle_new_block(
    block: Block,
    storage: &Arc<BlockchainDB>,
    sync_command_tx: Option<&mpsc::UnboundedSender<SwarmCommand>>,
    sync_state: &Arc<RwLock<SyncState>>,
    _eclipse_protection: &Arc<EclipseProtection>, // For future peer scoring on block delivery
) -> Result<()> {
    let block_height = block.header.height;
    let block_hash = block.hash();

    // Check if we already have this block
    if storage.get_block_by_height(block_height)?.is_some() {
        debug!("Already have block #{}, skipping", block_height);
        return Ok(());
    }

    // Validate block is sequential
    let current_height = storage.get_best_height()?.unwrap_or(0);

    if block_height <= current_height {
        debug!("Block #{} is not newer than current height {}", block_height, current_height);
        return Ok(());
    }

    if block_height > current_height + 1 {
        // We're missing blocks, need to sync
        warn!("Missing blocks between {} and {}, initiating sync", current_height, block_height);

        // Request sync if we have a sync command sender and not already syncing
        if let Some(tx) = sync_command_tx {
            let mut state = sync_state.write();
            if !state.is_syncing {
                state.is_syncing = true;
                state.target_height = block_height;

                let sync_cmd = SwarmCommand::RequestSync {
                    from_height: current_height + 1,
                    to_height: block_height,
                    my_id: state.node_id.clone(),
                };

                if let Err(e) = tx.send(sync_cmd) {
                    error!("Failed to send sync request: {}", e);
                    state.is_syncing = false;
                } else {
                    info!("🔄 Sync request sent for blocks {}-{}", current_height + 1, block_height);
                }
            } else {
                debug!("Already syncing to height {}", state.target_height);
            }
        }
        return Ok(());
    }

    // Validate previous hash matches our chain
    if let Some(prev_block) = storage.get_block_by_height(current_height)? {
        if block.header.previous_hash != prev_block.hash() {
            warn!("Block #{} has invalid previous hash", block_height);
            return Ok(());
        }
    }

    // Store the block
    storage.store_block(&block)?;
    info!("📥 Received and stored block #{} hash {:?} from P2P", block_height, &block_hash[..4]);

    // Reset sync state if we've caught up
    {
        let mut state = sync_state.write();
        if state.is_syncing && block_height >= state.target_height {
            state.is_syncing = false;
            info!("✅ Sync complete! Caught up to block #{}", block_height);
        }
    }

    Ok(())
}

/// Handle incoming transaction from P2P network
async fn handle_new_transaction(tx: Transaction, mempool: &Arc<Mempool>) -> Result<()> {
    let tx_hash = tx.hash();
    debug!("📥 Received transaction 0x{} from P2P", hex::encode(&tx_hash[..8]));

    // Validate signature
    if let Err(e) = tx.verify_signature() {
        warn!("Invalid transaction signature from P2P: {}", e);
        return Ok(());
    }

    // Add to mempool
    match mempool.add_transaction(tx) {
        Ok(_) => {
            info!("📬 Added P2P transaction 0x{} to mempool", hex::encode(&tx_hash[..8]));
        }
        Err(e) => {
            debug!("Could not add P2P transaction to mempool: {}", e);
        }
    }

    Ok(())
}

/// Check if this node is the leader for the current slot
pub fn is_leader_for_slot(
    validator_id: &str,
    slot: u64,
    validators: &[String],
) -> bool {
    if validators.is_empty() {
        // If no validators configured, everyone can produce
        return true;
    }

    let leader_index = (slot % validators.len() as u64) as usize;

    if let Some(leader) = validators.get(leader_index) {
        let is_leader = leader == validator_id;
        if is_leader {
            debug!("🎯 Slot {}: We are the leader (validator_id={})", slot, validator_id);
        } else {
            debug!("⏳ Slot {}: Leader is {}, we are {}", slot, leader, validator_id);
        }
        is_leader
    } else {
        false
    }
}

/// Calculate current slot from timestamp
pub fn calculate_slot(timestamp: u64, genesis_timestamp: u64, block_time: u64) -> u64 {
    if timestamp <= genesis_timestamp {
        return 0;
    }
    (timestamp - genesis_timestamp) / block_time
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_leader_for_slot() {
        let validators = vec![
            "validator-1".to_string(),
            "validator-2".to_string(),
            "validator-3".to_string(),
        ];

        // Slot 0 -> validator-1
        assert!(is_leader_for_slot("validator-1", 0, &validators));
        assert!(!is_leader_for_slot("validator-2", 0, &validators));
        assert!(!is_leader_for_slot("validator-3", 0, &validators));

        // Slot 1 -> validator-2
        assert!(!is_leader_for_slot("validator-1", 1, &validators));
        assert!(is_leader_for_slot("validator-2", 1, &validators));
        assert!(!is_leader_for_slot("validator-3", 1, &validators));

        // Slot 2 -> validator-3
        assert!(!is_leader_for_slot("validator-1", 2, &validators));
        assert!(!is_leader_for_slot("validator-2", 2, &validators));
        assert!(is_leader_for_slot("validator-3", 2, &validators));

        // Slot 3 -> validator-1 (wraps around)
        assert!(is_leader_for_slot("validator-1", 3, &validators));
    }

    #[test]
    fn test_calculate_slot() {
        let genesis = 1000;
        let block_time = 3;

        assert_eq!(calculate_slot(1000, genesis, block_time), 0);
        assert_eq!(calculate_slot(1003, genesis, block_time), 1);
        assert_eq!(calculate_slot(1006, genesis, block_time), 2);
        assert_eq!(calculate_slot(1009, genesis, block_time), 3);
    }

    #[test]
    fn test_empty_validators() {
        // If no validators, everyone can produce
        assert!(is_leader_for_slot("anyone", 0, &[]));
        assert!(is_leader_for_slot("anyone", 1, &[]));
    }
}

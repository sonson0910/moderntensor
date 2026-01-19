//! P2P Event Handler
//! Handles incoming P2P events like new blocks and transactions

use anyhow::Result;
use luxtensor_core::{Block, Transaction, StateDB};
use luxtensor_network::P2PEvent;
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use crate::mempool::Mempool;

/// Handle P2P events
pub async fn p2p_event_loop(
    mut event_rx: mpsc::UnboundedReceiver<P2PEvent>,
    storage: Arc<BlockchainDB>,
    state_db: Arc<RwLock<StateDB>>,
    mempool: Arc<Mempool>,
    mut shutdown: tokio::sync::broadcast::Receiver<()>,
) -> Result<()> {
    info!("🌐 P2P event loop started");

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                if let Err(e) = handle_p2p_event(event, &storage, &state_db, &mempool).await {
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
) -> Result<()> {
    match event {
        P2PEvent::NewBlock(block) => {
            handle_new_block(block, storage).await?;
        }
        P2PEvent::NewTransaction(tx) => {
            handle_new_transaction(tx, mempool).await?;
        }
        P2PEvent::PeerConnected(peer_id) => {
            info!("👋 Peer connected: {}", peer_id);
        }
        P2PEvent::PeerDisconnected(peer_id) => {
            info!("👋 Peer disconnected: {}", peer_id);
        }
        P2PEvent::GossipMessage { source, data, topic } => {
            debug!("📨 Gossip message from {:?} on topic {}: {} bytes",
                   source, topic, data.len());
        }
        _ => {
            debug!("Unhandled P2P event");
        }
    }

    Ok(())
}

/// Handle incoming block from P2P network
async fn handle_new_block(block: Block, storage: &Arc<BlockchainDB>) -> Result<()> {
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
        warn!("Missing blocks between {} and {}, need sync", current_height, block_height);
        // TODO: Request missing blocks from peer
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

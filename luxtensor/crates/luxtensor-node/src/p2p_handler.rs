//! P2P Event Handler — extracted from `startup.rs` for maintainability.
//!
//! Contains `P2PContext` which holds all shared state needed by the P2P event
//! handler, and dedicated handler methods for each `SwarmP2PEvent` variant.
//!
//! ## Lock Ordering (must be respected to prevent deadlocks)
//! 1. `state_db` (RwLock)
//! 2. `unified_state` (RwLock)
//! 3. `fast_finality` (RwLock)
//! 4. `liveness_monitor` (RwLock)
//! 5. `health_monitor` (RwLock)
//! 6. `fork_choice` (RwLock)

use crate::executor::TransactionExecutor;
use crate::health::HealthMonitor;
use crate::mempool::Mempool;
use crate::service::MAX_BLOCK_CLOCK_DRIFT_SECS;

use dashmap::DashMap;
use luxtensor_consensus::fast_finality::FastFinality;
use luxtensor_consensus::liveness::LivenessMonitor;
use luxtensor_consensus::long_range_protection::LongRangeProtection;
use luxtensor_consensus::ProofOfStake;
use luxtensor_core::{Block, Hash, Transaction};
use luxtensor_network::eclipse_protection::EclipseProtection;
use luxtensor_network::rate_limiter::RateLimiter as NetworkRateLimiter;
use luxtensor_network::{SwarmCommand, SwarmP2PEvent};
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Shared context for all P2P event handlers.
///
/// This struct holds cloned Arc references to every subsystem that the P2P
/// event loop needs to interact with. It is constructed once at node startup
/// and passed into the event handler task.
pub(crate) struct P2PContext {
    pub storage: Arc<BlockchainDB>,
    pub broadcast_tx: Option<mpsc::Sender<SwarmCommand>>,
    pub node_name: String,
    pub shared_pending_txs: Arc<DashMap<Hash, Transaction>>,
    pub eclipse_protection: Arc<EclipseProtection>,
    pub long_range_protection: Arc<LongRangeProtection>,
    pub liveness_monitor: Arc<RwLock<LivenessMonitor>>,
    pub fast_finality: Arc<RwLock<FastFinality>>,
    pub fork_choice: Arc<RwLock<luxtensor_consensus::fork_choice::ForkChoice>>,
    pub mempool: Arc<Mempool>,
    pub rpc_mempool: Arc<luxtensor_core::UnifiedMempool>,
    pub health_monitor: Arc<RwLock<HealthMonitor>>,
    pub rate_limiter: Arc<NetworkRateLimiter>,
    pub unified_state: Arc<RwLock<luxtensor_core::UnifiedStateDB>>,
    pub state_db: Arc<RwLock<luxtensor_core::StateDB>>,
    pub executor: Arc<TransactionExecutor>,
    pub consensus: Arc<RwLock<ProofOfStake>>,
    pub epoch_length: u64,
    pub best_height: Arc<AtomicU64>,
    pub is_syncing: Arc<AtomicBool>,
}

impl P2PContext {
    /// Run the P2P event handler loop, dispatching events to dedicated handlers.
    pub async fn run(self, mut event_rx: mpsc::Receiver<SwarmP2PEvent>) {
        while let Some(event) = event_rx.recv().await {
            match event {
                SwarmP2PEvent::NewBlock(block) => self.handle_new_block(block).await,
                SwarmP2PEvent::NewTransaction(tx) => self.handle_new_transaction(tx).await,
                SwarmP2PEvent::PeerConnected(peer_id) => {
                    self.handle_peer_connected(peer_id).await;
                }
                SwarmP2PEvent::PeerDisconnected(peer_id) => {
                    self.handle_peer_disconnected(peer_id);
                }
                SwarmP2PEvent::SyncRequest {
                    from_height,
                    to_height,
                    requester_id,
                } => {
                    self.handle_sync_request(from_height, to_height, requester_id)
                        .await;
                }
            }
        }
        info!("📡 P2P event handler loop exited (channel closed)");
    }

    // ========================================================================
    // Handler: NewBlock
    // ========================================================================

    async fn handle_new_block(&self, block: Block) {
        let height = block.header.height;
        let block_hash = block.hash();

        // 🛡️ Rate-limit: only for blocks far ahead of our chain tip.
        let current_best = self.best_height.load(Ordering::Relaxed);
        let proposer_id = hex::encode(&block.header.validator);
        if height > current_best + 100 && !self.rate_limiter.check(&proposer_id) {
            warn!("🛡️ Block #{} rate-limited from proposer {}", height, proposer_id);
            return;
        }

        // 🛡️ Long-range attack protection: validate against checkpoints
        if !self.long_range_protection.validate_against_checkpoints(block_hash, height) {
            warn!("🛡️ Block #{} rejected: checkpoint mismatch (potential long-range attack)", height);
            return;
        }

        // Check if we already have this block
        if self.storage.get_block_by_height(height).ok().flatten().is_some() {
            debug!("Already have block #{}, skipping", height);
            return;
        }

        // Check weak subjectivity
        if !self.long_range_protection.is_within_weak_subjectivity(height) {
            warn!("🛡️ Block #{} rejected: outside weak subjectivity window", height);
            return;
        }

        // ====================================================================
        // 🔐 BLOCK VALIDATION — verify before storing (CRITICAL)
        // ====================================================================

        if !self.validate_block_structure(&block, height) {
            return;
        }

        if !self.validate_block_proposer(&block, height) {
            return;
        }

        // Validate txs_root (Merkle root of transactions)
        let tx_hashes: Vec<[u8; 32]> = block.transactions.iter().map(|tx| tx.hash()).collect();
        let expected_txs_root = if tx_hashes.is_empty() {
            [0u8; 32]
        } else {
            luxtensor_crypto::MerkleTree::new(tx_hashes).root()
        };
        if block.header.txs_root != expected_txs_root {
            warn!("🚫 Block #{} rejected: txs_root mismatch", height);
            return;
        }

        // Validate reasonable timestamp (not too far in future)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if block.header.timestamp > now + MAX_BLOCK_CLOCK_DRIFT_SECS {
            warn!(
                "🚫 Block #{} rejected: timestamp {} is too far in the future (now={})",
                height, block.header.timestamp, now
            );
            return;
        }

        // Full structural validation via Block::validate()
        if let Err(e) = block.validate() {
            warn!("🚫 Block #{} rejected: validate() failed: {}", height, e);
            return;
        }

        // Sequential height check — P2P ingestion runs in a single task,
        // no concurrent writes from this path.
        let my_height = self.best_height.load(Ordering::SeqCst);
        if height != my_height + 1 {
            debug!("Block #{} out of order (current={}), skipping", height, my_height);
            return;
        }

        // ====================================================================
        // STORE & APPLY
        // ====================================================================

        if let Err(e) = self.storage.store_block(&block) {
            warn!("Failed to store received block: {}", e);
        } else {
            self.best_height.store(height, Ordering::SeqCst);
            info!("📥 Synced block #{} from peer", height);

            // Resume block production if we were syncing
            if self.is_syncing.load(Ordering::SeqCst) {
                info!("🔄 Sync progress: stored block #{}, resuming production...", height);
                self.is_syncing.store(false, Ordering::SeqCst);
            }

            // Remove confirmed txs from pending pool
            {
                let tx_hashes: Vec<[u8; 32]> =
                    block.transactions.iter().map(|tx| tx.hash()).collect();
                for hash in &tx_hashes {
                    self.shared_pending_txs.remove(hash);
                }
                self.mempool.remove_transactions(&tx_hashes);
            }

            self.execute_and_persist_block(&block, height, block_hash);
            self.update_post_block_state(&block, height, block_hash);
        }
    }

    /// Validate sequential height, parent hash chain link, and timestamp monotonicity.
    fn validate_block_structure(&self, block: &Block, height: u64) -> bool {
        let my_height = self
            .storage
            .get_best_height()
            .unwrap_or(Some(0))
            .unwrap_or(0);

        if height > my_height + 1 {
            // Gap — request sync
            const MAX_SYNC_RANGE: u64 = 1000;
            let sync_to = (my_height + MAX_SYNC_RANGE).min(height);
            debug!(
                "Block #{} is ahead of our height {}, requesting sync {}-{}",
                height,
                my_height,
                my_height + 1,
                sync_to
            );
            if let Some(ref tx) = self.broadcast_tx {
                let tx = tx.clone();
                let node_name = self.node_name.clone();
                tokio::spawn(async move {
                    if let Err(e) = tx
                        .send(SwarmCommand::RequestSync {
                            from_height: my_height + 1,
                            to_height: sync_to,
                            my_id: node_name,
                        })
                        .await
                    {
                        warn!("Failed to send sync request: {}", e);
                    }
                });
            }
            return false;
        }
        if height <= my_height {
            debug!("Block #{} is not newer than our height {}", height, my_height);
            return false;
        }

        // Validate previous_hash chain link
        if let Ok(Some(prev_block)) = self.storage.get_block_by_height(my_height) {
            if block.header.previous_hash != prev_block.hash() {
                warn!(
                    "🚫 Block #{} rejected: previous_hash mismatch (expected {:?}, got {:?})",
                    height,
                    &prev_block.hash()[..4],
                    &block.header.previous_hash[..4]
                );
                return false;
            }

            // SECURITY: Validate timestamp monotonicity against parent
            if block.header.timestamp < prev_block.header.timestamp {
                warn!(
                    "🚫 Block #{} rejected: timestamp regression ({} < parent {})",
                    height, block.header.timestamp, prev_block.header.timestamp
                );
                return false;
            }
        }

        true
    }

    /// Validate block signature, proposer registration, and VRF proof.
    fn validate_block_proposer(&self, block: &Block, height: u64) -> bool {
        // Validate block signature (if validator field is set)
        if block.header.validator != [0u8; 32] && !block.header.signature.is_empty() {
            let validator_addr = &block.header.validator[12..32];
            let mut unsigned_header = block.header.clone();
            let sig_backup = unsigned_header.signature.clone();
            unsigned_header.signature = vec![];
            let header_hash = unsigned_header.hash();

            if sig_backup.len() >= 64 {
                match luxtensor_crypto::recover_address(&header_hash, &sig_backup) {
                    Ok(recovered_addr) => {
                        if recovered_addr.as_bytes() != validator_addr {
                            warn!(
                                "🚫 Block #{} rejected: invalid validator signature (recovered {:?} != expected {:?})",
                                height,
                                &recovered_addr.as_bytes()[..4],
                                &validator_addr[..4]
                            );
                            return false;
                        }
                    }
                    Err(_) => {
                        warn!("🚫 Block #{} rejected: signature recovery failed", height);
                        return false;
                    }
                }
            }
        }

        // Validate proposer is a registered active validator
        if block.header.validator != [0u8; 32] {
            let validator_addr_bytes = &block.header.validator[12..32];
            let mut addr_20 = [0u8; 20];
            addr_20.copy_from_slice(validator_addr_bytes);
            let proposer_addr = luxtensor_core::Address::from(addr_20);

            let is_active_validator = {
                let pos = self.consensus.read();
                let vs = pos.validator_set();
                let vs_lock = vs.read();
                vs_lock
                    .active_validators()
                    .iter()
                    .any(|v| v.address == proposer_addr)
            };

            if !is_active_validator {
                warn!(
                    "🚫 Block #{} rejected: proposer {:?} is not an active validator",
                    height,
                    &addr_20[..4]
                );
                return false;
            }

            // VRF proof verification
            if let Some(ref vrf_bytes) = block.header.vrf_proof {
                if vrf_bytes.len() == 97 {
                    let vrf_valid = {
                        let pos = self.consensus.read();
                        let vs = pos.validator_set();
                        let vs_lock = vs.read();
                        if let Some(validator) = vs_lock.get_validator(&proposer_addr) {
                            let mut proof_arr = [0u8; 97];
                            proof_arr.copy_from_slice(vrf_bytes);
                            match luxtensor_crypto::vrf::VrfProof::from_bytes(&proof_arr) {
                                Ok(proof) => {
                                    let epoch = height / self.epoch_length.max(1);
                                    let mut alpha = Vec::with_capacity(48);
                                    alpha.extend_from_slice(&epoch.to_le_bytes());
                                    alpha.extend_from_slice(&height.to_le_bytes());
                                    alpha.extend_from_slice(&block.header.previous_hash);
                                    luxtensor_crypto::vrf::vrf_verify(
                                        &validator.public_key,
                                        &alpha,
                                        &proof,
                                    )
                                    .is_ok()
                                }
                                Err(_) => false,
                            }
                        } else {
                            false
                        }
                    };
                    if !vrf_valid {
                        warn!("🚫 Block #{} rejected: invalid VRF proof", height);
                        return false;
                    }
                    debug!("✅ VRF proof verified for block #{}", height);
                } else {
                    warn!(
                        "🚫 Block #{} rejected: VRF proof has wrong size ({} bytes, expected 97)",
                        height,
                        vrf_bytes.len()
                    );
                    return false;
                }
            }
        }

        true
    }

    /// Execute block transactions against StateDB and persist to storage.
    fn execute_and_persist_block(&self, block: &Block, height: u64, block_hash: [u8; 32]) {
        // Write lock only during TX execution
        {
            let mut state = self.state_db.write();
            for (tx_index, tx) in block.transactions.iter().enumerate() {
                if let Err(e) = self.executor.execute(
                    tx,
                    &mut state,
                    height,
                    block_hash,
                    tx_index,
                    block.header.timestamp,
                ) {
                    debug!(
                        "P2P block #{} tx {} execution failed: {} (may be expected for already-applied state)",
                        height, tx_index, e
                    );
                }
            }
        }
        // Persist state with read lock only
        {
            let state = self.state_db.read();
            if let Err(e) = state.flush_to_db(self.storage.as_ref()) {
                warn!("Failed to persist P2P block state: {}", e);
            }

            // Sync UnifiedStateDB
            {
                let mut unified = self.unified_state.write();
                unified.sync_from_state_db(&state, height);
                debug!("📊 P2P: UnifiedStateDB synced to height {}", height);
            }
        }
    }

    /// Update fork choice, fast finality, liveness, health, and long-range protection.
    fn update_post_block_state(&self, block: &Block, height: u64, block_hash: [u8; 32]) {
        // Feed block to ForkChoice (GHOST)
        if let Err(e) = self.fork_choice.read().add_block(block.clone()) {
            debug!("ForkChoice: {}", e);
        }

        // Fast finality: record the block producer's attestation
        if block.header.validator != [0u8; 32] {
            let mut validator_addr = [0u8; 20];
            validator_addr.copy_from_slice(&block.header.validator[12..32]);
            let addr = luxtensor_core::Address::from(validator_addr);
            let finalized = self.fast_finality.write().add_signature(block_hash, height, addr);
            match finalized {
                Ok(true) => info!("⚡ Block #{} reached fast finality!", height),
                Ok(false) => debug!("Block #{} collecting finality signatures", height),
                Err(e) => debug!("FastFinality skipped: {}", e),
            }
        }

        // Liveness monitoring
        self.liveness_monitor.write().record_block(height);

        // Health monitoring
        self.health_monitor.write().update_block_height(height);

        // Update finalized state for long-range protection
        let finality_depth = 32;
        if height > finality_depth {
            let finalized_height = height - finality_depth;
            if let Ok(Some(finalized_block)) = self.storage.get_block_by_height(finalized_height) {
                self.long_range_protection.update_finalized(
                    finalized_block.hash(),
                    finalized_height,
                    finalized_block.header.state_root,
                );
            }
        }
    }

    // ========================================================================
    // Handler: NewTransaction
    // ========================================================================

    async fn handle_new_transaction(&self, tx: Transaction) {
        // Rate-limit: check per-sender message rate
        let sender_id = hex::encode(&tx.from);
        if !self.rate_limiter.check(&sender_id) {
            warn!("🛡️ Transaction rate-limited from sender {}", sender_id);
            return;
        }

        let tx_hash = tx.hash();

        // Deduplicate
        if self.shared_pending_txs.contains_key(&tx_hash) {
            return;
        }

        // Validate through mempool first
        match self.mempool.add_transaction(tx.clone()) {
            Ok(_) => {
                // Mempool accepted — add to shared pending pool
                self.shared_pending_txs.insert(tx_hash, tx.clone());
                // Also add to RPC UnifiedMempool
                if let Err(e) = self.rpc_mempool.add_transaction(tx) {
                    warn!("⚠️ RPC mempool rejected P2P tx: {} — eth_getTransactionByHash won't find it", e);
                } else {
                    info!("✅ P2P tx also added to RPC UnifiedMempool");
                }
                info!("📥 Added validated transaction from peer to shared pool + RPC mempool");
            }
            Err(e) => {
                debug!("Mempool rejected P2P tx: {}", e);
            }
        }
    }

    // ========================================================================
    // Handler: PeerConnected
    // ========================================================================

    async fn handle_peer_connected(&self, peer_id: luxtensor_network::PeerId) {
        // Eclipse Protection: track peer
        let peer_id_str = peer_id.to_string();
        let synthetic_ip = crate::service::peer_id_to_synthetic_ip(&peer_id_str);
        self.eclipse_protection
            .add_peer(peer_id_str.clone(), synthetic_ip, false);
        info!("👋 Peer connected: {} (eclipse: tracked as {})", peer_id, synthetic_ip);

        // Update global peer count
        luxtensor_rpc::peer_count::increment_peer_count();
        let current_peer_count = luxtensor_rpc::peer_count::get_peer_count();

        // Update monitors
        self.liveness_monitor.write().update_peer_count(current_peer_count);
        self.health_monitor.write().update_peer_count(current_peer_count);

        // Request sync when peer connects
        let my_height = self
            .storage
            .get_best_height()
            .unwrap_or(Some(0))
            .unwrap_or(0);
        if let Some(ref tx) = self.broadcast_tx {
            if let Err(e) = tx
                .send(SwarmCommand::RequestSync {
                    from_height: my_height + 1,
                    to_height: my_height + 100,
                    my_id: self.node_name.clone(),
                })
                .await
            {
                warn!("Failed to send sync request on peer connect: {}", e);
            }
            info!("🔄 Requesting sync from height {}", my_height + 1);
        }
    }

    // ========================================================================
    // Handler: PeerDisconnected
    // ========================================================================

    fn handle_peer_disconnected(&self, peer_id: luxtensor_network::PeerId) {
        self.eclipse_protection.remove_peer(&peer_id.to_string());
        info!("👋 Peer disconnected: {}", peer_id);

        luxtensor_rpc::peer_count::decrement_peer_count();
        let current_peer_count = luxtensor_rpc::peer_count::get_peer_count();

        self.liveness_monitor.write().update_peer_count(current_peer_count);
        self.health_monitor.write().update_peer_count(current_peer_count);
    }

    // ========================================================================
    // Handler: SyncRequest
    // ========================================================================

    async fn handle_sync_request(&self, from_height: u64, to_height: u64, requester_id: String) {
        let max_blocks_per_response = 50u64;
        let capped_to = to_height.min(from_height + max_blocks_per_response - 1);
        debug!(
            "🔄 Got sync request from {} for blocks {}-{} (capped at {})",
            requester_id, from_height, to_height, capped_to
        );

        let mut blocks_to_send = Vec::new();
        for h in from_height..=capped_to {
            if let Ok(Some(block)) = self.storage.get_block_by_height(h) {
                blocks_to_send.push(block);
            }
        }

        if !blocks_to_send.is_empty() {
            let first_h = blocks_to_send.first().map(|b| b.header.height).unwrap_or(0);
            let last_h = blocks_to_send.last().map(|b| b.header.height).unwrap_or(0);
            debug!(
                "📤 Sending {} blocks (#{}-#{}) to {}",
                blocks_to_send.len(),
                first_h,
                last_h,
                requester_id
            );
            if let Some(ref tx) = self.broadcast_tx {
                if let Err(e) = tx
                    .send(SwarmCommand::SendBlocks {
                        blocks: blocks_to_send,
                    })
                    .await
                {
                    warn!("Failed to send blocks in sync response: {}", e);
                }
            }
        } else {
            debug!("📭 No blocks found for range {}-{}", from_height, capped_to);
        }
    }
}

//! Node startup orchestration extracted from `service.rs`.
//!
//! Contains `NodeService::start` which initialises all sub-systems:
//! P2P swarm, RPC server, WebSocket server, block production loop,
//! AI Task Dispatcher and the periodic sync task.

use crate::service::{detect_external_ip, NodeService, MAX_BLOCK_CLOCK_DRIFT_SECS};
use crate::task_dispatcher::DispatchService;

use anyhow::Result;
use dashmap::DashMap;
use luxtensor_core::Transaction;
use luxtensor_network::{
    get_seeds_for_chain, print_connection_info, NodeIdentity, SwarmCommand, SwarmP2PEvent,
    SwarmP2PNode,
};
use luxtensor_rpc::RpcServer;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

impl NodeService {
    /// Start all node services
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting node services...");

        // Create shared UnifiedMempool for transaction bridge (RPC + block production)
        let rpc_mempool = Arc::new(luxtensor_core::UnifiedMempool::new(
            self.config.mempool.max_size,
            self.config.node.chain_id as u64,
        ));

        // ============================================================
        // Create shared UnifiedStateDB for RPC state layer (shared by
        // block production, P2P handler, and RPC server so all three
        // can sync_from_state_db and eth_getBalance returns fresh data).
        // ============================================================
        let shared_unified_state: Arc<parking_lot::RwLock<luxtensor_core::UnifiedStateDB>> =
            Arc::new(parking_lot::RwLock::new(luxtensor_core::UnifiedStateDB::new(
                self.config.node.chain_id as u64,
            )));

        // ============================================================
        // Create shared pending_txs for unified TX storage (RPC + P2P)
        // ============================================================
        let shared_pending_txs: Arc<DashMap<luxtensor_core::Hash, Transaction>> =
            Arc::new(DashMap::new());

        // ============================================================
        // PHASE 1: Start P2P Swarm FIRST (to get command channel)
        // ============================================================
        info!("🌐 Starting P2P Swarm network...");
        let (p2p_event_tx, mut p2p_event_rx) = mpsc::channel::<SwarmP2PEvent>(4096);

        // NOTE: RPC→P2P transaction relay is now handled directly by SwarmBroadcaster
        // which sends transactions to the P2P swarm via the command channel.
        // The previously-unused mpsc channel has been removed.

        // Load or generate persistent node identity (Peer ID)
        let node_key_path = self
            .config
            .network
            .node_key_path
            .clone()
            .unwrap_or_else(|| self.config.node.data_dir.join("node.key"));
        let node_key_path_str = node_key_path.to_string_lossy().to_string();

        let node_identity = match NodeIdentity::load_or_generate(&node_key_path_str) {
            Ok(id) => {
                info!("🔑 Node Identity loaded");
                info!("   Peer ID: {}", id.peer_id_string());
                id
            }
            Err(e) => {
                warn!("⚠️ Failed to load node identity: {}. Using random ID.", e);
                NodeIdentity::generate_new()?
            }
        };

        // Print connection info for other nodes
        let peer_id_str = node_identity.peer_id_string();
        print_connection_info(
            &peer_id_str,
            self.config.network.listen_port,
            detect_external_ip().as_deref(),
        );

        // Create swarm with persistent identity
        let keypair = node_identity.into_keypair();

        // Get bootstrap nodes: config > hardcoded seeds > empty (use mDNS)
        let bootstrap_nodes = if !self.config.network.bootstrap_nodes.is_empty() {
            info!("📡 Using bootstrap nodes from config");
            self.config.network.bootstrap_nodes.clone()
        } else {
            let hardcoded = get_seeds_for_chain(self.config.node.chain_id);
            if !hardcoded.is_empty() {
                info!(
                    "📡 Using {} hardcoded seed node(s) for chain {}",
                    hardcoded.len(),
                    self.config.node.chain_id
                );
                hardcoded
            } else {
                info!("📡 No bootstrap nodes configured, using mDNS discovery");
                vec![]
            }
        };

        let enable_mdns = self.config.network.enable_mdns;

        match SwarmP2PNode::with_keypair(
            self.config.network.listen_port,
            p2p_event_tx,
            keypair,
            bootstrap_nodes.clone(),
            enable_mdns,
        )
        .await
        {
            Ok((mut swarm_node, command_tx)) => {
                info!("  ✓ P2P Swarm started");
                info!("    Listen port: {}", self.config.network.listen_port);
                if enable_mdns {
                    info!("    mDNS discovery: enabled");
                }
                if !bootstrap_nodes.is_empty() {
                    info!("    Bootstrap nodes: {}", bootstrap_nodes.len());
                }

                // Save broadcast_tx for block production
                self.broadcast_tx = Some(command_tx.clone());

                // 🔧 FIX: Run swarm in tokio::spawn (same runtime as RPC)
                // This ensures channels work correctly between tasks
                // 🔧 FIX: Track swarm JoinHandle in self.tasks so it is awaited on shutdown
                let swarm_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
                    swarm_node.run().await;
                    // 🔧 FIX #19: Log if swarm exits unexpectedly
                    tracing::error!(
                        "🚨 CRITICAL: P2P swarm event loop exited — node is now isolated!"
                    );
                    Ok(())
                });
                self.tasks.push(swarm_handle);

                // Start P2P event handler
                let storage_for_p2p = self.storage.clone();
                let broadcast_tx_for_sync = self.broadcast_tx.clone();
                let node_name = self.config.node.name.clone();
                let shared_pending_txs_for_p2p = shared_pending_txs.clone(); // Shared TX storage
                let eclipse_protection_for_p2p = self.eclipse_protection.clone(); // Eclipse attack protection
                let long_range_protection_for_p2p = self.long_range_protection.clone(); // Long-range attack protection
                let liveness_monitor_for_p2p = self.liveness_monitor.clone(); // Liveness monitoring
                let fast_finality_for_p2p = self.fast_finality.clone(); // Fast finality
                let fork_choice_for_p2p = self.fork_choice.clone(); // Fork choice
                let mempool_for_p2p = self.mempool.clone(); // Mempool for P2P txs
                let rpc_mempool_for_p2p = rpc_mempool.clone(); // RPC UnifiedMempool for P2P → RPC bridge
                let health_monitor_for_p2p = self.health_monitor.clone(); // Health monitoring
                let rate_limiter_for_p2p = self.network_rate_limiter.clone(); // Network rate limiter
                // 🔧 FIX: Clone shared UnifiedStateDB so P2P handler can sync after block execution
                let unified_state_for_p2p = shared_unified_state.clone();
                                                                              // 🔧 FIX #6: Clone state_db and executor for P2P block state execution
                let state_db_for_p2p = self.state_db.clone();
                let executor_for_p2p = self.executor.clone();
                // 🔐 SECURITY: Clone consensus for validator-set membership check on incoming blocks
                let consensus_for_p2p = self.consensus.clone();
                // 🎲 VRF: epoch_length needed for VRF alpha construction
                let epoch_length_for_p2p = self.epoch_length;
                // 🔧 FIX #META: Clone metagraph_db so P2P handler can sync neurons/validators/subnets
                // after receiving blocks. Without this, only the block-producing node's MetagraphDB
                // reflects registrations — non-producers' Yuma computation returns zeroes.
                let _metagraph_db_for_p2p = self.metagraph_db.clone();
                // 🔧 FIX #9: Atomic height guard to prevent block height race between
                // P2P handler and block production (both reading/writing at the same height)
                let best_height_guard = self.best_height_guard.clone();
                let best_height_for_p2p = best_height_guard.clone();
                let _best_height_for_block_prod_p2p = best_height_guard.clone();
                let is_syncing_for_p2p = self.is_syncing.clone();
                let event_task = tokio::spawn(async move {
                    while let Some(event) = p2p_event_rx.recv().await {
                        match event {
                            SwarmP2PEvent::NewBlock(block) => {
                                let height = block.header.height;
                                let block_hash = block.hash();

                                // 🛡️ Rate-limit: only for blocks far ahead of our chain tip.
                                // Sync batches deliver many historical blocks at once (same proposer)
                                // which would be incorrectly rate-limited.
                                let current_best = best_height_for_p2p
                                    .load(std::sync::atomic::Ordering::Relaxed);
                                let proposer_id = hex::encode(&block.header.validator);
                                if height > current_best + 100 && !rate_limiter_for_p2p.check(&proposer_id) {
                                    warn!(
                                        "🛡️ Block #{} rate-limited from proposer {}",
                                        height, proposer_id
                                    );
                                    continue;
                                }

                                // 🛡️ Long-range attack protection: validate against checkpoints
                                if !long_range_protection_for_p2p
                                    .validate_against_checkpoints(block_hash, height)
                                {
                                    warn!("🛡️ Block #{} rejected: checkpoint mismatch (potential long-range attack)", height);
                                    continue;
                                }

                                // Check if we already have this block
                                if storage_for_p2p
                                    .get_block_by_height(height)
                                    .ok()
                                    .flatten()
                                    .is_some()
                                {
                                    debug!("Already have block #{}, skipping", height);
                                    continue;
                                }

                                // Check weak subjectivity
                                if !long_range_protection_for_p2p
                                    .is_within_weak_subjectivity(height)
                                {
                                    warn!(
                                        "🛡️ Block #{} rejected: outside weak subjectivity window",
                                        height
                                    );
                                    continue;
                                }

                                // ====================================================================
                                // 🔐 BLOCK VALIDATION — verify before storing (CRITICAL)
                                // ====================================================================

                                // 1. Validate sequential height
                                let my_height = storage_for_p2p
                                    .get_best_height()
                                    .unwrap_or(Some(0))
                                    .unwrap_or(0);
                                if height > my_height + 1 {
                                    // Gap — request sync instead of storing out-of-order block
                                    // 🔧 FIX: Cap sync range to MAX_SYNC_RANGE (1000) to avoid
                                    // oversized sync rejection. The periodic sync will continue
                                    // requesting subsequent chunks until caught up.
                                    const MAX_SYNC_RANGE: u64 = 1000;
                                    let sync_to = (my_height + MAX_SYNC_RANGE).min(height);
                                    debug!(
                                        "Block #{} is ahead of our height {}, requesting sync {}-{}",
                                        height, my_height, my_height + 1, sync_to
                                    );
                                    if let Some(ref tx) = broadcast_tx_for_sync {
                                        if let Err(e) = tx
                                            .send(SwarmCommand::RequestSync {
                                                from_height: my_height + 1,
                                                to_height: sync_to,
                                                my_id: node_name.clone(),
                                            })
                                            .await
                                        {
                                            warn!("Failed to send sync request: {}", e);
                                        }
                                    }
                                    continue;
                                }
                                if height <= my_height {
                                    debug!(
                                        "Block #{} is not newer than our height {}",
                                        height, my_height
                                    );
                                    continue;
                                }

                                // 2. Validate previous_hash chain link
                                if let Ok(Some(prev_block)) =
                                    storage_for_p2p.get_block_by_height(my_height)
                                {
                                    if block.header.previous_hash != prev_block.hash() {
                                        warn!("🚫 Block #{} rejected: previous_hash mismatch (expected {:?}, got {:?})",
                                            height, &prev_block.hash()[..4], &block.header.previous_hash[..4]);
                                        continue;
                                    }

                                    // 2b. SECURITY: Validate timestamp monotonicity against parent
                                    if block.header.timestamp < prev_block.header.timestamp {
                                        warn!("🚫 Block #{} rejected: timestamp regression ({} < parent {})",
                                            height, block.header.timestamp, prev_block.header.timestamp);
                                        continue;
                                    }
                                }

                                // 3. Validate block signature (if validator field is set)
                                if block.header.validator != [0u8; 32]
                                    && !block.header.signature.is_empty()
                                {
                                    // Extract the 20-byte address from the 32-byte validator field
                                    let validator_addr = &block.header.validator[12..32];
                                    // Reconstruct unsigned header for hash verification
                                    let mut unsigned_header = block.header.clone();
                                    let sig_backup = unsigned_header.signature.clone();
                                    unsigned_header.signature = vec![];
                                    let header_hash = unsigned_header.hash();

                                    // Verify signature using ECDSA recovery
                                    if sig_backup.len() >= 64 {
                                        match luxtensor_crypto::recover_address(
                                            &header_hash,
                                            &sig_backup,
                                        ) {
                                            Ok(recovered_addr) => {
                                                if recovered_addr.as_bytes() != validator_addr {
                                                    warn!("🚫 Block #{} rejected: invalid validator signature (recovered {:?} != expected {:?})",
                                                        height, &recovered_addr.as_bytes()[..4], &validator_addr[..4]);
                                                    continue;
                                                }
                                            }
                                            Err(_) => {
                                                warn!("🚫 Block #{} rejected: signature recovery failed", height);
                                                continue;
                                            }
                                        }
                                    }
                                }

                                // 3b. 🔐 SECURITY: Validate proposer is a registered active validator
                                // Reject blocks from unknown or deactivated validators to prevent
                                // unauthorized block production (C3 fix)
                                if block.header.validator != [0u8; 32] {
                                    let validator_addr_bytes = &block.header.validator[12..32];
                                    let mut addr_20 = [0u8; 20];
                                    addr_20.copy_from_slice(validator_addr_bytes);
                                    let proposer_addr = luxtensor_core::Address::from(addr_20);

                                    let is_active_validator = {
                                        let pos = consensus_for_p2p.read();
                                        let vs = pos.validator_set();
                                        let vs_lock = vs.read();
                                        vs_lock.active_validators()
                                            .iter()
                                            .any(|v| v.address == proposer_addr)
                                    };

                                    if !is_active_validator {
                                        warn!("🚫 Block #{} rejected: proposer {:?} is not an active validator",
                                            height, &addr_20[..4]);
                                        continue;
                                    }

                                    // 3c. 🎲 VRF proof verification (C2 security fix)
                                    // If block carries a VRF proof, verify it against the proposer's
                                    // public key. Reject blocks with invalid/forged VRF proofs.
                                    if let Some(ref vrf_bytes) = block.header.vrf_proof {
                                        if vrf_bytes.len() == 97 {
                                            let vrf_valid = {
                                                let pos = consensus_for_p2p.read();
                                                let vs = pos.validator_set();
                                                let vs_lock = vs.read();
                                                if let Some(validator) = vs_lock.get_validator(&proposer_addr) {
                                                    let mut proof_arr = [0u8; 97];
                                                    proof_arr.copy_from_slice(vrf_bytes);
                                                    match luxtensor_crypto::vrf::VrfProof::from_bytes(&proof_arr) {
                                                        Ok(proof) => {
                                                            let epoch = height / epoch_length_for_p2p.max(1);
                                                            let mut alpha = Vec::with_capacity(48);
                                                            alpha.extend_from_slice(&epoch.to_le_bytes());
                                                            alpha.extend_from_slice(&height.to_le_bytes());
                                                            alpha.extend_from_slice(&block.header.previous_hash);
                                                            luxtensor_crypto::vrf::vrf_verify(
                                                                &validator.public_key, &alpha, &proof
                                                            ).is_ok()
                                                        }
                                                        Err(_) => false,
                                                    }
                                                } else {
                                                    false // Already rejected by C3 check above
                                                }
                                            };
                                            if !vrf_valid {
                                                warn!("🚫 Block #{} rejected: invalid VRF proof", height);
                                                continue;
                                            }
                                            debug!("✅ VRF proof verified for block #{}", height);
                                        } else {
                                            warn!("🚫 Block #{} rejected: VRF proof has wrong size ({} bytes, expected 97)",
                                                height, vrf_bytes.len());
                                            continue;
                                        }
                                    }
                                }

                                // 4. Validate txs_root (Merkle root of transactions)
                                let tx_hashes: Vec<[u8; 32]> =
                                    block.transactions.iter().map(|tx| tx.hash()).collect();
                                let expected_txs_root = if tx_hashes.is_empty() {
                                    [0u8; 32]
                                } else {
                                    luxtensor_crypto::MerkleTree::new(tx_hashes).root()
                                };
                                if block.header.txs_root != expected_txs_root {
                                    warn!("🚫 Block #{} rejected: txs_root mismatch", height);
                                    continue;
                                }

                                // 5. Validate reasonable timestamp (not too far in future)
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                if block.header.timestamp > now + MAX_BLOCK_CLOCK_DRIFT_SECS {
                                    warn!("🚫 Block #{} rejected: timestamp {} is too far in the future (now={})",
                                        height, block.header.timestamp, now);
                                    continue;
                                }

                                // 6. Full structural validation via Block::validate()
                                if let Err(e) = block.validate() {
                                    warn!(
                                        "🚫 Block #{} rejected: validate() failed: {}",
                                        height, e
                                    );
                                    continue;
                                }

                                // 🔧 FIX #9 (revised): Sequential height check instead of CAS
                                // CAS was silently rejecting blocks that arrived out-of-order
                                // during sync batches. Simple check is safe because P2P ingestion
                                // runs in a single task — no concurrent writes from this path.
                                let my_height = best_height_for_p2p
                                    .load(std::sync::atomic::Ordering::SeqCst);
                                if height != my_height + 1 {
                                    debug!("Block #{} out of order (current={}), skipping", height, my_height);
                                    continue;
                                }

                                if let Err(e) = storage_for_p2p.store_block(&block) {
                                    warn!("Failed to store received block: {}", e);
                                } else {
                                    best_height_for_p2p.store(
                                        height,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    info!("📥 Synced block #{} from peer", height);

                                    // 🔧 FIX: After syncing a block, check if we're caught up
                                    // If no more blocks are expected, resume block production
                                    // We consider synced when we've stored a block and it's
                                    // within 1 of what peers report
                                    if is_syncing_for_p2p.load(std::sync::atomic::Ordering::SeqCst) {
                                        info!("🔄 Sync progress: stored block #{}, resuming production...", height);
                                        is_syncing_for_p2p.store(false, std::sync::atomic::Ordering::SeqCst);
                                    }

                                    // 🔧 FIX #21: Remove confirmed txs from pending pool to prevent ghost entries
                                    {
                                        let tx_hashes: Vec<[u8; 32]> =
                                            block.transactions.iter().map(|tx| tx.hash()).collect();
                                        for hash in &tx_hashes {
                                            shared_pending_txs_for_p2p.remove(hash);
                                        }
                                        mempool_for_p2p.remove_transactions(&tx_hashes);
                                    }

                                    // ✅ MetagraphDB is now updated via Metagraph Precompile transactions.
                                    // When neuron_register/subnet_create/staking_registerValidator is called,
                                    // the RPC handler writes a MetagraphTx (to=PRECOMPILE_METAGRAPH) into
                                    // the mempool. The block producer includes this tx in the next block.
                                    // ALL nodes (including P2P receivers here) execute the tx via executor,
                                    // which calls execute_metagraph_precompile() → MetagraphDB.
                                    // No O(N) scan needed. Replaced sync_from_blockchain() call.

                                    // 🔧 FIX #6: Execute transactions against StateDB for P2P-received blocks
                                    // Previously only the block producer executed txs, causing state divergence
                                    // on non-validator nodes (incorrect RPC balances/nonces)
                                    //
                                    // SECURITY: Split lock scope — write lock only during TX execution,
                                    // read lock for disk flush to minimize RPC query starvation.
                                    {
                                        let mut state = state_db_for_p2p.write();
                                        for (tx_index, tx) in block.transactions.iter().enumerate()
                                        {
                                            if let Err(e) = executor_for_p2p.execute(
                                                tx,
                                                &mut state,
                                                height,
                                                block_hash,
                                                tx_index,
                                                block.header.timestamp,
                                            ) {
                                                debug!("P2P block #{} tx {} execution failed: {} (may be expected for already-applied state)", height, tx_index, e);
                                            }
                                        }
                                        // Drop write lock before disk I/O
                                    }
                                    // Persist state with read lock only (flush_to_db takes &self)
                                    {
                                        let state = state_db_for_p2p.read();
                                        if let Err(e) = state.flush_to_db(storage_for_p2p.as_ref())
                                        {
                                            warn!("Failed to persist P2P block state: {}", e);
                                        }

                                        // 🔧 FIX: Sync UnifiedStateDB after P2P block execution
                                        // so eth_getBalance returns correct balances on receiving nodes.
                                        // Without this, only block-producing nodes see balance updates.
                                        {
                                            let mut unified = unified_state_for_p2p.write();
                                            unified.sync_from_state_db(&state, height);
                                            debug!("📊 P2P: UnifiedStateDB synced to height {}", height);
                                        }
                                    }

                                    // 🔗 Feed block to ForkChoice (GHOST) for canonical chain tracking
                                    if let Err(e) =
                                        fork_choice_for_p2p.read().add_block(block.clone())
                                    {
                                        debug!("ForkChoice: {}", e);
                                    }

                                    // 🔐 FastFinality: record the block producer's attestation
                                    if block.header.validator != [0u8; 32] {
                                        let mut validator_addr = [0u8; 20];
                                        validator_addr
                                            .copy_from_slice(&block.header.validator[12..32]);
                                        let addr = luxtensor_core::Address::from(validator_addr);
                                        let finalized = fast_finality_for_p2p
                                            .write()
                                            .add_signature(block_hash, height, addr);
                                        match finalized {
                                            Ok(true) => {
                                                info!("⚡ Block #{} reached fast finality!", height)
                                            }
                                            Ok(false) => debug!(
                                                "Block #{} collecting finality signatures",
                                                height
                                            ),
                                            Err(e) => debug!("FastFinality skipped: {}", e),
                                        }
                                    }

                                    // 🎯 Record block for liveness monitoring
                                    liveness_monitor_for_p2p.write().record_block(height);

                                    // 🏥 Update health monitor with block height
                                    health_monitor_for_p2p.write().update_block_height(height);

                                    // Update finalized state for blocks past confirmation threshold
                                    let finality_depth = 32; // Same as min_finality_confirmations
                                    if height > finality_depth {
                                        let finalized_height = height - finality_depth;
                                        if let Ok(Some(finalized_block)) =
                                            storage_for_p2p.get_block_by_height(finalized_height)
                                        {
                                            long_range_protection_for_p2p.update_finalized(
                                                finalized_block.hash(),
                                                finalized_height,
                                                finalized_block.header.state_root,
                                            );
                                        }
                                    }
                                }
                            }
                            SwarmP2PEvent::NewTransaction(tx) => {
                                // 🛡️ Rate-limit: check per-sender message rate
                                let sender_id = hex::encode(&tx.from);
                                if !rate_limiter_for_p2p.check(&sender_id) {
                                    warn!("🛡️ Transaction rate-limited from sender {}", sender_id);
                                    continue;
                                }

                                // 🚀 Add received TX to shared pending_txs for RPC query
                                // SECURITY FIX: Validate via mempool FIRST, then insert into
                                // shared_pending_txs only if accepted. This prevents invalid
                                // transactions from polluting the shared pool.
                                let tx_hash = tx.hash();
                                {
                                    if shared_pending_txs_for_p2p.contains_key(&tx_hash) {
                                        // Already in pool, skip duplicate
                                        continue;
                                    }
                                }

                                // Validate through mempool first
                                match mempool_for_p2p.add_transaction(tx.clone()) {
                                    Ok(_) => {
                                        // Mempool accepted — now add to shared pending pool
                                        shared_pending_txs_for_p2p.insert(tx_hash, tx.clone());
                                        // Also add to RPC UnifiedMempool so eth_getTransactionByHash can find it
                                        if let Err(e) = rpc_mempool_for_p2p.add_transaction(tx) {
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
                            SwarmP2PEvent::PeerConnected(peer_id) => {
                                // 🛡️ Eclipse Protection: track peer for subnet diversity analysis.
                                // libp2p PeerConnected events don't carry real IPs, so we use a
                                // deterministic synthetic IP derived from PeerId hash. This enables:
                                // - Subnet diversity tracking (detect /16 and /24 concentration)
                                // - Peer rotation (evict low-score or stale peers)
                                // - Behavior scoring via update_peer_score()
                                let peer_id_str = peer_id.to_string();
                                let synthetic_ip = crate::service::peer_id_to_synthetic_ip(&peer_id_str);
                                eclipse_protection_for_p2p.add_peer(
                                    peer_id_str.clone(),
                                    synthetic_ip,
                                    false, // inbound — we don't know directionality from this event
                                );
                                info!(
                                    "👋 Peer connected: {} (eclipse: tracked as {})",
                                    peer_id, synthetic_ip
                                );

                                // Update global peer count for RPC
                                luxtensor_rpc::peer_count::increment_peer_count();

                                // 🎯 Update liveness monitor with current peer count
                                let current_peer_count =
                                    luxtensor_rpc::peer_count::get_peer_count();
                                liveness_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);

                                // 🏥 Update health monitor with peer count
                                health_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);

                                // Request sync when peer connects
                                let my_height = storage_for_p2p
                                    .get_best_height()
                                    .unwrap_or(Some(0))
                                    .unwrap_or(0);
                                if let Some(ref tx) = broadcast_tx_for_sync {
                                    // Request blocks we don't have (up to 100 ahead)
                                    if let Err(e) = tx
                                        .send(SwarmCommand::RequestSync {
                                            from_height: my_height + 1,
                                            to_height: my_height + 100,
                                            my_id: node_name.clone(),
                                        })
                                        .await
                                    {
                                        warn!("Failed to send sync request on peer connect: {}", e);
                                    }
                                    info!("🔄 Requesting sync from height {}", my_height + 1);
                                }
                            }
                            SwarmP2PEvent::PeerDisconnected(peer_id) => {
                                // 🛡️ Remove peer from Eclipse Protection tracking
                                eclipse_protection_for_p2p.remove_peer(&peer_id.to_string());
                                info!("👋 Peer disconnected: {}", peer_id);
                                // Update global peer count for RPC
                                luxtensor_rpc::peer_count::decrement_peer_count();

                                // 🎯 Update liveness monitor with current peer count
                                let current_peer_count =
                                    luxtensor_rpc::peer_count::get_peer_count();
                                liveness_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);

                                // 🏥 Update health monitor with peer count
                                health_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);
                            }
                            SwarmP2PEvent::SyncRequest { from_height, to_height, requester_id } => {
                                // Cap sync response to prevent memory exhaustion
                                let max_blocks_per_response = 50u64;
                                let capped_to =
                                    to_height.min(from_height + max_blocks_per_response - 1);
                                debug!(
                                    "🔄 Got sync request from {} for blocks {}-{} (capped at {})",
                                    requester_id, from_height, to_height, capped_to
                                );
                                // Collect blocks we have in range
                                let mut blocks_to_send = Vec::new();
                                for h in from_height..=capped_to {
                                    if let Ok(Some(block)) = storage_for_p2p.get_block_by_height(h)
                                    {
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
                                    if let Some(ref tx) = broadcast_tx_for_sync {
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
                    }
                    // 🔧 FIX #22: Log when P2P event handler exits (channel closed = swarm dropped or shutdown)
                    tracing::info!("📡 P2P event handler loop exited (channel closed)");
                    Ok::<(), anyhow::Error>(())
                });
                self.tasks.push(event_task);

                // ============================================================
                // PERIODIC SYNC TASK: Retry sync every 10 seconds
                // This ensures late-joining nodes can sync even if initial
                // sync request fails due to InsufficientPeers
                // ============================================================
                let sync_command_tx = command_tx.clone();
                let sync_storage = self.storage.clone();
                let sync_node_name = self.config.node.name.clone();
                let is_syncing_for_periodic = self.is_syncing.clone();
                let sync_task = tokio::spawn(async move {
                    let mut last_sync_height = 0u64;
                    let mut sync_interval_secs = 10u64;
                    let mut consecutive_no_progress = 0u32;
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(sync_interval_secs)).await;

                        // Check current height from storage
                        let my_height =
                            sync_storage.get_best_height().unwrap_or(Some(0)).unwrap_or(0);

                        if my_height > last_sync_height {
                            // Made progress since last check → stay aggressive
                            consecutive_no_progress = 0;
                            sync_interval_secs = 10;
                        } else {
                            // No progress → backoff: 10 → 20 → 40 → 60 (cap)
                            consecutive_no_progress += 1;
                            sync_interval_secs = (10u64 * 2u64.saturating_pow(consecutive_no_progress.min(3))).min(60);
                        }

                        // Only request sync if we've made no progress since last check
                        if my_height == last_sync_height {
                            let batch_size = 50u64;
                            if let Err(e) = sync_command_tx
                                .send(SwarmCommand::RequestSync {
                                    from_height: my_height + 1,
                                    to_height: my_height + batch_size,
                                    my_id: sync_node_name.clone(),
                                })
                                .await
                            {
                                warn!("Failed to send periodic sync request: {}", e);
                            }

                            if my_height == 0 {
                                info!("🔄 Initial sync: requesting blocks 1-{}...", batch_size);
                            } else {
                                debug!(
                                    "🔄 Periodic sync check: height={}, next check in {}s",
                                    my_height, sync_interval_secs
                                );
                            }
                        }
                        last_sync_height = my_height;

                        // 🔧 FIX: Timeout for syncing with no progress
                        // Case 1: Solo mode — no peers after 1 check → start producing
                        // Case 2: All-fresh network — peers connected but nobody
                        //         sent us any blocks (my_height still 0) → bootstrap
                        // Case 3: Node rejoining with existing data — no new blocks
                        //         received after 2 checks → already up-to-date, resume
                        let peer_count = luxtensor_rpc::peer_count::get_peer_count();
                        if is_syncing_for_periodic.load(std::sync::atomic::Ordering::SeqCst) {
                            if consecutive_no_progress >= 1 && peer_count == 0 {
                                info!("⏰ Solo mode: no peers, resuming block production");
                                is_syncing_for_periodic.store(false, std::sync::atomic::Ordering::SeqCst);
                            } else if consecutive_no_progress >= 1 && my_height == 0 && peer_count > 0 {
                                // Peers are connected but none of them has blocks to offer.
                                // This is a fresh network bootstrap scenario — start producing.
                                info!(
                                    "⏰ Fresh network: {} peer(s) connected but no blocks after {}s — bootstrapping",
                                    peer_count, sync_interval_secs
                                );
                                is_syncing_for_periodic.store(false, std::sync::atomic::Ordering::SeqCst);
                            } else if consecutive_no_progress >= 1 && my_height > 0 {
                                // 🔧 FIX: Node has existing data (my_height > 0) and has received
                                // no new blocks after 1 check (10 seconds). This means we are already
                                // at or near the tip — resume production immediately.
                                // Previously this required consecutive_no_progress >= 2 AND peer_count > 0,
                                // which caused a 20-40s pause loop on every restart even when fully synced.
                                info!(
                                    "⏰ Already synced: height={}, {} peer(s), no new blocks after {}s — resuming",
                                    my_height, peer_count, sync_interval_secs
                                );
                                is_syncing_for_periodic.store(false, std::sync::atomic::Ordering::SeqCst);
                            }
                        }
                    }
                });
                self.tasks.push(sync_task);
            }
            Err(e) => {
                warn!("Failed to start P2P Swarm: {}. Running in standalone mode.", e);
            }
        }

        // ============================================================
        // PHASE 2: Start RPC server WITH DIRECT Swarm broadcaster
        // ============================================================
        // Shared unified_state is created at startup and injected into RPC server.
        // Block production uses the same instance via unified_state_for_blocks.
        let unified_state_for_blocks: Option<
            Arc<parking_lot::RwLock<luxtensor_core::UnifiedStateDB>>,
        > = Some(shared_unified_state.clone());

        if self.config.rpc.enabled {
            info!("🔌 Starting RPC server with direct Swarm broadcaster...");

            // Use command_tx directly from P2P swarm (bypassing tx_relay task)
            let broadcaster: Arc<dyn luxtensor_rpc::TransactionBroadcaster> =
                match &self.broadcast_tx {
                    Some(cmd_tx) => {
                        Arc::new(crate::swarm_broadcaster::SwarmBroadcaster::new(cmd_tx.clone()))
                    }
                    None => {
                        warn!("No P2P swarm available, using NoOp broadcaster");
                        Arc::new(luxtensor_rpc::NoOpBroadcaster)
                    }
                };

            // Use shared pending_txs for unified TX storage between RPC and P2P
            // 🔧 FIX: Pass config chain_id instead of hardcoded 1337
            let mut rpc_server = RpcServer::new_with_shared_pending_txs(
                self.storage.clone(),
                rpc_mempool.clone(),
                broadcaster,
                shared_pending_txs.clone(),
                self.config.node.chain_id as u64,
            );

            // Wire optional subsystems into the RPC server
            rpc_server.set_bridge(self.bridge.clone());
            rpc_server.set_multisig_manager(self.multisig_manager.clone());
            rpc_server.set_merkle_cache(self.merkle_cache.clone());

            // Wire shared EVM executor for eth_call storage reads.
            // Clone shares the underlying Arc<RwLock<..>> state, so eth_call
            // reads the same storage that block execution has committed to.
            rpc_server.set_evm_executor(self.executor.evm().clone());

            // Wire NodeMetrics → RPC via callback closures
            {
                let metrics = self.metrics.clone();
                let json_fn = Arc::new(move || metrics.to_json());
                let metrics2 = self.metrics.clone();
                let prom_fn = Arc::new(move || metrics2.export());
                rpc_server.set_metrics_provider(json_fn, prom_fn);
            }

            // Wire HealthMonitor → RPC via callback closure
            {
                let hm = self.health_monitor.clone();
                let health_fn = Arc::new(move || {
                    let status = hm.read().get_health();
                    serde_json::json!({
                        "healthy": status.healthy,
                        "block": status.block_height,
                        "peerCount": status.peer_count,
                        "is_syncing": status.is_syncing,
                        "syncProgress": status.sync_progress,
                        "secondsSinceLastBlock": status.seconds_since_last_block,
                        "mempoolSize": status.mempool_size,
                        "uptimeSeconds": status.uptime_seconds,
                        "issues": status.issues.iter().map(|i| {
                            serde_json::json!({
                                "type": format!("{:?}", i),
                                "severity": i.severity(),
                                "critical": i.is_critical()
                            })
                        }).collect::<Vec<_>>(),
                        "version": "0.1.0",
                        "node_name": "luxtensor-node"
                    })
                });
                rpc_server.set_health_provider(health_fn);
            }

            // 🔧 FIX: Inject shared UnifiedStateDB into RPC server so P2P handler,
            // block production, and RPC all share the same state instance.
            rpc_server.set_unified_state(shared_unified_state.clone());

            // Wire shared RewardExecutor into RPC so rewards_getPending, rewards_getStats,
            // rewards_claim etc. query the same state that block production updates.
            rpc_server.set_reward_executor(self.reward_executor.clone());

            // 🔧 FIX: Inject the SAME MetagraphDB instance that NodeService / Yuma uses
            // into the RPC server. Without this, staking_registerValidator and neuron_register
            // write into a temp/<PID> DB while Yuma reads from data_dir/metagraph → all
            // validators appear missing and metrics stay at 0.
            rpc_server.set_metagraph(self.metagraph_db.clone());


            let addr = format!("{}:{}", self.config.rpc.listen_addr, self.config.rpc.listen_port);
            let rpc_threads = self.config.rpc.threads;
            let rpc_cors_origins = self.config.rpc.cors_origins.clone();

            // 🔧 FIX: Use shutdown_rx instead of a second ctrl_c handler.
            // Previously both this task and wait_for_shutdown() raced on ctrl_c,
            // requiring 2× Ctrl+C to stop the node.
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            let task = tokio::spawn(async move {
                info!("  ✓ RPC server listening on {}", addr);
                match rpc_server.start(&addr, rpc_threads, &rpc_cors_origins) {
                    Ok(_server) => {
                        info!("RPC server started successfully");
                        // Keep server alive until shutdown signal is received
                        let _ = shutdown_rx.recv().await;
                        info!("RPC server shutting down");
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                }
            });

            self.tasks.push(task);
        }

        // ============================================================
        // PHASE 2b: Start WebSocket server for real-time subscriptions
        // ============================================================
        if self.config.rpc.enabled && self.config.rpc.ws_enabled {
            info!("🔌 Starting WebSocket RPC server...");
            let ws_addr = format!("{}:{}", self.config.rpc.listen_addr, self.config.rpc.ws_port);
            let ws_server = luxtensor_rpc::WebSocketServer::new();

            // Store broadcast sender for block production to emit events
            let ws_broadcast_tx = ws_server.get_broadcast_sender();
            self.ws_broadcast = Some(ws_broadcast_tx);

            let task = tokio::spawn(async move {
                info!("  ✓ WebSocket RPC listening on ws://{}", ws_addr);
                if let Err(e) = ws_server.start(&ws_addr).await {
                    error!("WebSocket server error: {:?}", e);
                }
                Ok::<(), anyhow::Error>(())
            });

            self.tasks.push(task);
        }

        // Start block production if validator
        let best_height_for_block_prod = self.best_height_guard.clone();
        let is_syncing_for_block_prod = self.is_syncing.clone();
        if self.config.node.is_validator {
            info!("🔨 Starting block production...");
            let consensus = self.consensus.clone();
            let storage = self.storage.clone();
            let state_db = self.state_db.clone();
            let merkle_cache = self.merkle_cache.clone();
            let mempool = self.mempool.clone();
            let executor = self.executor.clone();
            let reward_executor = self.reward_executor.clone();
            let block_time = self.config.consensus.block_time;
            let epoch_length = self.epoch_length;
            let shutdown_rx = self.shutdown_tx.subscribe();
            let rpc_mempool_for_block = rpc_mempool.clone();

            // Leader election params
            let validator_id = self
                .config
                .node
                .validator_id
                .clone()
                .unwrap_or_else(|| self.config.node.name.clone());
            let validators = self.config.consensus.validators.clone();
            let genesis_timestamp = self.genesis_timestamp;
            let broadcast_tx = self.broadcast_tx.clone();
            let chain_id = self.config.node.chain_id as u64;
            // Get our validator address for PoS leader election
            let our_validator_address = self.validator_keypair.as_ref().map(|kp| kp.address());
            // 🔧 FIX: Clone keypair for the block production closure
            let validator_keypair_for_block = self.validator_keypair.clone();
            let metagraph_db_clone = self.metagraph_db.clone();
            let unified_state_clone = unified_state_for_blocks.clone();
            let randao_clone = self.randao.clone();
            let agent_trigger_clone = self.agent_trigger_engine.clone();
            let dispute_manager_clone = self.dispute_manager.clone();
            let slashing_manager_clone = self.slashing_manager.clone();
            let fast_finality_clone = self.fast_finality.clone();
            let metrics_for_loop = self.metrics.clone();
            let ws_broadcast_for_block = self.ws_broadcast.clone();
            let halving_schedule_clone = self.halving_schedule.clone();
            let burn_manager_clone = self.burn_manager.clone();
            let fee_market_clone = self.fee_market.clone();
            let governance_clone = self.governance.clone();
            let validator_rotation_clone = self.validator_rotation.clone();
            let commit_reveal_clone = self.commit_reveal.clone();
            let scoring_manager_clone = self.scoring_manager.clone();
            let vrf_keypair_for_block = self.vrf_keypair.clone();
            let ai_circuit_breaker_clone = self.ai_circuit_breaker.clone();
            let task = tokio::spawn(async move {
                Self::block_production_loop(
                    consensus,
                    storage,
                    state_db,
                    mempool,
                    executor,
                    reward_executor,
                    block_time,
                    epoch_length,
                    shutdown_rx,
                    rpc_mempool_for_block,
                    validator_id,
                    validators,
                    genesis_timestamp,
                    broadcast_tx,
                    chain_id,
                    our_validator_address,
                    validator_keypair_for_block,
                    best_height_for_block_prod, // 🔧 FIX #9: Atomic height guard
                    is_syncing_for_block_prod,  // 🔧 FIX: Sync guard
                    metagraph_db_clone,
                    unified_state_clone, // For syncing RPC state after each block
                    randao_clone,        // RANDAO mixer for epoch finalization
                    agent_trigger_clone, // Agentic EVM triggers
                    dispute_manager_clone, // Optimistic AI dispute processing
                    slashing_manager_clone, // For dispute slashing
                    merkle_cache,        // Merkle root caching layer
                    fast_finality_clone, // BFT fast finality hook
                    metrics_for_loop,    // NodeMetrics recording
                    ws_broadcast_for_block, // WebSocket event broadcast
                    halving_schedule_clone,  // 📊 Phase 3: Halving schedule
                    burn_manager_clone,      // 📊 Phase 3: Fee burning
                    fee_market_clone,        // 📊 Phase 3: EIP-1559 dynamic pricing
                    governance_clone,        // 🏛️ Phase 4+: Governance epoch hooks
                    validator_rotation_clone, // 🔄 Phase 4+: Validator rotation
                    commit_reveal_clone,     // 🔐 Phase 4+: Commit-reveal finalization
                    scoring_manager_clone,   // 📊 Phase 5+: Performance scoring
                    vrf_keypair_for_block,   // 🎲 VRF keypair for block proofs (C2 fix)
                    ai_circuit_breaker_clone, // 🛡️ AI layer circuit breaker
                )
                .await
            });

            self.tasks.push(task);
            info!("  ✓ Block production started");
            if let Some(ref vid) = self.config.node.validator_id {
                info!("    Validator ID: {}", vid);
            }
            info!("    Known validators: {:?}", self.config.consensus.validators);
        }

        // Start AI Task Dispatcher service (DePIN workload distribution)
        {
            let dispatch_service = if let Some(ref cmd_tx) = self.broadcast_tx {
                DispatchService::with_p2p(self.task_dispatcher.clone(), cmd_tx.clone())
            } else {
                DispatchService::new(self.task_dispatcher.clone())
            };
            let dispatch_handle = tokio::spawn(async move {
                dispatch_service.start().await;
                Ok::<(), anyhow::Error>(())
            });
            self.tasks.push(dispatch_handle);
            info!("  ✓ AI Task Dispatcher service started");
        }

        info!("✅ All services started successfully");
        self.print_status();

        Ok(())
    }
}

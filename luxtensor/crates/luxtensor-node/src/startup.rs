//! Node startup orchestration extracted from `service.rs`.
//!
//! Contains `NodeService::start` which initialises all sub-systems:
//! P2P swarm, RPC server, WebSocket server, block production loop,
//! AI Task Dispatcher and the periodic sync task.

use crate::service::{detect_external_ip, NodeService};
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
        let (p2p_event_tx, p2p_event_rx) = mpsc::channel::<SwarmP2PEvent>(4096);

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
            self.config.node.chain_id as u32,
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

                // Start P2P event handler via P2PContext (extracted to p2p_handler.rs)
                let p2p_ctx = crate::p2p_handler::P2PContext {
                    storage: self.storage.clone(),
                    broadcast_tx: self.broadcast_tx.clone(),
                    node_name: self.config.node.name.clone(),
                    shared_pending_txs: shared_pending_txs.clone(),
                    eclipse_protection: self.eclipse_protection.clone(),
                    long_range_protection: self.long_range_protection.clone(),
                    liveness_monitor: self.liveness_monitor.clone(),
                    fast_finality: self.fast_finality.clone(),
                    fork_choice: self.fork_choice.clone(),
                    mempool: self.mempool.clone(),
                    rpc_mempool: rpc_mempool.clone(),
                    health_monitor: self.health_monitor.clone(),
                    rate_limiter: self.network_rate_limiter.clone(),
                    unified_state: shared_unified_state.clone(),
                    state_db: self.state_db.clone(),
                    executor: self.executor.clone(),
                    consensus: self.consensus.clone(),
                    epoch_length: self.epoch_length,
                    best_height: self.best_height_guard.clone(),
                    is_syncing: self.is_syncing.clone(),
                    merkle_cache: self.merkle_cache.clone(),
                };
                let event_task: JoinHandle<Result<()>> = tokio::spawn(async move {
                    p2p_ctx.run(p2p_event_rx).await;
                    Ok(())
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
                let mut sync_shutdown_rx = self.shutdown_tx.subscribe();
                let sync_task = tokio::spawn(async move {
                    let mut last_sync_height = 0u64;
                    let mut sync_interval_secs = 10u64;
                    let mut consecutive_no_progress = 0u32;
                    loop {
                        // H4 FIX: Check shutdown signal alongside sleep to avoid hanging on shutdown
                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_secs(sync_interval_secs)) => {}
                            _ = sync_shutdown_rx.recv() => {
                                info!("🔄 Sync task shutting down");
                                break;
                            }
                        }

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
                            sync_interval_secs = (10u64
                                * 2u64.saturating_pow(consecutive_no_progress.min(3)))
                            .min(60);
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
                                is_syncing_for_periodic
                                    .store(false, std::sync::atomic::Ordering::SeqCst);
                            } else if consecutive_no_progress >= 1
                                && my_height == 0
                                && peer_count > 0
                            {
                                // Peers are connected but none of them has blocks to offer.
                                // This is a fresh network bootstrap scenario — start producing.
                                info!(
                                    "⏰ Fresh network: {} peer(s) connected but no blocks after {}s — bootstrapping",
                                    peer_count, sync_interval_secs
                                );
                                is_syncing_for_periodic
                                    .store(false, std::sync::atomic::Ordering::SeqCst);
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
                                is_syncing_for_periodic
                                    .store(false, std::sync::atomic::Ordering::SeqCst);
                            }
                        }
                    }
                    Ok(())
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

            // 🔧 FIX: Pass shutdown signal so WebSocket server shuts down gracefully
            let mut ws_shutdown_rx = self.shutdown_tx.subscribe();
            let task = tokio::spawn(async move {
                info!("  ✓ WebSocket RPC listening on ws://{}", ws_addr);
                tokio::select! {
                    result = ws_server.start(&ws_addr) => {
                        if let Err(e) = result {
                            error!("WebSocket server error: {:?}", e);
                        }
                    }
                    _ = ws_shutdown_rx.recv() => {
                        info!("WebSocket server shutting down");
                    }
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
                    halving_schedule_clone, // 📊 Phase 3: Halving schedule
                    burn_manager_clone,  // 📊 Phase 3: Fee burning
                    fee_market_clone,    // 📊 Phase 3: EIP-1559 dynamic pricing
                    governance_clone,    // 🏛️ Phase 4+: Governance epoch hooks
                    validator_rotation_clone, // 🔄 Phase 4+: Validator rotation
                    commit_reveal_clone, // 🔐 Phase 4+: Commit-reveal finalization
                    scoring_manager_clone, // 📊 Phase 5+: Performance scoring
                    vrf_keypair_for_block, // 🎲 VRF keypair for block proofs (C2 fix)
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
            // H4 FIX: Pass shutdown receiver so dispatch service can terminate gracefully
            let dispatch_shutdown_rx = self.shutdown_tx.subscribe();
            let dispatch_handle = tokio::spawn(async move {
                dispatch_service.start(dispatch_shutdown_rx).await;
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

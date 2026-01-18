use crate::config::Config;
use crate::mempool::Mempool;
use crate::executor::{TransactionExecutor, calculate_receipts_root};
use crate::p2p_handler::is_leader_for_slot;
use futures::FutureExt;
use anyhow::Result;
use luxtensor_consensus::{ConsensusConfig, ProofOfStake, RewardExecutor, UtilityMetrics, MinerInfo, ValidatorInfo, TokenAllocation, NodeRegistry};
use luxtensor_core::{Block, Transaction, StateDB};
use luxtensor_crypto::MerkleTree;
use luxtensor_network::{SwarmP2PNode, SwarmP2PEvent, SwarmCommand};
use luxtensor_rpc::RpcServer;
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Get current Unix timestamp (seconds since epoch)
/// Panics only if system time is before Unix epoch (practically impossible)
#[inline]
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("System time before Unix epoch")
        .as_secs()
}

/// Node service that orchestrates all components
pub struct NodeService {
    config: Config,
    storage: Arc<BlockchainDB>,
    state_db: Arc<RwLock<StateDB>>,
    consensus: Arc<RwLock<ProofOfStake>>,
    mempool: Arc<Mempool>,
    executor: Arc<TransactionExecutor>,
    reward_executor: Arc<RwLock<RewardExecutor>>,
    token_allocation: Arc<RwLock<TokenAllocation>>,
    node_registry: Arc<RwLock<NodeRegistry>>,
    shutdown_tx: broadcast::Sender<()>,
    tasks: Vec<JoinHandle<Result<()>>>,
    epoch_length: u64,
    /// Broadcast channel to P2P swarm for sending blocks/txs
    broadcast_tx: Option<mpsc::UnboundedSender<SwarmCommand>>,
    /// Genesis timestamp for slot calculation
    genesis_timestamp: u64,
}

impl NodeService {
    /// Create a new node service
    pub async fn new(config: Config) -> Result<Self> {
        info!("🦀 Initializing LuxTensor Node v{}", env!("CARGO_PKG_VERSION"));
        info!("Node name: {}", config.node.name);
        info!("Chain ID: {}", config.node.chain_id);

        // Validate configuration
        config.validate()?;

        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&config.node.data_dir)?;
        std::fs::create_dir_all(&config.storage.db_path)?;

        // Initialize storage
        info!("📦 Initializing storage...");
        let db_path_str = config.storage.db_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?;
        let storage = Arc::new(BlockchainDB::open(db_path_str)?);
        info!("  ✓ Storage initialized at {:?}", config.storage.db_path);

        // Initialize state database
        info!("💾 Initializing state database...");
        let state_db = Arc::new(RwLock::new(StateDB::new()));
        info!("  ✓ State database initialized");

        // Initialize transaction executor
        info!("⚡ Initializing transaction executor...");
        let executor = Arc::new(TransactionExecutor::new());
        info!("  ✓ Transaction executor initialized");

        // Initialize consensus
        info!("⚖️  Initializing consensus...");
        let consensus_config = ConsensusConfig {
            slot_duration: config.consensus.block_time,
            min_stake: config.consensus.min_stake.parse().unwrap_or(1_000_000_000_000_000_000),
            block_reward: 1_000_000_000_000_000_000, // 1 token reward
            epoch_length: config.consensus.epoch_length,
        };
        let consensus = Arc::new(RwLock::new(ProofOfStake::new(consensus_config)));
        info!("  ✓ PoS consensus initialized");
        info!("    - Min stake: {}", config.consensus.min_stake);
        info!("    - Max validators: {}", config.consensus.max_validators);
        info!("    - Epoch length: {} blocks", config.consensus.epoch_length);

        // Initialize mempool
        info!("📝 Initializing transaction mempool...");
        let mempool = Arc::new(Mempool::new(10000)); // Max 10k transactions
        info!("  ✓ Mempool initialized (max size: 10000)");

        // Check if genesis block exists, create if not
        if storage.get_block_by_height(0)?.is_none() {
            info!("🌱 Creating genesis block...");
            let genesis = Block::genesis();
            storage.store_block(&genesis)?;
            info!("  ✓ Genesis block created: {:?}", genesis.hash());
        } else {
            info!("  ✓ Genesis block found");
        }

        // ALWAYS initialize genesis accounts with balance for development
        // This ensures accounts have balance even after node restart
        // Hardhat account #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        let dev_accounts: &[[u8; 20]] = &[
            [0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce,
             0x6a, 0xb8, 0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66],
            [0x70, 0x99, 0x79, 0x70, 0xc5, 0x18, 0x12, 0xdc, 0x3a, 0x01,
             0x0c, 0x7d, 0x01, 0xb5, 0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xc8],
            [0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58,
             0x5d, 0xd2, 0x99, 0xe0, 0x3d, 0x12, 0xfa, 0x42, 0x93, 0xbc],
        ];

        for addr_bytes in dev_accounts {
            let dev_address = luxtensor_core::Address::from(*addr_bytes);
            let mut dev_account = luxtensor_core::Account::new();
            dev_account.balance = 10_000_000_000_000_000_000_000_u128; // 10000 ETH in wei
            state_db.write().set_account(dev_address, dev_account);
        }
        info!("  ✓ {} genesis accounts initialized with 10000 ETH each", dev_accounts.len());

        // Initialize reward executor for epoch processing
        let dao_address = [0u8; 20]; // TODO: Configure DAO address
        let reward_executor = Arc::new(RwLock::new(RewardExecutor::new(dao_address)));
        info!("  ✓ Reward executor initialized");

        // Initialize token allocation for TGE and vesting
        let tge_timestamp = current_timestamp();
        let token_allocation = Arc::new(RwLock::new(TokenAllocation::new(tge_timestamp)));
        info!("  ✓ Token allocation initialized");

        // Initialize node registry for progressive staking
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        info!("  ✓ Node registry initialized");

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(16);

        // Get epoch length from consensus config
        let epoch_length = config.consensus.epoch_length;

        // Get genesis timestamp from genesis block (for slot calculation)
        // This ensures all nodes use the same genesis timestamp from the chain
        let genesis_timestamp = storage.get_block_by_height(0)?
            .map(|block| block.header.timestamp)
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            });

        Ok(Self {
            config,
            storage,
            state_db,
            consensus,
            mempool,
            executor,
            reward_executor,
            token_allocation,
            node_registry,
            shutdown_tx,
            tasks: Vec::new(),
            epoch_length,
            broadcast_tx: None, // Will be initialized in start()
            genesis_timestamp,
        })
    }

    /// Start all node services
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting node services...");

        // Create shared EVM state for transaction bridge
        let evm_state = Arc::new(parking_lot::RwLock::new(
            luxtensor_rpc::EvmState::new(self.config.node.chain_id as u64)
        ));

        // Start RPC server if enabled
        if self.config.rpc.enabled {
            info!("🔌 Starting RPC server...");

            // For production, configure P2P and WebSocket broadcasters here
            // For now, use NoOp broadcaster (transactions stay in mempool only)
            let rpc_server = RpcServer::new_for_testing_with_evm(
                self.storage.clone(),
                self.state_db.clone(),
                evm_state.clone(),
            );

            let addr = format!("{}:{}", self.config.rpc.listen_addr, self.config.rpc.listen_port);

            let task = tokio::spawn(async move {
                info!("  ✓ RPC server listening on {}", addr);
                match rpc_server.start(&addr) {
                    Ok(_server) => {
                        info!("RPC server started successfully");
                        // Keep server running
                        tokio::signal::ctrl_c().await.ok();
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                }
            });

            self.tasks.push(task);
        }

        // Start P2P network with Swarm
        info!("🌐 Starting P2P Swarm network...");
        let (p2p_event_tx, mut p2p_event_rx) = mpsc::unbounded_channel::<SwarmP2PEvent>();

        match SwarmP2PNode::new(self.config.network.listen_port, p2p_event_tx).await {
            Ok((mut swarm_node, command_tx)) => {
                info!("  ✓ P2P Swarm started");
                info!("    Listen port: {}", self.config.network.listen_port);
                info!("    mDNS discovery enabled");

                // Save broadcast_tx for block production
                self.broadcast_tx = Some(command_tx);

                // Move swarm node into the task - it will run its own event loop
                // We don't hold a reference because Swarm is not Send-safe
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    rt.block_on(async move {
                        swarm_node.run().await;
                    });
                });

                // Start P2P event handler
                let storage_for_p2p = self.storage.clone();
                let broadcast_tx_for_sync = self.broadcast_tx.clone();
                let node_name = self.config.node.name.clone();
                let event_task = tokio::spawn(async move {
                    while let Some(event) = p2p_event_rx.recv().await {
                        match event {
                            SwarmP2PEvent::NewBlock(block) => {
                                let height = block.header.height;
                                // Check if we already have this block
                                if storage_for_p2p.get_block_by_height(height).ok().flatten().is_some() {
                                    debug!("Already have block #{}, skipping", height);
                                    continue;
                                }
                                if let Err(e) = storage_for_p2p.store_block(&block) {
                                    warn!("Failed to store received block: {}", e);
                                } else {
                                    info!("📥 Synced block #{} from peer", height);
                                }
                            }
                            SwarmP2PEvent::NewTransaction(_tx) => {
                                debug!("📥 Received transaction from peer");
                            }
                            SwarmP2PEvent::PeerConnected(peer_id) => {
                                info!("👋 Peer connected: {}", peer_id);
                                // Request sync when peer connects
                                let my_height = storage_for_p2p.get_best_height().unwrap_or(Some(0)).unwrap_or(0);
                                if let Some(ref tx) = broadcast_tx_for_sync {
                                    // Request blocks we don't have (up to 100 ahead)
                                    let _ = tx.send(SwarmCommand::RequestSync {
                                        from_height: my_height + 1,
                                        to_height: my_height + 100,
                                        my_id: node_name.clone(),
                                    });
                                    info!("🔄 Requesting sync from height {}", my_height + 1);
                                }
                            }
                            SwarmP2PEvent::PeerDisconnected(peer_id) => {
                                info!("👋 Peer disconnected: {}", peer_id);
                            }
                            SwarmP2PEvent::SyncRequest { from_height, to_height, requester_id } => {
                                info!("🔄 Got sync request from {} for blocks {}-{}", requester_id, from_height, to_height);
                                // Collect blocks we have in range
                                let mut blocks_to_send = Vec::new();
                                for h in from_height..=to_height {
                                    if let Ok(Some(block)) = storage_for_p2p.get_block_by_height(h) {
                                        blocks_to_send.push(block);
                                    }
                                }
                                if !blocks_to_send.is_empty() {
                                    info!("📤 Sending {} blocks to {}", blocks_to_send.len(), requester_id);
                                    if let Some(ref tx) = broadcast_tx_for_sync {
                                        let _ = tx.send(SwarmCommand::SendBlocks { blocks: blocks_to_send });
                                    }
                                }
                            }
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                });
                self.tasks.push(event_task);
            }
            Err(e) => {
                warn!("Failed to start P2P Swarm: {}. Running in standalone mode.", e);
            }
        }


        // Start block production if validator
        if self.config.node.is_validator {
            info!("🔨 Starting block production...");
            let consensus = self.consensus.clone();
            let storage = self.storage.clone();
            let state_db = self.state_db.clone();
            let mempool = self.mempool.clone();
            let executor = self.executor.clone();
            let reward_executor = self.reward_executor.clone();
            let block_time = self.config.consensus.block_time;
            let epoch_length = self.epoch_length;
            let shutdown_rx = self.shutdown_tx.subscribe();
            let evm_state_for_block = evm_state.clone();

            // Leader election params
            let validator_id = self.config.node.validator_id.clone()
                .unwrap_or_else(|| self.config.node.name.clone());
            let validators = self.config.consensus.validators.clone();
            let genesis_timestamp = self.genesis_timestamp;
            let broadcast_tx = self.broadcast_tx.clone();
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
                    evm_state_for_block,
                    validator_id,
                    validators,
                    genesis_timestamp,
                    broadcast_tx,
                ).await
            });

            self.tasks.push(task);
            info!("  ✓ Block production started");
            if let Some(ref vid) = self.config.node.validator_id {
                info!("    Validator ID: {}", vid);
            }
            info!("    Known validators: {:?}", self.config.consensus.validators);
        }

        info!("✅ All services started successfully");
        self.print_status();

        Ok(())
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&mut self) -> Result<()> {
        info!("Node is running. Press Ctrl+C to shutdown.");

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("Received shutdown signal");

        self.shutdown().await
    }

    /// Shutdown all services
    async fn shutdown(&mut self) -> Result<()> {
        info!("🛑 Shutting down node services...");

        // Send shutdown signal to all tasks
        let _ = self.shutdown_tx.send(());

        // Wait for all tasks to complete
        for task in self.tasks.drain(..) {
            match task.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => warn!("Task error during shutdown: {}", e),
                Err(e) => error!("Task panicked during shutdown: {}", e),
            }
        }

        // Flush storage
        info!("💾 Flushing storage...");
        // Storage flush happens automatically on drop

        info!("✅ Shutdown complete");
        Ok(())
    }

    /// Block production loop for validators
    async fn block_production_loop(
        consensus: Arc<RwLock<ProofOfStake>>,
        storage: Arc<BlockchainDB>,
        state_db: Arc<RwLock<StateDB>>,
        mempool: Arc<Mempool>,
        executor: Arc<TransactionExecutor>,
        reward_executor: Arc<RwLock<RewardExecutor>>,
        block_time: u64,
        epoch_length: u64,
        mut shutdown: broadcast::Receiver<()>,
        evm_state: Arc<parking_lot::RwLock<luxtensor_rpc::EvmState>>,
        validator_id: String,
        validators: Vec<String>,
        genesis_timestamp: u64,
        broadcast_tx: Option<mpsc::UnboundedSender<SwarmCommand>>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(block_time));
        let mut slot_counter: u64 = 0;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Calculate current slot
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let slot = if now > genesis_timestamp {
                        (now - genesis_timestamp) / block_time
                    } else {
                        slot_counter
                    };
                    slot_counter = slot + 1;

                    // Check if we are the leader for this slot
                    if !validators.is_empty() && !is_leader_for_slot(&validator_id, slot, &validators) {
                        debug!("⏳ Slot {}: Not our turn (leader: {})",
                               slot,
                               validators.get((slot % validators.len() as u64) as usize).unwrap_or(&"unknown".to_string()));
                        continue;
                    }

                    info!("🎯 Slot {}: We are the leader! Producing block...", slot);

                    // Poll tx_queue from EVM state and add to mempool
                    let pending_txs = evm_state.read().drain_tx_queue();
                    for ready_tx in pending_txs {
                        // Convert ReadyTransaction to core Transaction
                        let from_addr = luxtensor_core::Address::from(ready_tx.from);
                        let to_addr = ready_tx.to.map(luxtensor_core::Address::from);

                        let mut tx = Transaction::new(
                            ready_tx.nonce,
                            from_addr,
                            to_addr,
                            ready_tx.value,
                            1, // gas price
                            ready_tx.gas,
                            ready_tx.data,
                        );

                        // Sign transaction with proper secp256k1 using dev private key
                        // This is for development - in production, clients send pre-signed TX
                        if !sign_transaction_with_dev_key(&mut tx, &ready_tx.from) {
                            warn!("Failed to sign transaction from unknown account: {:?}", ready_tx.from);
                            continue;
                        }

                        if let Err(e) = mempool.add_transaction(tx) {
                            warn!("Failed to add RPC tx to mempool: {}", e);
                        } else {
                            info!("📥 Added signed transaction to mempool");
                        }
                    }

                    // Produce a block
                    match Self::produce_block(
                        &consensus, &storage, &state_db, &mempool, &executor,
                        &reward_executor, epoch_length
                    ).await {
                        Ok(block) => {
                            // Broadcast block to P2P network
                            if let Some(ref tx) = broadcast_tx {
                                if let Err(e) = tx.send(SwarmCommand::BroadcastBlock(block.clone())) {
                                    warn!("Failed to send block to broadcast channel: {}", e);
                                } else {
                                    info!("📡 Block #{} broadcasted to network", block.header.height);
                                }
                            } else {
                                info!("📦 Block #{} produced (standalone mode)", block.header.height);
                            }
                        }
                        Err(e) => {
                            error!("Failed to produce block: {}", e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("Block production shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Produce a single block
    async fn produce_block(
        _consensus: &Arc<RwLock<ProofOfStake>>,
        storage: &Arc<BlockchainDB>,
        state_db: &Arc<RwLock<StateDB>>,
        mempool: &Arc<Mempool>,
        executor: &Arc<TransactionExecutor>,
        reward_executor: &Arc<RwLock<RewardExecutor>>,
        epoch_length: u64,
    ) -> Result<Block> {
        // Get current height
        let height = storage.get_best_height()?.unwrap_or(0);
        let new_height = height + 1;

        // Get previous block
        let previous_block = storage.get_block_by_height(height)?
            .ok_or_else(|| anyhow::anyhow!("Previous block not found"))?;

        // Get transactions from mempool (up to 1000 per block)
        let transactions = mempool.get_transactions_for_block(1000);

        // Create preliminary header to get block hash
        let preliminary_header = luxtensor_core::BlockHeader {
            version: 1,
            height: new_height,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            previous_hash: previous_block.hash(),
            state_root: [0u8; 32], // Will be updated after execution
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: 0,
            gas_limit: 10_000_000,
            extra_data: vec![],
        };

        let preliminary_block = Block::new(preliminary_header.clone(), transactions.clone());
        let block_hash = preliminary_block.hash();

        // Execute transactions
        let mut state = state_db.write();
        let mut valid_transactions = Vec::new();
        let mut valid_receipts = Vec::new();
        let mut total_gas = 0u64;

        for (tx_index, tx) in transactions.into_iter().enumerate() {
            match executor.execute(&tx, &mut state, new_height, block_hash, tx_index) {
                Ok(receipt) => {
                    total_gas += receipt.gas_used;
                    valid_receipts.push(receipt);
                    valid_transactions.push(tx);
                }
                Err(e) => {
                    warn!("Transaction {:?} failed: {}", tx.hash(), e);
                }
            }
        }

        // Calculate transaction root
        let tx_hashes: Vec<[u8; 32]> = valid_transactions.iter()
            .map(|tx| tx.hash())
            .collect();
        let txs_root = if tx_hashes.is_empty() {
            [0u8; 32]
        } else {
            MerkleTree::new(tx_hashes).root()
        };

        // Calculate receipts root
        let receipts_root = calculate_receipts_root(&valid_receipts);

        // Calculate state root
        let state_root = state.commit()?;
        drop(state); // Release lock

        // Create new block header
        let header = luxtensor_core::BlockHeader {
            version: 1,
            height: new_height,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            previous_hash: previous_block.hash(),
            state_root,
            txs_root,
            receipts_root,
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: total_gas,
            gas_limit: 10_000_000,
            extra_data: vec![],
        };

        // Create new block
        let block = Block::new(header, valid_transactions.clone());

        // Store block
        storage.store_block(&block)?;

        // Remove transactions from mempool
        let tx_hashes: Vec<_> = valid_transactions.iter().map(|tx| tx.hash()).collect();
        mempool.remove_transactions(&tx_hashes);

        info!("📦 Produced block #{} with {} transactions, {} gas used, hash {:?}",
            new_height, valid_transactions.len(), total_gas, block.hash());

        // Check if this is an epoch boundary and process rewards
        if new_height % epoch_length == 0 && epoch_length > 0 {
            let epoch_num = new_height / epoch_length;
            info!("🎯 Epoch {} completed at block #{}, processing rewards...", epoch_num, new_height);

            // Create utility metrics for this epoch
            let utility = UtilityMetrics {
                active_validators: 1,
                active_subnets: 1,
                epoch_transactions: valid_transactions.len() as u64,
                epoch_ai_tasks: 0, // TODO: Track AI tasks
                block_utilization: 50, // TODO: Calculate actual utilization
            };

            // Get current miners and validators (simplified - in production get from metagraph)
            // For now, use the block producer as both miner and validator
            let miner_addr = [0u8; 20]; // TODO: Get actual miner address
            let miners = vec![
                MinerInfo { address: miner_addr, score: 1.0 },
            ];
            let validators = vec![
                ValidatorInfo { address: miner_addr, stake: 1000 },
            ];

            // Process epoch rewards
            let result = reward_executor.write().process_epoch(
                epoch_num,
                new_height,
                &utility,
                &miners,
                &validators,
                &[], // delegators
                &[], // subnets
            );

            info!("💰 Epoch {} rewards distributed: {} total emission, {} participants, {} DAO",
                epoch_num, result.total_emission, result.participants_rewarded, result.dao_allocation);
        }

        Ok(block)
    }

    /// Print node status
    fn print_status(&self) {
        info!("");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("  📊 LuxTensor Node Status");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("  Name:         {}", self.config.node.name);
        info!("  Chain ID:     {}", self.config.node.chain_id);
        info!("  Validator:    {}", self.config.node.is_validator);
        info!("");
        info!("  🌐 Network");
        info!("    Address:    {}:{}", self.config.network.listen_addr, self.config.network.listen_port);
        info!("    Max Peers:  {}", self.config.network.max_peers);
        info!("");
        if self.config.rpc.enabled {
            info!("  🔌 RPC");
            info!("    Enabled:    Yes");
            info!("    Address:    {}:{}", self.config.rpc.listen_addr, self.config.rpc.listen_port);
        } else {
            info!("  🔌 RPC:       Disabled");
        }
        info!("");
        info!("  💾 Storage");
        info!("    Path:       {:?}", self.config.storage.db_path);
        info!("    Cache:      {} MB", self.config.storage.cache_size);
        info!("");
        info!("  ⚖️  Consensus");
        info!("    Type:       Proof of Stake");
        info!("    Block Time: {} seconds", self.config.consensus.block_time);
        info!("    Epoch:      {} blocks", self.config.consensus.epoch_length);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("");
    }

    /// Get node statistics
    pub async fn get_stats(&self) -> Result<NodeStats> {
        let height = self.storage.get_best_height()?.unwrap_or(0);
        let validator_count = {
            let consensus = self.consensus.read();
            consensus.validator_count()
        };
        let mempool_size = self.mempool.len();

        Ok(NodeStats {
            height,
            validator_count,
            is_validator: self.config.node.is_validator,
            chain_id: self.config.node.chain_id,
            mempool_size,
        })
    }

    /// Add transaction to mempool
    pub fn add_transaction(&self, tx: Transaction) -> Result<()> {
        self.mempool.add_transaction(tx)
            .map_err(|e| anyhow::anyhow!("Failed to add transaction: {}", e))
    }

    /// Get mempool
    pub fn mempool(&self) -> &Arc<Mempool> {
        &self.mempool
    }
}

/// Node statistics
#[derive(Debug, Clone)]
pub struct NodeStats {
    pub height: u64,
    pub validator_count: usize,
    pub is_validator: bool,
    pub chain_id: u64,
    pub mempool_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_node_service_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.node.data_dir = temp_dir.path().to_path_buf();
        config.storage.db_path = temp_dir.path().join("db");
        config.rpc.enabled = false; // Disable RPC for test

        let service = NodeService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_node_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.node.data_dir = temp_dir.path().to_path_buf();
        config.storage.db_path = temp_dir.path().join("db");
        config.rpc.enabled = false;

        let service = NodeService::new(config).await.unwrap();
        let stats = service.get_stats().await.unwrap();

        assert_eq!(stats.height, 0); // Genesis block
        assert_eq!(stats.chain_id, 1);
    }
}

/// Sign a transaction with a development private key (proper secp256k1 signing)
/// Returns true if signing was successful, false if the address is not a known dev account
fn sign_transaction_with_dev_key(tx: &mut Transaction, from: &[u8; 20]) -> bool {
    use luxtensor_crypto::{keccak256, KeyPair};

    // Hardhat default private keys (for development only!)
    // These are well-known test keys - never use in production with real funds
    let dev_accounts: &[([u8; 20], [u8; 32])] = &[
        // Account #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        (
            [0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce,
             0x6a, 0xb8, 0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66],
            [0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4,
             0xa6, 0xb4, 0xd2, 0x38, 0xff, 0x94, 0x4b, 0xac, 0xb4, 0x78,
             0xcb, 0xed, 0x5e, 0xfc, 0xae, 0x78, 0x4d, 0x7b, 0xf4, 0xf2,
             0xff, 0x80]
        ),
        // Account #1: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
        (
            [0x70, 0x99, 0x79, 0x70, 0xc5, 0x18, 0x12, 0xdc, 0x3a, 0x01,
             0x0c, 0x7d, 0x01, 0xb5, 0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xc8],
            [0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04,
             0x49, 0x66, 0xf0, 0x94, 0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda,
             0xe8, 0x8c, 0x7a, 0x84, 0x12, 0xf4, 0x60, 0x3b, 0x6b, 0x78,
             0x69, 0x0d]
        ),
        // Account #2: 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
        (
            [0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58,
             0x5d, 0xd2, 0x99, 0xe0, 0x3d, 0x12, 0xfa, 0x42, 0x93, 0xbc],
            [0x5d, 0xe4, 0x11, 0x1a, 0xfa, 0x1a, 0x4b, 0x94, 0x90, 0x8f,
             0x83, 0x10, 0x3e, 0xb1, 0xf1, 0x70, 0x63, 0x67, 0xc2, 0xe6,
             0x8c, 0xa8, 0x70, 0xfc, 0x3f, 0xb9, 0xa8, 0x04, 0xcd, 0xab,
             0x36, 0x5a]
        ),
    ];

    // Find matching private key (case-insensitive by converting to lowercase)
    let from_lower: [u8; 20] = from.map(|b| b.to_ascii_lowercase());

    for (addr, privkey) in dev_accounts {
        let addr_lower: [u8; 20] = addr.map(|b| b.to_ascii_lowercase());
        if addr_lower == from_lower {
            // Create keypair from private key using proper secp256k1
            match KeyPair::from_secret(privkey) {
                Ok(keypair) => {
                    // Verify keypair address matches expected address
                    let keypair_addr = keypair.address();
                    info!("🔑 Signing with keypair address: {:?}", keypair_addr);
                    info!("🔑 Expected from address: {:?}", from);

                    // Get signing message and hash (same format as verify_signature)
                    let msg = tx.signing_message();
                    let msg_hash = keccak256(&msg);
                    info!("🔑 Message hash: {:?}", &msg_hash[..8]);

                    // Sign with secp256k1 ECDSA
                    match keypair.sign(&msg_hash) {
                        Ok(signature) => {
                            tx.r.copy_from_slice(&signature[..32]);
                            tx.s.copy_from_slice(&signature[32..]);

                            // Try recovery ID 0 first, then 1 if verification fails
                            for v in [0u8, 1u8] {
                                tx.v = v;
                                if tx.verify_signature().is_ok() {
                                    info!("✅ Signature verified with v={}", v);
                                    return true;
                                }
                            }

                            // If neither works, log error
                            warn!("❌ Signature verification failed for both v=0 and v=1");
                            return false;
                        }
                        Err(e) => {
                            warn!("Failed to sign transaction: {}", e);
                            return false;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create keypair: {}", e);
                    return false;
                }
            }
        }
    }

    warn!("No matching dev account found for address: {:?}", from);
    false
}

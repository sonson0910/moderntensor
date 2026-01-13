use crate::config::Config;
use crate::mempool::Mempool;
use crate::executor::{TransactionExecutor, calculate_receipts_root};
use anyhow::Result;
use luxtensor_consensus::{ConsensusConfig, ProofOfStake, RewardExecutor, UtilityMetrics, MinerInfo, ValidatorInfo, TokenAllocation, NodeRegistry};
use luxtensor_core::{Block, Transaction, StateDB};
use luxtensor_crypto::MerkleTree;
use luxtensor_rpc::RpcServer;
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

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
        let storage = Arc::new(BlockchainDB::open(
            config.storage.db_path.to_str().unwrap(),
        )?);
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

        // Initialize reward executor for epoch processing
        let dao_address = [0u8; 20]; // TODO: Configure DAO address
        let reward_executor = Arc::new(RwLock::new(RewardExecutor::new(dao_address)));
        info!("  ✓ Reward executor initialized");

        // Initialize token allocation for TGE and vesting
        let tge_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let token_allocation = Arc::new(RwLock::new(TokenAllocation::new(tge_timestamp)));
        info!("  ✓ Token allocation initialized");

        // Initialize node registry for progressive staking
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        info!("  ✓ Node registry initialized");

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(16);

        // Get epoch length from consensus config
        let epoch_length = config.consensus.epoch_length;

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
        })
    }

    /// Start all node services
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting node services...");

        // Start RPC server if enabled
        if self.config.rpc.enabled {
            info!("🔌 Starting RPC server...");

            // For production, configure P2P and WebSocket broadcasters here
            // For now, use NoOp broadcaster (transactions stay in mempool only)
            let rpc_server = RpcServer::new_for_testing(
                self.storage.clone(),
                self.state_db.clone(),
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

        // Start P2P network
        info!("🌐 Starting P2P network...");
        // Note: P2P is currently stubbed. Will be fully implemented in future
        info!("  ⏳ P2P network configuration prepared");
        info!("    Listen address: {}:{}", self.config.network.listen_addr, self.config.network.listen_port);
        info!("    Max peers: {}", self.config.network.max_peers);


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
                ).await
            });

            self.tasks.push(task);
            info!("  ✓ Block production started");
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
    ) -> Result<()> {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(block_time));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Produce a block
                    if let Err(e) = Self::produce_block(
                        &consensus, &storage, &state_db, &mempool, &executor,
                        &reward_executor, epoch_length
                    ).await {
                        error!("Failed to produce block: {}", e);
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
    ) -> Result<()> {
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

        Ok(())
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

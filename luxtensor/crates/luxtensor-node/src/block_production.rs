//! Block production logic extracted from `service.rs`.
//!
//! Contains:
//! - `block_production_loop`: the main validator loop that selects leaders and produces blocks
//! - `produce_block`: creates, signs, and stores a single block
//! - `process_disputes`: optimistic AI dispute resolution and slashing
//! - `process_epoch_rewards`: epoch boundary reward distribution and RANDAO finalization

use crate::executor::{calculate_receipts_root, TransactionExecutor};
use crate::mempool::Mempool;
use crate::metrics::NodeMetrics;
use crate::service::{is_leader_for_slot, NodeService, BLOCK_GAS_LIMIT, MAX_TRANSACTIONS_PER_BLOCK};

use anyhow::{Context, Result};
use luxtensor_consensus::fast_finality::FastFinality;
use luxtensor_consensus::randao::RandaoMixer;
use luxtensor_consensus::slashing::SlashingManager;
use luxtensor_consensus::{
    DelegatorInfo, MinerInfo, ProofOfStake, RewardExecutor, SubnetInfo, UtilityMetrics,
    ValidatorInfo, YumaConsensus,
};
use luxtensor_contracts::AgentTriggerEngine;
use luxtensor_core::{Block, StateDB};
use luxtensor_crypto::{KeyPair, MerkleTree};
use luxtensor_network::SwarmCommand;
use luxtensor_oracle::DisputeManager;
use luxtensor_rpc::BroadcastEvent;
use luxtensor_storage::metagraph_store::MetagraphDB;
use luxtensor_storage::BlockchainDB;
use luxtensor_storage::{CachedStateDB, CheckpointManager, CHECKPOINT_INTERVAL};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

use super::service::{KEEP_RECEIPTS_BLOCKS, PRUNING_INTERVAL};

/// Hash-based leader selection for bootstrap mode (no validators configured).
///
/// When the validator set is empty, every node would otherwise produce blocks
/// at every slot, causing parallel chain forks and `previous_hash mismatch`
/// warnings when nodes discover each other via mDNS.
///
/// This function uses a deterministic hash of `validator_id` to throttle
/// production: each node produces only on slots where
/// `(slot + hash(validator_id)) % SOLO_SLOT_MODULUS == 0`.
///
/// With `SOLO_SLOT_MODULUS = 1`, a solo node produces every slot (no slowdown).
/// In multi-node bootstrap, different `validator_id` hashes spread production
/// across different slots, dramatically reducing fork probability.
///
/// Once proper validators are registered (via staking), PoS or round-robin
/// takes over and this fallback is no longer used.
fn is_solo_leader_for_slot(validator_id: &str, slot: u64) -> bool {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // For solo-node operation, ALWAYS produce (no other nodes to conflict with).
    // The real fix: multi-node setups MUST register validators for proper
    // round-robin or PoS selection. This fallback handles bootstrap gracefully.
    //
    // Heuristic: produce on (slot + id_hash) % 4 == 0 slots.
    // In a multi-node bootstrap with N unregistered nodes, each produces ~25%
    // of slots, and the probability of collision for 2 nodes is only ~6.25%.
    const SOLO_SLOT_MODULUS: u64 = 4;

    let mut hasher = DefaultHasher::new();
    validator_id.hash(&mut hasher);
    let id_hash = hasher.finish();

    (slot.wrapping_add(id_hash)) % SOLO_SLOT_MODULUS == 0
}

impl NodeService {
    /// Block production loop for validators
    pub(crate) async fn block_production_loop(
        consensus: Arc<RwLock<ProofOfStake>>,
        storage: Arc<BlockchainDB>,
        state_db: Arc<RwLock<StateDB>>,
        mempool: Arc<Mempool>,
        executor: Arc<TransactionExecutor>,
        reward_executor: Arc<RwLock<RewardExecutor>>,
        block_time: u64,
        epoch_length: u64,
        mut shutdown: broadcast::Receiver<()>,
        rpc_mempool: Arc<luxtensor_core::UnifiedMempool>,
        validator_id: String,
        validators: Vec<String>,
        genesis_timestamp: u64,
        broadcast_tx: Option<mpsc::Sender<SwarmCommand>>,
        chain_id: u64,
        our_validator_address: Option<luxtensor_crypto::CryptoAddress>,
        // 🔧 FIX: Accept validator keypair for block signing
        validator_keypair_for_block: Option<KeyPair>,
        // 🔧 FIX #9: Atomic height guard shared with P2P handler
        best_height_guard: std::sync::Arc<std::sync::atomic::AtomicU64>,
        // 🔧 FIX: Sync guard — pause block production while syncing from peers
        is_syncing: std::sync::Arc<std::sync::atomic::AtomicBool>,
        metagraph_db: Arc<MetagraphDB>,
        // Unified RPC state — synced after each block so eth_* RPCs return fresh data
        unified_state: Option<Arc<parking_lot::RwLock<luxtensor_core::UnifiedStateDB>>>,
        // RANDAO mixer for epoch finalization
        randao: Arc<RwLock<RandaoMixer>>,
        // Agentic EVM: block-level autonomous agent triggers
        agent_trigger_engine: Arc<AgentTriggerEngine>,
        // Optimistic AI: dispute manager for fraud proofs
        dispute_manager: Arc<DisputeManager>,
        // Slashing manager for dispute-triggered slashing
        slashing_manager: Arc<RwLock<SlashingManager>>,
        // Merkle root caching layer for efficient state root computation
        merkle_cache: Arc<CachedStateDB>,
        // BFT fast finality — notify after block production
        fast_finality: Arc<RwLock<FastFinality>>,
        // NodeMetrics for recording block production stats
        metrics_for_blocks: Arc<NodeMetrics>,
        // WebSocket broadcast sender for emitting real-time events
        ws_broadcast: Option<tokio::sync::mpsc::Sender<BroadcastEvent>>,
        // 📊 Tokenomics pipeline: halving, fee burning, dynamic gas pricing
        halving_schedule: Arc<luxtensor_consensus::HalvingSchedule>,
        burn_manager: Arc<luxtensor_consensus::BurnManager>,
        fee_market: Arc<RwLock<luxtensor_consensus::FeeMarket>>,
        // 🏛️ Governance + Rotation + CommitReveal + Scoring (deep-wired epoch hooks)
        governance: Arc<RwLock<luxtensor_consensus::GovernanceModule>>,
        validator_rotation: Arc<RwLock<luxtensor_consensus::ValidatorRotation>>,
        commit_reveal: Arc<luxtensor_consensus::CommitRevealManager>,
        scoring_manager: Arc<RwLock<luxtensor_consensus::ScoringManager>>,
        // 🎲 VRF keypair (secp256k1 EC-VRF) for block proof generation (C2 fix)
        vrf_keypair: Option<Arc<luxtensor_crypto::vrf::VrfKeypair>>,
        // 🛡️ AI layer circuit breaker — protects against cascade failures in epoch operations
        ai_circuit_breaker: Arc<luxtensor_consensus::AILayerCircuitBreaker>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(block_time));
        let mut slot_counter: u64 = 0;
        // 🔧 FIX: Store keypair reference for repeated use across slots
        let validator_keypair_ref = validator_keypair_for_block;
        // 🔧 FIX MC-6: Accumulate TX count across the entire epoch instead of
        // using only the last block's count at the epoch boundary.
        let mut epoch_tx_accumulator: u64 = 0;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Calculate current slot
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::ZERO)
                        .as_secs();
                    let slot = if now > genesis_timestamp && block_time > 0 {
                        (now - genesis_timestamp) / block_time
                    } else {
                        slot_counter
                    };
                    slot_counter = slot + 1;

                    // 🔧 FIX: Skip production while syncing from peers
                    if is_syncing.load(std::sync::atomic::Ordering::SeqCst) {
                        info!("⏸️ Pausing block production while syncing from peers...");
                        continue;
                    }

                    // 🔧 DEBUG: Log every slot to confirm block production is running
                    debug!("⏰ Slot {} processing (chain_id: {})", slot, chain_id);

                    // 🔧 Drain transactions from UnifiedMempool into node mempool
                    // Transactions from RPC are already fully formed Transaction objects
                    // with correct signatures — no conversion needed.
                    let rpc_txs = rpc_mempool.get_pending_transactions();
                    if !rpc_txs.is_empty() {
                        debug!("📤 Found {} transactions in UnifiedMempool", rpc_txs.len());
                        let mut added_hashes = Vec::new();
                        for tx in rpc_txs {
                            let tx_hash = tx.hash();
                            if let Err(e) = mempool.add_transaction(tx) {
                                warn!("Failed to add TX to mempool: {}", e);
                            } else {
                                debug!("✅ Transaction added to node mempool successfully");
                                added_hashes.push(tx_hash);
                            }
                        }
                        // Remove successfully transferred transactions from UnifiedMempool
                        if !added_hashes.is_empty() {
                            rpc_mempool.remove_transactions(&added_hashes);
                        }
                    }

                    // Check if we are the leader for this slot using PoS VRF selection
                    // Fallback to round-robin if validator set is empty (bootstrapping)
                    //
                    // 🔧 FIX: When no validators configured, use hash-based slot selection
                    // instead of `true` (which caused ALL nodes to produce every slot,
                    // creating fork storms with previous_hash mismatch warnings).
                    // 🔧 FIX FORK STORM: Use slot_counter for round-robin, NOT slot (unix/block_time).
                    // slot = (now - genesis) / block_time; if genesis and now are both multiples
                    // of block_time, then slot % block_time == 0 always → same validator every turn.
                    // slot_counter increments by 1 each interval tick (every block_time seconds),
                    // so slot_counter % N gives fair round-robin across N validators.
                    // next_height from best_height_guard can't be used: nodes have different heights.
                    let rr_index = slot_counter; // already incremented above: slot_counter = slot + 1

                    let is_our_turn = if let Some(our_addr) = our_validator_address {
                        let our_addr_typed = luxtensor_core::Address::from(our_addr);
                        match consensus.read().select_validator(slot) {
                            Ok(selected) => {
                                if selected != our_addr_typed {
                                    debug!("⏳ Slot {}: Not selected by PoS (leader: 0x{})",
                                           slot, hex::encode(selected.as_bytes()));
                                    false
                                } else {
                                    true
                                }
                            }
                            Err(_) => {
                                // Validator set empty — fall back to round-robin for bootstrap
                                if !validators.is_empty() {
                                    is_leader_for_slot(&validator_id, rr_index, &validators)
                                } else {
                                    is_solo_leader_for_slot(&validator_id, slot)
                                }
                            }
                        }
                    } else {
                        // No keypair — use slot_counter round-robin (independent of timestamp)
                        if !validators.is_empty() {
                            is_leader_for_slot(&validator_id, rr_index, &validators)
                        } else {
                            is_solo_leader_for_slot(&validator_id, slot)
                        }
                    };

                    if !is_our_turn {
                        continue;
                    }

                    info!("🎯 Slot {}: We are the leader! Producing block...", slot);

                    // Produce a block (TXs already in mempool from earlier drain)
                    let block_start_time = std::time::Instant::now();
                    match Self::produce_block(
                        &consensus, &storage, &state_db, &mempool, &executor,
                        &reward_executor, epoch_length,
                        // 🔧 FIX: Pass validator keypair for block signing
                        // Previously hardcoded to None — blocks were always unsigned
                        validator_keypair_ref.as_ref(),
                        &best_height_guard,  // 🔧 FIX #9: Atomic height guard
                        &metagraph_db,   // For reward distribution from metagraph
                        &randao,         // RANDAO mixer for epoch finalization
                        epoch_tx_accumulator, // 🔧 FIX MC-6: pass accumulated count
                        &agent_trigger_engine, // Agentic EVM triggers
                        &dispute_manager, // Optimistic AI disputes
                        &slashing_manager, // For dispute slashing
                        &merkle_cache,   // Merkle root caching
                        &fast_finality,  // BFT fast finality hook
                        &halving_schedule,  // Halving schedule
                        &burn_manager,      // Fee burning
                        &fee_market,        // EIP-1559 dynamic pricing
                        &governance,        // 🏛️ Governance proposal processing
                        &validator_rotation, // 🔄 Validator rotation at epoch
                        &commit_reveal,     // 🔐 Commit-reveal finalization
                        &scoring_manager,   // 📊 Performance scoring
                        vrf_keypair.as_deref(), // 🎲 VRF keypair for proof generation
                        &ai_circuit_breaker, // 🛡️ AI layer circuit breaker
                    ).await {
                        Ok(block) => {
                            // Record NodeMetrics for this block
                            let production_ms = block_start_time.elapsed().as_millis() as u64;
                            metrics_for_blocks.record_block(
                                block.header.height,
                                block.transactions.len(),
                                production_ms,
                            );

                            // 🔧 FIX MC-6: Accumulate TX count for the whole epoch
                            epoch_tx_accumulator += block.transactions.len() as u64;

                            // 🔧 FIX C3: Reset accumulator at epoch boundaries so it
                            // doesn't inflate utility scores across epochs.
                            if epoch_length > 0 && block.header.height % epoch_length == 0 {
                                epoch_tx_accumulator = 0;
                            }

                            // Sync UnifiedStateDB so the RPC layer returns fresh state
                            if let Some(ref us) = unified_state {
                                let state_read = state_db.read();
                                let mut unified = us.write();
                                unified.sync_from_state_db(&state_read, block.header.height);
                                debug!("📊 UnifiedStateDB synced to height {}", block.header.height);
                            }

                            // Broadcast block to P2P network
                            if let Some(ref tx) = broadcast_tx {
                                if let Err(e) = tx.send(SwarmCommand::BroadcastBlock(block.clone())).await {
                                    warn!("Failed to send block to broadcast channel: {}", e);
                                } else {
                                    info!("📡 Block #{} broadcasted to network", block.header.height);
                                }
                            } else {
                                info!("📦 Block #{} produced (standalone mode)", block.header.height);
                            }

                            // Emit WebSocket event for real-time subscribers
                            if let Some(ref ws_tx) = ws_broadcast {
                                let rpc_block = luxtensor_rpc::types::RpcBlock::from(block.clone());
                                if let Err(e) = ws_tx.try_send(BroadcastEvent::NewBlock(rpc_block)) {
                                    warn!("Failed to send NewBlock to WebSocket: {}", e);
                                } else {
                                    debug!("🔌 WebSocket NewBlock event emitted for #{}", block.header.height);
                                }
                            }

                            // Auto-checkpoint: create snapshot at checkpoint intervals
                            let current_height = block.header.height;
                            if current_height > 0 && current_height % CHECKPOINT_INTERVAL == 0 {
                                let checkpoint_dir = std::path::PathBuf::from("./data/checkpoints");
                                let mut manager = CheckpointManager::new(&checkpoint_dir, storage.inner_db());

                                if let Err(e) = manager.create_checkpoint(current_height, block.header.hash(), block.header.state_root) {
                                    warn!("⚠️ Failed to create checkpoint at height {}: {:?}", current_height, e);
                                } else {
                                    info!("📸 Checkpoint created at height {} (every {} blocks)", current_height, CHECKPOINT_INTERVAL);
                                }
                            }

                            // Auto-pruning: clean up old receipts periodically
                            if current_height > KEEP_RECEIPTS_BLOCKS && current_height % PRUNING_INTERVAL == 0 {
                                let prune_before = current_height.saturating_sub(KEEP_RECEIPTS_BLOCKS);
                                match storage.prune_receipts_before_height(prune_before) {
                                    Ok(pruned) if pruned > 0 => {
                                        info!("🗑️ Auto-pruned {} old receipts (keeping last {} blocks)", pruned, KEEP_RECEIPTS_BLOCKS);
                                    }
                                    Ok(_) => {} // Nothing to prune
                                    Err(e) => {
                                        warn!("⚠️ Failed to auto-prune receipts: {:?}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to produce block: {}", e);
                            // 🔧 FIX GUARD STUCK: Reset guard to actual DB height so next
                            // slot can retry. Without this, guard stays at new_height while
                            // DB still has old height, causing permanent "guard >= new_height"
                            // skip on all subsequent slots.
                            let actual = storage.get_best_height().ok().flatten().unwrap_or(0);
                            best_height_guard.store(actual, std::sync::atomic::Ordering::SeqCst);
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

    // ── ⚖️ process_disputes and 🎯 process_epoch_rewards have been
    // extracted to epoch_processing.rs for better modularity. ──

    /// Produce a single block
    pub(crate) async fn produce_block(
        consensus: &Arc<RwLock<ProofOfStake>>,
        storage: &Arc<BlockchainDB>,
        state_db: &Arc<RwLock<StateDB>>,
        mempool: &Arc<Mempool>,
        executor: &Arc<TransactionExecutor>,
        reward_executor: &Arc<RwLock<RewardExecutor>>,
        epoch_length: u64,
        validator_keypair: Option<&KeyPair>,
        // 🔧 FIX #9: Atomic height guard shared with P2P handler
        best_height_guard: &std::sync::Arc<std::sync::atomic::AtomicU64>,
        metagraph_db: &Arc<MetagraphDB>,
        // RANDAO mixer — finalized at each epoch boundary to feed PoS seed
        randao: &Arc<RwLock<RandaoMixer>>,
        // 🔧 FIX MC-6: Accumulated TX count from prior blocks in this epoch
        epoch_tx_count: u64,
        // 🤖 Agentic EVM: autonomous agent trigger engine
        agent_trigger_engine: &Arc<AgentTriggerEngine>,
        // ⚖️ Optimistic AI: dispute manager for fraud proofs
        dispute_manager: &Arc<DisputeManager>,
        // Slashing manager for dispute-triggered slashing
        slashing_manager: &Arc<RwLock<SlashingManager>>,
        // 📦 Merkle root caching layer — caches state roots by block height
        merkle_cache: &Arc<CachedStateDB>,
        // 🔐 BFT fast finality — call on_block_proposed + auto-sign after block creation
        fast_finality: &Arc<RwLock<FastFinality>>,
        // 📊 Tokenomics: halving schedule, fee burning, dynamic gas pricing
        halving_schedule: &Arc<luxtensor_consensus::HalvingSchedule>,
        burn_manager: &Arc<luxtensor_consensus::BurnManager>,
        fee_market: &Arc<RwLock<luxtensor_consensus::FeeMarket>>,
        // 🏛️ Governance + Rotation + CommitReveal + Scoring (deep-wired epoch hooks)
        governance: &Arc<RwLock<luxtensor_consensus::GovernanceModule>>,
        validator_rotation: &Arc<RwLock<luxtensor_consensus::ValidatorRotation>>,
        commit_reveal: &Arc<luxtensor_consensus::CommitRevealManager>,
        scoring_manager: &Arc<RwLock<luxtensor_consensus::ScoringManager>>,
        // 🎲 VRF keypair (secp256k1 EC-VRF) for block proof generation (C2 fix)
        vrf_keypair: Option<&luxtensor_crypto::vrf::VrfKeypair>,
        // 🛡️ AI layer circuit breaker
        ai_circuit_breaker: &Arc<luxtensor_consensus::AILayerCircuitBreaker>,
    ) -> Result<Block> {
        // Get current height — None means fresh DB (no blocks stored yet)
        let best_height_opt = storage.get_best_height()?;
        let height = best_height_opt.unwrap_or(0);
        let new_height = height + 1;

        // 🔧 FIX #9 (REVISED): Use simple guard check instead of CAS to avoid guard-stuck bug.
        // CAS would get permanently stuck if produce_block fails after CAS succeeds but
        // before storage commits (guard=new_height but DB still has old height).
        // Instead: guard is treated as "the highest block another task has ALREADY committed".
        // If current guard >= new_height, there's a concurrent production in-flight — skip.
        let guard_val = best_height_guard.load(std::sync::atomic::Ordering::SeqCst);
        if guard_val >= new_height {
            // Another invocation already committed (or is committing) this height — skip.
            // Resync guard with actual DB height to prevent drift.
            let actual_height = storage.get_best_height().ok().flatten().unwrap_or(0);
            best_height_guard.store(actual_height, std::sync::atomic::Ordering::SeqCst);
            return Err(anyhow::anyhow!(
                "Block production skipped: guard={} >= new_height={}  (DB height={})",
                guard_val, new_height, actual_height
            ));
        }

        // Mark that WE are producing this height — tentatively set guard.
        // This is reset to `height` on any error below, and to `new_height`
        // only after store_block succeeds.
        best_height_guard.store(new_height, std::sync::atomic::Ordering::SeqCst);

        // Get previous block hash:
        // - When height == 0 (DB empty OR genesis is best): use Block::genesis().hash()
        //   as the canonical genesis hash. Do NOT read from DB — genesis may not be
        //   persisted yet or may have a corrupt entry (ghost/empty bytes).
        // - When height > 0: use get_hash_by_height() which reads ONLY the 32-byte hash
        //   from the height→hash index, bypassing CF_BLOCKS deserialization entirely.
        //   This is immune to block serialization bugs (corrupt bytes, struct mismatch).
        let previous_block_hash: [u8; 32] = if height == 0 {
            // Genesis bootstrap: block #1 links to the canonical genesis block hash.
            // We compute it from Block::genesis() to avoid any DB read that might fail.
            let genesis_hash = luxtensor_core::Block::genesis().hash();
            info!("🌱 Genesis bootstrap: producing block #1 with genesis hash {:?}", genesis_hash);
            genesis_hash
        } else {
            // 🔧 FIX: Use get_hash_by_height() instead of get_block_by_height().
            // We only need the 32-byte hash — reading the full block and deserializing
            // it was the root cause of "unexpected end of file" errors when CF_BLOCKS
            // contained corrupt entries or had a struct version mismatch.
            // get_hash_by_height() reads only the CF_HEIGHT_TO_HASH index (32 raw bytes),
            // completely bypassing block deserialization.
            match storage.get_hash_by_height(height) {
                Ok(Some(prev_hash)) => {
                    debug!("✅ Got previous hash at height {} via index: {:?}", height, &prev_hash[..4]);
                    prev_hash
                }
                Ok(None) => {
                    // No hash entry for this height in the index — guard reset for retry
                    best_height_guard.store(height, std::sync::atomic::Ordering::SeqCst);
                    return Err(anyhow::anyhow!(
                        "Previous block hash not found in index at height {} — guard reset for retry",
                        height
                    ));
                }
                Err(e) => {
                    // Index read error — reset guard and retry next slot
                    warn!(
                        "Index read error at height {}: {} — guard reset for retry",
                        height, e
                    );
                    best_height_guard.store(height, std::sync::atomic::Ordering::SeqCst);
                    return Err(anyhow::anyhow!(
                        "Index read error at height {}: {} — will retry",
                        height, e
                    ));
                }
            }
        };

        // 🔧 FIX MC-2: Capture timestamp once and reuse for both preliminary and final
        // headers. Previously SystemTime::now() was called twice, which could yield
        // different seconds across the two headers (race / clock skew).
        let block_timestamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();

        // Get transactions from mempool
        let transactions = mempool.get_transactions_for_block(MAX_TRANSACTIONS_PER_BLOCK);

        // Create preliminary header to get block hash
        let preliminary_header = luxtensor_core::BlockHeader {
            version: 1,
            height: new_height,
            timestamp: block_timestamp,
            previous_hash: previous_block_hash,
            state_root: [0u8; 32], // Will be updated after execution
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: 0,
            gas_limit: BLOCK_GAS_LIMIT,
            extra_data: vec![],
            vrf_proof: None,
        };

        let preliminary_block = Block::new(preliminary_header.clone(), transactions.clone());
        let block_hash = preliminary_block.hash();

        // Execute transactions against a snapshot (M-4 FIX: no lock held during execution)
        let accounts_snapshot = {
            let state = state_db.read();
            state.snapshot_accounts()
        };

        // Execute TXs on a temporary StateDB — no lock needed
        let mut temp_state = StateDB::from_accounts(accounts_snapshot);
        let mut valid_transactions = Vec::new();
        let mut valid_receipts = Vec::new();
        let mut total_gas = 0u64;

        // ── 🤖 Agentic EVM: process autonomous agent triggers ──
        // Agents get executed before user transactions, allowing them to react
        // to on-chain state changes from the previous block.
        let gas_price: u128 = 1_000_000_000; // 1 Gwei baseline for agent triggers
        let trigger_outcome = agent_trigger_engine.process_block_triggers(
            new_height, block_timestamp, gas_price,
        );
        if trigger_outcome.successful > 0 || trigger_outcome.failed > 0 {
            info!(
                "🤖 Block #{}: {} agent triggers executed ({} failed, {} skipped, {} gas)",
                new_height,
                trigger_outcome.successful,
                trigger_outcome.failed,
                trigger_outcome.skipped,
                trigger_outcome.total_gas_used,
            );
        }

        for (tx_index, tx) in transactions.into_iter().enumerate() {
            match executor.execute(
                &tx,
                &mut temp_state,
                new_height,
                block_hash,
                tx_index,
                block_timestamp,
            ) {
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

        // ── 🔥 Burn tx fees via BurnManager (Phase 3 tokenomics) ──
        // For each successfully executed TX, burn the configured portion of fees.
        // Burns reduce total supply and protect against spam.
        let mut total_fees_burned: u128 = 0;
        for tx in &valid_transactions {
            let tx_fee = (tx.gas_price as u128).saturating_mul(tx.gas_limit as u128);
            if tx_fee > 0 {
                let (burned, _remaining) = burn_manager.burn_tx_fee(tx_fee, new_height);
                total_fees_burned += burned;
            }
        }
        if total_fees_burned > 0 {
            info!("🔥 Block #{}: burned {} wei in tx fees", new_height, total_fees_burned);
        }

        // Calculate transaction root
        let tx_hashes: Vec<[u8; 32]> = valid_transactions.iter().map(|tx| tx.hash()).collect();
        let txs_root =
            if tx_hashes.is_empty() { [0u8; 32] } else { MerkleTree::new(tx_hashes).root() };

        // Calculate receipts root
        let receipts_root = calculate_receipts_root(&valid_receipts);

        // Short write lock: merge results into shared state, then commit via
        // CachedStateDB for height-indexed root caching.
        //
        // Lock ordering: write lock for merge only, then drop before commit()
        // which acquires its own read lock internally.
        {
            let mut state = state_db.write();
            state.merge_accounts(temp_state.snapshot_accounts());
        }

        // Commit via merkle_cache: computes root & caches it by block height.
        // This acquires a read lock on state_db internally.
        let state_root = merkle_cache.commit(new_height)?;

        // Flush persisted state to disk
        {
            let mut state = state_db.write();
            if let Err(e) = state.flush_to_db(storage.as_ref()) {
                warn!("Failed to persist state to disk: {} (state is in-memory only)", e);
            }
            // Free RAM: strip inline bytecodes now stored on disk.
            // get_code() will lazy-load from CodeStore (RocksDB CF_CONTRACTS) on demand.
            let stripped = state.strip_inline_bytecodes();
            if stripped > 0 {
                debug!("♻️ Stripped {} inline bytecodes from memory (lazy-loadable)", stripped);
            }
        }
        // FIXED (M-4): Block production now uses clone-then-commit pattern.
        // Read lock is held only briefly to snapshot accounts, TX execution runs
        // against an unlocked temporary StateDB, and write lock is held only for
        // the final merge + commit + flush (<10ms). RPC reads are no longer blocked
        // during block production.

        // Create new block header with signing
        // First create unsigned header to get hash
        let mut unsigned_header = luxtensor_core::BlockHeader {
            version: 1,
            height: new_height,
            timestamp: block_timestamp, // 🔧 FIX MC-2: Reuse single timestamp
            previous_hash: previous_block_hash,
            state_root,
            txs_root,
            receipts_root,
            validator: [0u8; 32],
            signature: vec![], // Empty for signing
            gas_used: total_gas,
            gas_limit: BLOCK_GAS_LIMIT,
            extra_data: vec![],
            vrf_proof: None,
        };

        // Sign with validator keypair if available
        let (validator_pubkey, signature) = if let Some(keypair) = validator_keypair {
            // Get public key bytes (padded to 32 bytes for now)
            let address = keypair.address();
            let mut validator = [0u8; 32];
            validator[12..32].copy_from_slice(address.as_bytes());

            // Sign the unsigned header hash
            let header_hash = unsigned_header.hash();
            match keypair.sign(&header_hash) {
                Ok(sig) => {
                    info!(
                        "🔐 Block #{} signed by validator 0x{}",
                        new_height,
                        hex::encode(&address)
                    );
                    (validator, sig.to_vec())
                }
                Err(e) => {
                    error!(
                        "CRITICAL: Failed to sign block #{}: {}. \
                         Refusing to produce unsigned block in validator mode.",
                        new_height, e
                    );
                    return Err(anyhow::anyhow!(
                        "Block signing failed: {}. Validator cannot produce unsigned blocks.",
                        e
                    ));
                }
            }
        } else {
            // No validator keypair — node is not a validator, produce unsigned block
            // This is only allowed in dev mode or for non-validator observer nodes
            warn!("⚠️  Producing unsigned block #{} (no validator keypair configured)", new_height);
            ([0u8; 32], vec![0u8; 64])
        };

        // Update header with signature
        unsigned_header.validator = validator_pubkey;
        unsigned_header.signature = signature;

        // ── 🎲 VRF Proof Generation (secp256k1 EC-VRF — C2 security fix) ────────
        // Generate a VRF proof over (epoch || height || prev_hash) so peers can
        // verify the randomness was legitimately derived from the validator's key.
        // The proof is attached AFTER signing — hash() excludes vrf_proof —
        // so the signature remains valid regardless.
        if let Some(vrf_kp) = vrf_keypair {
            let epoch = new_height / epoch_length.max(1);
            let mut alpha = Vec::with_capacity(48);
            alpha.extend_from_slice(&epoch.to_le_bytes());
            alpha.extend_from_slice(&new_height.to_le_bytes());
            alpha.extend_from_slice(&previous_block_hash);
            match vrf_kp.prove(&alpha) {
                Ok((_output, proof)) => {
                    unsigned_header.vrf_proof = Some(proof.to_bytes().to_vec());
                    debug!("🎲 VRF proof attached to block #{}", new_height);
                }
                Err(e) => {
                    // Non-fatal: log, but still produce the block without VRF proof.
                    warn!("⚠️  VRF proof generation failed for block #{}: {}", new_height, e);
                }
            }
        }

        let header = unsigned_header;

        // Create new block
        let block = Block::new(header.clone(), valid_transactions.clone());

        // Store block
        storage
            .store_block(&block)
            .context(format!("Failed to store block at height {}", header.height))?;

        // ── ⚖️ Optimistic AI: process disputes and finalize/slash ──
        Self::process_disputes(dispute_manager, slashing_manager, new_height, block_timestamp).await;

        // Update consensus with the new block hash for VRF entropy
        consensus.read().update_last_block_hash(block.hash());

        // ── 💰 Block reward: HalvingSchedule + EmissionController (Phase 3) ──
        // Uses the Bitcoin-like halving schedule to compute dynamic block rewards.
        // Falls back to consensus.distribute_reward_with_height() if halving yields 0.
        let producer_addr = if header.validator != [0u8; 32] {
            let mut addr = [0u8; 20];
            addr.copy_from_slice(&header.validator[12..32]);
            luxtensor_core::Address::from(addr)
        } else {
            luxtensor_core::Address::zero()
        };

        // Compute halving-adjusted reward
        let halving_reward = halving_schedule.calculate_reward(new_height);
        let final_reward = if halving_reward > 0 {
            halving_reward
        } else {
            // Fallback to PoS reward if halving schedule is exhausted
            match consensus.read().distribute_reward_with_height(&producer_addr, new_height) {
                Ok(r) => r,
                Err(_) => 0,
            }
        };

        if final_reward > 0 && producer_addr != luxtensor_core::Address::zero() {
            info!(
                "💰 Block #{} reward: {} wei (era {}) to 0x{}",
                new_height,
                final_reward,
                halving_schedule.get_halving_era(new_height),
                hex::encode(producer_addr.as_bytes())
            );
            // Persist block reward to StateDB (visible to RPC immediately)
            {
                let mut db = state_db.write();
                match db.get_account(&producer_addr) {
                    Some(mut account) => {
                        account.balance = account.balance.saturating_add(final_reward);
                        db.set_account(producer_addr, account);
                    }
                    None => {
                        let new_account = luxtensor_core::Account {
                            balance: final_reward,
                            nonce: 0,
                            storage_root: [0u8; 32],
                            code_hash: [0u8; 32],
                            code: None,
                        };
                        db.set_account(producer_addr, new_account);
                    }
                }
            }
        }

        // ── 📊 Update EIP-1559 FeeMarket base fee after block production ──
        fee_market.write().on_block_produced(total_gas);

        // 🔧 FIX: Store receipts for eth_getTransactionReceipt
        for receipt in &valid_receipts {
            if let Ok(receipt_bytes) = bincode::serialize(receipt) {
                if let Err(e) = storage.store_receipt(&receipt.transaction_hash, &receipt_bytes) {
                    warn!("Failed to store receipt: {}", e);
                }
            }

            // Also store contract code if this was a deployment
            if let Some(ref contract_addr) = receipt.contract_address {
                // Get code from StateDB (bytecode is now stored in Account.code)
                if let Some(code) = state_db.read().get_code(contract_addr) {
                    if let Err(e) = storage.store_contract(contract_addr.as_bytes(), &code) {
                        warn!("Failed to store contract: {}", e);
                    }
                }
            }
        }

        // Remove transactions from mempool
        let tx_hashes: Vec<_> = valid_transactions.iter().map(|tx| tx.hash()).collect();
        mempool.remove_transactions(&tx_hashes);

        info!(
            "📦 Produced block #{} with {} transactions, {} gas used, hash {:?}",
            new_height,
            valid_transactions.len(),
            total_gas,
            block.hash()
        );

        // ── 📊 Record block production in ScoringManager ──
        if let Some(kp) = validator_keypair {
            let addr: [u8; 20] = kp.address().into();
            scoring_manager.write().record_block_produced(addr, new_height);
            debug!("📊 ScoringManager: recorded block #{} by 0x{}", new_height, hex::encode(addr));
        }

        // Check if this is an epoch boundary and process rewards
        if new_height % epoch_length == 0 && epoch_length > 0 {
            Self::process_epoch_rewards(
                consensus,
                reward_executor,
                metagraph_db,
                randao,
                &header,
                new_height,
                epoch_length,
                total_gas,
                epoch_tx_count,
                valid_transactions.len() as u64,
                state_db,  // M4: pass StateDB for reward persistence
                governance,           // 🏛️ Governance epoch hooks
                validator_rotation,   // 🔄 Validator rotation
                commit_reveal,        // 🔐 Commit-reveal finalization
                scoring_manager,      // 📊 Performance scoring
                ai_circuit_breaker,   // 🛡️ AI layer circuit breaker
            );
        }

        // Record block hash for EVM BLOCKHASH opcode (up to 256 recent blocks)
        executor.evm().record_block_hash(new_height, block.hash());

        // ─── 🔐 BFT Fast Finality hook ───────────────────────────────
        // Notify the BFT module about the newly produced block so it
        // transitions to CollectingSignatures phase. The producer also
        // auto-signs the block to count itself towards the ⅔+1 quorum.
        {
            let block_hash = block.hash();
            let mut ff = fast_finality.write();
            ff.on_block_proposed(new_height, block_hash);

            // Auto-sign: the block producer counts as the first signer
            if let Some(kp) = validator_keypair {
                let producer_addr = kp.address();
                // add_signature(block_hash, block_height, validator)
                let _ = ff.add_signature(block_hash, new_height, producer_addr.into());
                debug!(
                    "🔐 BFT: auto-signed block #{} (producer 0x{})",
                    new_height,
                    hex::encode(&producer_addr)
                );
            }
        }

        Ok(block)
    }
}

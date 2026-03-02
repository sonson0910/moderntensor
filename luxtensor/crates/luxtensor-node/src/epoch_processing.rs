//! Epoch boundary processing extracted from `block_production.rs`.
//!
//! Contains:
//! - `process_disputes`: optimistic AI dispute resolution and slashing
//! - `process_epoch_rewards`: epoch boundary reward distribution, RANDAO finalization,
//!   governance, validator rotation, commit-reveal, and scoring

use crate::service::{NodeService, BLOCK_GAS_LIMIT};

use luxtensor_consensus::randao::RandaoMixer;
use luxtensor_consensus::slashing::SlashingManager;
use luxtensor_consensus::{
    DelegatorInfo, MinerInfo, ProofOfStake, RewardExecutor, SubnetInfo, UtilityMetrics,
    ValidatorInfo, YumaConsensus,
};
use luxtensor_core::StateDB;
use luxtensor_oracle::DisputeManager;
use luxtensor_storage::metagraph_store::MetagraphDB;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, info, warn};

impl NodeService {
    /// ── ⚖️ Optimistic AI: process disputes and apply slashing ──
    ///
    /// Extracted from `produce_block` for readability and testability.
    /// Run after the block is stored so all state is committed.
    pub(crate) async fn process_disputes(
        dispute_manager: &Arc<DisputeManager>,
        slashing_manager: &Arc<RwLock<SlashingManager>>,
        new_height: u64,
        block_timestamp: u64,
    ) {
        let slash_percent = slashing_manager.read().config().fraudulent_ai_slash_percent;
        let dispute_outcome = dispute_manager.process_block(new_height, slash_percent).await;
        if dispute_outcome.finalized_count > 0 || dispute_outcome.disputes_verified > 0 {
            info!(
                "⚖️ Block #{}: {} results finalized, {} disputes verified, {} rejected",
                new_height,
                dispute_outcome.finalized_count,
                dispute_outcome.disputes_verified,
                dispute_outcome.rejected_disputes,
            );
        }
        // Apply slashing for miners proven fraudulent via SlashingManager
        for (miner_addr, _slash_amount) in &dispute_outcome.slashed_miners {
            let miner_address = luxtensor_core::Address::from(*miner_addr);
            let evidence = luxtensor_consensus::slashing::SlashingEvidence {
                validator: miner_address,
                reason: luxtensor_consensus::slashing::SlashReason::FraudulentAI,
                height: new_height,
                evidence_hash: None,
                timestamp: block_timestamp,
            };
            match slashing_manager.write().slash(evidence, new_height) {
                Ok(event) => {
                    info!(
                        "⚖️ Slashed miner 0x{} for {} wei (fraudulent AI result, jailed: {})",
                        hex::encode(miner_addr),
                        event.amount_slashed,
                        event.jailed,
                    );
                }
                Err(e) => {
                    warn!(
                        "⚠️ Failed to slash miner 0x{} for FraudulentAI: {}",
                        hex::encode(miner_addr),
                        e,
                    );
                }
            }
        }
    }

    /// ── 🎯 Epoch boundary: compute metrics, distribute rewards, finalize RANDAO ──
    ///
    /// Extracted from `produce_block` for readability and testability (~130 lines).
    pub(crate) fn process_epoch_rewards(
        consensus: &Arc<RwLock<ProofOfStake>>,
        reward_executor: &Arc<RwLock<RewardExecutor>>,
        metagraph_db: &Arc<MetagraphDB>,
        randao: &Arc<RwLock<RandaoMixer>>,
        header: &luxtensor_core::BlockHeader,
        new_height: u64,
        epoch_length: u64,
        total_gas: u64,
        epoch_tx_count: u64,
        valid_tx_count: u64,
        // M4: StateDB reference for persistent reward crediting
        state_db: &Arc<RwLock<StateDB>>,
        // 🏛️ Governance + Rotation + CommitReveal + Scoring (deep-wired epoch hooks)
        governance: &Arc<RwLock<luxtensor_consensus::GovernanceModule>>,
        validator_rotation: &Arc<RwLock<luxtensor_consensus::ValidatorRotation>>,
        commit_reveal: &Arc<luxtensor_consensus::CommitRevealManager>,
        scoring_manager: &Arc<RwLock<luxtensor_consensus::ScoringManager>>,
        ai_circuit_breaker: &Arc<luxtensor_consensus::AILayerCircuitBreaker>,
    ) {
        let epoch_num = new_height / epoch_length;
        info!("🎯 Epoch {} completed at block #{}, processing rewards...", epoch_num, new_height);

        // Create utility metrics for this epoch
        let actual_utilization = ((total_gas as f64 / BLOCK_GAS_LIMIT as f64) * 100.0) as u32;

        // Query metagraph for active validators and neurons
        let metagraph_validators = metagraph_db.get_all_validators().unwrap_or_default();
        let metagraph_subnets = metagraph_db.get_all_subnets().unwrap_or_default();
        let metagraph_delegations = metagraph_db.get_all_delegations().unwrap_or_default();

        let active_validator_count = metagraph_validators.iter().filter(|v| v.is_active).count();

        let utility = UtilityMetrics {
            active_validators: active_validator_count.max(1) as u64,
            active_subnets: metagraph_subnets.len().max(1) as u64,
            // 🔧 FIX MC-6: Use accumulated epoch TX count (prior blocks + this block)
            epoch_transactions: epoch_tx_count + valid_tx_count,
            epoch_ai_tasks: 0, // Tracked via MetagraphDB AI task store
            block_utilization: actual_utilization.min(100) as u8,
        };

        // ── 🧠 STEP 1: SAC Yuma Consensus (BEFORE reward) ────────────────────
        // Compute trust/rank/incentive/dividends from weight matrix FIRST so
        // rewards use *current* epoch scores instead of stale data.
        let yuma_updates = {
            let updates = YumaConsensus::compute(metagraph_db, epoch_num);
            if !updates.is_empty() {
                YumaConsensus::apply_updates(metagraph_db, updates.clone(), epoch_num);
                info!(
                    "🧠 Yuma consensus: updated {} neurons for epoch {}",
                    metagraph_db.get_all_subnets().map(|s| s.len()).unwrap_or(0),
                    epoch_num
                );
            } else {
                debug!("⚠️  YumaConsensus: no updates for epoch {} (no weights set?)", epoch_num);
            }
            updates
        };

        // ── 🎯 STEP 2: Build miner/validator lists with FRESH scores ──────────
        // Re-read neurons AFTER Yuma update so incentive scores are current.
        let mut miners: Vec<MinerInfo> = Vec::new();
        for subnet in &metagraph_subnets {
            let neurons = metagraph_db.get_neurons_by_subnet(subnet.id).unwrap_or_default();
            for neuron in &neurons {
                if neuron.active {
                    let score = neuron.incentive as f64 / 65535.0;
                    miners.push(MinerInfo {
                        address: neuron.hotkey,
                        score: if score > 0.0 { score } else { 0.01 },
                    });
                }
            }
        }

        // Build validator list from metagraph
        let validators: Vec<ValidatorInfo> = metagraph_validators
            .iter()
            .filter(|v| v.is_active && v.stake > 0)
            .map(|v| ValidatorInfo { address: v.address, stake: v.stake })
            .collect();

        // Build delegator list from metagraph
        let delegators: Vec<DelegatorInfo> = metagraph_delegations
            .iter()
            .map(|d| DelegatorInfo {
                address: d.delegator,
                stake: d.amount,
                lock_days: d.lock_days,
            })
            .collect();

        // Build subnet list for emission
        let subnets: Vec<SubnetInfo> = metagraph_subnets
            .iter()
            .map(|s| SubnetInfo { owner: s.owner, emission_weight: s.emission_rate })
            .collect();

        // Fallback: if metagraph is empty (bootstrapping), use block producer
        let miners = if miners.is_empty() {
            let miner_addr = if header.validator != [0u8; 32] {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&header.validator[12..32]);
                addr
            } else {
                [0u8; 20]
            };
            vec![MinerInfo { address: miner_addr, score: 1.0 }]
        } else {
            miners
        };
        let validators = if validators.is_empty() {
            let miner_addr = if header.validator != [0u8; 32] {
                let mut addr = [0u8; 20];
                addr.copy_from_slice(&header.validator[12..32]);
                addr
            } else {
                [0u8; 20]
            };
            vec![ValidatorInfo { address: miner_addr, stake: 1000 }]
        } else {
            validators
        };

        // ── 💰 STEP 3: Process epoch rewards (using fresh Yuma scores) ────────
        let result = reward_executor.write().process_epoch(
            epoch_num,
            new_height,
            &utility,
            &miners,
            &validators,
            &delegators,
            &subnets,
        );

        info!(
            "💰 Epoch {} rewards distributed: {} total emission, {} participants, {} DAO",
            epoch_num, result.total_emission, result.participants_rewarded, result.dao_allocation
        );

        // ── M4: Flush epoch pending rewards → StateDB (persistent storage) ──
        {
            let snapshot = reward_executor.read().pending_rewards_snapshot();
            if !snapshot.is_empty() {
                let mut db = state_db.write();
                for (addr_bytes, amount) in &snapshot {
                    let addr = luxtensor_core::Address::from(*addr_bytes);
                    match db.get_account(&addr) {
                        Some(mut account) => {
                            account.balance = account.balance.saturating_add(*amount);
                            db.set_account(addr, account);
                        }
                        None => {
                            // Participant not yet in StateDB — create account with reward balance
                            let new_account = luxtensor_core::Account {
                                balance: *amount,
                                nonce: 0,
                                storage_root: [0u8; 32],
                                code_hash: [0u8; 32],
                                code: None,
                            };
                            db.set_account(addr, new_account);
                        }
                    }
                }
                // set_account is infallible — always drain to avoid double-crediting
                drop(db); // release write lock before re-acquiring read below
                reward_executor.read().drain_pending_rewards();
                info!(
                    "✅ Epoch {} rewards flushed to StateDB: {} accounts credited",
                    epoch_num,
                    snapshot.len()
                );
            }
        }

        // Finalize RANDAO mix for this epoch and feed it into PoS seed.
        match randao.write().finalize_epoch() {
            Ok(mix) => {
                consensus.read().update_randao_mix(mix);
                info!("🎲 Epoch {} RANDAO mix finalized: {:?}", epoch_num, &mix[..8]);

                // Pre-compute the next epoch's committee using the finalized
                // RANDAO mix. This makes committee membership unpredictable
                // until the epoch boundary and deterministic afterwards.
                let pos = consensus.read();
                let validator_addrs: Vec<luxtensor_core::Address> = pos
                    .validator_set()
                    .read()
                    .active_validators()
                    .iter()
                    .map(|v| v.address)
                    .collect();
                if !validator_addrs.is_empty() {
                    let block_hash = header.hash();
                    let committee_size = validator_addrs.len().min(21); // cap at 21 BFT members
                    let committee = luxtensor_consensus::weight_consensus::select_committee_with_randao(
                        &validator_addrs,
                        committee_size,
                        &block_hash,
                        0, // subnet 0 = main chain
                        &mix,
                    );
                    info!(
                        "🗳️ Epoch {} committee selected ({}/{} validators, RANDAO-seeded)",
                        epoch_num + 1, committee.len(), validator_addrs.len()
                    );
                    debug!(
                        "🗳️ Committee members: {:?}",
                        committee.iter().map(|a| hex::encode(a.as_bytes())).collect::<Vec<_>>()
                    );
                }
            }
            Err(e) => {
                debug!("⚠️  RANDAO finalize skipped for epoch {}: {}", epoch_num, e);
            }
        }

        // ── 🏛️ STEP 4: Governance epoch housekeeping ─────────────────────
        {
            let gov = governance.read();

            // 4a — Tally votes for proposals past their voting deadline
            let active =
                gov.list_proposals(Some(luxtensor_consensus::governance::ProposalStatus::Active));
            let mut finalized_count = 0u32;
            for proposal in &active {
                if new_height > proposal.voting_deadline {
                    match gov.finalize_voting(proposal.id, new_height) {
                        Ok(status) => {
                            info!(
                                "🏛️ Governance: proposal #{} finalized → {:?} at epoch {}",
                                proposal.id, status, epoch_num
                            );
                            finalized_count += 1;
                        }
                        Err(e) => {
                            debug!(
                                "🏛️ Governance: proposal #{} finalize skipped: {}",
                                proposal.id, e
                            );
                        }
                    }
                }
            }
            if finalized_count > 0 {
                info!(
                    "🏛️ Governance: {} proposals vote-tallied at epoch {}",
                    finalized_count, epoch_num
                );
            }

            // 4b — Expire proposals past their absolute expiry
            let expired = gov.expire_stale(new_height);
            if !expired.is_empty() {
                info!("🏛️ Governance: {} proposals expired at epoch {}", expired.len(), epoch_num);
            }

            // 4c — GC terminal proposals older than 10 epochs
            let retain_blocks = epoch_length * 10;
            let cleaned = gov.cleanup_finalized(new_height, retain_blocks);
            if cleaned > 0 {
                info!("🏛️ Governance: cleaned up {} finalized proposals", cleaned);
            }
        }

        // ── 🔄 STEP 5: Validator rotation at epoch boundary ──────────────
        {
            let result = validator_rotation.write().process_epoch_transition(epoch_num);
            info!(
                "🔄 Epoch {} rotation: {} activated, {} exited",
                epoch_num,
                result.activated_validators.len(),
                result.exited_validators.len()
            );
        }

        // ── 🔐 STEP 6: Commit-reveal finalization for all active subnets ─
        {
            let subnets = metagraph_db.get_all_subnets().unwrap_or_default();
            for subnet in &subnets {
                // 🛡️ Wrap commit-reveal in circuit breaker to prevent cascade failures
                if !ai_circuit_breaker.commit_reveal.allow_request() {
                    warn!(
                        "🛡️ Circuit breaker OPEN for commit-reveal — skipping subnet {} finalization",
                        subnet.id
                    );
                    continue;
                }
                match commit_reveal.finalize_epoch_with_slashing(subnet.id, new_height) {
                    Ok(result) => {
                        ai_circuit_breaker.commit_reveal.record_success();
                        if result.has_slashing() {
                            warn!(
                                "🔐 Subnet {} commit-reveal: {} validators slashed at epoch {}",
                                subnet.id,
                                result.slashed_validators().len(),
                                epoch_num
                            );
                        } else {
                            debug!(
                                "🔐 Subnet {} commit-reveal finalized for epoch {}",
                                subnet.id, epoch_num
                            );
                        }
                    }
                    Err(_) => {
                        // Expected failures (no active epoch) are NOT circuit breaker failures
                        debug!("🔐 Subnet {} commit-reveal: no finalization needed", subnet.id);
                    }
                }
                // Start next epoch for this subnet (commit window opens)
                commit_reveal.start_epoch(subnet.id, epoch_num + 1, new_height);
            }
        }

        // ── 📊 STEP 7: Scoring housekeeping ──────────────────────────────
        {
            // H1 FIX: Reuse Yuma results from STEP 1 instead of recomputing.
            // Computing twice could produce different results since STEP 1 modified metagraph.
            if !yuma_updates.is_empty() {
                let yuma_scores: Vec<([u8; 20], u32)> = yuma_updates
                    .iter()
                    .filter_map(|u| {
                        let neurons =
                            metagraph_db.get_neurons_by_subnet(u.subnet_id).unwrap_or_default();
                        neurons
                            .iter()
                            .find(|n| n.uid as u64 == u.uid)
                            .map(|n| (n.hotkey, u.incentive))
                    })
                    .collect();
                scoring_manager.write().merge_yuma_output(&yuma_scores);
                debug!("📊 Merged {} Yuma scores into ScoringManager", yuma_scores.len());
            }

            // Reset per-epoch counters
            scoring_manager.write().reset_epoch_stats();

            // Evict inactive participants (not seen for 100 epochs)
            let max_inactive = epoch_length * 100;
            scoring_manager.write().evict_inactive(new_height, max_inactive);
        }
    }
}

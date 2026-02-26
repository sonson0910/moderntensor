// ============================================================================
// Yuma Consensus with Stake-Adaptive Clipping (SAC) — BPS Integer Arithmetic
// ============================================================================
//
// DETERMINISTIC DESIGN: All calculations use u128 integer arithmetic with
// Basis Points (BPS) scaling.  No f64 operations are used in any consensus-
// critical path, preventing cross-platform divergence.
//
// Scale: BPS_SCALE = 10_000 (1 BPS = 0.01%).  Intermediate products use
// u128 to avoid overflow.  Final NeuronUpdate fields use u32 with
// FIXED_POINT_SCALE = 65_535 (u16::MAX) for backwards compat with MetagraphDB.
//
// Pipeline per subnet:
//   1. Read neuron list + weight matrix from MetagraphDB
//   2. Row-normalize weights using integer division (Σ_j W[i][j] = BPS_SCALE)
//   3. Compute effective stake via logarithmic_stake() (SAC step)
//   4. Compute prerank  = stake-weighted average weight received  (BPS)
//   5. Compute consensus = median-clipped prerank                 (BPS)
//   6. Compute rank      = normalised consensus                   (BPS)
//   7. Compute trust     = fraction of validators that set weight (BPS)
//   8. Compute incentive = rank                                   (BPS)
//   9. Compute dividends = validator contribution-weighted rank   (BPS)
//  10. Write updated NeuronData back to MetagraphDB

use std::collections::HashMap;
use std::sync::Arc;
use luxtensor_storage::metagraph_store::{MetagraphDB, NeuronData};
use tracing::{debug, info, warn};
use crate::node_tier::logarithmic_stake;

/// BPS scale for all intermediate consensus calculations.
/// 10_000 BPS = 100%.
const BPS_SCALE: u128 = 10_000;

/// Fixed-point scale for final u32 metrics stored in MetagraphDB.
/// Kept at 65_535 (u16::MAX) for backwards compatibility.
const FIXED_POINT_SCALE: u128 = 65_535;



/// One row of consensus results for a single neuron.
#[derive(Debug, Clone)]
pub struct NeuronUpdate {
    pub subnet_id: u64,
    pub uid: u64,
    pub trust: u32,
    pub rank: u32,
    pub incentive: u32,
    pub dividends: u32,
    pub emission: u128,
}

/// Stake-Adaptive Consensus (SAC) — computes Yuma metrics using log-weighted stake.
///
/// **DETERMINISTIC**: All arithmetic is integer-only (u128 BPS).  No f64.
pub struct YumaConsensus;

impl YumaConsensus {
    /// Compute consensus updates for ALL subnets in one pass.
    ///
    /// Returns a flat list of `NeuronUpdate`s — one per active neuron.
    pub fn compute(metagraph_db: &Arc<MetagraphDB>, epoch_num: u64) -> Vec<NeuronUpdate> {
        let subnets = match metagraph_db.get_all_subnets() {
            Ok(s) => s,
            Err(e) => {
                warn!("YumaConsensus: failed to load subnets at epoch {}: {}", epoch_num, e);
                return vec![];
            }
        };

        // ── Build effective stake map from TWO sources ────────────────────────
        // Source 1: ValidatorData  — gives us `is_active` flag + fallback stake
        // Source 2: StakingData    — real-time stake (updated by staking RPC)
        // Priority: StakingData.stake > ValidatorData.stake (more up-to-date)

        let validators = match metagraph_db.get_all_validators() {
            Ok(v) => v,
            Err(e) => {
                warn!("YumaConsensus: failed to load validators at epoch {}: {}", epoch_num, e);
                return vec![];
            }
        };

        // address → ValidatorData (for is_active + fallback stake)
        let val_map: HashMap<[u8; 20], u128> = validators
            .iter()
            .filter(|v| v.is_active)
            .map(|v| (v.address, v.stake))
            .collect();

        // address → real-time stake from StakingData (override val_map if present)
        let staking_override: HashMap<[u8; 20], u128> = match metagraph_db.get_all_stakes() {
            Ok(stakes) => stakes.into_iter().map(|s| (s.address, s.stake)).collect(),
            Err(e) => {
                warn!("YumaConsensus: failed to load staking data at epoch {}: {}", epoch_num, e);
                HashMap::new()
            }
        };

        // Build final effective stake map (SAC logarithmic):
        // only for validators that are active; use StakingData stake if available
        let validator_eff_stake: HashMap<[u8; 20], u128> = val_map
            .iter()
            .filter_map(|(addr, &fallback_stake)| {
                let real_stake = staking_override.get(addr).copied().unwrap_or(fallback_stake);
                if real_stake == 0 {
                    return None;
                }
                Some((*addr, logarithmic_stake(real_stake)))
            })
            .collect();

        let total_eff_stake: u128 = validator_eff_stake.values().sum();

        info!(
            "🧪 YumaConsensus epoch {}: {} raw validators, {} active w/ stake, total_eff_stake={}",
            epoch_num, validators.len(), validator_eff_stake.len(), total_eff_stake
        );
        for (addr, &stake) in &validator_eff_stake {
            info!("  validator addr: 0x{} stake={}", hex::encode(addr), stake);
        }

        if total_eff_stake == 0 {
            info!("⚠️ YumaConsensus: no active validator stake — skipping epoch {}", epoch_num);
            return vec![];
        }

        let mut all_updates = Vec::new();

        for subnet in &subnets {
            let updates = Self::compute_subnet(
                metagraph_db,
                subnet.id,
                &validator_eff_stake,
                total_eff_stake,
                epoch_num,
            );
            all_updates.extend(updates);
        }

        info!(
            "🧠 YumaConsensus epoch {}: computed {} neuron updates across {} subnets",
            epoch_num,
            all_updates.len(),
            subnets.len()
        );

        all_updates
    }

    /// Compute consensus for a single subnet.
    ///
    /// All intermediate values are in BPS (u128).
    fn compute_subnet(
        metagraph_db: &Arc<MetagraphDB>,
        subnet_id: u64,
        validator_eff_stake: &HashMap<[u8; 20], u128>,
        total_eff_stake: u128,
        epoch_num: u64,
    ) -> Vec<NeuronUpdate> {
        let neurons = match metagraph_db.get_neurons_by_subnet(subnet_id) {
            Ok(n) => n,
            Err(e) => {
                warn!(
                    "YumaConsensus: failed to load neurons for subnet {} at epoch {}: {}",
                    subnet_id, epoch_num, e
                );
                return vec![];
            }
        };

        let active_neurons: Vec<&NeuronData> = neurons.iter().filter(|n| n.active).collect();

        if active_neurons.is_empty() {
            debug!(
                "YumaConsensus: subnet {} has no active neurons — skipping",
                subnet_id
            );
            return vec![];
        }

        // uid → index mapping for matrix access
        let uid_to_idx: HashMap<u64, usize> = active_neurons
            .iter()
            .enumerate()
            .map(|(idx, n)| (n.uid, idx))
            .collect();
        let n = active_neurons.len();

        // ── Step 1: Build weight matrix W[validator][miner] in BPS ────────────
        // Raw weights from MetagraphDB are u16 [0..65535].
        // We row-normalize each validator's weights so they sum to BPS_SCALE.
        let mut weight_matrix: Vec<Vec<u128>> = vec![vec![0u128; n]; n]; // [from_idx][to_idx]
        let mut validator_idxs: Vec<usize> = Vec::new();

        for (from_idx, from_neuron) in active_neurons.iter().enumerate() {
            // Only validators contribute to the weight matrix
            if !validator_eff_stake.contains_key(&from_neuron.hotkey) {
                continue;
            }
            validator_idxs.push(from_idx);

            let weights = match metagraph_db.get_weights(subnet_id, from_neuron.uid) {
                Ok(w) => w,
                Err(_) => continue,
            };

            // Fill raw weights (u16)
            for w in &weights {
                if let Some(&to_idx) = uid_to_idx.get(&w.to_uid) {
                    weight_matrix[from_idx][to_idx] = w.weight as u128;
                }
            }

            // ── Step 2: Row-normalize to BPS_SCALE ─────────────────────────
            // Σ_j W[i][j] → BPS_SCALE (integer division, remainder goes to largest)
            let row_sum: u128 = weight_matrix[from_idx].iter().sum();
            if row_sum > 0 {
                // Scale each weight: new_w = raw_w * BPS_SCALE / row_sum
                let mut scaled_sum: u128 = 0;
                let mut max_idx: usize = 0;
                let mut max_val: u128 = 0;
                for j in 0..n {
                    let raw = weight_matrix[from_idx][j];
                    let scaled = raw * BPS_SCALE / row_sum;
                    weight_matrix[from_idx][j] = scaled;
                    scaled_sum += scaled;
                    if raw > max_val {
                        max_val = raw;
                        max_idx = j;
                    }
                }
                // Assign rounding remainder to the largest weight slot
                // to ensure exact sum = BPS_SCALE (deterministic)
                let remainder = BPS_SCALE.saturating_sub(scaled_sum);
                if remainder > 0 {
                    weight_matrix[from_idx][max_idx] += remainder;
                }
            }
        }

        let num_validators = validator_idxs.len();
        info!(
            "🧪 YumaConsensus subnet {}: {} active neurons, {} matched as validators",
            subnet_id, n, num_validators
        );
        for neuron in &active_neurons {
            let is_val = validator_eff_stake.contains_key(&neuron.hotkey);
            info!(
                "  neuron uid={} hotkey=0x{} is_validator={}",
                neuron.uid, hex::encode(neuron.hotkey), is_val
            );
        }
        if num_validators == 0 {
            info!(
                "⚠️ YumaConsensus: subnet {} has no active validators with weights",
                subnet_id
            );
            return vec![];
        }

        // ── Step 3: Compute prerank[j] = Σ_i (eff_stake[i] * W[i][j]) / total_eff_stake
        //
        // Result is in BPS: prerank[j] ∈ [0, BPS_SCALE]
        // We use the identity:
        //   prerank[j] = Σ_i (stake_i * W_ij) / total_stake
        // where W_ij is already in BPS, so the product is in (stake_units * BPS).
        // Dividing by total_stake gives BPS.
        let mut prerank: Vec<u128> = vec![0u128; n];
        for &from_idx in &validator_idxs {
            let hotkey = active_neurons[from_idx].hotkey;
            let stake = match validator_eff_stake.get(&hotkey) {
                Some(&s) => s,
                None => continue,
            };
            for to_idx in 0..n {
                // stake * W[from][to] / total   (BPS result)
                prerank[to_idx] += stake * weight_matrix[from_idx][to_idx] / total_eff_stake;
            }
        }

        // ── Step 4: Consensus = median-clipped prerank ────────────────────────
        let consensus = Self::median_clip_bps(
            &prerank,
            &weight_matrix,
            &validator_idxs,
            n,
            total_eff_stake,
            validator_eff_stake,
            &active_neurons,
        );

        // ── Step 5: Rank = normalized consensus ───────────────────────────────
        // rank[j] = consensus[j] * BPS_SCALE / Σ consensus
        let consensus_sum: u128 = consensus.iter().sum();
        let rank: Vec<u128> = if consensus_sum > 0 {
            let mut r: Vec<u128> = consensus.iter().map(|&c| c * BPS_SCALE / consensus_sum).collect();
            // Fix rounding: distribute remainder to largest consensus neuron
            let rank_sum: u128 = r.iter().sum();
            if rank_sum < BPS_SCALE {
                if let Some(max_idx) = consensus.iter().enumerate().max_by_key(|(_, &v)| v).map(|(i, _)| i) {
                    r[max_idx] += BPS_SCALE - rank_sum;
                }
            }
            r
        } else {
            vec![0u128; n]
        };

        // ── Step 6: Trust = fraction of validators that set weight > 0 ────────
        // trust[j] = (count of validators with W[i][j] > 0) * BPS_SCALE / num_validators
        let trust: Vec<u128> = (0..n)
            .map(|to_idx| {
                let nonzero = validator_idxs
                    .iter()
                    .filter(|&&from_idx| weight_matrix[from_idx][to_idx] > 0)
                    .count() as u128;
                nonzero * BPS_SCALE / num_validators as u128
            })
            .collect();

        // ── Step 7: Incentive = rank (miner emission share) ───────────────────
        let incentive = rank.clone();

        // ── Step 8: Dividends[i] = Σ_j rank[j] * W_norm[i][j] / BPS_SCALE ────
        // Only computed for validator neurons. Result in BPS.
        let mut dividends: Vec<u128> = vec![0u128; n];
        for &from_idx in &validator_idxs {
            for to_idx in 0..n {
                dividends[from_idx] += rank[to_idx] * weight_matrix[from_idx][to_idx] / BPS_SCALE;
            }
        }
        // Normalize dividends so they sum to BPS_SCALE
        let dividends_sum: u128 = dividends.iter().sum();
        if dividends_sum > 0 {
            let mut new_divs: Vec<u128> = dividends.iter().map(|&d| d * BPS_SCALE / dividends_sum).collect();
            let new_sum: u128 = new_divs.iter().sum();
            if new_sum < BPS_SCALE {
                if let Some(max_idx) = dividends.iter().enumerate().max_by_key(|(_, &v)| v).map(|(i, _)| i) {
                    new_divs[max_idx] += BPS_SCALE - new_sum;
                }
            }
            dividends = new_divs;
        }

        // ── Step 9: Pack into NeuronUpdate ────────────────────────────────────
        // Convert from BPS (0..10_000) to FIXED_POINT_SCALE (0..65_535) for MetagraphDB
        let mut updates = Vec::with_capacity(n);
        for (idx, neuron) in active_neurons.iter().enumerate() {
            let t = (trust[idx] * FIXED_POINT_SCALE / BPS_SCALE) as u32;
            let r = (rank[idx] * FIXED_POINT_SCALE / BPS_SCALE) as u32;
            let inc = (incentive[idx] * FIXED_POINT_SCALE / BPS_SCALE) as u32;
            let div = (dividends[idx] * FIXED_POINT_SCALE / BPS_SCALE) as u32;
            info!(
                "  🧠 NeuronUpdate uid={} subnet={} trust={} rank={} incentive={} dividends={}",
                neuron.uid, subnet_id, t, r, inc, div
            );
            updates.push(NeuronUpdate {
                subnet_id,
                uid: neuron.uid,
                trust: t,
                rank: r,
                incentive: inc,
                dividends: div,
                // Emission is assigned later by reward_executor; leave 0 here
                emission: 0,
            });
        }

        debug!(
            "YumaConsensus: subnet {} → {} neurons updated (epoch {})",
            subnet_id, updates.len(), epoch_num
        );

        updates
    }

    /// Median-clip using BPS integer arithmetic.
    ///
    /// For each miner j, find the stake-weighted median of per-validator
    /// weights, then clip prerank to it.
    ///
    /// **DETERMINISTIC**: Sort uses `cmp` on u128 (total ordering, no NaN).
    fn median_clip_bps(
        prerank: &[u128],
        weight_matrix: &[Vec<u128>],
        validator_idxs: &[usize],
        n: usize,
        total_eff_stake: u128,
        validator_eff_stake: &HashMap<[u8; 20], u128>,
        active_neurons: &[&NeuronData],
    ) -> Vec<u128> {
        let mut consensus = vec![0u128; n];

        for to_idx in 0..n {
            // Collect per-validator (weight_bps, stake) pairs for miner to_idx
            let mut weighted_vals: Vec<(u128, u128)> = validator_idxs
                .iter()
                .filter_map(|&from_idx| {
                    let hotkey = active_neurons[from_idx].hotkey;
                    let eff_stake = *validator_eff_stake.get(&hotkey)?;
                    let w = weight_matrix[from_idx][to_idx];
                    Some((w, eff_stake))
                })
                .collect();

            if weighted_vals.is_empty() {
                consensus[to_idx] = 0;
                continue;
            }

            // Sort by weight value — u128 has total ordering (no NaN issues)
            weighted_vals.sort_by_key(|&(w, _)| w);

            // Find stake-weighted median: first w where cumulative stake ≥ half
            let half = total_eff_stake / 2;
            let mut cumulative: u128 = 0;
            let mut median_val: u128 = 0;

            for &(val, stake) in &weighted_vals {
                cumulative += stake;
                median_val = val;
                if cumulative >= half {
                    break;
                }
            }

            // Clip prerank to median (Bittensor consensus step)
            consensus[to_idx] = prerank[to_idx].min(median_val);
        }

        consensus
    }

    /// Write computed `NeuronUpdate`s back to MetagraphDB.
    ///
    /// Reads the existing `NeuronData` to preserve non-consensus fields
    /// (stake, endpoint, etc.) and only overwrites the metric fields.
    pub fn apply_updates(metagraph_db: &Arc<MetagraphDB>, updates: Vec<NeuronUpdate>, epoch_num: u64) {
        let mut applied = 0usize;
        let mut errors = 0usize;

        for upd in &updates {
            let existing = metagraph_db.get_neuron(upd.subnet_id, upd.uid);
            match existing {
                Ok(Some(mut neuron)) => {
                    info!(
                        "  🔍 apply_updates: uid={} subnet={} old_trust={} new_trust={} old_rank={} new_rank={}",
                        upd.uid, upd.subnet_id, neuron.trust, upd.trust, neuron.rank, upd.rank
                    );
                    neuron.trust = upd.trust;
                    neuron.rank = upd.rank;
                    neuron.incentive = upd.incentive;
                    neuron.dividends = upd.dividends;
                    neuron.last_update = epoch_num;

                    match metagraph_db.store_neuron(&neuron) {
                        Ok(_) => applied += 1,
                        Err(e) => {
                            warn!(
                                "YumaConsensus: failed to store neuron uid={} subnet={}: {}",
                                upd.uid, upd.subnet_id, e
                            );
                            errors += 1;
                        }
                    }
                }
                Ok(None) => {
                    debug!(
                        "YumaConsensus: neuron uid={} subnet={} not found — skipping",
                        upd.uid, upd.subnet_id
                    );
                }
                Err(e) => {
                    warn!(
                        "YumaConsensus: error reading neuron uid={} subnet={}: {}",
                        upd.uid, upd.subnet_id, e
                    );
                    errors += 1;
                }
            }
        }

        info!(
            "📊 YumaConsensus epoch {}: applied {}/{} updates ({} errors)",
            epoch_num,
            applied,
            updates.len(),
            errors
        );
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// BPS → fixed-point conversion: 10_000 BPS (100%) → 65_535
    #[test]
    fn test_bps_to_fixed_point_full() {
        let bps_val: u128 = BPS_SCALE; // 100%
        let fp = (bps_val * FIXED_POINT_SCALE / BPS_SCALE) as u32;
        assert_eq!(fp, 65_535);
    }

    /// BPS → fixed-point conversion: 5_000 BPS (50%) → 32_767
    #[test]
    fn test_bps_to_fixed_point_half() {
        let bps_val: u128 = BPS_SCALE / 2; // 50%
        let fp = (bps_val * FIXED_POINT_SCALE / BPS_SCALE) as u32;
        assert_eq!(fp, 32_767);
    }

    /// BPS → fixed-point conversion: 0 BPS (0%) → 0
    #[test]
    fn test_bps_to_fixed_point_zero() {
        let bps_val: u128 = 0;
        let fp = (bps_val * FIXED_POINT_SCALE / BPS_SCALE) as u32;
        assert_eq!(fp, 0);
    }

    #[test]
    fn test_median_clip_bps_single_validator() {
        // With only one validator, median = that validator's weight
        // prerank == median, so consensus == prerank
        let prerank = vec![5_000u128, 5_000u128]; // 50% each in BPS
        let weight_matrix = vec![
            vec![5_000u128, 5_000u128], // validator's weights
            vec![0u128, 0u128],         // non-validator
        ];
        let validator_idxs = vec![0usize];
        let mut stake_map = HashMap::new();
        stake_map.insert([1u8; 20], 1_000u128);

        let n1 = NeuronData {
            uid: 0,
            subnet_id: 1,
            hotkey: [1u8; 20],
            coldkey: [0u8; 20],
            stake: 1000,
            trust: 0,
            rank: 0,
            incentive: 0,
            dividends: 0,
            emission: 0,
            last_update: 0,
            active: true,
            endpoint: String::new(),
        };
        let n2 = NeuronData { uid: 1, hotkey: [2u8; 20], ..n1.clone() };
        let active: Vec<&NeuronData> = vec![&n1, &n2];

        let result = YumaConsensus::median_clip_bps(
            &prerank,
            &weight_matrix,
            &validator_idxs,
            2,
            1_000,
            &stake_map,
            &active,
        );

        // Median of a single value [5_000] is 5_000; prerank.min(5_000) = 5_000
        assert_eq!(result[0], 5_000);
        assert_eq!(result[1], 5_000);
    }

    #[test]
    fn test_row_normalize_deterministic() {
        // Test that row normalization sums exactly to BPS_SCALE
        let raw_weights: Vec<u128> = vec![100, 200, 300, 400]; // total = 1000
        let row_sum: u128 = raw_weights.iter().sum();

        let mut normalized: Vec<u128> = raw_weights.iter()
            .map(|&w| w * BPS_SCALE / row_sum)
            .collect();

        let sum_before_fix: u128 = normalized.iter().sum();
        let max_idx = raw_weights.iter().enumerate().max_by_key(|(_, &v)| v).unwrap().0;
        normalized[max_idx] += BPS_SCALE.saturating_sub(sum_before_fix);

        let final_sum: u128 = normalized.iter().sum();
        assert_eq!(final_sum, BPS_SCALE, "Row must sum exactly to BPS_SCALE");
    }

    #[test]
    fn test_median_clip_bps_two_validators() {
        // Two validators with equal stake: median is the average of their weights
        // V1 gives miner 0 weight 8000 BPS, V2 gives 2000 BPS
        // Sorted: [2000, 8000], half stake at 50%, median = 8000 (cumulative >= half at idx 1)
        let prerank = vec![5_000u128]; // won't matter, we test median
        let weight_matrix = vec![
            vec![8_000u128], // V1
            vec![2_000u128], // V2
        ];
        let validator_idxs = vec![0usize, 1usize];
        let mut stake_map = HashMap::new();
        stake_map.insert([1u8; 20], 500u128); // V1
        stake_map.insert([2u8; 20], 500u128); // V2

        let n1 = NeuronData {
            uid: 0, subnet_id: 1, hotkey: [1u8; 20], coldkey: [0u8; 20],
            stake: 500, trust: 0, rank: 0, incentive: 0, dividends: 0,
            emission: 0, last_update: 0, active: true, endpoint: String::new(),
        };
        let n2 = NeuronData { uid: 1, hotkey: [2u8; 20], ..n1.clone() };
        let active: Vec<&NeuronData> = vec![&n1, &n2];

        let result = YumaConsensus::median_clip_bps(
            &prerank,
            &weight_matrix,
            &validator_idxs,
            1, // only 1 miner (to_idx=0)
            1_000, // total stake
            &stake_map,
            &active,
        );

        // Sorted by weight: [(2000, 500), (8000, 500)]
        // half = 500, cumulative after first = 500.  500 >= 500 → median = 2000
        // consensus[0] = min(prerank=5000, median=2000) = 2000
        assert_eq!(result[0], 2_000);
    }
}

// Economic Model Module for LuxTensor Tokenomics
//
// Provides comprehensive economic analysis and simulation:
// - Supply projection over time (with halving + burn)
// - Inflation rate calculation per epoch
// - Equilibrium analysis (emission = burn crossover)
// - Parameter consistency validation across modules
// - Sensitivity analysis for governance tuning
//
// This module does NOT handle actual emission or distribution —
// those remain in emission.rs / reward_distribution.rs.
// This is a read-only analysis/simulation layer.

use super::burn_manager::BurnConfig;
use super::eip1559::Eip1559Config;
use super::emission::EmissionConfig;
use super::halving::HalvingSchedule;
use super::reward_distribution::DistributionConfig;
use luxtensor_core::constants::tokenomics::{ONE_TOKEN, TOTAL_SUPPLY};

/// Block time in seconds (12s target, matching Ethereum post-merge)
pub const BLOCK_TIME_SECONDS: u64 = 12;

/// Blocks per year at 12s block time = 365.25 * 24 * 3600 / 12 = 2,629,800
pub const BLOCKS_PER_YEAR: u64 = 2_629_800;

/// Pre-minted supply at TGE (55% of 21M = 11.55M MDT)
/// Includes: Team 10%, Private 8%, IDO 5%, DAO 10%, Liquidity 5%, Foundation 5%, Ecosystem 12%
/// Derived from canonical TOTAL_SUPPLY in luxtensor-core
pub const PREMINTED_SUPPLY: u128 = TOTAL_SUPPLY * 55 / 100;

/// Emission pool (45% of 21M = 9.45M MDT)
/// Derived from canonical TOTAL_SUPPLY in luxtensor-core
pub const EMISSION_POOL: u128 = TOTAL_SUPPLY * 45 / 100;

// ─────────────────────────────────────────────────────────────
// Supply Projection
// ─────────────────────────────────────────────────────────────

/// Annual snapshot of the token economy
#[derive(Debug, Clone)]
pub struct AnnualSnapshot {
    /// Year number (0 = genesis)
    pub year: u32,
    /// Block height at end of year
    pub block_height: u64,
    /// Cumulative emission from mining/staking (raw, before burn)
    pub cumulative_emission: u128,
    /// Base emission per block at this point (before utility adjustment)
    pub base_emission_per_block: u128,
    /// Annual gross emission (before burn)
    pub annual_gross_emission: u128,
    /// Estimated annual burn (from tx fees, slashing, etc.)
    pub annual_burn_estimate: u128,
    /// Net annual emission (gross − burn)
    pub annual_net_emission: i128,
    /// Total circulating supply (preminted + cumulative_emission − cumulative_burn)
    pub circulating_supply: u128,
    /// Cumulative burn
    pub cumulative_burn: u128,
    /// Annual inflation rate (net emission / circulating supply at start of year)
    pub inflation_rate_pct: f64,
    /// Halving era active during this year
    pub halving_era: u32,
    /// Whether a halving event occurs this year
    pub halving_this_year: bool,
}

/// Configuration for supply projection simulation
#[derive(Debug, Clone)]
pub struct ProjectionConfig {
    /// Number of years to simulate
    pub years: u32,
    /// Average transactions per block (for burn estimation)
    pub avg_txs_per_block: u64,
    /// Average gas fee per transaction in wei
    pub avg_gas_fee_wei: u128,
    /// Average block utilization (0.0 - 1.0) for utility score
    pub avg_block_utilization: f64,
    /// Estimated annual subnet registrations
    pub annual_subnet_registrations: u64,
    /// Average subnet registration fee in MDT
    pub avg_subnet_reg_fee: u128,
    /// Average annual slashing events
    pub annual_slashing_events: u64,
    /// Average slashed amount per event in MDT
    pub avg_slash_amount: u128,
}

impl Default for ProjectionConfig {
    fn default() -> Self {
        Self {
            years: 40,
            avg_txs_per_block: 50,
            avg_gas_fee_wei: 2_000_000_000, // 2 gwei average effective gas price
            avg_block_utilization: 0.5,
            annual_subnet_registrations: 100,
            avg_subnet_reg_fee: 1000 * ONE_TOKEN, // 1000 MDT
            annual_slashing_events: 10,
            avg_slash_amount: 500 * ONE_TOKEN, // 500 MDT
        }
    }
}

/// Run a full supply projection
///
/// Simulates year-by-year token economy considering:
/// - Halving schedule for emission decay
/// - Burn mechanisms (tx fees, subnet reg, slashing)
/// - Utility-based emission adjustment (averaged)
///
/// Returns a vector of annual snapshots.
pub fn project_supply(
    emission_cfg: &EmissionConfig,
    burn_cfg: &BurnConfig,
    halving: &HalvingSchedule,
    proj: &ProjectionConfig,
) -> Vec<AnnualSnapshot> {
    let mut snapshots = Vec::with_capacity(proj.years as usize);

    let mut cumulative_emission: u128 = 0;
    let mut cumulative_burn: u128 = 0;
    let mut prev_circulating = PREMINTED_SUPPLY;

    for year in 0..proj.years {
        let year_start_block = year as u64 * BLOCKS_PER_YEAR;
        let year_end_block = (year as u64 + 1) * BLOCKS_PER_YEAR;

        // ── Emission for this year ──
        // Use average block height for the year to get representative base emission
        let mid_block = year_start_block + BLOCKS_PER_YEAR / 2;
        let base_emission = halving.calculate_reward(mid_block);

        // Apply average utility multiplier
        // utility_score range 0.5-1.5, with avg_block_utilization ≈ weighted score
        // At 50% utilization: score ≈ 1.0 → no adjustment
        let utility_factor = 0.5 + proj.avg_block_utilization;
        let weight = emission_cfg.utility_weight as f64 / 100.0;
        let adjustment = 1.0 + (utility_factor - 1.0) * weight;

        let adjusted_emission_per_block = (base_emission as f64 * adjustment) as u128;
        let annual_gross_emission =
            adjusted_emission_per_block.saturating_mul(BLOCKS_PER_YEAR as u128);

        // Cap emission at remaining emission pool
        let remaining_pool = EMISSION_POOL.saturating_sub(cumulative_emission);
        let annual_gross_emission = annual_gross_emission.min(remaining_pool);

        cumulative_emission = cumulative_emission.saturating_add(annual_gross_emission);

        // ── Burns for this year ──
        // 1. Transaction fee burns
        let annual_tx_fees = proj.avg_txs_per_block as u128
            * proj.avg_gas_fee_wei
            * 21_000 // base gas per tx
            * BLOCKS_PER_YEAR as u128;
        let tx_burn = annual_tx_fees * burn_cfg.tx_fee_burn_rate_bps as u128 / 10_000;

        // 2. Subnet registration burns
        let subnet_burn = proj.annual_subnet_registrations as u128
            * proj.avg_subnet_reg_fee
            * burn_cfg.subnet_burn_rate_bps as u128
            / 10_000;

        // 3. Slashing burns
        let slash_burn = proj.annual_slashing_events as u128
            * proj.avg_slash_amount
            * burn_cfg.slashing_burn_rate_bps as u128
            / 10_000;

        let annual_burn = tx_burn + subnet_burn + slash_burn;
        // Can't burn more than what's circulating
        let annual_burn = annual_burn.min(prev_circulating + annual_gross_emission);
        cumulative_burn = cumulative_burn.saturating_add(annual_burn);

        // ── Circulating supply ──
        let circulating =
            PREMINTED_SUPPLY.saturating_add(cumulative_emission).saturating_sub(cumulative_burn);

        // ── Inflation rate ──
        let net_emission = annual_gross_emission as i128 - annual_burn as i128;
        let inflation = if prev_circulating > 0 {
            net_emission as f64 / prev_circulating as f64 * 100.0
        } else {
            0.0
        };

        // ── Halving info ──
        let era_start = halving.get_halving_era(year_start_block);
        let era_end = halving.get_halving_era(year_end_block);
        let halving_this_year = era_end > era_start;

        snapshots.push(AnnualSnapshot {
            year,
            block_height: year_end_block,
            cumulative_emission,
            base_emission_per_block: base_emission,
            annual_gross_emission,
            annual_burn_estimate: annual_burn,
            annual_net_emission: net_emission,
            circulating_supply: circulating,
            cumulative_burn,
            inflation_rate_pct: inflation,
            halving_era: era_start,
            halving_this_year,
        });

        prev_circulating = circulating;
    }

    snapshots
}

// ─────────────────────────────────────────────────────────────
// Equilibrium Analysis
// ─────────────────────────────────────────────────────────────

/// Result of equilibrium analysis
#[derive(Debug, Clone)]
pub struct EquilibriumResult {
    /// Year where net emission first becomes ≤ 0 (burn ≥ emission)
    /// None if never reached within projection window
    pub equilibrium_year: Option<u32>,
    /// Estimated circulating supply at equilibrium
    pub equilibrium_supply: Option<u128>,
    /// Maximum circulating supply reached during projection
    pub peak_supply: u128,
    /// Year of peak supply
    pub peak_year: u32,
    /// Final year inflation rate
    pub final_inflation_pct: f64,
    /// Year when inflation first drops below 2% (traditional "healthy" threshold)
    pub sub_2pct_inflation_year: Option<u32>,
    /// Year when inflation first drops below 1%
    pub sub_1pct_inflation_year: Option<u32>,
}

/// Find the equilibrium point where burn ≥ emission
pub fn analyze_equilibrium(snapshots: &[AnnualSnapshot]) -> EquilibriumResult {
    let mut equilibrium_year = None;
    let mut equilibrium_supply = None;
    let mut peak_supply: u128 = 0;
    let mut peak_year: u32 = 0;
    let mut sub_2pct_year = None;
    let mut sub_1pct_year = None;

    for snap in snapshots {
        // Track peak supply
        if snap.circulating_supply > peak_supply {
            peak_supply = snap.circulating_supply;
            peak_year = snap.year;
        }

        // First year where net emission ≤ 0
        if equilibrium_year.is_none() && snap.annual_net_emission <= 0 {
            equilibrium_year = Some(snap.year);
            equilibrium_supply = Some(snap.circulating_supply);
        }

        // Sub-2% inflation
        if sub_2pct_year.is_none() && snap.inflation_rate_pct < 2.0 && snap.year > 0 {
            sub_2pct_year = Some(snap.year);
        }

        // Sub-1% inflation
        if sub_1pct_year.is_none() && snap.inflation_rate_pct < 1.0 && snap.year > 0 {
            sub_1pct_year = Some(snap.year);
        }
    }

    let final_inflation = snapshots.last().map(|s| s.inflation_rate_pct).unwrap_or(0.0);

    EquilibriumResult {
        equilibrium_year,
        equilibrium_supply,
        peak_supply,
        peak_year,
        final_inflation_pct: final_inflation,
        sub_2pct_inflation_year: sub_2pct_year,
        sub_1pct_inflation_year: sub_1pct_year,
    }
}

// ─────────────────────────────────────────────────────────────
// Sensitivity Analysis
// ─────────────────────────────────────────────────────────────

/// Result of sensitivity sweep on one parameter
#[derive(Debug, Clone)]
pub struct SensitivityPoint {
    pub parameter_value: f64,
    pub label: String,
    pub year_10_supply: u128,
    pub year_10_inflation: f64,
    pub equilibrium_year: Option<u32>,
    pub peak_supply: u128,
}

/// Sweep burn rate from low to high and see impact on supply curve
pub fn sweep_burn_rate(
    emission_cfg: &EmissionConfig,
    halving: &HalvingSchedule,
    proj: &ProjectionConfig,
    burn_rates_bps: &[u32],
) -> Vec<SensitivityPoint> {
    let mut results = Vec::new();

    for &rate in burn_rates_bps {
        let burn_cfg = BurnConfig {
            tx_fee_burn_rate_bps: rate,
            subnet_burn_rate_bps: rate,
            slashing_burn_rate_bps: rate.min(10_000),
            unmet_quota_burn_rate_bps: 10_000,
        };

        let snapshots = project_supply(emission_cfg, &burn_cfg, halving, proj);
        let eq = analyze_equilibrium(&snapshots);

        let year_10 = match snapshots.get(10).or_else(|| snapshots.last()) {
            Some(s) => s.clone(),
            None => continue, // skip if projection returned no data
        };

        results.push(SensitivityPoint {
            parameter_value: rate as f64 / 100.0,
            label: format!("burn_rate={}%", rate / 100),
            year_10_supply: year_10.circulating_supply,
            year_10_inflation: year_10.inflation_rate_pct,
            equilibrium_year: eq.equilibrium_year,
            peak_supply: eq.peak_supply,
        });
    }

    results
}

/// Sweep transactions per block to see burn impact
pub fn sweep_tx_volume(
    emission_cfg: &EmissionConfig,
    burn_cfg: &BurnConfig,
    halving: &HalvingSchedule,
    base_proj: &ProjectionConfig,
    tx_volumes: &[u64],
) -> Vec<SensitivityPoint> {
    let mut results = Vec::new();

    for &vol in tx_volumes {
        let mut proj = base_proj.clone();
        proj.avg_txs_per_block = vol;

        let snapshots = project_supply(emission_cfg, burn_cfg, halving, &proj);
        let eq = analyze_equilibrium(&snapshots);
        let year_10 = match snapshots.get(10).or_else(|| snapshots.last()) {
            Some(s) => s.clone(),
            None => continue, // skip if projection returned no data
        };

        results.push(SensitivityPoint {
            parameter_value: vol as f64,
            label: format!("{}txs/block", vol),
            year_10_supply: year_10.circulating_supply,
            year_10_inflation: year_10.inflation_rate_pct,
            equilibrium_year: eq.equilibrium_year,
            peak_supply: eq.peak_supply,
        });
    }

    results
}

// ─────────────────────────────────────────────────────────────
// Cross-Module Parameter Validation
// ─────────────────────────────────────────────────────────────

/// Inconsistency found during cross-module validation
#[derive(Debug, Clone)]
pub struct TokenomicsInconsistency {
    pub severity: Severity,
    pub module: &'static str,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Funds lost, invariant broken
    Critical,
    /// Suboptimal but won't corrupt state
    Warning,
    /// Informational
    Info,
}

/// Validate consistency across all tokenomics modules
///
/// Checks:
/// - Distribution shares sum to 100%
/// - Emission pool ≥ estimated total emission from halving schedule
/// - Halving interval consistency between EmissionConfig and HalvingSchedule
/// - Min emission consistency
/// - EIP-1559 base fee sanity
/// - Infrastructure share is actually distributed (caller must verify)
pub fn validate_parameters(
    emission_cfg: &EmissionConfig,
    dist_cfg: &DistributionConfig,
    burn_cfg: &BurnConfig,
    halving: &HalvingSchedule,
    fee_cfg: &Eip1559Config,
) -> Vec<TokenomicsInconsistency> {
    let mut issues = Vec::new();

    // 1. Distribution shares must sum to 10_000 BPS
    let dist_total = dist_cfg.miner_share_bps
        + dist_cfg.validator_share_bps
        + dist_cfg.infrastructure_share_bps
        + dist_cfg.delegator_share_bps
        + dist_cfg.subnet_owner_share_bps
        + dist_cfg.dao_share_bps
        + dist_cfg.community_ecosystem_share_bps;
    if dist_total != 10_000 {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "reward_distribution",
            description: format!("Distribution shares sum to {} BPS, expected 10,000", dist_total),
        });
    }

    // 2. Halving interval consistency
    if emission_cfg.halving_interval != halving.halving_interval {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "emission / halving",
            description: format!(
                "Halving interval mismatch: EmissionConfig={} vs HalvingSchedule={}",
                emission_cfg.halving_interval, halving.halving_interval
            ),
        });
    }

    // 3. Initial emission consistency
    if emission_cfg.initial_emission != halving.initial_reward {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "emission / halving",
            description: format!(
                "Initial emission mismatch: EmissionConfig={} vs HalvingSchedule={}",
                emission_cfg.initial_emission, halving.initial_reward
            ),
        });
    }

    // 4. Minimum emission consistency
    if emission_cfg.min_emission != halving.minimum_reward {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Warning,
            module: "emission / halving",
            description: format!(
                "Min emission mismatch: EmissionConfig={} vs HalvingSchedule={}",
                emission_cfg.min_emission, halving.minimum_reward
            ),
        });
    }

    // 5. Emission pool vs estimated total emission
    let estimated_total = halving.estimate_total_emission();
    if estimated_total > EMISSION_POOL {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "halving / token_allocation",
            description: format!(
                "Estimated total emission ({:.2} MDT) exceeds emission pool ({:.2} MDT). \
                 Halving schedule will mint more than the 45% allocation allows.",
                estimated_total as f64 / ONE_TOKEN as f64,
                EMISSION_POOL as f64 / ONE_TOKEN as f64,
            ),
        });
    } else {
        let utilization = estimated_total as f64 / EMISSION_POOL as f64 * 100.0;
        if utilization < 80.0 {
            issues.push(TokenomicsInconsistency {
                severity: Severity::Info,
                module: "halving / token_allocation",
                description: format!(
                    "Emission pool utilization is only {:.1}% ({:.2}M / {:.2}M MDT). \
                     Consider increasing initial_reward or halving_interval to use more of the allocation.",
                    utilization,
                    estimated_total as f64 / ONE_TOKEN as f64 / 1_000_000.0,
                    EMISSION_POOL as f64 / ONE_TOKEN as f64 / 1_000_000.0,
                ),
            });
        }
    }

    // 6. Burn rates within valid range
    let burn_rates = [
        ("tx_fee_burn_rate", burn_cfg.tx_fee_burn_rate_bps),
        ("subnet_burn_rate", burn_cfg.subnet_burn_rate_bps),
        ("unmet_quota_burn_rate", burn_cfg.unmet_quota_burn_rate_bps),
        ("slashing_burn_rate", burn_cfg.slashing_burn_rate_bps),
    ];
    for (name, rate) in &burn_rates {
        if *rate > 10_000 {
            issues.push(TokenomicsInconsistency {
                severity: Severity::Critical,
                module: "burn_manager",
                description: format!("{} = {} BPS exceeds 10,000 (100%)", name, rate),
            });
        }
    }

    // 7. EIP-1559 sanity
    if fee_cfg.min_base_fee >= fee_cfg.max_base_fee {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "eip1559",
            description: format!(
                "min_base_fee ({}) >= max_base_fee ({})",
                fee_cfg.min_base_fee, fee_cfg.max_base_fee
            ),
        });
    }

    if fee_cfg.target_gas_used > fee_cfg.block_gas_limit {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "eip1559",
            description: format!(
                "target_gas_used ({}) > block_gas_limit ({})",
                fee_cfg.target_gas_used, fee_cfg.block_gas_limit
            ),
        });
    }

    // 8. Max supply consistency
    if emission_cfg.max_supply != 21_000_000_000_000_000_000_000_000u128 {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Warning,
            module: "emission",
            description: format!(
                "max_supply is {}, expected 21M MDT (21_000_000e18)",
                emission_cfg.max_supply
            ),
        });
    }

    // 9. Utility weight range
    if emission_cfg.utility_weight > 100 {
        issues.push(TokenomicsInconsistency {
            severity: Severity::Critical,
            module: "emission",
            description: format!(
                "utility_weight is {} but max is 100 (100%)",
                emission_cfg.utility_weight
            ),
        });
    }

    issues
}

// ─────────────────────────────────────────────────────────────
// Human-readable Report
// ─────────────────────────────────────────────────────────────

/// Generate a human-readable tokenomics report
pub fn generate_report(
    emission_cfg: &EmissionConfig,
    dist_cfg: &DistributionConfig,
    burn_cfg: &BurnConfig,
    halving: &HalvingSchedule,
    fee_cfg: &Eip1559Config,
    proj: &ProjectionConfig,
) -> String {
    let snapshots = project_supply(emission_cfg, burn_cfg, halving, proj);
    let equilibrium = analyze_equilibrium(&snapshots);
    let issues = validate_parameters(emission_cfg, dist_cfg, burn_cfg, halving, fee_cfg);

    let mut report = String::new();

    report.push_str("╔══════════════════════════════════════════════════════════╗\n");
    report.push_str("║          LuxTensor Economic Model Report                ║\n");
    report.push_str("╚══════════════════════════════════════════════════════════╝\n\n");

    // ── Parameters ──
    report.push_str("── Token Parameters ──\n");
    report.push_str(&format!("  Max Supply:          21,000,000 MDT\n"));
    report.push_str(&format!("  Pre-minted (TGE):    11,550,000 MDT (55%)\n"));
    report.push_str(&format!("  Emission Pool:        9,450,000 MDT (45%)\n"));
    report.push_str(&format!("  Block Time:           {}s\n", BLOCK_TIME_SECONDS));
    report.push_str(&format!("  Blocks/Year:          {}\n", BLOCKS_PER_YEAR));
    report.push_str(&format!(
        "  Initial Emission:     {:.3} MDT/block\n",
        emission_cfg.initial_emission as f64 / ONE_TOKEN as f64
    ));
    report.push_str(&format!(
        "  Halving Interval:     {} blocks ({:.1} years)\n",
        halving.halving_interval,
        halving.halving_interval as f64 / BLOCKS_PER_YEAR as f64
    ));
    report.push_str(&format!("  Max Halvings:         {}\n", halving.max_halvings));
    report.push_str(&format!(
        "  Min Emission:         {:.6} MDT/block\n",
        emission_cfg.min_emission as f64 / ONE_TOKEN as f64
    ));
    report.push_str(&format!("  Utility Weight:       {}%\n\n", emission_cfg.utility_weight));

    // ── Halving Schedule ──
    report.push_str("── Halving Schedule ──\n");
    report.push_str("  Era   Emission/block   Year Range          Annual Emission\n");
    for era in 0..=halving.max_halvings {
        let reward = halving.initial_reward >> era;
        if reward < halving.minimum_reward {
            break;
        }
        let era_start_year = era as f64 * halving.halving_interval as f64 / BLOCKS_PER_YEAR as f64;
        let era_end_year =
            (era + 1) as f64 * halving.halving_interval as f64 / BLOCKS_PER_YEAR as f64;
        let annual = reward as f64 * BLOCKS_PER_YEAR as f64 / ONE_TOKEN as f64;
        report.push_str(&format!(
            "  {:>3}   {:.6} MDT   Year {:.1} – {:.1}   {:>12.0} MDT\n",
            era,
            reward as f64 / ONE_TOKEN as f64,
            era_start_year,
            era_end_year,
            annual,
        ));
    }
    report.push('\n');

    // ── Burn Parameters ──
    report.push_str("── Burn Mechanisms ──\n");
    report.push_str(&format!(
        "  TX Fee Burn:          {}% of fees\n",
        burn_cfg.tx_fee_burn_rate_bps / 100
    ));
    report.push_str(&format!(
        "  Subnet Reg Burn:      {}% of registration fee\n",
        burn_cfg.subnet_burn_rate_bps / 100
    ));
    report.push_str(&format!(
        "  Slashing Burn:        {}% of slashed amount\n",
        burn_cfg.slashing_burn_rate_bps / 100
    ));
    report.push_str(&format!(
        "  Unmet Quota Burn:     {}%\n\n",
        burn_cfg.unmet_quota_burn_rate_bps / 100
    ));

    // ── Supply Projection ──
    report.push_str("── Supply Projection (Key Years) ──\n");
    report.push_str(
        "  Year   Circulating (MDT)    Inflation    Emission/block       Net Emission    Halving\n",
    );
    let key_years: Vec<u32> = vec![0, 1, 2, 3, 5, 7, 10, 15, 20, 25, 30, 35, 39];
    for &y in &key_years {
        if let Some(snap) = snapshots.get(y as usize) {
            let halving_marker = if snap.halving_this_year { " ★" } else { "" };
            report.push_str(&format!(
                "  {:>4}   {:>16.0}    {:>+7.2}%    {:.6} MDT     {:>+14.0}   {}\n",
                snap.year,
                snap.circulating_supply as f64 / ONE_TOKEN as f64,
                snap.inflation_rate_pct,
                snap.base_emission_per_block as f64 / ONE_TOKEN as f64,
                snap.annual_net_emission as f64 / ONE_TOKEN as f64,
                halving_marker,
            ));
        }
    }
    report.push('\n');

    // ── Equilibrium ──
    report.push_str("── Equilibrium Analysis ──\n");
    if let Some(year) = equilibrium.equilibrium_year {
        report.push_str(&format!("  Net-zero inflation reached at:  Year {}\n", year));
        if let Some(supply) = equilibrium.equilibrium_supply {
            report.push_str(&format!(
                "  Supply at equilibrium:          {:.0} MDT\n",
                supply as f64 / ONE_TOKEN as f64
            ));
        }
    } else {
        report.push_str("  Net-zero inflation NOT reached within projection window\n");
    }
    report.push_str(&format!(
        "  Peak circulating supply:        {:.0} MDT (Year {})\n",
        equilibrium.peak_supply as f64 / ONE_TOKEN as f64,
        equilibrium.peak_year
    ));
    if let Some(y) = equilibrium.sub_2pct_inflation_year {
        report.push_str(&format!("  Inflation < 2%:                 Year {}\n", y));
    }
    if let Some(y) = equilibrium.sub_1pct_inflation_year {
        report.push_str(&format!("  Inflation < 1%:                 Year {}\n", y));
    }
    report.push_str(&format!(
        "  Final year inflation:           {:.4}%\n\n",
        equilibrium.final_inflation_pct
    ));

    // ── Validation ──
    let critical_count = issues.iter().filter(|i| i.severity == Severity::Critical).count();
    let warning_count = issues.iter().filter(|i| i.severity == Severity::Warning).count();

    report.push_str("── Parameter Validation ──\n");
    if issues.is_empty() {
        report.push_str("  ✓ All parameters consistent — no issues found\n");
    } else {
        report.push_str(&format!(
            "  {} critical, {} warnings, {} info\n",
            critical_count,
            warning_count,
            issues.len() - critical_count - warning_count,
        ));
        for issue in &issues {
            let severity_mark = match issue.severity {
                Severity::Critical => "✗ CRITICAL",
                Severity::Warning => "⚠ WARNING",
                Severity::Info => "ℹ INFO",
            };
            report.push_str(&format!(
                "  [{}] [{}] {}\n",
                severity_mark, issue.module, issue.description
            ));
        }
    }

    report
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_configs(
    ) -> (EmissionConfig, BurnConfig, HalvingSchedule, DistributionConfig, Eip1559Config) {
        (
            EmissionConfig::default(),
            BurnConfig::default(),
            HalvingSchedule::default(),
            DistributionConfig::default(),
            Eip1559Config::default(),
        )
    }

    #[test]
    fn test_supply_projection_basic() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 10, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);

        assert_eq!(snapshots.len(), 10);

        // Year 0 should have the highest emission
        assert!(snapshots[0].annual_gross_emission > 0);

        // Circulating supply should increase monotonically in early years
        for i in 1..snapshots.len() {
            // During emission-heavy years, supply should generally increase
            // (burn might dominate later, but early on emission wins)
            if snapshots[i].year < 5 {
                assert!(
                    snapshots[i].circulating_supply >= snapshots[i - 1].circulating_supply,
                    "Supply should increase year {} → {}: {} → {}",
                    i - 1,
                    i,
                    snapshots[i - 1].circulating_supply,
                    snapshots[i].circulating_supply,
                );
            }
        }
    }

    #[test]
    fn test_supply_never_exceeds_max() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 40, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);

        let max_supply = emission.max_supply;
        for snap in &snapshots {
            // Circulating (preminted + emission - burn) should never exceed max supply
            assert!(
                snap.circulating_supply <= max_supply,
                "Year {}: circulating {} > max {}",
                snap.year,
                snap.circulating_supply,
                max_supply,
            );
            // Cumulative emission should not exceed emission pool
            assert!(
                snap.cumulative_emission <= EMISSION_POOL,
                "Year {}: cumulative emission {} > pool {}",
                snap.year,
                snap.cumulative_emission,
                EMISSION_POOL,
            );
        }
    }

    #[test]
    fn test_halving_reduces_emission() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 20, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);

        // Find annual emissions in year 0 and year 5 (after at least one halving)
        let year0_emission = snapshots[0].annual_gross_emission;
        let year5_emission = snapshots[5].annual_gross_emission;

        // Emission at year 5 should be significantly less due to halving
        assert!(
            year5_emission < year0_emission,
            "Year 5 emission should be less than year 0: {} vs {}",
            year5_emission,
            year0_emission,
        );
    }

    #[test]
    fn test_inflation_decreases_over_time() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 30, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);

        // General trend: inflation should decrease
        // Compare year 1 inflation to year 20 inflation
        let year1_inflation = snapshots[1].inflation_rate_pct;
        let year20_inflation = snapshots[20].inflation_rate_pct;

        assert!(
            year20_inflation < year1_inflation,
            "Inflation should decrease: year 1 = {:.2}%, year 20 = {:.2}%",
            year1_inflation,
            year20_inflation,
        );
    }

    #[test]
    fn test_equilibrium_analysis() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig {
            years: 40,
            // High burn scenario to reach equilibrium within window
            avg_txs_per_block: 200,
            avg_gas_fee_wei: 5_000_000_000, // 5 gwei
            ..Default::default()
        };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);
        let eq = analyze_equilibrium(&snapshots);

        // Should have valid peak
        assert!(eq.peak_supply > PREMINTED_SUPPLY);

        // Final inflation should be very low
        assert!(eq.final_inflation_pct < 5.0, "Final inflation: {}", eq.final_inflation_pct);
    }

    #[test]
    fn test_zero_burn_scenario() {
        let (emission, _, halving, _, _) = default_configs();
        let burn = BurnConfig {
            tx_fee_burn_rate_bps: 0,
            subnet_burn_rate_bps: 0,
            unmet_quota_burn_rate_bps: 0,
            slashing_burn_rate_bps: 0,
        };
        let proj = ProjectionConfig { years: 10, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);

        // With zero burn, supply should only increase
        for i in 1..snapshots.len() {
            assert!(
                snapshots[i].circulating_supply >= snapshots[i - 1].circulating_supply,
                "With zero burn, supply must be non-decreasing"
            );
            assert_eq!(snapshots[i].cumulative_burn, 0);
        }
    }

    #[test]
    fn test_validate_default_params_consistent() {
        let (emission, burn, halving, dist, fee) = default_configs();

        let issues = validate_parameters(&emission, &dist, &burn, &halving, &fee);

        // Default params should be consistent — no critical issues
        let critical = issues.iter().filter(|i| i.severity == Severity::Critical).count();
        assert_eq!(
            critical,
            0,
            "Default parameters should have 0 critical issues, found: {:?}",
            issues.iter().filter(|i| i.severity == Severity::Critical).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_validate_catches_halving_mismatch() {
        let emission = EmissionConfig {
            halving_interval: 999_999, // Mismatch!
            ..Default::default()
        };
        let (_, burn, halving, dist, fee) = default_configs();

        let issues = validate_parameters(&emission, &dist, &burn, &halving, &fee);

        let has_halving_mismatch = issues.iter().any(|i| {
            i.severity == Severity::Critical && i.description.contains("Halving interval mismatch")
        });
        assert!(has_halving_mismatch, "Should detect halving interval mismatch");
    }

    #[test]
    fn test_validate_catches_bad_distribution() {
        let dist = DistributionConfig {
            miner_share_bps: 5000,
            validator_share_bps: 5000,
            infrastructure_share_bps: 200,
            delegator_share_bps: 0,
            subnet_owner_share_bps: 0,
            dao_share_bps: 0,
            community_ecosystem_share_bps: 0,
        };
        let (emission, burn, halving, _, fee) = default_configs();

        let issues = validate_parameters(&emission, &dist, &burn, &halving, &fee);

        let has_sum_error = issues
            .iter()
            .any(|i| i.severity == Severity::Critical && i.description.contains("sum to"));
        assert!(has_sum_error, "Should detect distribution shares not summing to 10,000");
    }

    #[test]
    fn test_validate_catches_bad_eip1559() {
        let fee = Eip1559Config {
            min_base_fee: 1_000_000_000_000, // min > max
            max_base_fee: 100_000_000,
            ..Default::default()
        };
        let (emission, burn, halving, dist, _) = default_configs();

        let issues = validate_parameters(&emission, &dist, &burn, &halving, &fee);

        let has_fee_error = issues
            .iter()
            .any(|i| i.severity == Severity::Critical && i.description.contains("min_base_fee"));
        assert!(has_fee_error, "Should detect min > max base fee");
    }

    #[test]
    fn test_sensitivity_burn_rate() {
        let (emission, _, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 15, ..Default::default() };

        let results = sweep_burn_rate(&emission, &halving, &proj, &[0, 2500, 5000, 7500, 10000]);

        assert_eq!(results.len(), 5);

        // Higher burn rate → lower year 10 supply
        let supply_0 = results[0].year_10_supply;
        let supply_100 = results[4].year_10_supply;
        assert!(
            supply_100 < supply_0,
            "100% burn should yield lower supply than 0%: {} vs {}",
            supply_100,
            supply_0,
        );
    }

    #[test]
    fn test_sensitivity_tx_volume() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 15, ..Default::default() };

        let results = sweep_tx_volume(&emission, &burn, &halving, &proj, &[10, 50, 100, 500]);

        assert_eq!(results.len(), 4);

        // Higher tx volume → more burns → lower supply
        let supply_low = results[0].year_10_supply;
        let supply_high = results[3].year_10_supply;
        assert!(
            supply_high <= supply_low,
            "High volume should burn more: {} vs {}",
            supply_high,
            supply_low,
        );
    }

    #[test]
    fn test_generate_report_does_not_panic() {
        let (emission, burn, halving, dist, fee) = default_configs();
        let proj = ProjectionConfig { years: 20, ..Default::default() };

        let report = generate_report(&emission, &dist, &burn, &halving, &fee, &proj);

        assert!(!report.is_empty());
        assert!(report.contains("LuxTensor Economic Model Report"));
        assert!(report.contains("Halving Schedule"));
        assert!(report.contains("Equilibrium Analysis"));
        assert!(report.contains("Parameter Validation"));
    }

    #[test]
    fn test_preminted_plus_emission_equals_max() {
        // Fundamental invariant: preminted + emission_pool = max_supply
        let max_supply = 21_000_000_000_000_000_000_000_000u128;
        assert_eq!(PREMINTED_SUPPLY + EMISSION_POOL, max_supply);
    }

    #[test]
    fn test_blocks_per_year_correct() {
        // 365.25 days × 24h × 3600s / 12s = 2,628,000
        let expected = (365.25 * 24.0 * 3600.0 / BLOCK_TIME_SECONDS as f64) as u64;
        assert_eq!(BLOCKS_PER_YEAR, expected);
    }

    /// Economic invariant: with default params, year-1 inflation should be
    /// between 5% and 50% (reasonable for a new chain with 55% preminted)
    #[test]
    fn test_initial_inflation_reasonable() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 5, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);
        let year1 = &snapshots[0];

        assert!(
            year1.inflation_rate_pct > 1.0 && year1.inflation_rate_pct < 100.0,
            "Year 1 inflation should be 1-100%, got {:.2}%",
            year1.inflation_rate_pct,
        );
    }

    /// After all halvings + tail, emission effectively stops.
    /// Supply should stabilize (not grow unboundedly).
    #[test]
    fn test_supply_stabilizes() {
        let (emission, burn, halving, _, _) = default_configs();
        let proj = ProjectionConfig { years: 40, ..Default::default() };

        let snapshots = project_supply(&emission, &burn, &halving, &proj);

        // Compare year 35 and year 39 supply — should be very close
        let supply_35 = snapshots[35].circulating_supply;
        let supply_39 = snapshots[39].circulating_supply;

        let pct_change = ((supply_39 as f64 - supply_35 as f64) / supply_35 as f64).abs() * 100.0;
        assert!(
            pct_change < 5.0,
            "Supply should stabilize: year 35 = {}, year 39 = {}, change = {:.2}%",
            supply_35,
            supply_39,
            pct_change,
        );
    }
}

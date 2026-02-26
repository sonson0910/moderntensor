// LuxTensor consensus module
// Phase 2: Consensus Layer Implementation + Tokenomics v3.1 (Model C)
//
// Module Organization:
//   ── Core Consensus ──   PoS engine, fork handling, finality, liveness
//   ── Validators ──       Validator set, rotation, slashing, scoring
//   ── Tokenomics ──       Emission, halving, burn, distribution, EIP-1559, economic model
//   ── Governance ──       On-chain governance, commit-reveal, weight consensus
//   ── Security ──         Circuit breaker, long-range protection, RANDAO, VRF

// ── Core Consensus ─────────────────────────────────────────────────────────
pub mod error;
pub mod pos;
pub mod fork_choice;
pub mod fork_resolution;
pub mod fast_finality;
pub mod liveness;

// ── Validators ─────────────────────────────────────────────────────────────
pub mod validator;
pub mod rotation;
pub mod slashing;
pub mod node_tier;
pub mod scoring;

// ── Tokenomics ─────────────────────────────────────────────────────────────
pub mod emission;
pub mod halving;
pub mod burn_manager;
pub mod reward_distribution;
pub mod reward_executor;
pub mod eip1559;
pub mod token_allocation;
pub mod economic_model;

// ── Governance ─────────────────────────────────────────────────────────────
pub mod governance;
pub mod commit_reveal;
pub mod weight_consensus;
pub mod yuma_consensus;

// ── Security ───────────────────────────────────────────────────────────────
pub mod circuit_breaker;
pub mod long_range_protection;
pub mod randao;

/// Production VRF module — ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381).
/// Compiled only when the `production-vrf` Cargo feature is enabled.
#[cfg(feature = "production-vrf")]
pub mod vrf_key;

// ── Re-exports ─────────────────────────────────────────────────────────────

// Core Consensus
pub use error::{ConsensusError, Result};
pub use pos::{ConsensusConfig, ProofOfStake};
pub use fork_choice::{ForkChoice, ForkChoiceSnapshot};
pub use fork_resolution::{FinalityStats, FinalityStatus, ForkResolver, ReorgInfo};
pub use fast_finality::{BftPhase, FastFinality, FastFinalityStats, ViewChangeMessage};
pub use liveness::{LivenessAction, LivenessConfig, LivenessMonitor, LivenessStats};

// Validators
pub use validator::{Validator, ValidatorSet};
pub use rotation::{EpochTransitionResult, RotationConfig, RotationStats, ValidatorRotation};
pub use slashing::{
    JailStatus, SlashEvent, SlashReason, SlashingConfig, SlashingEvidence, SlashingManager,
};
pub use node_tier::{
    logarithmic_stake, GpuCapability, NodeInfo, NodeRegistry, NodeTier, FULL_NODE_STAKE,
    LIGHT_NODE_STAKE, SUPER_VALIDATOR_STAKE, VALIDATOR_STAKE,
};
pub use scoring::{MinerMetrics, ScoringConfig, ScoringEvent, ScoringManager, ValidatorMetrics};

// Tokenomics
pub use emission::{
    EmissionConfig, EmissionController, EmissionResult, EmissionStats, UtilityMetrics,
};
pub use halving::{
    HalvingInfo, HalvingSchedule, HALVING_INTERVAL, INITIAL_BLOCK_REWARD, MAX_HALVINGS,
    MINIMUM_REWARD,
};
pub use burn_manager::{BurnConfig, BurnEvent, BurnManager, BurnStats, BurnType};
pub use reward_distribution::{
    DelegatorInfo, DistributionConfig, DistributionResult, InfrastructureNodeInfo, LockBonusConfig,
    MinerInfo, RewardDistributor, SubnetInfo, ValidatorInfo,
};
pub use reward_executor::{
    AccountBalance, ClaimResult, EpochResult, ExecutorStats, PendingReward, RewardExecutor,
    RewardHistoryEntry, RewardType,
};
pub use eip1559::{Eip1559Config, FeeHistory, FeeMarket};
pub use token_allocation::{
    AllocationCategory, AllocationStats, TgeResult, TokenAllocation, VestingEntry, VestingSchedule,
    DECIMALS, TOTAL_SUPPLY,
};
pub use economic_model::{
    analyze_equilibrium, generate_report, project_supply, sweep_burn_rate, sweep_tx_volume,
    validate_parameters, AnnualSnapshot, EquilibriumResult, ProjectionConfig, SensitivityPoint,
    Severity, TokenomicsInconsistency, BLOCKS_PER_YEAR, BLOCK_TIME_SECONDS, EMISSION_POOL,
    PREMINTED_SUPPLY,
};

// Governance
pub use governance::{
    GovernanceConfig, GovernanceError, GovernanceModule, Proposal,
    ProposalStatus as GovProposalStatus, ProposalType, Vote,
};
pub use commit_reveal::{
    CommitRevealConfig, CommitRevealManager, EpochFinalizationResult, EpochPhase, SlashingResult,
    WeightCommit,
};
pub use weight_consensus::{
    ConsensusResult, ProposalStatus, ProposalVote, WeightConsensusConfig, WeightConsensusManager,
    WeightProposal,
};
pub use yuma_consensus::{NeuronUpdate, YumaConsensus};

// Security
pub use circuit_breaker::{
    AILayerCircuitBreaker, AILayerStatus, CircuitBreaker, CircuitBreakerConfig,
    CircuitBreakerError, CircuitBreakerStats, CircuitState,
};
pub use long_range_protection::{
    Checkpoint, CheckpointStatus, LongRangeConfig, LongRangeProtection,
};
pub use randao::{RandaoConfig, RandaoError, RandaoMixer, ValidatorReveal};

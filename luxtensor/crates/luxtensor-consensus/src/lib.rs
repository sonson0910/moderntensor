// LuxTensor consensus module
// Phase 2: Consensus Layer Implementation + Tokenomics v3.1 (Model C)

pub mod error;
pub mod validator;
pub mod pos;
pub mod fork_choice;
pub mod rotation;
pub mod fork_resolution;
pub mod fast_finality;
pub mod slashing;
pub mod emission;
pub mod reward_distribution;
pub mod burn_manager;
pub mod reward_executor;
pub mod node_tier;
// NOTE: gpu_detector module removed - never used in actual code
// GPU detection now happens via task-based verification in ScoringManager
pub mod token_allocation;
pub mod commit_reveal;
pub mod weight_consensus;
pub mod long_range_protection;
pub mod halving;
pub mod liveness;
pub mod circuit_breaker;
pub mod randao;
pub mod eip1559;
pub mod economic_model;
// NOTE: oracle.rs removed - LuxTensor uses luxtensor-oracle crate for AI inference
// Price oracle functionality is not needed for AI blockchain

pub use error::*;
pub use validator::{Validator, ValidatorSet};
pub use pos::{ProofOfStake, ConsensusConfig};
pub use fork_choice::{ForkChoice, ForkChoiceSnapshot};
pub use rotation::{ValidatorRotation, RotationConfig, RotationStats, EpochTransitionResult};
pub use fork_resolution::{ForkResolver, ReorgInfo, FinalityStatus, FinalityStats};
pub use fast_finality::{FastFinality, FastFinalityStats, BftPhase, ViewChangeMessage};
pub use slashing::{SlashingManager, SlashingConfig, SlashReason, SlashingEvidence, SlashEvent, JailStatus};
pub use emission::{EmissionController, EmissionConfig, EmissionResult, EmissionStats, UtilityMetrics};
pub use reward_distribution::{
    RewardDistributor, DistributionConfig, LockBonusConfig, DistributionResult,
    MinerInfo, ValidatorInfo, DelegatorInfo, SubnetInfo, InfrastructureNodeInfo
};
pub use burn_manager::{BurnManager, BurnConfig, BurnType, BurnEvent, BurnStats};
pub use reward_executor::{
    RewardExecutor, EpochResult, ClaimResult, ExecutorStats,
    PendingReward, RewardHistoryEntry, RewardType, AccountBalance
};
pub use node_tier::{
    NodeTier, NodeInfo, NodeRegistry, GpuCapability, logarithmic_stake,
    LIGHT_NODE_STAKE, FULL_NODE_STAKE, VALIDATOR_STAKE, SUPER_VALIDATOR_STAKE
};
// pub use gpu_detector::{GpuDetector, GpuInfo}; // REMOVED: Module unused
pub use token_allocation::{
    TokenAllocation, AllocationCategory, VestingSchedule, VestingEntry,
    TgeResult, AllocationStats, TOTAL_SUPPLY, DECIMALS
};
pub use commit_reveal::{
    CommitRevealManager, CommitRevealConfig, WeightCommit, EpochPhase,
    SlashingResult, EpochFinalizationResult
};
pub use weight_consensus::{
    WeightConsensusManager, WeightConsensusConfig, WeightProposal,
    ProposalStatus, ConsensusResult, ProposalVote
};
pub use halving::{
    HalvingSchedule, HalvingInfo,
    INITIAL_BLOCK_REWARD, HALVING_INTERVAL, MINIMUM_REWARD, MAX_HALVINGS
};
pub use liveness::{
    LivenessMonitor, LivenessConfig, LivenessAction, LivenessStats
};
pub use long_range_protection::{
    LongRangeProtection, LongRangeConfig, Checkpoint, CheckpointStatus
};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerStats, CircuitState,
    CircuitBreakerError, AILayerCircuitBreaker, AILayerStatus
};
pub use eip1559::{
    FeeMarket, Eip1559Config, FeeHistory
};
pub use economic_model::{
    project_supply, analyze_equilibrium, validate_parameters, generate_report,
    sweep_burn_rate, sweep_tx_volume,
    AnnualSnapshot, EquilibriumResult, SensitivityPoint,
    ProjectionConfig, TokenomicsInconsistency, Severity,
    BLOCKS_PER_YEAR, BLOCK_TIME_SECONDS, PREMINTED_SUPPLY, EMISSION_POOL,
};
pub use randao::{
    RandaoMixer, RandaoConfig, ValidatorReveal, RandaoError
};

pub mod scoring;
pub use scoring::{
    ScoringManager, ScoringConfig, ScoringEvent,
    MinerMetrics, ValidatorMetrics
};


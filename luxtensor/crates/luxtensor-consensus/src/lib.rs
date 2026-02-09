// LuxTensor consensus module
// Phase 2: Consensus Layer Implementation + Tokenomics v3.1 (Model C)

pub mod burn_manager;
pub mod emission;
pub mod error;
pub mod fast_finality;
pub mod fork_choice;
pub mod fork_resolution;
pub mod node_tier;
pub mod pos;
pub mod reward_distribution;
pub mod reward_executor;
pub mod rotation;
pub mod slashing;
pub mod validator;
// NOTE: gpu_detector module removed - never used in actual code
// GPU detection now happens via task-based verification in ScoringManager
pub mod circuit_breaker;
pub mod commit_reveal;
pub mod economic_model;
pub mod eip1559;
pub mod halving;
pub mod liveness;
pub mod long_range_protection;
pub mod randao;
pub mod token_allocation;
pub mod weight_consensus;
// NOTE: oracle.rs removed - LuxTensor uses luxtensor-oracle crate for AI inference
// Price oracle functionality is not needed for AI blockchain

pub use burn_manager::{BurnConfig, BurnEvent, BurnManager, BurnStats, BurnType};
pub use emission::{
    EmissionConfig, EmissionController, EmissionResult, EmissionStats, UtilityMetrics,
};
pub use error::{ConsensusError, Result};
pub use fast_finality::{BftPhase, FastFinality, FastFinalityStats, ViewChangeMessage};
pub use fork_choice::{ForkChoice, ForkChoiceSnapshot};
pub use fork_resolution::{FinalityStats, FinalityStatus, ForkResolver, ReorgInfo};
pub use node_tier::{
    logarithmic_stake, GpuCapability, NodeInfo, NodeRegistry, NodeTier, FULL_NODE_STAKE,
    LIGHT_NODE_STAKE, SUPER_VALIDATOR_STAKE, VALIDATOR_STAKE,
};
pub use pos::{ConsensusConfig, ProofOfStake};
pub use reward_distribution::{
    DelegatorInfo, DistributionConfig, DistributionResult, InfrastructureNodeInfo, LockBonusConfig,
    MinerInfo, RewardDistributor, SubnetInfo, ValidatorInfo,
};
pub use reward_executor::{
    AccountBalance, ClaimResult, EpochResult, ExecutorStats, PendingReward, RewardExecutor,
    RewardHistoryEntry, RewardType,
};
pub use rotation::{EpochTransitionResult, RotationConfig, RotationStats, ValidatorRotation};
pub use slashing::{
    JailStatus, SlashEvent, SlashReason, SlashingConfig, SlashingEvidence, SlashingManager,
};
pub use validator::{Validator, ValidatorSet};
// pub use gpu_detector::{GpuDetector, GpuInfo}; // REMOVED: Module unused
pub use circuit_breaker::{
    AILayerCircuitBreaker, AILayerStatus, CircuitBreaker, CircuitBreakerConfig,
    CircuitBreakerError, CircuitBreakerStats, CircuitState,
};
pub use commit_reveal::{
    CommitRevealConfig, CommitRevealManager, EpochFinalizationResult, EpochPhase, SlashingResult,
    WeightCommit,
};
pub use economic_model::{
    analyze_equilibrium, generate_report, project_supply, sweep_burn_rate, sweep_tx_volume,
    validate_parameters, AnnualSnapshot, EquilibriumResult, ProjectionConfig, SensitivityPoint,
    Severity, TokenomicsInconsistency, BLOCKS_PER_YEAR, BLOCK_TIME_SECONDS, EMISSION_POOL,
    PREMINTED_SUPPLY,
};
pub use eip1559::{Eip1559Config, FeeHistory, FeeMarket};
pub use halving::{
    HalvingInfo, HalvingSchedule, HALVING_INTERVAL, INITIAL_BLOCK_REWARD, MAX_HALVINGS,
    MINIMUM_REWARD,
};
pub use liveness::{LivenessAction, LivenessConfig, LivenessMonitor, LivenessStats};
pub use long_range_protection::{
    Checkpoint, CheckpointStatus, LongRangeConfig, LongRangeProtection,
};
pub use randao::{RandaoConfig, RandaoError, RandaoMixer, ValidatorReveal};
pub use token_allocation::{
    AllocationCategory, AllocationStats, TgeResult, TokenAllocation, VestingEntry, VestingSchedule,
    DECIMALS, TOTAL_SUPPLY,
};
pub use weight_consensus::{
    ConsensusResult, ProposalStatus, ProposalVote, WeightConsensusConfig, WeightConsensusManager,
    WeightProposal,
};

pub mod scoring;
pub use scoring::{MinerMetrics, ScoringConfig, ScoringEvent, ScoringManager, ValidatorMetrics};

pub mod governance;
pub use governance::{
    GovernanceConfig, GovernanceError, GovernanceModule, Proposal,
    ProposalStatus as GovProposalStatus, ProposalType, Vote,
};

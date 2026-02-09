//! Scoring Manager - Performance-based scoring for miners and validators
//!
//! This module provides:
//! - Performance tracking for miners (AI task completion rate, latency, quality)
//! - Performance tracking for validators (uptime, block production, attestations)
//! - Score calculation and updates
//! - Integration with reward distribution

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================
// Types
// ============================================================

/// Miner performance metrics
#[derive(Debug, Clone, Default)]
pub struct MinerMetrics {
    /// Total tasks completed
    pub tasks_completed: u64,
    /// Total tasks failed/timeout
    pub tasks_failed: u64,
    /// Average execution time in ms
    pub avg_execution_time: u64,
    /// Average quality score (0-100)
    pub avg_quality_score: u32,
    /// Last activity timestamp
    pub last_active: u64,
    /// Cumulative score (0-100_000 for precision)
    pub score: u32,
    /// GPU tasks completed this epoch (verified by validator)
    pub gpu_tasks_completed: u32,
    /// GPU tasks assigned this epoch
    pub gpu_tasks_assigned: u32,
}

/// Validator performance metrics
#[derive(Debug, Clone, Default)]
pub struct ValidatorMetrics {
    /// Total blocks produced
    pub blocks_produced: u64,
    /// Missed block opportunities
    pub blocks_missed: u64,
    /// Average attestation delay in ms
    pub avg_attestation_delay: u64,
    /// Total attestations made
    pub attestations_made: u64,
    /// Uptime percentage (0-100)
    pub uptime: u32,
    /// Last activity timestamp
    pub last_active: u64,
    /// Cumulative score (0-100_000 for precision)
    pub score: u32,
}

/// Scoring event types
#[derive(Debug, Clone)]
pub enum ScoringEvent {
    /// Miner completed a task
    TaskCompleted {
        miner: [u8; 20],
        execution_time: u64,
        quality_score: u32,
    },
    /// Miner failed a task
    TaskFailed {
        miner: [u8; 20],
        reason: String,
    },
    /// GPU task assigned to miner (for AI subnets)
    GpuTaskAssigned {
        miner: [u8; 20],
    },
    /// GPU task completed by miner (verified by validator)
    GpuTaskCompleted {
        miner: [u8; 20],
        execution_time: u64,
        quality_score: u32,
    },
    /// Validator produced a block
    BlockProduced {
        validator: [u8; 20],
    },
    /// Validator missed block opportunity
    BlockMissed {
        validator: [u8; 20],
    },
    /// Validator made attestation
    AttestationMade {
        validator: [u8; 20],
        delay: u64,
    },
}

/// Scoring configuration
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// Decay factor for old performance (0-1)
    pub decay_factor: f64,
    /// Weight for task completion rate
    pub task_completion_weight: f64,
    /// Weight for execution time
    pub execution_time_weight: f64,
    /// Weight for quality score
    pub quality_weight: f64,
    /// Weight for block production
    pub block_production_weight: f64,
    /// Weight for uptime
    pub uptime_weight: f64,
    /// Minimum score (never goes below this)
    pub min_score: u32,
    /// Maximum score
    pub max_score: u32,
    /// Score decay interval (seconds)
    pub decay_interval: u64,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            decay_factor: 0.95,
            task_completion_weight: 0.4,
            execution_time_weight: 0.2,
            quality_weight: 0.4,
            block_production_weight: 0.5,
            uptime_weight: 0.5,
            min_score: 1000,     // 1% minimum
            max_score: 100_000,  // 100% maximum
            decay_interval: 86400, // 1 day
        }
    }
}

// ============================================================
// Scoring Manager
// ============================================================

/// Scoring Manager - tracks and calculates performance scores
pub struct ScoringManager {
    config: ScoringConfig,
    miner_metrics: HashMap<[u8; 20], MinerMetrics>,
    validator_metrics: HashMap<[u8; 20], ValidatorMetrics>,
    last_decay: u64,
}

impl ScoringManager {
    /// Create new ScoringManager with default config
    pub fn new() -> Self {
        Self::with_config(ScoringConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: ScoringConfig) -> Self {
        Self {
            config,
            miner_metrics: HashMap::new(),
            validator_metrics: HashMap::new(),
            last_decay: current_timestamp(),
        }
    }

    /// Process a scoring event
    pub fn process_event(&mut self, event: ScoringEvent) {
        match event {
            ScoringEvent::TaskCompleted {
                miner,
                execution_time,
                quality_score,
            } => {
                self.record_task_completed(miner, execution_time, quality_score);
            }
            ScoringEvent::TaskFailed { miner, .. } => {
                self.record_task_failed(miner);
            }
            ScoringEvent::GpuTaskAssigned { miner } => {
                self.record_gpu_task_assigned(miner);
            }
            ScoringEvent::GpuTaskCompleted {
                miner,
                execution_time,
                quality_score,
            } => {
                self.record_gpu_task_completed(miner, execution_time, quality_score);
            }
            ScoringEvent::BlockProduced { validator } => {
                self.record_block_produced(validator);
            }
            ScoringEvent::BlockMissed { validator } => {
                self.record_block_missed(validator);
            }
            ScoringEvent::AttestationMade { validator, delay } => {
                self.record_attestation(validator, delay);
            }
        }
    }

    /// Record successful task completion
    pub fn record_task_completed(
        &mut self,
        miner: [u8; 20],
        execution_time: u64,
        quality_score: u32,
    ) {
        let metrics = self
            .miner_metrics
            .entry(miner)
            .or_insert_with(MinerMetrics::default);

        metrics.tasks_completed += 1;
        metrics.last_active = current_timestamp();

        // Update average execution time (rolling average)
        if metrics.avg_execution_time == 0 {
            metrics.avg_execution_time = execution_time;
        } else {
            metrics.avg_execution_time = (metrics.avg_execution_time + execution_time) / 2;
        }

        // Update average quality score
        if metrics.avg_quality_score == 0 {
            metrics.avg_quality_score = quality_score;
        } else {
            metrics.avg_quality_score = (metrics.avg_quality_score + quality_score) / 2;
        }

        // Recalculate score
        self.recalculate_miner_score(miner);
    }

    /// Record failed task
    pub fn record_task_failed(&mut self, miner: [u8; 20]) {
        let metrics = self
            .miner_metrics
            .entry(miner)
            .or_insert_with(MinerMetrics::default);

        metrics.tasks_failed += 1;
        metrics.last_active = current_timestamp();

        // Recalculate score
        self.recalculate_miner_score(miner);
    }

    /// Record GPU task assigned (for AI subnets)
    pub fn record_gpu_task_assigned(&mut self, miner: [u8; 20]) {
        let metrics = self
            .miner_metrics
            .entry(miner)
            .or_insert_with(MinerMetrics::default);

        metrics.gpu_tasks_assigned += 1;
        metrics.last_active = current_timestamp();
    }

    /// Record GPU task completed (verified by validator)
    pub fn record_gpu_task_completed(
        &mut self,
        miner: [u8; 20],
        execution_time: u64,
        quality_score: u32,
    ) {
        // Record as regular task completion
        self.record_task_completed(miner, execution_time, quality_score);

        // Also increment GPU-specific counter
        if let Some(metrics) = self.miner_metrics.get_mut(&miner) {
            metrics.gpu_tasks_completed += 1;
        }
    }

    /// Reset GPU task counters for new epoch
    pub fn reset_epoch_stats(&mut self) {
        for metrics in self.miner_metrics.values_mut() {
            metrics.gpu_tasks_completed = 0;
            metrics.gpu_tasks_assigned = 0;
        }
    }

    /// Record block production
    pub fn record_block_produced(&mut self, validator: [u8; 20]) {
        let metrics = self
            .validator_metrics
            .entry(validator)
            .or_insert_with(ValidatorMetrics::default);

        metrics.blocks_produced += 1;
        metrics.last_active = current_timestamp();
        metrics.uptime = 100; // Active = 100% uptime for this cycle

        self.recalculate_validator_score(validator);
    }

    /// Record missed block
    pub fn record_block_missed(&mut self, validator: [u8; 20]) {
        let metrics = self
            .validator_metrics
            .entry(validator)
            .or_insert_with(ValidatorMetrics::default);

        metrics.blocks_missed += 1;

        self.recalculate_validator_score(validator);
    }

    /// Record attestation
    pub fn record_attestation(&mut self, validator: [u8; 20], delay: u64) {
        let metrics = self
            .validator_metrics
            .entry(validator)
            .or_insert_with(ValidatorMetrics::default);

        metrics.attestations_made += 1;
        metrics.last_active = current_timestamp();

        if metrics.avg_attestation_delay == 0 {
            metrics.avg_attestation_delay = delay;
        } else {
            metrics.avg_attestation_delay = (metrics.avg_attestation_delay + delay) / 2;
        }

        self.recalculate_validator_score(validator);
    }

    /// Recalculate miner score
    fn recalculate_miner_score(&mut self, miner: [u8; 20]) {
        if let Some(metrics) = self.miner_metrics.get_mut(&miner) {
            let total_tasks = metrics.tasks_completed + metrics.tasks_failed;
            if total_tasks == 0 {
                metrics.score = self.config.min_score;
                return;
            }

            // Completion rate: 0-1
            let completion_rate = metrics.tasks_completed as f64 / total_tasks as f64;

            // Execution time score: faster = better (inverse, capped at 10s = 1.0)
            let time_score = if metrics.avg_execution_time > 0 {
                (10_000.0 / metrics.avg_execution_time as f64).min(1.0)
            } else {
                0.5
            };

            // Quality score: 0-1
            let quality_score = metrics.avg_quality_score as f64 / 100.0;

            // Weighted score
            let raw_score = (completion_rate * self.config.task_completion_weight
                + time_score * self.config.execution_time_weight
                + quality_score * self.config.quality_weight)
                * self.config.max_score as f64;

            metrics.score = if raw_score.is_nan() || raw_score < 0.0 {
                self.config.min_score
            } else {
                (raw_score as u32).clamp(self.config.min_score, self.config.max_score)
            };
        }
    }

    /// Recalculate validator score
    fn recalculate_validator_score(&mut self, validator: [u8; 20]) {
        if let Some(metrics) = self.validator_metrics.get_mut(&validator) {
            let total_blocks = metrics.blocks_produced + metrics.blocks_missed;
            if total_blocks == 0 {
                metrics.score = self.config.min_score;
                return;
            }

            // Block production rate
            let production_rate = metrics.blocks_produced as f64 / total_blocks as f64;

            // Uptime factor
            let uptime_score = metrics.uptime as f64 / 100.0;

            // Weighted score
            let raw_score = (production_rate * self.config.block_production_weight
                + uptime_score * self.config.uptime_weight)
                * self.config.max_score as f64;

            metrics.score = if raw_score.is_nan() || raw_score < 0.0 {
                self.config.min_score
            } else {
                (raw_score as u32).clamp(self.config.min_score, self.config.max_score)
            };
        }
    }

    /// Apply decay to all scores (call periodically)
    pub fn apply_decay(&mut self) {
        let now = current_timestamp();
        if now - self.last_decay < self.config.decay_interval {
            return;
        }

        for metrics in self.miner_metrics.values_mut() {
            let decayed = metrics.score as f64 * self.config.decay_factor;
            metrics.score = if decayed.is_nan() || decayed < 0.0 {
                self.config.min_score
            } else {
                (decayed as u32).max(self.config.min_score)
            };
        }

        for metrics in self.validator_metrics.values_mut() {
            let decayed = metrics.score as f64 * self.config.decay_factor;
            metrics.score = if decayed.is_nan() || decayed < 0.0 {
                self.config.min_score
            } else {
                (decayed as u32).max(self.config.min_score)
            };
        }

        self.last_decay = now;
    }

    /// Get miner score (0-100_000, representing 0-100%)
    pub fn get_miner_score(&self, miner: &[u8; 20]) -> u32 {
        self.miner_metrics
            .get(miner)
            .map(|m| m.score)
            .unwrap_or(self.config.min_score)
    }

    /// Get validator score (0-100_000, representing 0-100%)
    pub fn get_validator_score(&self, validator: &[u8; 20]) -> u32 {
        self.validator_metrics
            .get(validator)
            .map(|m| m.score)
            .unwrap_or(self.config.min_score)
    }

    /// Get all miner scores as address -> score map
    pub fn get_all_miner_scores(&self) -> HashMap<[u8; 20], u32> {
        self.miner_metrics
            .iter()
            .map(|(addr, m)| (*addr, m.score))
            .collect()
    }

    /// Get all validator scores as address -> score map
    pub fn get_all_validator_scores(&self) -> HashMap<[u8; 20], u32> {
        self.validator_metrics
            .iter()
            .map(|(addr, m)| (*addr, m.score))
            .collect()
    }

    /// Get miner metrics
    pub fn get_miner_metrics(&self, miner: &[u8; 20]) -> Option<&MinerMetrics> {
        self.miner_metrics.get(miner)
    }

    /// Get all miners as MinerEpochStats for reward distribution
    /// This bridges ScoringManager to RewardDistributor
    pub fn get_all_miner_epoch_stats(&self) -> Vec<crate::reward_distribution::MinerEpochStats> {
        self.miner_metrics
            .iter()
            .map(|(addr, m)| {
                crate::reward_distribution::MinerEpochStats::with_tasks(
                    *addr,
                    m.score as f64 / self.config.max_score as f64,  // Normalize to 0.0-1.0
                    (m.tasks_completed.saturating_sub(m.gpu_tasks_completed as u64)).min(u32::MAX as u64) as u32,  // CPU tasks (clamped)
                    m.gpu_tasks_completed,
                    m.gpu_tasks_assigned,
                )
            })
            .collect()
    }

    /// Get validator metrics
    pub fn get_validator_metrics(&self, validator: &[u8; 20]) -> Option<&ValidatorMetrics> {
        self.validator_metrics.get(validator)
    }

    /// Get total miner count
    pub fn miner_count(&self) -> usize {
        self.miner_metrics.len()
    }

    /// Get total validator count
    pub fn validator_count(&self) -> usize {
        self.validator_metrics.len()
    }
}

impl Default for ScoringManager {
    fn default() -> Self {
        Self::new()
    }
}

// Helper function
// NOTE: Scoring uses wall-clock time for local tracking only (not consensus-critical).
// The `last_active` and `last_decay` fields are per-node observations used for
// local score decay heuristics, not replicated consensus state. If scoring ever
// becomes consensus-critical (e.g., used in reward calculation on-chain), this
// must be replaced with deterministic block timestamps.
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miner_scoring() {
        let mut manager = ScoringManager::new();
        let miner = [1u8; 20];

        // Complete some tasks
        manager.record_task_completed(miner, 1000, 80);
        manager.record_task_completed(miner, 2000, 90);

        let score = manager.get_miner_score(&miner);
        assert!(score > 0);
        assert!(score <= 100_000);
    }

    #[test]
    fn test_validator_scoring() {
        let mut manager = ScoringManager::new();
        let validator = [2u8; 20];

        // Produce some blocks
        manager.record_block_produced(validator);
        manager.record_block_produced(validator);
        manager.record_block_missed(validator);

        let score = manager.get_validator_score(&validator);
        assert!(score > 0);
    }

    #[test]
    fn test_failed_task_affects_score() {
        let mut manager = ScoringManager::new();
        let miner = [3u8; 20];

        // Complete tasks
        manager.record_task_completed(miner, 1000, 90);
        let score_before = manager.get_miner_score(&miner);

        // Fail a task
        manager.record_task_failed(miner);
        let score_after = manager.get_miner_score(&miner);

        assert!(score_after < score_before);
    }

    #[test]
    fn test_process_events() {
        let mut manager = ScoringManager::new();

        manager.process_event(ScoringEvent::TaskCompleted {
            miner: [1u8; 20],
            execution_time: 500,
            quality_score: 95,
        });

        manager.process_event(ScoringEvent::BlockProduced {
            validator: [2u8; 20],
        });

        assert_eq!(manager.miner_count(), 1);
        assert_eq!(manager.validator_count(), 1);
    }
}

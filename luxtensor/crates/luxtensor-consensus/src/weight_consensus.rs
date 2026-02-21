// Multi-Validator Weight Consensus
// Requires multiple validators to agree on weights before applying
//
// Flow:
// 1. Validator proposes weights
// 2. Random committee is selected (Fix 6 — anti-collusion)
// 3. Committee members vote (approve/reject), weighted by V-Trust (Fix 5)
// 4. If threshold met (e.g., 2/3 majority), weights are applied
// 5. Proposer gets small reward for successful proposal

use luxtensor_core::types::{Address, Hash};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use tracing::{info, warn};

/// Configuration for weight consensus mechanism
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightConsensusConfig {
    /// Minimum validators needed for consensus (e.g., 2)
    pub min_validators: usize,
    /// Threshold percentage for approval (e.g., 67 = 2/3)
    pub approval_threshold_percent: u8,
    /// Blocks before proposal expires
    pub proposal_timeout: u64,
    /// Blocks between proposals from same validator
    pub proposal_cooldown: u64,
    /// Whether to slash for rejected proposals
    pub slash_rejected_proposals: bool,
    /// Reward for successful proposal (in basis points of emission)
    pub proposer_reward_bps: u16,
}

impl Default for WeightConsensusConfig {
    fn default() -> Self {
        Self {
            // 🔧 FIX: Raised from 2 to 5 — with min_validators=2, a single
            // compromised node + the proposer can approve malicious weight updates.
            // 5 validators require at least 4 votes at 67% threshold.
            min_validators: 5,
            approval_threshold_percent: 67, // 2/3 majority
            proposal_timeout: 200,          // ~40 minutes
            proposal_cooldown: 50,          // ~10 minutes
            slash_rejected_proposals: false,
            proposer_reward_bps: 10, // 0.1% bonus
        }
    }
}

/// Status of a weight proposal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Proposal is active and accepting votes
    Pending,
    /// Proposal reached approval threshold
    Approved,
    /// Proposal was rejected (timeout or votes)
    Rejected,
    /// Weights have been applied
    Applied,
    /// Proposal expired without enough votes
    Expired,
}

/// A vote on a weight proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalVote {
    pub voter: Address,
    pub approve: bool,
    pub block: u64,
    pub signature: Option<Hash>,
    /// Stake of the voter at the time of voting (for weighted calculations)
    pub stake: u128,
}

/// A weight proposal from a validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightProposal {
    /// Unique proposal ID
    pub id: Hash,
    /// Proposer address
    pub proposer: Address,
    /// Subnet ID
    pub subnet_uid: u64,
    /// Proposed weights (uid, weight)
    pub weights: Vec<(u64, u16)>,
    /// Hash of weights for verification
    pub weights_hash: Hash,
    /// Block when proposed
    pub proposed_at: u64,
    /// Expiration block
    pub expires_at: u64,
    /// Current status
    pub status: ProposalStatus,
    /// Votes received
    pub votes: Vec<ProposalVote>,
    /// Number of eligible voters at proposal time
    pub eligible_voters: usize,
}

impl WeightProposal {
    /// Create new proposal
    pub fn new(
        proposer: Address,
        subnet_uid: u64,
        weights: Vec<(u64, u16)>,
        current_block: u64,
        timeout: u64,
        eligible_voters: usize,
    ) -> Self {
        let weights_hash = compute_weights_hash(&weights);
        let id = compute_proposal_id(&proposer, subnet_uid, current_block, &weights_hash);

        Self {
            id,
            proposer,
            subnet_uid,
            weights,
            weights_hash,
            proposed_at: current_block,
            expires_at: current_block + timeout,
            status: ProposalStatus::Pending,
            votes: Vec::new(),
            eligible_voters,
        }
    }

    /// Count approval votes
    pub fn approval_count(&self) -> usize {
        self.votes.iter().filter(|v| v.approve).count()
    }

    /// Count rejection votes
    pub fn rejection_count(&self) -> usize {
        self.votes.iter().filter(|v| !v.approve).count()
    }

    /// Calculate approval percentage by head-count (0-100)
    ///
    /// Each vote counts equally regardless of stake. Used as a secondary
    /// check alongside stake-weighted approval.
    pub fn approval_percentage(&self) -> u8 {
        if self.votes.is_empty() {
            return 0;
        }
        ((self.approval_count() * 100) / self.votes.len()).min(100) as u8
    }

    /// Calculate stake-weighted approval percentage (0-100)
    ///
    /// SECURITY: Primary consensus metric — weights each vote by the voter's
    /// stake at the time of voting. This prevents a coalition of low-stake
    /// validators from overriding high-stake validators.
    pub fn stake_weighted_approval(&self) -> u8 {
        let total_stake: u128 = self.votes.iter().map(|v| v.stake).sum();
        if total_stake == 0 {
            return 0;
        }
        let approval_stake: u128 = self.votes.iter().filter(|v| v.approve).map(|v| v.stake).sum();
        ((approval_stake * 100) / total_stake).min(100) as u8
    }

    /// Check if voter has already voted
    pub fn has_voted(&self, voter: &Address) -> bool {
        self.votes.iter().any(|v| &v.voter == voter)
    }

    /// Check if expired
    pub fn is_expired(&self, current_block: u64) -> bool {
        current_block >= self.expires_at
    }
}

/// Compute hash of weights for verification
pub fn compute_weights_hash(weights: &[(u64, u16)]) -> Hash {
    let mut hasher = Keccak256::new();

    for (uid, weight) in weights {
        hasher.update(uid.to_be_bytes());
        hasher.update(weight.to_be_bytes());
    }

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Compute unique proposal ID
pub fn compute_proposal_id(
    proposer: &Address,
    subnet_uid: u64,
    block: u64,
    weights_hash: &Hash,
) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(proposer);
    hasher.update(subnet_uid.to_be_bytes());
    hasher.update(block.to_be_bytes());
    hasher.update(weights_hash);

    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Result of a consensus check
#[derive(Debug, Clone, PartialEq)]
pub struct ConsensusResult {
    pub reached: bool,
    pub approval_count: usize,
    pub rejection_count: usize,
    pub total_votes: usize,
    pub approval_percentage: u8,
    pub threshold: u8,
}

/// Single voting event captured for pattern analysis.
///
/// SECURITY: This struct is used to detect collusion patterns
/// (e.g., synchronized voting, unusually correlated approval/rejection).
#[derive(Debug, Clone)]
pub struct VotingRecord {
    pub voter: Address,
    pub proposal_id: Hash,
    pub approve: bool,
    pub block: u64,
}

/// Statistics for a single voter's historical behavior.
#[derive(Debug, Clone)]
pub struct VoterStats {
    pub total_votes: usize,
    pub approvals: usize,
    pub rejections: usize,
    pub approval_rate: f64,
}

/// Tracks validator voting patterns to detect potential collusion.
///
/// Uses a ring buffer per voter (max_records_per_voter) to bound memory.
/// Correlation detection requires >= MIN_COMMON_PROPOSALS shared proposals
/// between two voters before flagging.
pub struct VotingPatternTracker {
    /// Per-voter voting history (ring buffer)
    records: HashMap<Address, Vec<VotingRecord>>,
    /// Maximum records kept per voter (prevents unbounded growth)
    max_records_per_voter: usize,
}

impl VotingPatternTracker {
    /// Minimum shared proposals before correlation is meaningful.
    const MIN_COMMON_PROPOSALS: usize = 5;

    pub fn new(max_records_per_voter: usize) -> Self {
        Self {
            records: HashMap::new(),
            max_records_per_voter,
        }
    }

    /// Record a voting event.
    pub fn record_vote(&mut self, voter: Address, proposal_id: Hash, approve: bool, block: u64) {
        let records = self.records.entry(voter).or_insert_with(Vec::new);
        records.push(VotingRecord {
            voter,
            proposal_id,
            approve,
            block,
        });
        // Ring buffer: evict oldest when full
        if records.len() > self.max_records_per_voter {
            let excess = records.len() - self.max_records_per_voter;
            records.drain(..excess);
        }
    }

    /// Detect pairs of voters whose agreement rate exceeds `threshold` (0.0–1.0).
    ///
    /// Returns: Vec of (voter_a, voter_b, agreement_rate, common_proposal_count)
    /// Only considers pairs with at least MIN_COMMON_PROPOSALS in common.
    pub fn detect_correlated_voters(
        &self,
        threshold: f64,
    ) -> Vec<(Address, Address, f64, usize)> {
        let voters: Vec<&Address> = self.records.keys().collect();
        let mut correlated = Vec::new();

        for i in 0..voters.len() {
            for j in (i + 1)..voters.len() {
                let a = voters[i];
                let b = voters[j];

                // Build proposal→approve maps for each voter
                let map_a: HashMap<Hash, bool> = self.records[a]
                    .iter()
                    .map(|r| (r.proposal_id, r.approve))
                    .collect();
                let map_b: HashMap<Hash, bool> = self.records[b]
                    .iter()
                    .map(|r| (r.proposal_id, r.approve))
                    .collect();

                // Count common proposals and agreements
                let mut common = 0usize;
                let mut agree = 0usize;
                for (pid, &vote_a) in &map_a {
                    if let Some(&vote_b) = map_b.get(pid) {
                        common += 1;
                        if vote_a == vote_b {
                            agree += 1;
                        }
                    }
                }

                if common >= Self::MIN_COMMON_PROPOSALS {
                    let rate = agree as f64 / common as f64;
                    if rate >= threshold {
                        correlated.push((*a, *b, rate, common));
                    }
                }
            }
        }

        correlated
    }

    /// Get per-voter statistics.
    pub fn voter_stats(&self) -> HashMap<Address, VoterStats> {
        self.records
            .iter()
            .map(|(&addr, records)| {
                let approvals = records.iter().filter(|r| r.approve).count();
                let total = records.len();
                (
                    addr,
                    VoterStats {
                        total_votes: total,
                        approvals,
                        rejections: total - approvals,
                        approval_rate: if total > 0 {
                            approvals as f64 / total as f64
                        } else {
                            0.0
                        },
                    },
                )
            })
            .collect()
    }
}

// ==================== V-Trust Scoring (Fix 5) ====================

/// Tracks each validator's historical accuracy to build a trust score.
///
/// V-Trust = (consensus-aligned votes) / (total votes).
/// Validators that consistently vote with the final outcome earn higher trust,
/// which can be used to weight their future votes or prioritize committee slots.
#[derive(Debug, Clone)]
pub struct VTrustScorer {
    /// (aligned_count, total_count) per validator
    scores: HashMap<Address, (u64, u64)>,
}

impl VTrustScorer {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
        }
    }

    /// Record the outcome of a finalized proposal for each voter.
    ///
    /// `final_approved`: whether the proposal was ultimately approved.
    /// Each voter who agreed with the final outcome gets an "aligned" count.
    pub fn record_outcome(&mut self, votes: &[ProposalVote], final_approved: bool) {
        for vote in votes {
            let entry = self.scores.entry(vote.voter).or_insert((0, 0));
            entry.1 += 1; // total
            if vote.approve == final_approved {
                entry.0 += 1; // aligned
            }
        }
    }

    /// Get the trust score for a validator (0.0 – 1.0).
    ///
    /// Returns `None` if the validator has no history.
    pub fn trust_score(&self, validator: &Address) -> Option<f64> {
        self.scores.get(validator).map(|&(aligned, total)| {
            if total == 0 {
                0.0
            } else {
                aligned as f64 / total as f64
            }
        })
    }

    /// Get trust scores for all tracked validators.
    pub fn all_scores(&self) -> HashMap<Address, f64> {
        self.scores
            .iter()
            .map(|(&addr, &(aligned, total))| {
                let score = if total == 0 { 0.0 } else { aligned as f64 / total as f64 };
                (addr, score)
            })
            .collect()
    }

    /// Penalty multiplier for collusion detection.
    /// Each penalty inflates the total by this factor, reducing V-Trust.
    const PENALTY_FACTOR: u64 = 10;

    /// Apply a collusion penalty to a specific validator.
    ///
    /// Inflates their total vote count by `PENALTY_FACTOR`, which reduces
    /// their trust score without erasing aligned history. Multiple penalties
    /// compound (each call adds `PENALTY_FACTOR` to total).
    pub fn apply_collusion_penalty(&mut self, validator: &Address) {
        let entry = self.scores.entry(*validator).or_insert((0, 0));
        // Inflate total to reduce aligned/total ratio
        entry.1 = entry.1.saturating_add(Self::PENALTY_FACTOR);
    }
}

// ==================== Random Committee Selection (Fix 6) ====================

/// Deterministically select a random committee from a set of validators.
///
/// Uses `block_hash ⊕ subnet_uid` as a seed for a Fisher-Yates-like shuffle,
/// then takes the first `committee_size` validators. This ensures:
/// - Different committees per block and subnet (unpredictable to colluders)
/// - Determinism (all honest nodes agree on the same committee)
/// - No external randomness source required
pub fn select_committee(
    validators: &[Address],
    committee_size: usize,
    block_hash: &Hash,
    subnet_uid: u64,
) -> Vec<Address> {
    if validators.is_empty() || committee_size == 0 {
        return Vec::new();
    }
    let size = committee_size.min(validators.len());

    // Build seed: H(block_hash || subnet_uid)
    let mut hasher = Keccak256::new();
    hasher.update(block_hash);
    hasher.update(subnet_uid.to_le_bytes());
    let seed: [u8; 32] = hasher.finalize().into();

    // Fisher-Yates partial shuffle using seed bytes
    let mut indices: Vec<usize> = (0..validators.len()).collect();
    for i in 0..size {
        // Derive a pseudo-random index from the seed
        let mut idx_hasher = Keccak256::new();
        idx_hasher.update(seed);
        idx_hasher.update((i as u64).to_le_bytes());
        let h: [u8; 32] = idx_hasher.finalize().into();
        let rand_val = u64::from_le_bytes(h[0..8].try_into().unwrap());
        let j = (rand_val as usize % (validators.len() - i)) + i;
        indices.swap(i, j);
    }

    indices[..size].iter().map(|&i| validators[i]).collect()
}

/// Weight Consensus Manager
pub struct WeightConsensusManager {
    config: WeightConsensusConfig,
    /// Active proposals by subnet
    proposals: RwLock<HashMap<u64, Vec<WeightProposal>>>,
    /// Last proposal time per validator
    last_proposal: RwLock<HashMap<Address, u64>>,
    /// Applied weights history
    applied_history: RwLock<Vec<(u64, Hash, Vec<(u64, u16)>)>>,
    /// SECURITY: Tracks voting patterns for collusion detection
    voting_tracker: RwLock<VotingPatternTracker>,
    /// SECURITY: Tracks validator accuracy for trust-weighted voting
    vtrust_scorer: RwLock<VTrustScorer>,
}

impl WeightConsensusManager {
    /// Create new manager
    pub fn new(config: WeightConsensusConfig) -> Self {
        Self {
            config,
            proposals: RwLock::new(HashMap::new()),
            last_proposal: RwLock::new(HashMap::new()),
            applied_history: RwLock::new(Vec::new()),
            voting_tracker: RwLock::new(VotingPatternTracker::new(1000)),
            vtrust_scorer: RwLock::new(VTrustScorer::new()),
        }
    }

    /// Create a new weight proposal
    pub fn propose_weights(
        &self,
        subnet_uid: u64,
        proposer: Address,
        weights: Vec<(u64, u16)>,
        current_block: u64,
        validator_count: usize,
    ) -> Result<Hash, ConsensusError> {
        // SECURITY: Use write lock for cooldown check + update atomically.
        // 🔧 FIX: Previously used read lock for check, then separate write lock for update,
        // allowing concurrent calls to bypass the cooldown via TOCTOU race.
        let mut last_proposal = self.last_proposal.write();
        if let Some(last_block) = last_proposal.get(&proposer).copied() {
            if current_block < last_block + self.config.proposal_cooldown {
                return Err(ConsensusError::ProposalCooldown);
            }
        }

        // Check minimum validators
        if validator_count < self.config.min_validators {
            return Err(ConsensusError::InsufficientValidators);
        }

        // Check weights not empty
        if weights.is_empty() {
            return Err(ConsensusError::EmptyWeights);
        }

        // Create proposal
        let proposal = WeightProposal::new(
            proposer,
            subnet_uid,
            weights,
            current_block,
            self.config.proposal_timeout,
            validator_count,
        );

        let proposal_id = proposal.id;

        // Store proposal
        self.proposals.write().entry(subnet_uid).or_insert_with(Vec::new).push(proposal);

        // Update last proposal time (already holding write lock from cooldown check)
        last_proposal.insert(proposer, current_block);
        drop(last_proposal); // explicit drop

        info!("New weight proposal {:?} for subnet {} by {:?}", proposal_id, subnet_uid, proposer);

        Ok(proposal_id)
    }

    /// Vote on a proposal
    ///
    /// SECURITY: `voter_stake` MUST be looked up from the `ValidatorSet` by the caller.
    /// Passing user-supplied stake values would allow vote weight manipulation.
    pub fn vote(
        &self,
        proposal_id: Hash,
        voter: Address,
        approve: bool,
        current_block: u64,
        voter_stake: u128,
    ) -> Result<ConsensusResult, ConsensusError> {
        // SECURITY: Reject zero-stake voters (not a real validator)
        if voter_stake == 0 {
            return Err(ConsensusError::InsufficientValidators);
        }

        let mut proposals = self.proposals.write();

        // Find proposal
        let proposal = Self::find_proposal_mut(&mut proposals, proposal_id)?;

        // Validate vote eligibility (extracted for lower complexity)
        Self::validate_vote_eligibility(proposal, &voter, current_block)?;

        // Record vote with stake for weighted approval calculation
        proposal.votes.push(ProposalVote {
            voter,
            approve,
            block: current_block,
            signature: None,
            stake: voter_stake,
        });

        // SECURITY: Record voting pattern for collusion detection
        {
            let mut tracker = self.voting_tracker.write();
            tracker.record_vote(voter, proposal_id, approve, current_block);
        }

        // Check and update consensus status
        let result = self.check_consensus_internal(proposal);
        if result.reached {
            proposal.status = ProposalStatus::Approved;
            info!(
                "Proposal {:?} approved with {}% approval",
                proposal_id, result.approval_percentage
            );
        }

        Ok(result)
    }

    /// Find proposal by ID across all subnets (mutable)
    fn find_proposal_mut<'a>(
        proposals: &'a mut HashMap<u64, Vec<WeightProposal>>,
        proposal_id: Hash,
    ) -> Result<&'a mut WeightProposal, ConsensusError> {
        proposals
            .values_mut()
            .flat_map(|v| v.iter_mut())
            .find(|p| p.id == proposal_id)
            .ok_or(ConsensusError::ProposalNotFound)
    }

    /// Validate that a voter is eligible to vote on a proposal
    fn validate_vote_eligibility(
        proposal: &mut WeightProposal,
        voter: &Address,
        current_block: u64,
    ) -> Result<(), ConsensusError> {
        // Check expiration
        if proposal.is_expired(current_block) {
            proposal.status = ProposalStatus::Expired;
            return Err(ConsensusError::ProposalExpired);
        }

        // Check pending status
        if proposal.status != ProposalStatus::Pending {
            return Err(ConsensusError::ProposalNotPending);
        }

        // Check duplicate vote
        if proposal.has_voted(voter) {
            return Err(ConsensusError::AlreadyVoted);
        }

        // Check self-voting
        if proposal.proposer == *voter {
            return Err(ConsensusError::CannotVoteOwnProposal);
        }

        Ok(())
    }

    /// Check if proposal has reached consensus
    pub fn check_consensus(&self, proposal_id: Hash) -> Result<ConsensusResult, ConsensusError> {
        let proposals = self.proposals.read();

        let proposal = proposals
            .values()
            .flat_map(|v| v.iter())
            .find(|p| p.id == proposal_id)
            .ok_or(ConsensusError::ProposalNotFound)?;

        Ok(self.check_consensus_internal(proposal))
    }

    fn check_consensus_internal(&self, proposal: &WeightProposal) -> ConsensusResult {
        let approval_count = proposal.approval_count();
        let total_votes = proposal.votes.len();
        let approval_percentage = proposal.stake_weighted_approval();

        // Need at least min_validators - 1 votes (excluding proposer)
        let min_votes = self.config.min_validators.saturating_sub(1);
        let enough_votes = total_votes >= min_votes;

        // SECURITY: Use stake-weighted approval for consensus threshold
        // This prevents a coalition of low-stake validators from overriding
        // high-stake validators who have more at risk.
        let reached = enough_votes && approval_percentage >= self.config.approval_threshold_percent;

        ConsensusResult {
            reached,
            approval_count,
            rejection_count: proposal.rejection_count(),
            total_votes,
            approval_percentage,
            threshold: self.config.approval_threshold_percent,
        }
    }

    /// Finalize an approved proposal (apply weights)
    pub fn finalize_proposal(
        &self,
        proposal_id: Hash,
        _current_block: u64,
    ) -> Result<Vec<(u64, u16)>, ConsensusError> {
        let mut proposals = self.proposals.write();

        // Find proposal
        let proposal = proposals
            .values_mut()
            .flat_map(|v| v.iter_mut())
            .find(|p| p.id == proposal_id)
            .ok_or(ConsensusError::ProposalNotFound)?;

        // Check status
        if proposal.status != ProposalStatus::Approved {
            return Err(ConsensusError::NotApproved);
        }

        // Mark as applied
        proposal.status = ProposalStatus::Applied;

        // Record in history
        {
            const MAX_APPLIED_HISTORY: usize = 1000;
            let mut history = self.applied_history.write();
            history.push((
                proposal.subnet_uid,
                proposal_id,
                proposal.weights.clone(),
            ));
            // Prune old history to prevent unbounded memory growth
            if history.len() > MAX_APPLIED_HISTORY {
                let excess = history.len() - MAX_APPLIED_HISTORY;
                history.drain(..excess);
            }
        }

        info!("Proposal {:?} finalized, applying {} weights", proposal_id, proposal.weights.len());

        Ok(proposal.weights.clone())
    }

    /// Get active proposals for subnet
    pub fn get_proposals(&self, subnet_uid: u64) -> Vec<WeightProposal> {
        self.proposals.read().get(&subnet_uid).cloned().unwrap_or_default()
    }

    /// Get pending proposals for subnet
    pub fn get_pending_proposals(&self, subnet_uid: u64) -> Vec<WeightProposal> {
        self.get_proposals(subnet_uid)
            .into_iter()
            .filter(|p| p.status == ProposalStatus::Pending)
            .collect()
    }

    /// Get proposal by ID
    pub fn get_proposal(&self, proposal_id: Hash) -> Option<WeightProposal> {
        self.proposals.read().values().flat_map(|v| v.iter()).find(|p| p.id == proposal_id).cloned()
    }

    /// Clean up expired proposals
    pub fn cleanup_expired(&self, current_block: u64) {
        let mut proposals = self.proposals.write();

        for subnet_proposals in proposals.values_mut() {
            for proposal in subnet_proposals.iter_mut() {
                if proposal.status == ProposalStatus::Pending && proposal.is_expired(current_block)
                {
                    proposal.status = ProposalStatus::Expired;
                    warn!("Proposal {:?} expired at block {}", proposal.id, current_block);
                }
            }

            // Remove old finalized/expired proposals (keep last 100)
            if subnet_proposals.len() > 100 {
                let to_remove = subnet_proposals.len() - 100;
                subnet_proposals.drain(..to_remove);
            }
        }
    }

    /// Get configuration
    pub fn config(&self) -> &WeightConsensusConfig {
        &self.config
    }

    /// Detect suspicious voting patterns (potential collusion).
    ///
    /// Returns pairs of voters with abnormally high correlation.
    /// Default threshold: 0.9 (90%+ agreement across ≥5 common proposals).
    pub fn detect_suspicious_patterns(&self) -> Vec<(Address, Address, f64, usize)> {
        self.voting_tracker.read().detect_correlated_voters(0.9)
    }

    /// Get per-voter statistics for monitoring.
    pub fn voter_statistics(&self) -> HashMap<Address, VoterStats> {
        self.voting_tracker.read().voter_stats()
    }

    /// Record the outcome of a finalized proposal for V-Trust scoring.
    ///
    /// Should be called after `finalize_proposal` to update trust scores.
    pub fn record_proposal_outcome(&self, proposal_id: Hash, final_approved: bool) {
        let proposals = self.proposals.read();
        if let Some(proposal) = proposals
            .values()
            .flat_map(|v| v.iter())
            .find(|p| p.id == proposal_id)
        {
            let mut scorer = self.vtrust_scorer.write();
            scorer.record_outcome(&proposal.votes, final_approved);
        }
    }

    /// Get V-Trust score for a specific validator.
    pub fn vtrust_score(&self, validator: &Address) -> Option<f64> {
        self.vtrust_scorer.read().trust_score(validator)
    }

    /// Get all V-Trust scores.
    pub fn all_vtrust_scores(&self) -> HashMap<Address, f64> {
        self.vtrust_scorer.read().all_scores()
    }

    // ==================== Collusion Penalty (Fix 8) ====================

    /// Apply collusion penalties to validators detected as colluding.
    ///
    /// Runs collusion detection, then reduces V-Trust scores for flagged pairs.
    /// Returns the list of penalized addresses and their new scores.
    pub fn apply_collusion_penalties(&self) -> Vec<(Address, f64)> {
        let correlated = self.detect_suspicious_patterns();
        if correlated.is_empty() {
            return Vec::new();
        }

        let mut scorer = self.vtrust_scorer.write();
        let mut penalized = Vec::new();

        for (a, b, _rate, _count) in &correlated {
            scorer.apply_collusion_penalty(a);
            scorer.apply_collusion_penalty(b);
        }

        // Collect new scores for penalized validators
        let mut seen = std::collections::HashSet::new();
        for (a, b, _, _) in &correlated {
            for addr in [a, b] {
                if seen.insert(*addr) {
                    if let Some(score) = scorer.trust_score(addr) {
                        penalized.push((*addr, score));
                    }
                }
            }
        }

        if !penalized.is_empty() {
            warn!(
                "Applied collusion penalties to {} validators",
                penalized.len()
            );
        }

        penalized
    }
}

/// Errors for consensus operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusError {
    ProposalNotFound,
    ProposalExpired,
    ProposalNotPending,
    ProposalCooldown,
    AlreadyVoted,
    CannotVoteOwnProposal,
    InsufficientValidators,
    EmptyWeights,
    NotApproved,
}

impl std::fmt::Display for ConsensusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProposalNotFound => write!(f, "Proposal not found"),
            Self::ProposalExpired => write!(f, "Proposal has expired"),
            Self::ProposalNotPending => write!(f, "Proposal is not pending"),
            Self::ProposalCooldown => write!(f, "Must wait before proposing again"),
            Self::AlreadyVoted => write!(f, "Already voted on this proposal"),
            Self::CannotVoteOwnProposal => write!(f, "Cannot vote on own proposal"),
            Self::InsufficientValidators => write!(f, "Not enough validators for consensus"),
            Self::EmptyWeights => write!(f, "Cannot propose empty weights"),
            Self::NotApproved => write!(f, "Proposal not approved"),
        }
    }
}

impl std::error::Error for ConsensusError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(n: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[0] = n;
        Address::from(addr)
    }

    #[test]
    fn test_propose_and_vote() {
        let manager = WeightConsensusManager::new(WeightConsensusConfig {
            min_validators: 2,
            approval_threshold_percent: 50,
            proposal_timeout: 100,
            proposal_cooldown: 10,
            ..Default::default()
        });

        let proposer = test_address(1);
        let voter = test_address(2);
        let weights = vec![(0, 500u16), (1, 500u16)];

        // Propose
        let proposal_id = manager.propose_weights(1, proposer, weights, 0, 3).unwrap();

        // Vote (with stake=1000 simulating a validator with 1000 stake)
        let result = manager.vote(proposal_id, voter, true, 5, 1000).unwrap();

        // Should reach consensus (1 vote, 100% stake-weighted approval)
        assert!(result.reached);
        assert_eq!(result.approval_count, 1);
    }

    #[test]
    fn test_proposal_cooldown() {
        let manager = WeightConsensusManager::new(WeightConsensusConfig {
            proposal_cooldown: 50,
            ..Default::default()
        });

        let proposer = test_address(1);
        let weights = vec![(0, 500u16)];

        // First proposal
        manager.propose_weights(1, proposer, weights.clone(), 0, 5).unwrap();

        // Second proposal too soon
        assert_eq!(
            manager.propose_weights(1, proposer, weights.clone(), 10, 5),
            Err(ConsensusError::ProposalCooldown)
        );

        // After cooldown
        assert!(manager.propose_weights(1, proposer, weights, 60, 5).is_ok());
    }

    #[test]
    fn test_cannot_vote_own_proposal() {
        let manager = WeightConsensusManager::new(Default::default());

        let proposer = test_address(1);
        let weights = vec![(0, 500u16)];

        let proposal_id = manager.propose_weights(1, proposer, weights, 0, 5).unwrap();

        // Proposer tries to vote
        assert_eq!(
            manager.vote(proposal_id, proposer, true, 5, 1000),
            Err(ConsensusError::CannotVoteOwnProposal)
        );
    }

    #[test]
    fn test_proposal_expiry() {
        let config = WeightConsensusConfig {
            proposal_timeout: 50,
            ..Default::default()
        };
        let manager = WeightConsensusManager::new(config);

        let proposer = test_address(1);
        let voter1 = test_address(2);

        let weights = vec![(1, 100)];

        // Propose at block 0 (expires at block 50)
        let proposal_id = manager
            .propose_weights(1, proposer, weights, 0, 5)
            .unwrap();

        // Vote after expiry (block 100 > 50)
        let result = manager.vote(proposal_id, voter1, true, 100, 1000);
        assert_eq!(result, Err(ConsensusError::ProposalExpired));
    }

    // ==================== Voting Pattern Tracker Tests ====================

    #[test]
    fn test_voting_tracker_records() {
        let mut tracker = VotingPatternTracker::new(100);
        let v1 = test_address(1);
        let p1 = [1u8; 32];

        tracker.record_vote(v1, p1, true, 10);
        tracker.record_vote(v1, [2u8; 32], false, 11);

        let stats = tracker.voter_stats();
        let s = stats.get(&v1).unwrap();
        assert_eq!(s.total_votes, 2);
        assert_eq!(s.approvals, 1);
        assert_eq!(s.rejections, 1);
    }

    #[test]
    fn test_voting_tracker_ring_buffer() {
        let mut tracker = VotingPatternTracker::new(3);
        let v1 = test_address(1);

        // Insert 5 records — only last 3 should remain
        for i in 0..5u8 {
            tracker.record_vote(v1, [i; 32], true, i as u64);
        }

        let stats = tracker.voter_stats();
        assert_eq!(stats[&v1].total_votes, 3);
    }

    #[test]
    fn test_detect_correlated_voters() {
        let mut tracker = VotingPatternTracker::new(100);
        let v1 = test_address(1);
        let v2 = test_address(2);
        let v3 = test_address(3);

        // v1 and v2 agree on 6/6 proposals — perfect correlation
        for i in 0..6u8 {
            let pid = [i + 10; 32];
            tracker.record_vote(v1, pid, true, i as u64);
            tracker.record_vote(v2, pid, true, i as u64);
        }

        // v3 disagrees with both on all 6
        for i in 0..6u8 {
            let pid = [i + 10; 32];
            tracker.record_vote(v3, pid, false, i as u64);
        }

        let correlated = tracker.detect_correlated_voters(0.9);

        // v1-v2 should be flagged (100% agreement)
        assert!(correlated.iter().any(|(a, b, rate, _)| {
            ((*a == v1 && *b == v2) || (*a == v2 && *b == v1)) && *rate >= 0.99
        }));

        // v1-v3 should NOT be flagged (0% agreement)
        assert!(!correlated.iter().any(|(a, b, _, _)| {
            (*a == v1 && *b == v3) || (*a == v3 && *b == v1)
        }));
    }

    #[test]
    fn test_detect_insufficient_common_proposals() {
        let mut tracker = VotingPatternTracker::new(100);
        let v1 = test_address(1);
        let v2 = test_address(2);

        // Only 3 common proposals — below MIN_COMMON_PROPOSALS (5)
        for i in 0..3u8 {
            let pid = [i + 10; 32];
            tracker.record_vote(v1, pid, true, i as u64);
            tracker.record_vote(v2, pid, true, i as u64);
        }

        let correlated = tracker.detect_correlated_voters(0.5);
        assert!(correlated.is_empty(), "Should not flag with <5 common proposals");
    }

    #[test]
    fn test_manager_voting_tracker_integration() {
        let config = WeightConsensusConfig::default();
        let manager = WeightConsensusManager::new(config);

        let proposer = test_address(1);
        let voter1 = test_address(2);
        let voter2 = test_address(3);

        // Create proposal and vote on it
        let pid = manager
            .propose_weights(1, proposer, vec![(1, 100)], 100, 5)
            .unwrap();

        manager.vote(pid, voter1, true, 101, 100).unwrap();
        manager.vote(pid, voter2, true, 101, 100).unwrap();

        // Should have voter stats for both voters
        let stats = manager.voter_statistics();
        assert!(stats.contains_key(&voter1));
        assert!(stats.contains_key(&voter2));
        assert_eq!(stats[&voter1].total_votes, 1);
        assert_eq!(stats[&voter2].total_votes, 1);

        // No suspicious patterns with just 1 proposal
        assert!(manager.detect_suspicious_patterns().is_empty());
    }
    #[test]
    fn test_finalize_approved() {
        let manager = WeightConsensusManager::new(WeightConsensusConfig {
            min_validators: 2,
            approval_threshold_percent: 50,
            ..Default::default()
        });

        let proposer = test_address(1);
        let voter = test_address(2);
        let weights = vec![(0, 500u16), (1, 500u16)];

        let proposal_id = manager.propose_weights(1, proposer, weights.clone(), 0, 3).unwrap();

        // Vote to approve (stake=1000)
        manager.vote(proposal_id, voter, true, 5, 1000).unwrap();

        // Finalize
        let final_weights = manager.finalize_proposal(proposal_id, 10).unwrap();
        assert_eq!(final_weights, weights);

        // Check status
        let proposal = manager.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Applied);
    }

    // ==================== V-Trust Scoring Tests ====================

    #[test]
    fn test_vtrust_perfect_alignment() {
        let mut scorer = VTrustScorer::new();
        let v1 = test_address(1);

        // Voter always votes with final outcome
        let votes = vec![
            ProposalVote { voter: v1, approve: true, block: 1, signature: None, stake: 100 },
        ];
        scorer.record_outcome(&votes, true); // aligned
        scorer.record_outcome(&votes, true); // aligned

        let score = scorer.trust_score(&v1).unwrap();
        assert!((score - 1.0).abs() < f64::EPSILON, "Perfect alignment should be 1.0");
    }

    #[test]
    fn test_vtrust_mixed_alignment() {
        let mut scorer = VTrustScorer::new();
        let v1 = test_address(1);

        // Voter approves, outcome is approved → aligned
        let approve_vote = vec![
            ProposalVote { voter: v1, approve: true, block: 1, signature: None, stake: 100 },
        ];
        scorer.record_outcome(&approve_vote, true);

        // Voter approves, outcome is rejected → NOT aligned
        scorer.record_outcome(&approve_vote, false);

        let score = scorer.trust_score(&v1).unwrap();
        assert!((score - 0.5).abs() < f64::EPSILON, "50% alignment expected");
    }

    #[test]
    fn test_vtrust_unknown_validator() {
        let scorer = VTrustScorer::new();
        let unknown = test_address(99);
        assert!(scorer.trust_score(&unknown).is_none());
    }

    #[test]
    fn test_vtrust_integration_with_manager() {
        let manager = WeightConsensusManager::new(WeightConsensusConfig {
            min_validators: 2,
            approval_threshold_percent: 50,
            ..Default::default()
        });

        let proposer = test_address(1);
        let voter = test_address(2);
        let weights = vec![(0, 500u16)];

        let pid = manager.propose_weights(1, proposer, weights, 0, 3).unwrap();
        manager.vote(pid, voter, true, 5, 1000).unwrap();

        // Record outcome — voter was aligned with final approval
        manager.record_proposal_outcome(pid, true);

        let score = manager.vtrust_score(&voter).unwrap();
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    // ==================== Committee Selection Tests ====================

    #[test]
    fn test_committee_selection_deterministic() {
        let validators: Vec<Address> = (1..=10).map(test_address).collect();
        let block_hash = [42u8; 32];

        let c1 = select_committee(&validators, 3, &block_hash, 1);
        let c2 = select_committee(&validators, 3, &block_hash, 1);

        assert_eq!(c1, c2, "Same inputs must produce same committee");
    }

    #[test]
    fn test_committee_selection_varies_by_block() {
        let validators: Vec<Address> = (1..=10).map(test_address).collect();

        let c1 = select_committee(&validators, 3, &[1u8; 32], 1);
        let c2 = select_committee(&validators, 3, &[2u8; 32], 1);

        // Different block hashes should (almost certainly) produce different committees
        assert_ne!(c1, c2, "Different block hashes should yield different committees");
    }

    #[test]
    fn test_committee_selection_varies_by_subnet() {
        let validators: Vec<Address> = (1..=10).map(test_address).collect();
        let block_hash = [42u8; 32];

        let c1 = select_committee(&validators, 3, &block_hash, 1);
        let c2 = select_committee(&validators, 3, &block_hash, 2);

        assert_ne!(c1, c2, "Different subnets should yield different committees");
    }

    #[test]
    fn test_committee_selection_no_duplicates() {
        let validators: Vec<Address> = (1..=20).map(test_address).collect();
        let block_hash = [99u8; 32];

        let committee = select_committee(&validators, 7, &block_hash, 5);
        assert_eq!(committee.len(), 7);

        // Check no duplicates
        let mut seen = std::collections::HashSet::new();
        for addr in &committee {
            assert!(seen.insert(addr), "Duplicate found in committee");
        }
    }

    #[test]
    fn test_committee_capped_at_validators() {
        let validators: Vec<Address> = (1..=3).map(test_address).collect();
        let committee = select_committee(&validators, 10, &[0u8; 32], 0);
        assert_eq!(committee.len(), 3, "Committee size should be capped at validator count");
    }

    #[test]
    fn test_committee_empty_validators() {
        let committee = select_committee(&[], 5, &[0u8; 32], 0);
        assert!(committee.is_empty());
    }

    // ==================== Fix 8: Collusion Penalty Tests ====================

    #[test]
    fn test_collusion_penalty_reduces_score() {
        let mut scorer = VTrustScorer::new();
        let validator = test_address(1);

        // Record 10 aligned votes out of 10 → score = 1.0
        let votes: Vec<ProposalVote> = (0..10)
            .map(|_| ProposalVote {
                voter: validator,
                approve: true,
                stake: 100,
                block: 0,
                signature: None,
            })
            .collect();
        scorer.record_outcome(&votes, true);
        assert_eq!(scorer.trust_score(&validator), Some(1.0));

        // Apply penalty → score should drop
        scorer.apply_collusion_penalty(&validator);
        let score_after = scorer.trust_score(&validator).unwrap();
        assert!(score_after < 1.0, "Penalty should reduce score, got {}", score_after);
        // Expected: 10 aligned / (10 + 10 penalty) = 0.5
        assert!((score_after - 0.5).abs() < 0.01, "Score should be ~0.5, got {}", score_after);
    }

    #[test]
    fn test_collusion_penalty_compounds() {
        let mut scorer = VTrustScorer::new();
        let validator = test_address(2);

        // Record 5 aligned votes out of 5 → score = 1.0
        let votes: Vec<ProposalVote> = (0..5)
            .map(|_| ProposalVote {
                voter: validator,
                approve: true,
                stake: 100,
                block: 0,
                signature: None,
            })
            .collect();
        scorer.record_outcome(&votes, true);

        // Apply two penalties → score drops more
        scorer.apply_collusion_penalty(&validator);
        let score1 = scorer.trust_score(&validator).unwrap();
        scorer.apply_collusion_penalty(&validator);
        let score2 = scorer.trust_score(&validator).unwrap();

        assert!(score2 < score1, "Second penalty should reduce score further");
        // After 2 penalties: 5 / (5 + 20) = 0.2
        assert!((score2 - 0.2).abs() < 0.01, "Score should be ~0.2, got {}", score2);
    }

    #[test]
    fn test_collusion_penalty_unknown_validator() {
        let mut scorer = VTrustScorer::new();
        let validator = test_address(99);

        // Penalty on unknown validator → creates entry with score 0
        scorer.apply_collusion_penalty(&validator);
        let score = scorer.trust_score(&validator).unwrap();
        assert_eq!(score, 0.0, "Unknown validator after penalty should have score 0");
    }
}

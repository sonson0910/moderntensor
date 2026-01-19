// Multi-Validator Weight Consensus
// Requires multiple validators to agree on weights before applying
//
// Flow:
// 1. Validator proposes weights
// 2. Other validators vote (approve/reject)
// 3. If threshold met (e.g., 2/3 majority), weights are applied
// 4. Proposer gets small reward for successful proposal

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
            min_validators: 2,
            approval_threshold_percent: 67, // 2/3 majority
            proposal_timeout: 200,           // ~40 minutes
            proposal_cooldown: 50,           // ~10 minutes
            slash_rejected_proposals: false,
            proposer_reward_bps: 10,         // 0.1% bonus
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

    /// Calculate approval percentage (0-100)
    pub fn approval_percentage(&self) -> u8 {
        if self.votes.is_empty() {
            return 0;
        }
        ((self.approval_count() * 100) / self.votes.len()) as u8
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

/// Weight Consensus Manager
pub struct WeightConsensusManager {
    config: WeightConsensusConfig,
    /// Active proposals by subnet
    proposals: RwLock<HashMap<u64, Vec<WeightProposal>>>,
    /// Last proposal time per validator
    last_proposal: RwLock<HashMap<Address, u64>>,
    /// Applied weights history
    applied_history: RwLock<Vec<(u64, Hash, Vec<(u64, u16)>)>>,
}

impl WeightConsensusManager {
    /// Create new manager
    pub fn new(config: WeightConsensusConfig) -> Self {
        Self {
            config,
            proposals: RwLock::new(HashMap::new()),
            last_proposal: RwLock::new(HashMap::new()),
            applied_history: RwLock::new(Vec::new()),
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
        // Check cooldown
        let last = self.last_proposal.read().get(&proposer).copied();
        if let Some(last_block) = last {
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
        self.proposals
            .write()
            .entry(subnet_uid)
            .or_insert_with(Vec::new)
            .push(proposal);

        // Update last proposal time
        self.last_proposal.write().insert(proposer, current_block);

        info!(
            "New weight proposal {:?} for subnet {} by {:?}",
            proposal_id, subnet_uid, proposer
        );

        Ok(proposal_id)
    }

    /// Vote on a proposal
    pub fn vote(
        &self,
        proposal_id: Hash,
        voter: Address,
        approve: bool,
        current_block: u64,
    ) -> Result<ConsensusResult, ConsensusError> {
        let mut proposals = self.proposals.write();

        // Find proposal
        let proposal = proposals
            .values_mut()
            .flat_map(|v| v.iter_mut())
            .find(|p| p.id == proposal_id)
            .ok_or(ConsensusError::ProposalNotFound)?;

        // Check not expired
        if proposal.is_expired(current_block) {
            proposal.status = ProposalStatus::Expired;
            return Err(ConsensusError::ProposalExpired);
        }

        // Check status
        if proposal.status != ProposalStatus::Pending {
            return Err(ConsensusError::ProposalNotPending);
        }

        // Check not already voted
        if proposal.has_voted(&voter) {
            return Err(ConsensusError::AlreadyVoted);
        }

        // Check voter is not proposer (can't vote on own proposal)
        if proposal.proposer == voter {
            return Err(ConsensusError::CannotVoteOwnProposal);
        }

        // Add vote
        proposal.votes.push(ProposalVote {
            voter,
            approve,
            block: current_block,
            signature: None,
        });

        // Check consensus
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
        let approval_percentage = proposal.approval_percentage();

        // Need at least min_validators - 1 votes (excluding proposer)
        let min_votes = self.config.min_validators.saturating_sub(1);
        let enough_votes = total_votes >= min_votes;

        // Check threshold
        let reached = enough_votes
            && approval_percentage >= self.config.approval_threshold_percent;

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
        current_block: u64,
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
        self.applied_history.write().push((
            proposal.subnet_uid,
            proposal_id,
            proposal.weights.clone(),
        ));

        info!(
            "Proposal {:?} finalized, applying {} weights",
            proposal_id,
            proposal.weights.len()
        );

        Ok(proposal.weights.clone())
    }

    /// Get active proposals for subnet
    pub fn get_proposals(&self, subnet_uid: u64) -> Vec<WeightProposal> {
        self.proposals
            .read()
            .get(&subnet_uid)
            .cloned()
            .unwrap_or_default()
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
        self.proposals
            .read()
            .values()
            .flat_map(|v| v.iter())
            .find(|p| p.id == proposal_id)
            .cloned()
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
        let proposal_id = manager
            .propose_weights(1, proposer, weights, 0, 3)
            .unwrap();

        // Vote
        let result = manager.vote(proposal_id, voter, true, 5).unwrap();

        // Should reach consensus (1 vote, 100% approval)
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
        manager
            .propose_weights(1, proposer, weights.clone(), 0, 3)
            .unwrap();

        // Second proposal too soon
        assert_eq!(
            manager.propose_weights(1, proposer, weights.clone(), 10, 3),
            Err(ConsensusError::ProposalCooldown)
        );

        // After cooldown
        assert!(manager.propose_weights(1, proposer, weights, 60, 3).is_ok());
    }

    #[test]
    fn test_cannot_vote_own_proposal() {
        let manager = WeightConsensusManager::new(Default::default());

        let proposer = test_address(1);
        let weights = vec![(0, 500u16)];

        let proposal_id = manager
            .propose_weights(1, proposer, weights, 0, 3)
            .unwrap();

        // Proposer tries to vote
        assert_eq!(
            manager.vote(proposal_id, proposer, true, 5),
            Err(ConsensusError::CannotVoteOwnProposal)
        );
    }

    #[test]
    fn test_proposal_expiry() {
        let manager = WeightConsensusManager::new(WeightConsensusConfig {
            proposal_timeout: 50,
            ..Default::default()
        });

        let proposer = test_address(1);
        let voter = test_address(2);
        let weights = vec![(0, 500u16)];

        let proposal_id = manager
            .propose_weights(1, proposer, weights, 0, 3)
            .unwrap();

        // Vote after expiry
        assert_eq!(
            manager.vote(proposal_id, voter, true, 100),
            Err(ConsensusError::ProposalExpired)
        );
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

        let proposal_id = manager
            .propose_weights(1, proposer, weights.clone(), 0, 3)
            .unwrap();

        // Vote to approve
        manager.vote(proposal_id, voter, true, 5).unwrap();

        // Finalize
        let final_weights = manager.finalize_proposal(proposal_id, 10).unwrap();
        assert_eq!(final_weights, weights);

        // Check status
        let proposal = manager.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Applied);
    }
}

// LuxTensor On-Chain Governance Module
//
// Provides decentralised governance for protocol parameter changes, emission
// adjustments, slashing updates, and protocol upgrades.  Follows the proven
// propose → vote → timelock → execute lifecycle used by `multisig.rs` and
// `weight_consensus.rs`.

use crate::validator::ValidatorSet;
use luxtensor_core::types::{Address, Hash};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Error ───────────────────────────────────────────────────────────

/// Governance-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum GovernanceError {
    #[error("proposal {0} not found")]
    ProposalNotFound(u64),

    #[error("proposer {0:?} does not meet the minimum stake requirement")]
    InsufficientStake(Address),

    #[error("voter {0:?} already voted on proposal {1}")]
    AlreadyVoted(Address, u64),

    #[error("voter {0:?} is not an active validator")]
    NotAValidator(Address),

    #[error("proposal {0} is not in voting phase (status: {1:?})")]
    NotInVotingPhase(u64, ProposalStatus),

    #[error("proposal {0} voting period has not ended yet")]
    VotingNotEnded(u64),

    #[error("proposal {0} did not reach quorum ({1}% < {2}%)")]
    QuorumNotReached(u64, u64, u64),

    #[error("proposal {0} was rejected ({1} against >= {2} for)")]
    Rejected(u64, u128, u128),

    #[error("proposal {0} is still in timelock (until block {1})")]
    TimelockActive(u64, u64),

    #[error("proposal {0} has expired")]
    Expired(u64),

    #[error("proposal {0} already executed")]
    AlreadyExecuted(u64),

    #[error("only the proposer or a supervalidator can cancel")]
    UnauthorizedCancel,

    #[error("too many active proposals ({max} max)")]
    TooManyActiveProposals { max: usize },
}

pub type Result<T> = std::result::Result<T, GovernanceError>;

// ─── Types ───────────────────────────────────────────────────────────

/// The kind of change a proposal represents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalType {
    /// Change a protocol parameter (key → new_value).
    ParameterChange { key: String, value: String },
    /// Adjust emission curve parameters.
    EmissionAdjustment { new_rate_bps: u64 },
    /// Update slashing penalties.
    SlashingUpdate { offence: String, new_penalty_bps: u64 },
    /// Signal a protocol upgrade (version string + activation height).
    ProtocolUpgrade { version: String, activation_height: u64 },
    /// Emergency proposal – shorter timelock (24 h vs 48 h).
    Emergency { description: String },
}

/// Lifecycle status of a proposal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalStatus {
    /// Open for voting.
    Active,
    /// Voting finished, approved, waiting for timelock.
    Approved,
    /// Voting finished, rejected.
    Rejected,
    /// Timelock elapsed, ready to execute.
    ReadyToExecute,
    /// Executed on-chain.
    Executed,
    /// Cancelled by proposer / supervalidator.
    Cancelled,
    /// Expired (voting period ended with no quorum).
    Expired,
}

/// A single vote cast on a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter: Address,
    /// Stake-weighted voting power at the time of voting.
    pub power: u128,
    pub approve: bool,
    pub cast_at_block: u64,
}

/// A governance proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub status: ProposalStatus,
    /// Block height at which the proposal was created.
    pub created_at: u64,
    /// Block height after which voting closes.
    pub voting_deadline: u64,
    /// Block height after which the proposal can be executed (post-timelock).
    pub execute_after: u64,
    /// Absolute deadline — proposal expires if not executed by this height.
    pub expires_at: u64,
    pub votes_for: u128,
    pub votes_against: u128,
    pub total_eligible_power: u128,
    pub votes: Vec<Vote>,
    /// Optional execution tx hash (set after execution).
    pub execution_hash: Option<Hash>,
}

// ─── Config ──────────────────────────────────────────────────────────

/// Tuneable governance parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    /// Minimum stake (in base units) required to submit a proposal.
    pub min_proposal_stake: u128,
    /// Voting period length in blocks.  Default: 50_400 (~7 days @ 12 s).
    pub voting_period_blocks: u64,
    /// Normal timelock in blocks.  Default: 14_400 (~48 h).
    pub timelock_blocks: u64,
    /// Emergency timelock in blocks.  Default: 7_200 (~24 h).
    pub emergency_timelock_blocks: u64,
    /// Quorum as basis-points of total eligible power (3300 = 33%).
    pub quorum_bps: u64,
    /// Supermajority threshold for approval in basis-points (6667 = 66.67%).
    pub approval_threshold_bps: u64,
    /// Maximum age (in blocks) before an un-executed approved proposal expires.
    pub max_proposal_age_blocks: u64,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            min_proposal_stake: 100_000_000_000_000_000, // 0.1 MDT
            voting_period_blocks: 50_400,                // ~7 days
            timelock_blocks: 14_400,                     // ~48 h
            emergency_timelock_blocks: 7_200,            // ~24 h
            quorum_bps: 3_300,                           // 33 %
            approval_threshold_bps: 6_667,               // 66.67 %
            max_proposal_age_blocks: 201_600,            // ~28 days
        }
    }
}

// ─── Module ──────────────────────────────────────────────────────────

/// On-chain governance manager.
///
/// Thread-safe via `RwLock`; all time-sensitive logic uses *block height*
/// (not wall-clock) for determinism.
pub struct GovernanceModule {
    config: GovernanceConfig,
    proposals: RwLock<HashMap<u64, Proposal>>,
    /// Tracks which (voter, proposal_id) pairs have already been cast.
    voted: RwLock<HashMap<(Address, u64), bool>>,
    next_id: RwLock<u64>,
}

impl GovernanceModule {
    pub fn new(config: GovernanceConfig) -> Self {
        Self {
            config,
            proposals: RwLock::new(HashMap::new()),
            voted: RwLock::new(HashMap::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Create a governance proposal.
    ///
    /// * `proposer_stake` – the proposer's current staked balance (for
    ///   eligibility check).
    /// * `total_eligible_power` – total staked supply eligible to vote.
    /// * `current_block` – the chain height at proposal time.
    pub fn create_proposal(
        &self,
        proposer: Address,
        proposer_stake: u128,
        title: String,
        description: String,
        proposal_type: ProposalType,
        total_eligible_power: u128,
        current_block: u64,
    ) -> Result<u64> {
        if proposer_stake < self.config.min_proposal_stake {
            return Err(GovernanceError::InsufficientStake(proposer));
        }

        // SECURITY: Limit active proposals to prevent governance spam.
        // An attacker could create thousands of proposals to exhaust memory
        // or make governance unusable by diluting voter attention.
        const MAX_ACTIVE_PROPOSALS: usize = 100;
        let active_count = self.active_proposal_count();
        if active_count >= MAX_ACTIVE_PROPOSALS {
            return Err(GovernanceError::TooManyActiveProposals { max: MAX_ACTIVE_PROPOSALS });
        }

        let mut next = self.next_id.write();
        let id = *next;
        *next += 1;

        let timelock = match &proposal_type {
            ProposalType::Emergency { .. } => self.config.emergency_timelock_blocks,
            _ => self.config.timelock_blocks,
        };

        let voting_deadline = current_block + self.config.voting_period_blocks;
        let execute_after = voting_deadline + timelock;
        let expires_at = current_block + self.config.max_proposal_age_blocks;

        let proposal = Proposal {
            id,
            proposer,
            title,
            description,
            proposal_type,
            status: ProposalStatus::Active,
            created_at: current_block,
            voting_deadline,
            execute_after,
            expires_at,
            votes_for: 0,
            votes_against: 0,
            total_eligible_power,
            votes: Vec::new(),
            execution_hash: None,
        };

        self.proposals.write().insert(id, proposal);
        Ok(id)
    }

    /// Cast a vote on a proposal.
    ///
    /// The voter's stake is looked up from the provided `ValidatorSet` to
    /// prevent callers from passing inflated voting power.
    ///
    /// # Security
    /// This method never trusts caller-supplied power values. The stake is
    /// read directly from the authoritative `ValidatorSet`. Voters with
    /// zero stake (non-validators) are rejected.
    pub fn vote(
        &self,
        proposal_id: u64,
        voter: Address,
        validator_set: &ValidatorSet,
        approve: bool,
        current_block: u64,
    ) -> Result<()> {
        // SECURITY: Look up actual stake from ValidatorSet — never trust
        // caller-supplied voting power (H-2 governance takeover fix).
        let voting_power = validator_set
            .get_validator(&voter)
            .map(|v| v.stake)
            .unwrap_or(0);

        if voting_power == 0 {
            return Err(GovernanceError::NotAValidator(voter));
        }

        // H-6 FIX: Acquire voted as a *write* guard up-front and hold it
        // for the entire method.  This eliminates the TOCTOU window where
        // two concurrent vote() calls from the same voter could both pass
        // the duplicate check.
        let mut voted = self.voted.write();

        // Check duplicate using the write guard
        if voted.contains_key(&(voter, proposal_id)) {
            return Err(GovernanceError::AlreadyVoted(voter, proposal_id));
        }

        let mut proposals = self.proposals.write();
        let proposal = proposals
            .get_mut(&proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(proposal_id))?;

        if proposal.status != ProposalStatus::Active {
            return Err(GovernanceError::NotInVotingPhase(proposal_id, proposal.status));
        }
        if current_block > proposal.voting_deadline {
            return Err(GovernanceError::Expired(proposal_id));
        }

        // Record vote
        if approve {
            proposal.votes_for = proposal.votes_for.saturating_add(voting_power);
        } else {
            proposal.votes_against = proposal.votes_against.saturating_add(voting_power);
        }

        proposal.votes.push(Vote {
            voter,
            power: voting_power,
            approve,
            cast_at_block: current_block,
        });

        // Insert into voted using the same write guard — no TOCTOU gap
        voted.insert((voter, proposal_id), true);
        Ok(())
    }

    /// Finalise voting for a proposal once the deadline has passed.
    ///
    /// Transitions status to `Approved`, `Rejected`, or `Expired`.
    pub fn finalize_voting(&self, proposal_id: u64, current_block: u64) -> Result<ProposalStatus> {
        let mut proposals = self.proposals.write();
        let proposal = proposals
            .get_mut(&proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(proposal_id))?;

        if proposal.status != ProposalStatus::Active {
            return Err(GovernanceError::NotInVotingPhase(proposal_id, proposal.status));
        }
        if current_block <= proposal.voting_deadline {
            return Err(GovernanceError::VotingNotEnded(proposal_id));
        }

        let total_votes = proposal.votes_for.saturating_add(proposal.votes_against);
        let quorum_required = proposal.total_eligible_power
            .checked_mul(self.config.quorum_bps as u128)
            .unwrap_or(u128::MAX) / 10_000;
        let approval_required = total_votes
            .checked_mul(self.config.approval_threshold_bps as u128)
            .unwrap_or(u128::MAX) / 10_000;

        if total_votes < quorum_required {
            proposal.status = ProposalStatus::Expired;
            return Ok(ProposalStatus::Expired);
        }

        if proposal.votes_for >= approval_required {
            proposal.status = ProposalStatus::Approved;
            Ok(ProposalStatus::Approved)
        } else {
            proposal.status = ProposalStatus::Rejected;
            Ok(ProposalStatus::Rejected)
        }
    }

    /// Execute an approved proposal after its timelock has elapsed.
    ///
    /// Returns the `Proposal` for the caller to apply the change externally.
    pub fn execute(
        &self,
        proposal_id: u64,
        current_block: u64,
        execution_hash: Hash,
    ) -> Result<Proposal> {
        let mut proposals = self.proposals.write();
        let proposal = proposals
            .get_mut(&proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(proposal_id))?;

        match proposal.status {
            ProposalStatus::Approved => {}
            ProposalStatus::ReadyToExecute => {}
            ProposalStatus::Executed => return Err(GovernanceError::AlreadyExecuted(proposal_id)),
            other => return Err(GovernanceError::NotInVotingPhase(proposal_id, other)),
        }

        if current_block < proposal.execute_after {
            return Err(GovernanceError::TimelockActive(proposal_id, proposal.execute_after));
        }
        if current_block > proposal.expires_at {
            proposal.status = ProposalStatus::Expired;
            return Err(GovernanceError::Expired(proposal_id));
        }

        proposal.status = ProposalStatus::Executed;
        proposal.execution_hash = Some(execution_hash);
        Ok(proposal.clone())
    }

    /// Cancel a proposal (only proposer or super-validator).
    pub fn cancel(&self, proposal_id: u64, caller: Address, is_supervalidator: bool) -> Result<()> {
        let mut proposals = self.proposals.write();
        let proposal = proposals
            .get_mut(&proposal_id)
            .ok_or(GovernanceError::ProposalNotFound(proposal_id))?;

        if proposal.status == ProposalStatus::Executed {
            return Err(GovernanceError::AlreadyExecuted(proposal_id));
        }

        if caller != proposal.proposer && !is_supervalidator {
            return Err(GovernanceError::UnauthorizedCancel);
        }

        proposal.status = ProposalStatus::Cancelled;
        Ok(())
    }

    /// Retrieve a proposal by ID.
    pub fn get_proposal(&self, id: u64) -> Option<Proposal> {
        self.proposals.read().get(&id).cloned()
    }

    /// List proposals filtered by status.
    pub fn list_proposals(&self, status: Option<ProposalStatus>) -> Vec<Proposal> {
        let proposals = self.proposals.read();
        proposals.values().filter(|p| status.map_or(true, |s| p.status == s)).cloned().collect()
    }

    /// Housekeeping: transition any proposals that have passed their absolute
    /// expiry block to `Expired`.
    pub fn expire_stale(&self, current_block: u64) -> Vec<u64> {
        let mut proposals = self.proposals.write();
        let mut expired = Vec::new();
        for (id, p) in proposals.iter_mut() {
            if current_block > p.expires_at
                && p.status != ProposalStatus::Executed
                && p.status != ProposalStatus::Cancelled
                && p.status != ProposalStatus::Expired
            {
                p.status = ProposalStatus::Expired;
                expired.push(*id);
            }
        }
        expired
    }

    /// Return the current configuration (useful for RPC inspection).
    pub fn config(&self) -> &GovernanceConfig {
        &self.config
    }

    /// Remove proposals in terminal states (Executed, Cancelled, Expired) older
    /// than `retain_blocks` to prevent unbounded memory growth.
    ///
    /// Call periodically (e.g., once per epoch) to garbage-collect.
    pub fn cleanup_finalized(&self, current_block: u64, retain_blocks: u64) -> usize {
        let cutoff = current_block.saturating_sub(retain_blocks);
        let mut proposals = self.proposals.write();
        let before = proposals.len();
        proposals.retain(|_, p| {
            // Keep if not in terminal state, or if created recently
            !matches!(
                p.status,
                ProposalStatus::Executed | ProposalStatus::Cancelled | ProposalStatus::Expired
            ) || p.created_at > cutoff
        });
        let removed = before - proposals.len();

        // Also clean stale voted entries
        if removed > 0 {
            let remaining_ids: std::collections::HashSet<u64> = proposals.keys().copied().collect();
            self.voted.write().retain(|(_, pid), _| remaining_ids.contains(pid));
        }

        removed
    }

    /// Get the count of currently active proposals.
    pub fn active_proposal_count(&self) -> usize {
        self.proposals.read().values().filter(|p| p.status == ProposalStatus::Active).count()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::Validator;

    fn addr(b: u8) -> Address {
        let mut a = [0u8; 20];
        a[0] = b;
        Address::from(a)
    }

    fn hash(b: u8) -> Hash {
        let mut h = [0u8; 32];
        h[0] = b;
        h
    }

    fn module() -> GovernanceModule {
        GovernanceModule::new(GovernanceConfig {
            voting_period_blocks: 100,
            timelock_blocks: 50,
            emergency_timelock_blocks: 25,
            quorum_bps: 3_300,
            approval_threshold_bps: 5_000, // simple majority for tests
            max_proposal_age_blocks: 500,
            min_proposal_stake: 1_000,
        })
    }

    /// Build a ValidatorSet with the given (address_byte, stake) pairs.
    fn make_validators(entries: &[(u8, u128)]) -> ValidatorSet {
        let mut vs = ValidatorSet::new();
        for &(b, stake) in entries {
            vs.add_validator(Validator::new(addr(b), stake, [b; 32])).unwrap();
        }
        vs
    }

    #[test]
    fn test_full_lifecycle() {
        let gov = module();
        let proposer = addr(1);

        // Create proposal
        let id = gov
            .create_proposal(
                proposer,
                10_000,
                "Increase gas limit".into(),
                "Set gas_limit to 30M".into(),
                ProposalType::ParameterChange { key: "gas_limit".into(), value: "30000000".into() },
                100_000,
                10,
            )
            .unwrap();

        assert_eq!(id, 1);
        let p = gov.get_proposal(id).unwrap();
        assert_eq!(p.status, ProposalStatus::Active);

        // Vote with >33% quorum and >50% approval
        let vs = make_validators(&[(2, 40_000), (3, 10_000)]);
        gov.vote(id, addr(2), &vs, true, 50).unwrap();
        gov.vote(id, addr(3), &vs, false, 60).unwrap();

        // Cannot vote twice
        assert!(gov.vote(id, addr(2), &vs, true, 70).is_err());

        // Finalize after deadline (block 110 > voting_deadline 110)
        let status = gov.finalize_voting(id, 111).unwrap();
        assert_eq!(status, ProposalStatus::Approved);

        // Cannot execute during timelock
        assert!(gov.execute(id, 150, hash(1)).is_err()); // 150 < 160

        // Execute after timelock
        let executed = gov.execute(id, 161, hash(1)).unwrap();
        assert_eq!(executed.status, ProposalStatus::Executed);
        assert_eq!(executed.execution_hash, Some(hash(1)));
    }

    #[test]
    fn test_insufficient_stake_rejected() {
        let gov = module();
        let result = gov.create_proposal(
            addr(1),
            500, // below min_proposal_stake
            "Bad".into(),
            "".into(),
            ProposalType::Emergency { description: "test".into() },
            100_000,
            1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_quorum_not_reached() {
        let gov = module();
        let id = gov
            .create_proposal(
                addr(1),
                10_000,
                "Low participation".into(),
                "".into(),
                ProposalType::ParameterChange { key: "x".into(), value: "y".into() },
                100_000,
                10,
            )
            .unwrap();

        // Only 5% of eligible power votes
        let vs = make_validators(&[(2, 5_000)]);
        gov.vote(id, addr(2), &vs, true, 50).unwrap();

        let status = gov.finalize_voting(id, 111).unwrap();
        assert_eq!(status, ProposalStatus::Expired);
    }

    #[test]
    fn test_rejection() {
        let gov = module();
        let id = gov
            .create_proposal(
                addr(1),
                10_000,
                "Bad idea".into(),
                "".into(),
                ProposalType::SlashingUpdate {
                    offence: "double_sign".into(),
                    new_penalty_bps: 10_000,
                },
                100_000,
                10,
            )
            .unwrap();

        let vs = make_validators(&[(2, 20_000), (3, 15_000)]);
        gov.vote(id, addr(2), &vs, false, 50).unwrap();
        gov.vote(id, addr(3), &vs, true, 60).unwrap();

        let status = gov.finalize_voting(id, 111).unwrap();
        assert_eq!(status, ProposalStatus::Rejected);
    }

    #[test]
    fn test_cancel() {
        let gov = module();
        let proposer = addr(1);
        let id = gov
            .create_proposal(
                proposer,
                10_000,
                "Will cancel".into(),
                "".into(),
                ProposalType::ParameterChange { key: "x".into(), value: "y".into() },
                100_000,
                10,
            )
            .unwrap();

        // Non-proposer cannot cancel
        assert!(gov.cancel(id, addr(99), false).is_err());

        // Proposer can cancel
        gov.cancel(id, proposer, false).unwrap();
        assert_eq!(gov.get_proposal(id).unwrap().status, ProposalStatus::Cancelled);
    }

    #[test]
    fn test_emergency_shorter_timelock() {
        let gov = module();
        let id = gov
            .create_proposal(
                addr(1),
                10_000,
                "Emergency halt".into(),
                "".into(),
                ProposalType::Emergency { description: "Critical bug".into() },
                100_000,
                10,
            )
            .unwrap();

        let p = gov.get_proposal(id).unwrap();
        // emergency timelock = 25, voting period = 100
        // execute_after = 10 + 100 + 25 = 135
        assert_eq!(p.execute_after, 135);
    }

    #[test]
    fn test_expire_stale() {
        let gov = module();
        let id = gov
            .create_proposal(
                addr(1),
                10_000,
                "Old".into(),
                "".into(),
                ProposalType::ParameterChange { key: "x".into(), value: "y".into() },
                100_000,
                10,
            )
            .unwrap();

        // max_proposal_age_blocks = 500 → expires_at = 510
        let expired = gov.expire_stale(511);
        assert_eq!(expired, vec![id]);
        assert_eq!(gov.get_proposal(id).unwrap().status, ProposalStatus::Expired);
    }

    #[test]
    fn test_list_proposals_by_status() {
        let gov = module();
        gov.create_proposal(
            addr(1),
            10_000,
            "A".into(),
            "".into(),
            ProposalType::ParameterChange { key: "a".into(), value: "b".into() },
            100_000,
            10,
        )
        .unwrap();
        gov.create_proposal(
            addr(1),
            10_000,
            "B".into(),
            "".into(),
            ProposalType::ParameterChange { key: "c".into(), value: "d".into() },
            100_000,
            10,
        )
        .unwrap();

        let active = gov.list_proposals(Some(ProposalStatus::Active));
        assert_eq!(active.len(), 2);

        let executed = gov.list_proposals(Some(ProposalStatus::Executed));
        assert_eq!(executed.len(), 0);
    }
}

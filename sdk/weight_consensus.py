"""
ModernTensor SDK - Multi-Validator Weight Consensus

Implements consensus mechanism requiring multiple validators to agree on weights.

Flow:
1. Validator proposes weights
2. Other validators vote (approve/reject)
3. If threshold met (e.g., 2/3 majority), weights are applied
4. Proposer gets small reward for successful proposal

Usage:
    from sdk.weight_consensus import WeightConsensusClient, ProposalStatus

    client = WeightConsensusClient(rpc_url="http://localhost:8545")

    # Propose weights
    proposal_id = client.propose_weights(
        subnet_uid=1,
        weights=[(0, 500), (1, 300), (2, 200)],
        signer=signer
    )

    # Other validators vote
    client.vote_on_proposal(proposal_id, approve=True, signer=other_signer)

    # Check status
    status = client.get_proposal_status(proposal_id)
    if status.reached_consensus:
        client.finalize_proposal(proposal_id, signer)

Author: ModernTensor Team
Version: 1.0.0
"""

import hashlib
import logging
from typing import List, Tuple, Optional, Dict, Any, TYPE_CHECKING
from dataclasses import dataclass, field
from enum import Enum

if TYPE_CHECKING:
    from sdk.keymanager.transaction_signer import TransactionSigner

logger = logging.getLogger(__name__)


# =============================================================================
# Data Types
# =============================================================================

class ProposalStatus(str, Enum):
    """Status of a weight proposal"""
    PENDING = "pending"
    APPROVED = "approved"
    REJECTED = "rejected"
    APPLIED = "applied"
    EXPIRED = "expired"


@dataclass
class WeightConsensusConfig:
    """Configuration for consensus mechanism

    Updated 2026-01-29: Added security-related fields from luxtensor.
    """
    min_validators: int = 2
    approval_threshold_percent: int = 67  # 2/3 majority
    proposal_timeout: int = 200  # blocks
    proposal_cooldown: int = 50  # blocks between proposals
    # Security additions (luxtensor compatibility 2026-01-29)
    ai_scoring_timeout_blocks: int = 100  # ~20 minutes before fallback
    use_fallback_weights_on_timeout: bool = True
    commit_phase_blocks: int = 50  # ~10 minutes to commit
    reveal_phase_blocks: int = 50  # ~10 minutes to reveal
    commit_deadline_jitter_blocks: int = 5  # Anti-timing attack


@dataclass
class ProposalVote:
    """A vote on a proposal

    Updated 2026-01-29: Added stake_weight for Sybil-resistant voting.
    """
    voter: str
    approve: bool
    block: int
    stake_weight: int = 0  # Voter's stake for weighted consensus


@dataclass
class WeightProposal:
    """A weight proposal from a validator"""
    id: str
    proposer: str
    subnet_uid: int
    weights: List[Tuple[int, int]]
    weights_hash: str
    proposed_at: int
    expires_at: int
    status: ProposalStatus
    votes: List[ProposalVote] = field(default_factory=list)
    eligible_voters: int = 0

    def approval_count(self) -> int:
        """Count approval votes (deprecated: use approval_stake_weight)"""
        return sum(1 for v in self.votes if v.approve)

    def rejection_count(self) -> int:
        """Count rejection votes (deprecated: use rejection_stake_weight)"""
        return sum(1 for v in self.votes if not v.approve)

    def approval_stake_weight(self) -> int:
        """Sum of stake weights from approval votes (Sybil-resistant)"""
        return sum(v.stake_weight for v in self.votes if v.approve)

    def rejection_stake_weight(self) -> int:
        """Sum of stake weights from rejection votes"""
        return sum(v.stake_weight for v in self.votes if not v.approve)

    def total_stake_weight(self) -> int:
        """Total stake weight of all votes cast"""
        return sum(v.stake_weight for v in self.votes)

    def approval_percentage(self) -> int:
        """Calculate approval percentage by stake weight (0-100)"""
        total = self.total_stake_weight()
        if total == 0:
            return 0
        return (self.approval_stake_weight() * 100) // total

    def has_voted(self, voter: str) -> bool:
        return any(v.voter.lower() == voter.lower() for v in self.votes)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "proposer": self.proposer,
            "subnet_uid": self.subnet_uid,
            "weights_count": len(self.weights),
            "weights_hash": self.weights_hash,
            "proposed_at": self.proposed_at,
            "expires_at": self.expires_at,
            "status": self.status.value,
            "approval_count": self.approval_count(),
            "rejection_count": self.rejection_count(),
            "approval_stake_weight": self.approval_stake_weight(),
            "rejection_stake_weight": self.rejection_stake_weight(),
            "total_stake_weight": self.total_stake_weight(),
            "total_votes": len(self.votes),
            "approval_percentage": self.approval_percentage(),
        }


@dataclass
class ConsensusResult:
    """Result of consensus check

    Updated 2026-01-29: Added stake-weighted fields for Sybil resistance.
    """
    reached: bool
    approval_stake_weight: int = 0  # Sum of stake from approvals
    rejection_stake_weight: int = 0  # Sum of stake from rejections
    total_stake_weight: int = 0
    # Keep for backwards compatibility
    approval_count: int = 0
    rejection_count: int = 0
    total_votes: int = 0
    approval_percentage: int = 0
    threshold: int = 67

    def to_dict(self) -> Dict[str, Any]:
        return {
            "reached": self.reached,
            "approval_stake_weight": self.approval_stake_weight,
            "rejection_stake_weight": self.rejection_stake_weight,
            "total_stake_weight": self.total_stake_weight,
            "approval_count": self.approval_count,
            "rejection_count": self.rejection_count,
            "total_votes": self.total_votes,
            "approval_percentage": self.approval_percentage,
            "threshold": self.threshold,
        }


# =============================================================================
# Hash Functions
# =============================================================================

def compute_weights_hash(weights: List[Tuple[int, int]]) -> str:
    """Compute hash of weights for verification"""
    try:
        from Crypto.Hash import keccak
        hasher = keccak.new(digest_bits=256)
    except ImportError:
        hasher = hashlib.sha3_256()

    for uid, weight in sorted(weights, key=lambda x: x[0]):
        hasher.update(uid.to_bytes(8, 'big'))
        hasher.update(weight.to_bytes(2, 'big'))

    return "0x" + hasher.hexdigest()


def compute_proposal_id(
    proposer: str,
    subnet_uid: int,
    block: int,
    weights_hash: str,
) -> str:
    """Compute unique proposal ID"""
    try:
        from Crypto.Hash import keccak
        hasher = keccak.new(digest_bits=256)
    except ImportError:
        hasher = hashlib.sha3_256()

    proposer_bytes = bytes.fromhex(proposer.replace('0x', ''))
    hasher.update(proposer_bytes)
    hasher.update(subnet_uid.to_bytes(8, 'big'))
    hasher.update(block.to_bytes(8, 'big'))
    hasher.update(bytes.fromhex(weights_hash.replace('0x', '')))

    return "0x" + hasher.hexdigest()


# =============================================================================
# Client
# =============================================================================

class WeightConsensusClient:
    """
    Client for multi-validator weight consensus.

    Syncs with on-chain WeightConsensusManager in LuxTensor.

    Example:
        from sdk.weight_consensus import WeightConsensusClient

        client = WeightConsensusClient()

        # Propose weights
        weights = [(0, 500), (1, 300), (2, 200)]
        proposal_id = client.propose_weights(1, weights, signer)

        # Other validator votes
        client.vote_on_proposal(proposal_id, True, other_signer)

        # Check and finalize
        result = client.check_consensus(proposal_id)
        if result.reached:
            client.finalize_proposal(proposal_id, signer)
    """

    def __init__(self, rpc_url: str = "http://localhost:8545"):
        """Initialize client"""
        from sdk.luxtensor_client import LuxtensorClient

        self.rpc_url = rpc_url
        self.client = LuxtensorClient(url=rpc_url)
        self._config = None

    def get_config(self) -> WeightConsensusConfig:
        """Get consensus configuration from chain.

        Note: weightConsensus_getConfig RPC is not yet implemented on the
        LuxTensor server. Returns default configuration.
        """
        if self._config:
            return self._config

        logger.warning(
            "weightConsensus_getConfig is not yet available on the server. "
            "Using default WeightConsensusConfig values."
        )
        self._config = WeightConsensusConfig()
        return self._config

    def propose_weights(
        self,
        subnet_uid: int,
        weights: List[Tuple[int, int]],
        signer: "TransactionSigner",
    ) -> str:
        """
        Create a new weight proposal.

        Args:
            subnet_uid: Subnet ID
            weights: List of (uid, weight) tuples
            signer: Transaction signer with proposer's private key

        Returns:
            Proposal ID (hex string)
        """
        from sdk.luxtensor_pallets import encode_propose_weights

        proposer_address = signer.get_address()

        # Encode transaction
        encoded = encode_propose_weights(subnet_uid, weights)

        # Get nonce
        nonce = self.client.get_nonce(proposer_address)

        # Build and sign
        tx = signer.build_and_sign_transaction(
            to=encoded.contract_address or proposer_address,
            value=0,
            nonce=nonce,
            gas_price=1_000_000_000,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=self.client.chain_id,
        )

        # Submit
        result = self.client.submit_transaction(tx)

        # Get proposal ID from event/receipt
        proposal_id = self._extract_proposal_id(result)

        logger.info(f"Created proposal {proposal_id} for subnet {subnet_uid}")

        return proposal_id

    def vote_on_proposal(
        self,
        proposal_id: str,
        approve: bool,
        signer: "TransactionSigner",
    ) -> ConsensusResult:
        """
        Vote on a weight proposal.

        Args:
            proposal_id: Proposal ID to vote on
            approve: True to approve, False to reject
            signer: Transaction signer with voter's private key

        Returns:
            ConsensusResult after vote
        """
        from sdk.luxtensor_pallets import encode_vote_proposal

        voter_address = signer.get_address()

        # Get proposal to verify
        proposal = self.get_proposal(proposal_id)
        if not proposal:
            raise ValueError(f"Proposal {proposal_id} not found")

        if proposal.status != ProposalStatus.PENDING:
            raise ValueError(f"Proposal is not pending: {proposal.status}")

        if proposal.has_voted(voter_address):
            raise ValueError("Already voted on this proposal")

        if proposal.proposer.lower() == voter_address.lower():
            raise ValueError("Cannot vote on own proposal")

        # Encode transaction
        encoded = encode_vote_proposal(proposal_id, approve)

        # Get nonce
        nonce = self.client.get_nonce(voter_address)

        # Build and sign
        tx = signer.build_and_sign_transaction(
            to=encoded.contract_address or voter_address,
            value=0,
            nonce=nonce,
            gas_price=1_000_000_000,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=self.client.chain_id,
        )

        # Submit\n        _ = self.client.submit_transaction(tx)  # Store result for logging/debugging

        logger.info(
            f"Voted {'approve' if approve else 'reject'} on proposal {proposal_id}"
        )

        # Return updated consensus status
        return self.check_consensus(proposal_id)

    def check_consensus(self, proposal_id: str) -> ConsensusResult:
        """Check if proposal has reached consensus.

        Note: weightConsensus_checkConsensus RPC is not yet implemented
        on the LuxTensor server. Returns not-reached by default.
        """
        logger.warning(
            "weightConsensus_checkConsensus is not yet available on the server. "
            "Weight consensus feature is planned for a future release."
        )
        return ConsensusResult(reached=False)

    def finalize_proposal(
        self,
        proposal_id: str,
        signer: "TransactionSigner",
    ) -> List[Tuple[int, int]]:
        """
        Finalize an approved proposal.

        Args:
            proposal_id: Proposal ID to finalize
            signer: Transaction signer

        Returns:
            Applied weights
        """
        from sdk.luxtensor_pallets import encode_finalize_proposal

        address = signer.get_address()

        # Check proposal is approved
        proposal = self.get_proposal(proposal_id)
        if not proposal:
            raise ValueError(f"Proposal {proposal_id} not found")

        if proposal.status != ProposalStatus.APPROVED:
            raise ValueError(f"Proposal not approved: {proposal.status}")

        # Encode transaction
        encoded = encode_finalize_proposal(proposal_id)

        # Get nonce
        nonce = self.client.get_nonce(address)

        # Build and sign
        tx = signer.build_and_sign_transaction(
            to=encoded.contract_address or address,
            value=0,
            nonce=nonce,
            gas_price=1_000_000_000,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=self.client.chain_id,
        )

        # Submit
        self.client.submit_transaction(tx)

        logger.info(f"Finalized proposal {proposal_id}")

        return proposal.weights

    def get_proposal(self, proposal_id: str) -> Optional[WeightProposal]:
        """Get proposal by ID.

        Note: weightConsensus_getProposal RPC is not yet implemented on the
        LuxTensor server. Returns None.
        """
        logger.warning(
            "weightConsensus_getProposal is not yet available on the server. "
            "Weight consensus feature is planned for a future release."
        )
        return None

    def get_pending_proposals(self, subnet_uid: int) -> List[WeightProposal]:
        """Get all pending proposals for subnet.

        Note: weightConsensus_getPendingProposals RPC is not yet implemented
        on the LuxTensor server. Returns empty list.
        """
        logger.warning(
            "weightConsensus_getPendingProposals is not yet available on the server. "
            "Weight consensus feature is planned for a future release."
        )
        return []

    def _extract_proposal_id(self, tx_result) -> str:
        """Extract proposal ID from transaction result"""
        # In real implementation, parse from event logs
        # For now, return from RPC response
        if hasattr(tx_result, 'events'):
            for event in tx_result.events:
                if event.get('event') == 'ProposalCreated':
                    return event.get('proposal_id', '')
        return tx_result.tx_hash  # Fallback to tx hash


# =============================================================================
# Helper for Validators
# =============================================================================

class ValidatorConsensusHelper:
    """
    High-level helper for validators to participate in consensus.

    Provides automatic voting and proposal management.
    """

    def __init__(
        self,
        signer: "TransactionSigner",
        rpc_url: str = "http://localhost:8545",
        auto_vote: bool = False,
    ):
        self.signer = signer
        self.client = WeightConsensusClient(rpc_url)
        self.auto_vote = auto_vote
        self.my_address = signer.get_address()

    def create_proposal(
        self,
        subnet_uid: int,
        weights: List[Tuple[int, int]],
    ) -> str:
        """Create a new weight proposal"""
        return self.client.propose_weights(subnet_uid, weights, self.signer)

    def vote(self, proposal_id: str, approve: bool = True) -> ConsensusResult:
        """Vote on a proposal"""
        return self.client.vote_on_proposal(proposal_id, approve, self.signer)

    def auto_vote_pending(
        self,
        subnet_uid: int,
        vote_approve: bool = True,
    ) -> List[str]:
        """Vote on all pending proposals that I haven't voted on"""
        proposals = self.client.get_pending_proposals(subnet_uid)
        voted = []

        for proposal in proposals:
            # Skip own proposals
            if proposal.proposer.lower() == self.my_address.lower():
                continue

            # Skip already voted
            if proposal.has_voted(self.my_address):
                continue

            try:
                self.vote(proposal.id, vote_approve)
                voted.append(proposal.id)
            except Exception as e:
                logger.warning(f"Failed to vote on {proposal.id}: {e}")

        return voted

    def finalize_approved(self, subnet_uid: int) -> List[str]:
        """Finalize all approved proposals"""
        proposals = self.client.get_pending_proposals(subnet_uid)
        finalized = []

        for proposal in proposals:
            if proposal.status == ProposalStatus.APPROVED:
                try:
                    self.client.finalize_proposal(proposal.id, self.signer)
                    finalized.append(proposal.id)
                except Exception as e:
                    logger.warning(f"Failed to finalize {proposal.id}: {e}")

        return finalized


# =============================================================================
# Module exports
# =============================================================================

__all__ = [
    "WeightConsensusClient",
    "ValidatorConsensusHelper",
    "WeightConsensusConfig",
    "WeightProposal",
    "ProposalVote",
    "ProposalStatus",
    "ConsensusResult",
    "compute_weights_hash",
    "compute_proposal_id",
]

"""
ModernTensor SDK - Commit-Reveal for Validator Weights

Implements commit-reveal scheme to prevent weight manipulation.

Flow:
1. Validator commits: hash(weights || salt) during commit window
2. After commit window, reveal window opens
3. Validator reveals: actual weights + salt (verified against commit)
4. After reveal window, weights are finalized on-chain

Usage:
    from sdk.commit_reveal import CommitRevealClient, compute_commit_hash

    client = CommitRevealClient(rpc_url="http://localhost:8545")

    # Commit
    weights = [(0, 500), (1, 300), (2, 200)]
    salt = client.generate_salt()
    commit_hash = compute_commit_hash(weights, salt)

    tx = client.commit_weights(subnet_uid=1, commit_hash=commit_hash, signer=signer)

    # Wait for reveal phase...

    # Reveal
    tx = client.reveal_weights(subnet_uid=1, weights=weights, salt=salt, signer=signer)

Author: ModernTensor Team
Version: 1.0.0
"""

import hashlib
import secrets
import time
import logging
from typing import List, Tuple, Optional, Dict, Any
from dataclasses import dataclass, field
from enum import Enum

logger = logging.getLogger(__name__)


# =============================================================================
# Data Types
# =============================================================================

class EpochPhase(str, Enum):
    """Phase of commit-reveal epoch"""
    COMMITTING = "committing"
    REVEALING = "revealing"
    FINALIZING = "finalizing"
    FINALIZED = "finalized"


@dataclass
class CommitRevealConfig:
    """Configuration for commit-reveal mechanism"""
    commit_window: int = 100  # blocks
    reveal_window: int = 100  # blocks
    min_commits: int = 1
    slash_on_no_reveal: bool = True
    no_reveal_slash_percent: int = 1


@dataclass
class WeightCommit:
    """A weight commit from a validator"""
    validator: str
    subnet_uid: int
    commit_hash: str
    committed_at: int
    revealed: bool = False
    weights: Optional[List[Tuple[int, int]]] = None
    salt: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "validator": self.validator,
            "subnet_uid": self.subnet_uid,
            "commit_hash": self.commit_hash,
            "committed_at": self.committed_at,
            "revealed": self.revealed,
            "weights": self.weights,
        }


@dataclass
class EpochState:
    """State of a commit-reveal epoch"""
    subnet_uid: int
    epoch_number: int
    phase: EpochPhase
    commit_start_block: int
    reveal_start_block: int
    finalize_block: int
    commits: List[WeightCommit] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "subnet_uid": self.subnet_uid,
            "epoch_number": self.epoch_number,
            "phase": self.phase.value,
            "commit_start_block": self.commit_start_block,
            "reveal_start_block": self.reveal_start_block,
            "finalize_block": self.finalize_block,
            "commits_count": len(self.commits),
            "revealed_count": sum(1 for c in self.commits if c.revealed),
        }


# =============================================================================
# Hash Functions
# =============================================================================

def compute_commit_hash(weights: List[Tuple[int, int]], salt: bytes) -> str:
    """
    Compute commit hash from weights and salt.

    Uses keccak256 to match Rust implementation.

    Args:
        weights: List of (uid, weight) tuples
        salt: 32-byte salt

    Returns:
        Hex string of commit hash (with 0x prefix)
    """
    try:
        from Crypto.Hash import keccak
        hasher = keccak.new(digest_bits=256)
    except ImportError:
        # Fallback to sha3
        hasher = hashlib.sha3_256()

    # Encode weights deterministically (same as Rust)
    for uid, weight in sorted(weights, key=lambda x: x[0]):
        hasher.update(uid.to_bytes(8, 'big'))
        hasher.update(weight.to_bytes(2, 'big'))

    # Add salt
    if isinstance(salt, str):
        salt = bytes.fromhex(salt.replace('0x', ''))
    hasher.update(salt)

    return "0x" + hasher.hexdigest()


def generate_salt() -> bytes:
    """Generate cryptographically secure 32-byte salt"""
    return secrets.token_bytes(32)


def verify_commit(
    commit_hash: str,
    weights: List[Tuple[int, int]],
    salt: bytes,
) -> bool:
    """Verify that weights and salt match commit hash"""
    computed = compute_commit_hash(weights, salt)
    return computed.lower() == commit_hash.lower()


# =============================================================================
# Client
# =============================================================================

class CommitRevealClient:
    """
    Client for commit-reveal weight operations.

    Syncs with on-chain CommitRevealManager in LuxTensor.

    Example:
        from sdk.commit_reveal import CommitRevealClient
        from sdk.keymanager.transaction_signer import TransactionSigner

        client = CommitRevealClient()
        signer = TransactionSigner(private_key)

        # Generate weights and salt
        weights = [(0, 500), (1, 300), (2, 200)]
        salt = client.generate_salt()

        # Commit (during commit phase)
        commit_hash = client.compute_hash(weights, salt)
        result = client.commit_weights(1, commit_hash, signer)

        # IMPORTANT: Save salt securely!

        # Reveal (during reveal phase)
        result = client.reveal_weights(1, weights, salt, signer)
    """

    def __init__(self, rpc_url: str = "http://localhost:8545"):
        """
        Initialize client.

        Args:
            rpc_url: LuxTensor RPC endpoint
        """
        from sdk.luxtensor_client import LuxtensorClient

        self.rpc_url = rpc_url
        self.client = LuxtensorClient(url=rpc_url)
        self._config = None

    @staticmethod
    def generate_salt() -> bytes:
        """Generate secure salt"""
        return generate_salt()

    @staticmethod
    def compute_hash(
        weights: List[Tuple[int, int]],
        salt: bytes,
    ) -> str:
        """Compute commit hash"""
        return compute_commit_hash(weights, salt)

    def get_config(self) -> CommitRevealConfig:
        """Get commit-reveal configuration from chain"""
        if self._config:
            return self._config

        try:
            result = self.client._call_rpc("commitReveal_getConfig", [])
            self._config = CommitRevealConfig(
                commit_window=result.get("commitWindow", 100),
                reveal_window=result.get("revealWindow", 100),
                min_commits=result.get("minCommits", 1),
            )
            return self._config
        except Exception:
            return CommitRevealConfig()

    def get_epoch_state(self, subnet_uid: int) -> Optional[EpochState]:
        """
        Get current epoch state for subnet.

        Args:
            subnet_uid: Subnet ID

        Returns:
            EpochState or None if no active epoch
        """
        try:
            result = self.client._call_rpc(
                "commitReveal_getEpochState",
                [subnet_uid]
            )

            if not result:
                return None

            commits = [
                WeightCommit(
                    validator=c["validator"],
                    subnet_uid=subnet_uid,
                    commit_hash=c["commitHash"],
                    committed_at=c["committedAt"],
                    revealed=c.get("revealed", False),
                )
                for c in result.get("commits", [])
            ]

            return EpochState(
                subnet_uid=subnet_uid,
                epoch_number=result["epochNumber"],
                phase=EpochPhase(result["phase"]),
                commit_start_block=result["commitStartBlock"],
                reveal_start_block=result["revealStartBlock"],
                finalize_block=result["finalizeBlock"],
                commits=commits,
            )

        except Exception as e:
            logger.error(f"Failed to get epoch state: {e}")
            return None

    def get_current_phase(self, subnet_uid: int) -> Optional[EpochPhase]:
        """Get current phase for subnet"""
        state = self.get_epoch_state(subnet_uid)
        return state.phase if state else None

    def commit_weights(
        self,
        subnet_uid: int,
        commit_hash: str,
        signer: "TransactionSigner",
    ) -> Dict[str, Any]:
        """
        Submit weight commit to chain.

        Args:
            subnet_uid: Subnet ID
            commit_hash: Hash of weights+salt (from compute_hash)
            signer: Transaction signer with validator's private key

        Returns:
            Transaction result
        """
        from sdk.luxtensor_pallets import encode_commit_weights

        # Get validator address
        validator_address = signer.get_address()

        # Check phase
        state = self.get_epoch_state(subnet_uid)
        if state and state.phase != EpochPhase.COMMITTING:
            raise ValueError(f"Not in commit phase. Current phase: {state.phase}")

        # Check not already committed
        if state:
            for c in state.commits:
                if c.validator.lower() == validator_address.lower():
                    raise ValueError("Already committed in this epoch")

        # Encode transaction
        encoded = encode_commit_weights(subnet_uid, commit_hash)

        # Get nonce
        nonce = self.client.get_nonce(validator_address)

        # Build and sign transaction
        tx = signer.build_and_sign_transaction(
            to=encoded.contract_address,
            value=0,
            nonce=nonce,
            gas_price=1_000_000_000,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=self.client.chain_id,
        )

        # Submit
        result = self.client.submit_transaction(tx)

        logger.info(
            f"Committed weights for subnet {subnet_uid}: {result.tx_hash}"
        )

        return {
            "success": result.success,
            "tx_hash": result.tx_hash,
            "commit_hash": commit_hash,
        }

    def reveal_weights(
        self,
        subnet_uid: int,
        weights: List[Tuple[int, int]],
        salt: bytes,
        signer: "TransactionSigner",
    ) -> Dict[str, Any]:
        """
        Reveal weights during reveal phase.

        Args:
            subnet_uid: Subnet ID
            weights: List of (uid, weight) tuples
            salt: Salt used for commit
            signer: Transaction signer

        Returns:
            Transaction result
        """
        from sdk.luxtensor_pallets import encode_reveal_weights

        validator_address = signer.get_address()

        # Check phase
        state = self.get_epoch_state(subnet_uid)
        if state and state.phase != EpochPhase.REVEALING:
            raise ValueError(f"Not in reveal phase. Current phase: {state.phase}")

        # Check commit exists
        if state:
            commit = next(
                (c for c in state.commits if c.validator.lower() == validator_address.lower()),
                None
            )
            if not commit:
                raise ValueError("No commit found for this validator")
            if commit.revealed:
                raise ValueError("Already revealed")

        # Verify hash matches locally first
        if isinstance(salt, str):
            salt = bytes.fromhex(salt.replace('0x', ''))

        computed_hash = compute_commit_hash(weights, salt)
        if state:
            commit = next(
                c for c in state.commits
                if c.validator.lower() == validator_address.lower()
            )
            if computed_hash.lower() != commit.commit_hash.lower():
                raise ValueError("Weights/salt do not match commit hash")

        # Encode transaction
        salt_hex = salt.hex() if isinstance(salt, bytes) else salt
        encoded = encode_reveal_weights(subnet_uid, weights, salt_hex)

        # Get nonce
        nonce = self.client.get_nonce(validator_address)

        # Build and sign
        tx = signer.build_and_sign_transaction(
            to=encoded.contract_address,
            value=0,
            nonce=nonce,
            gas_price=1_000_000_000,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=self.client.chain_id,
        )

        # Submit
        result = self.client.submit_transaction(tx)

        logger.info(
            f"Revealed weights for subnet {subnet_uid}: {result.tx_hash}"
        )

        return {
            "success": result.success,
            "tx_hash": result.tx_hash,
            "weights": weights,
        }

    def get_pending_commits(self, subnet_uid: int) -> List[WeightCommit]:
        """Get all pending commits for subnet"""
        state = self.get_epoch_state(subnet_uid)
        return state.commits if state else []

    def get_finalized_weights(self, subnet_uid: int) -> List[Tuple[int, int]]:
        """Get finalized weights from last epoch"""
        try:
            result = self.client._call_rpc(
                "commitReveal_getFinalizedWeights",
                [subnet_uid]
            )
            return [(w["uid"], w["weight"]) for w in result]
        except Exception as e:
            logger.error(f"Failed to get finalized weights: {e}")
            return []

    def has_committed(self, subnet_uid: int, validator: str) -> bool:
        """Check if validator has committed in current epoch"""
        state = self.get_epoch_state(subnet_uid)
        if not state:
            return False
        return any(
            c.validator.lower() == validator.lower()
            for c in state.commits
        )

    def has_revealed(self, subnet_uid: int, validator: str) -> bool:
        """Check if validator has revealed in current epoch"""
        state = self.get_epoch_state(subnet_uid)
        if not state:
            return False
        return any(
            c.validator.lower() == validator.lower() and c.revealed
            for c in state.commits
        )


# =============================================================================
# Helper for Validators
# =============================================================================

class ValidatorCommitReveal:
    """
    High-level helper for validators to manage commit-reveal.

    Automatically tracks salt and handles the full flow.

    Example:
        helper = ValidatorCommitReveal(signer, rpc_url)

        # During commit phase
        weights = [(0, 500), (1, 300)]
        helper.commit(subnet_uid=1, weights=weights)

        # During reveal phase (salt is tracked internally)
        helper.reveal(subnet_uid=1)
    """

    def __init__(
        self,
        signer: "TransactionSigner",
        rpc_url: str = "http://localhost:8545",
    ):
        self.signer = signer
        self.client = CommitRevealClient(rpc_url)
        self._pending_commits: Dict[int, Dict] = {}  # subnet_uid -> {weights, salt}

    def commit(
        self,
        subnet_uid: int,
        weights: List[Tuple[int, int]],
    ) -> Dict[str, Any]:
        """
        Commit weights for subnet.

        Salt is generated and stored internally for later reveal.

        Args:
            subnet_uid: Subnet ID
            weights: List of (uid, weight) tuples

        Returns:
            Transaction result with commit_hash and salt
        """
        # Generate salt
        salt = generate_salt()

        # Compute hash
        commit_hash = compute_commit_hash(weights, salt)

        # Submit commit
        result = self.client.commit_weights(subnet_uid, commit_hash, self.signer)

        # Store for reveal
        self._pending_commits[subnet_uid] = {
            "weights": weights,
            "salt": salt,
            "commit_hash": commit_hash,
        }

        result["salt"] = salt.hex()
        return result

    def reveal(self, subnet_uid: int) -> Dict[str, Any]:
        """
        Reveal weights for subnet.

        Uses previously stored weights and salt.

        Args:
            subnet_uid: Subnet ID

        Returns:
            Transaction result
        """
        if subnet_uid not in self._pending_commits:
            raise ValueError(f"No pending commit for subnet {subnet_uid}")

        pending = self._pending_commits[subnet_uid]

        result = self.client.reveal_weights(
            subnet_uid,
            pending["weights"],
            pending["salt"],
            self.signer,
        )

        # Clear pending
        del self._pending_commits[subnet_uid]

        return result

    def get_pending(self, subnet_uid: int) -> Optional[Dict]:
        """Get pending commit data (weights, salt, hash)"""
        return self._pending_commits.get(subnet_uid)

    def restore_pending(
        self,
        subnet_uid: int,
        weights: List[Tuple[int, int]],
        salt: bytes,
    ):
        """
        Restore pending commit from saved data.

        Useful for recovering after restart.
        """
        self._pending_commits[subnet_uid] = {
            "weights": weights,
            "salt": salt,
            "commit_hash": compute_commit_hash(weights, salt),
        }


# =============================================================================
# Module exports
# =============================================================================

__all__ = [
    "CommitRevealClient",
    "ValidatorCommitReveal",
    "CommitRevealConfig",
    "WeightCommit",
    "EpochState",
    "EpochPhase",
    "compute_commit_hash",
    "generate_salt",
    "verify_commit",
]

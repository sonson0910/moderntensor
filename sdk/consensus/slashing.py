"""
Slashing Module for Validator Penalties.

Matches luxtensor-consensus/src/slashing.rs.

Handles:
- Offline validators (missed blocks)
- Double signing detection
- Invalid block/weight proposals
- Jailing mechanism
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Tuple
from decimal import Decimal
import time


class SlashReason(str, Enum):
    """Reasons for slashing a validator."""
    OFFLINE = "offline"              # Missed too many blocks
    DOUBLE_SIGNING = "double_signing"  # Signed two blocks at same height
    INVALID_BLOCK = "invalid_block"   # Proposed invalid block
    INVALID_WEIGHTS = "invalid_weights"  # Submitted invalid weights
    CUSTOM = "custom"                 # Custom reason

    def slash_percent(self, config: 'SlashingConfig') -> int:
        """Get slash percentage for this reason."""
        return {
            SlashReason.OFFLINE: config.offline_slash_percent,
            SlashReason.DOUBLE_SIGNING: config.double_sign_slash_percent,
            SlashReason.INVALID_BLOCK: config.invalid_block_slash_percent,
            SlashReason.INVALID_WEIGHTS: config.invalid_weights_slash_percent,
            SlashReason.CUSTOM: 1,
        }[self]


@dataclass
class SlashingConfig:
    """
    Slashing configuration.

    Matches luxtensor-consensus SlashingConfig.
    """
    # Slash percentages
    offline_slash_percent: int = 1          # 1% for being offline
    double_sign_slash_percent: int = 5      # 5% for double signing
    invalid_block_slash_percent: int = 2    # 2% for invalid block
    invalid_weights_slash_percent: int = 1  # 1% for invalid weights

    # Thresholds
    max_missed_blocks: int = 100            # Blocks before offline slash
    signed_blocks_window: int = 1000        # Window for tracking missed blocks

    # Jail duration
    jail_duration: int = 10000              # Blocks of jail time

    @classmethod
    def default(cls) -> 'SlashingConfig':
        return cls()


@dataclass
class SlashingEvidence:
    """
    Evidence of misbehavior.

    Matches luxtensor-consensus SlashingEvidence.
    """
    validator: str          # Validator address
    reason: SlashReason     # Reason for slashing
    height: int             # Block height when detected
    evidence_hash: str      # Hash of evidence data
    timestamp: int = 0      # When detected
    details: str = ""       # Additional details

    def __post_init__(self):
        if self.timestamp == 0:
            self.timestamp = int(time.time())


@dataclass
class SlashEvent:
    """
    Slashing event record.

    Matches luxtensor-consensus SlashEvent.
    """
    validator: str              # Validator address
    reason: SlashReason         # Reason
    slash_percent: int          # Percentage slashed
    amount_slashed: int         # Amount slashed (in base units)
    height: int                 # Block height
    timestamp: int              # Unix timestamp
    jailed: bool = False        # Whether validator was jailed
    jail_until: int = 0         # Block height to unjail

    def to_dict(self) -> Dict:
        return {
            "validator": self.validator,
            "reason": self.reason.value,
            "slash_percent": self.slash_percent,
            "amount_slashed": str(self.amount_slashed),
            "amount_slashed_mdt": str(Decimal(self.amount_slashed) / Decimal(10**18)),
            "height": self.height,
            "timestamp": self.timestamp,
            "jailed": self.jailed,
            "jail_until": self.jail_until if self.jailed else None,
        }


@dataclass
class JailStatus:
    """
    Jail status for a validator.

    Matches luxtensor-consensus JailStatus.
    """
    validator: str
    jailed_at: int          # Block height when jailed
    jail_until: int         # Block height when can unjail
    reason: SlashReason     # Reason for jailing
    slash_count: int = 1    # Number of times slashed


class SlashingManager:
    """
    Slashing Manager - handles validator penalties.

    Matches luxtensor-consensus SlashingManager.
    """

    def __init__(self, config: Optional[SlashingConfig] = None):
        self.config = config or SlashingConfig.default()
        self._missed_blocks: Dict[str, int] = {}  # validator -> count
        self._slash_history: Dict[str, List[SlashEvent]] = {}  # validator -> events
        self._jail_status: Dict[str, JailStatus] = {}  # validator -> status
        self._block_signatures: Dict[int, Dict[str, List[str]]] = {}  # height -> validator -> signatures

    def record_missed_block(self, validator: str):
        """Record a missed block for validator."""
        validator = validator.lower()
        self._missed_blocks[validator] = self._missed_blocks.get(validator, 0) + 1

    def reset_missed_blocks(self, validator: str):
        """Reset missed blocks for validator (called when they produce a block)."""
        self._missed_blocks[validator.lower()] = 0

    def check_offline(self, validator: str) -> Optional[SlashingEvidence]:
        """Check if validator should be slashed for being offline."""
        validator = validator.lower()
        missed = self._missed_blocks.get(validator, 0)

        if missed >= self.config.max_missed_blocks:
            return SlashingEvidence(
                validator=validator,
                reason=SlashReason.OFFLINE,
                height=0,  # Will be set by caller
                evidence_hash=f"offline_{validator}_{missed}",
                details=f"Missed {missed} blocks",
            )
        return None

    def record_block_signature(
        self,
        height: int,
        block_hash: str,
        validator: str,
        signature_hash: str,
    ):
        """Record potential double signing."""
        validator = validator.lower()
        if height not in self._block_signatures:
            self._block_signatures[height] = {}
        if validator not in self._block_signatures[height]:
            self._block_signatures[height][validator] = []

        self._block_signatures[height][validator].append(f"{block_hash}:{signature_hash}")

    def check_double_signing(self, height: int) -> List[SlashingEvidence]:
        """Check for double signing at a height."""
        evidence = []

        sigs = self._block_signatures.get(height, {})
        for validator, signatures in sigs.items():
            # Check if validator signed multiple different blocks
            unique_blocks = set(s.split(":")[0] for s in signatures)
            if len(unique_blocks) > 1:
                evidence.append(SlashingEvidence(
                    validator=validator,
                    reason=SlashReason.DOUBLE_SIGNING,
                    height=height,
                    evidence_hash=f"double_sign_{validator}_{height}",
                    details=f"Signed {len(unique_blocks)} different blocks at height {height}",
                ))

        return evidence

    def slash(
        self,
        evidence: SlashingEvidence,
        current_height: int,
        validator_stake: int,
    ) -> SlashEvent:
        """
        Execute slash on a validator.

        Args:
            evidence: Evidence of misbehavior
            current_height: Current block height
            validator_stake: Validator's current stake

        Returns:
            SlashEvent with details
        """
        validator = evidence.validator.lower()
        slash_percent = evidence.reason.slash_percent(self.config)

        # Calculate slashed amount
        amount_slashed = (validator_stake * slash_percent) // 100

        # Check if should be jailed
        should_jail = evidence.reason in [SlashReason.DOUBLE_SIGNING, SlashReason.OFFLINE]
        jail_until = current_height + self.config.jail_duration if should_jail else 0

        # Create event
        event = SlashEvent(
            validator=validator,
            reason=evidence.reason,
            slash_percent=slash_percent,
            amount_slashed=amount_slashed,
            height=current_height,
            timestamp=int(time.time()),
            jailed=should_jail,
            jail_until=jail_until,
        )

        # Record in history
        if validator not in self._slash_history:
            self._slash_history[validator] = []
        self._slash_history[validator].append(event)

        # Update jail status
        if should_jail:
            self._jail_status[validator] = JailStatus(
                validator=validator,
                jailed_at=current_height,
                jail_until=jail_until,
                reason=evidence.reason,
                slash_count=len(self._slash_history[validator]),
            )

        # Reset missed blocks
        self.reset_missed_blocks(validator)

        return event

    def is_jailed(self, validator: str) -> bool:
        """Check if validator is jailed."""
        return validator.lower() in self._jail_status

    def get_jail_status(self, validator: str) -> Optional[JailStatus]:
        """Get jail status."""
        return self._jail_status.get(validator.lower())

    def process_unjail(self, current_height: int) -> List[str]:
        """Process unjailing (called each block). Returns list of unjailed validators."""
        unjailed = []

        for validator, status in list(self._jail_status.items()):
            if current_height >= status.jail_until:
                del self._jail_status[validator]
                unjailed.append(validator)

        return unjailed

    def get_slash_history(self, validator: str) -> List[SlashEvent]:
        """Get slash history for an address."""
        return self._slash_history.get(validator.lower(), [])

    def get_all_slash_events(self) -> List[SlashEvent]:
        """Get all slash events."""
        all_events = []
        for events in self._slash_history.values():
            all_events.extend(events)
        return sorted(all_events, key=lambda e: e.height, reverse=True)

    def get_total_slashed(self, validator: str) -> int:
        """Get total slashed amount for a validator."""
        events = self.get_slash_history(validator)
        return sum(e.amount_slashed for e in events)

    def cleanup_old_signatures(self, current_height: int, max_age: int = 1000):
        """Clean up old signatures (call periodically)."""
        min_height = current_height - max_age
        self._block_signatures = {
            h: sigs for h, sigs in self._block_signatures.items()
            if h >= min_height
        }


# Module exports
__all__ = [
    "SlashReason",
    "SlashingConfig",
    "SlashingEvidence",
    "SlashEvent",
    "JailStatus",
    "SlashingManager",
]

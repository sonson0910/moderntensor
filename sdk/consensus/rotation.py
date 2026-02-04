"""
Validator Rotation - Epoch-based Validator Set Management

Ported from luxtensor-consensus/src/rotation.rs

Implements automatic validator set updates based on epochs,
including activation delays, exit delays, and slashing.
"""

from dataclasses import dataclass, field
from threading import RLock
from typing import Dict, List, Optional, Set
import logging

logger = logging.getLogger(__name__)


@dataclass
class ValidatorInfo:
    """Validator information."""
    address: str
    stake: int
    public_key: Optional[str] = None
    active: bool = True
    rewards: int = 0
    activation_epoch: int = 0


@dataclass
class RotationConfig:
    """Configuration for validator rotation."""
    # Number of slots per epoch
    epoch_length: int = 32
    # Epochs a validator must wait before joining
    activation_delay_epochs: int = 2
    # Epochs a validator must wait before exiting
    exit_delay_epochs: int = 2
    # Maximum number of validators in the active set
    max_validators: int = 100
    # Minimum stake required (32 tokens in wei)
    min_stake: int = 32_000_000_000_000_000_000


@dataclass
class PendingValidator:
    """A pending validator waiting to join."""
    validator: ValidatorInfo
    activation_epoch: int


@dataclass
class EpochTransitionResult:
    """Result of epoch transition."""
    activated_validators: List[str] = field(default_factory=list)
    exited_validators: List[str] = field(default_factory=list)
    new_epoch: int = 0


@dataclass
class RotationStats:
    """Validator rotation statistics."""
    current_epoch: int = 0
    active_validators: int = 0
    pending_validators: int = 0
    exiting_validators: int = 0
    total_stake: int = 0


class ValidatorRotationError(Exception):
    """Error during validator rotation."""
    pass


class ValidatorRotation:
    """
    Validator rotation manager for epoch-based validator set updates.

    Thread-safe implementation using RLock.

    Usage:
        rotation = ValidatorRotation(RotationConfig())

        # Request to add validator
        activation_epoch = rotation.request_validator_addition(validator)

        # Process epoch transition
        result = rotation.process_epoch_transition(new_epoch)
    """

    def __init__(
        self,
        config: Optional[RotationConfig] = None,
        initial_validators: Optional[Dict[str, ValidatorInfo]] = None,
    ):
        self._lock = RLock()
        self.config = config or RotationConfig()
        self._current_epoch = 0

        self._validators: Dict[str, ValidatorInfo] = initial_validators or {}
        self._pending_validators: Dict[str, PendingValidator] = {}
        self._exiting_validators: Set[str] = set()

        logger.info(f"ValidatorRotation initialized with {len(self._validators)} validators")

    def current_validators(self) -> Dict[str, ValidatorInfo]:
        """Get current active validators."""
        with self._lock:
            return dict(self._validators)

    def request_validator_addition(self, validator: ValidatorInfo) -> int:
        """
        Request to add a new validator.

        Returns:
            Activation epoch

        Raises:
            ValidatorRotationError: If stake insufficient or already exists
        """
        with self._lock:
            # Validate minimum stake
            if validator.stake < self.config.min_stake:
                raise ValidatorRotationError(
                    f"Insufficient stake: {validator.stake} < {self.config.min_stake}"
                )

            # Check if already exists
            if validator.address in self._validators:
                raise ValidatorRotationError(f"Validator already exists: {validator.address[:16]}...")

            # Check if already pending
            if validator.address in self._pending_validators:
                raise ValidatorRotationError(f"Validator already pending: {validator.address[:16]}...")

            # Calculate activation epoch
            activation_epoch = self._current_epoch + self.config.activation_delay_epochs

            self._pending_validators[validator.address] = PendingValidator(
                validator=validator,
                activation_epoch=activation_epoch,
            )

            logger.info(
                f"Validator {validator.address[:16]}... requested to join, "
                f"activation at epoch {activation_epoch}"
            )

            return activation_epoch

    def request_validator_exit(self, address: str) -> int:
        """
        Request validator exit.

        Returns:
            Exit epoch

        Raises:
            ValidatorRotationError: If validator not found or already exiting
        """
        with self._lock:
            if address not in self._validators:
                raise ValidatorRotationError(f"Validator not found: {address[:16]}...")

            if address in self._exiting_validators:
                raise ValidatorRotationError("Validator already scheduled to exit")

            exit_epoch = self._current_epoch + self.config.exit_delay_epochs
            self._exiting_validators.add(address)

            logger.info(
                f"Validator {address[:16]}... requested to exit at epoch {exit_epoch}"
            )

            return exit_epoch

    def process_epoch_transition(self, new_epoch: int) -> EpochTransitionResult:
        """Process epoch transition, activating and exiting validators."""
        with self._lock:
            self._current_epoch = new_epoch

            # Activate pending validators
            activated = self._activate_pending_validators(new_epoch)

            # Process exits
            exited = self._process_validator_exits(new_epoch)

            return EpochTransitionResult(
                activated_validators=activated,
                exited_validators=exited,
                new_epoch=new_epoch,
            )

    def _activate_pending_validators(self, new_epoch: int) -> List[str]:
        """Activate validators whose activation epoch has arrived."""
        activated = []

        ready_addresses = [
            addr for addr, pending in self._pending_validators.items()
            if pending.activation_epoch <= new_epoch
        ]

        for address in ready_addresses:
            pending = self._pending_validators.pop(address)

            if len(self._validators) < self.config.max_validators:
                self._validators[address] = pending.validator
                activated.append(address)
                logger.info(f"Activated validator {address[:16]}... at epoch {new_epoch}")
            else:
                # Re-queue for next epoch
                self._pending_validators[address] = PendingValidator(
                    validator=pending.validator,
                    activation_epoch=new_epoch + 1,
                )
                logger.warning(
                    f"Cannot activate validator {address[:16]}..., max count reached"
                )

        return activated

    def _process_validator_exits(self, new_epoch: int) -> List[str]:
        """Process validators scheduled for exit."""
        exited = []

        for address in list(self._exiting_validators):
            if address in self._validators:
                del self._validators[address]
            self._exiting_validators.remove(address)
            exited.append(address)
            logger.info(f"Exited validator {address[:16]}... at epoch {new_epoch}")

        return exited

    def slash_validator(self, address: str, slash_amount: int) -> None:
        """
        Slash a validator for misbehavior.

        Reduces stake and schedules exit if below minimum.
        """
        with self._lock:
            if address not in self._validators:
                raise ValidatorRotationError(f"Validator not found: {address[:16]}...")

            validator = self._validators[address]
            if slash_amount > validator.stake:
                raise ValidatorRotationError("Slash amount exceeds stake")

            new_stake = validator.stake - slash_amount
            validator.stake = new_stake

            logger.warning(
                f"Slashed validator {address[:16]}... by {slash_amount}, new stake: {new_stake}"
            )

            # Schedule exit if below minimum
            if new_stake < self.config.min_stake:
                self._exiting_validators.add(address)
                logger.warning(f"Validator {address[:16]}... stake below minimum, scheduling exit")

    def pending_count(self) -> int:
        """Get pending validator count."""
        with self._lock:
            return len(self._pending_validators)

    def exiting_count(self) -> int:
        """Get exiting validator count."""
        with self._lock:
            return len(self._exiting_validators)

    def current_epoch(self) -> int:
        """Get current epoch."""
        with self._lock:
            return self._current_epoch

    def get_stats(self) -> RotationStats:
        """Get rotation statistics."""
        with self._lock:
            total_stake = sum(v.stake for v in self._validators.values())
            return RotationStats(
                current_epoch=self._current_epoch,
                active_validators=len(self._validators),
                pending_validators=len(self._pending_validators),
                exiting_validators=len(self._exiting_validators),
                total_stake=total_stake,
            )

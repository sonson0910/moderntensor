"""
Proof of Stake consensus mechanism for ModernTensor Layer 1 blockchain.

Integrates with existing consensus state management and validator scoring.
"""
import hashlib
import logging
import secrets
from typing import Dict, List, Optional, Set, TYPE_CHECKING
from dataclasses import dataclass, field

from ..blockchain.block import Block
from ..blockchain.state import StateDB

# Type checking imports to avoid circular dependencies and missing modules
if TYPE_CHECKING:
    from ..core.datatypes import ValidatorInfo

# Constants for validator status
STATUS_ACTIVE = 1
STATUS_JAILED = 2
STATUS_INACTIVE = 3

logger = logging.getLogger(__name__)


@dataclass
class Validator:
    """
    Validator information for PoS consensus.
    
    Attributes:
        address: Validator address (20 bytes)
        public_key: Validator public key (32 bytes)
        stake: Amount staked
        active: Whether validator is active
        last_block_height: Last block produced
        missed_blocks: Count of missed block production slots
    """
    address: bytes  # 20 bytes
    public_key: bytes  # 32 bytes
    stake: int
    active: bool = True
    last_block_height: int = 0
    missed_blocks: int = 0


@dataclass
class ConsensusConfig:
    """
    Consensus configuration parameters.
    
    Attributes:
        epoch_length: Number of blocks per epoch
        validator_count: Target number of active validators
        min_stake: Minimum stake required to be a validator
        block_time: Target time between blocks (seconds)
        max_missed_blocks: Maximum missed blocks before jailing
        slash_rate: Percentage to slash for misbehavior (0-1)
    """
    epoch_length: int = 100
    validator_count: int = 21
    min_stake: int = 1_000_000  # 1M tokens minimum
    block_time: int = 12  # seconds
    max_missed_blocks: int = 10
    slash_rate: float = 0.05  # 5% slash rate


class ValidatorSet:
    """
    Manages the active set of validators.
    
    Maintains validator registration, staking, and status.
    """
    
    def __init__(self):
        """Initialize empty validator set."""
        self.validators: Dict[bytes, Validator] = {}  # address -> Validator
        self.active_validators: Set[bytes] = set()  # Set of active validator addresses
    
    def add_validator(self, address: bytes, public_key: bytes, stake: int) -> bool:
        """
        Add or update a validator.
        
        Args:
            address: Validator address
            public_key: Validator public key
            stake: Stake amount
            
        Returns:
            bool: True if successful
        """
        if address in self.validators:
            # Update existing validator
            validator = self.validators[address]
            validator.stake = stake
            logger.info(f"Updated validator {address.hex()[:8]}... stake to {stake}")
        else:
            # Add new validator
            validator = Validator(
                address=address,
                public_key=public_key,
                stake=stake,
                active=True,
            )
            self.validators[address] = validator
            self.active_validators.add(address)
            logger.info(f"Added new validator {address.hex()[:8]}... with stake {stake}")
        
        return True
    
    def remove_validator(self, address: bytes) -> bool:
        """
        Remove a validator from the active set.
        
        Args:
            address: Validator address
            
        Returns:
            bool: True if successful
        """
        if address in self.validators:
            validator = self.validators[address]
            validator.active = False
            self.active_validators.discard(address)
            logger.info(f"Removed validator {address.hex()[:8]}... from active set")
            return True
        return False
    
    def jail_validator(self, address: bytes) -> bool:
        """
        Jail a validator for misbehavior.
        
        Args:
            address: Validator address
            
        Returns:
            bool: True if successful
        """
        return self.remove_validator(address)
    
    def get_validator(self, address: bytes) -> Optional[Validator]:
        """Get validator by address."""
        return self.validators.get(address)
    
    def get_total_stake(self) -> int:
        """
        Get total staked amount from all active validators.
        
        Returns:
            int: Total stake
        """
        return sum(
            v.stake for v in self.validators.values()
            if v.active
        )
    
    def get_active_validators(self) -> List[Validator]:
        """
        Get list of active validators.
        
        Returns:
            List[Validator]: Active validators
        """
        return [
            v for v in self.validators.values()
            if v.active
        ]
    
    def select_validators_for_epoch(self, count: int) -> List[Validator]:
        """
        Select top validators by stake for an epoch.
        
        Args:
            count: Number of validators to select
            
        Returns:
            List[Validator]: Selected validators sorted by stake
        """
        active = self.get_active_validators()
        # Sort by stake (descending)
        sorted_validators = sorted(active, key=lambda v: v.stake, reverse=True)
        return sorted_validators[:count]


class ProofOfStake:
    """
    Proof of Stake consensus mechanism.
    
    Implements validator selection, block production, and epoch management.
    """
    
    def __init__(self, state_db: StateDB, config: ConsensusConfig):
        """
        Initialize PoS consensus.
        
        Args:
            state_db: State database
            config: Consensus configuration
        """
        self.state = state_db
        self.config = config
        self.validator_set = ValidatorSet()
        self.current_epoch = 0
        self.current_slot = 0
        
        logger.info("PoS consensus initialized")
    
    def select_validator(self, slot: int, seed: Optional[bytes] = None) -> Optional[bytes]:
        """
        Select validator for a given slot using weighted random selection.
        
        Uses stake-weighted random selection to choose validator.
        For determinism, uses VRF-like seed based on slot number.
        
        Args:
            slot: Slot number (block height)
            seed: Optional random seed (uses slot number if None)
            
        Returns:
            Optional[bytes]: Selected validator address, or None if no validators
        """
        active_validators = self.validator_set.get_active_validators()
        if not active_validators:
            logger.warning("No active validators available for selection")
            return None
        
        # Calculate total stake
        total_stake = sum(v.stake for v in active_validators)
        if total_stake == 0:
            logger.warning("Total stake is zero")
            return None
        
        # Generate deterministic random value from slot
        if seed is None:
            seed = slot.to_bytes(8, 'big')
        random_hash = hashlib.sha256(seed).digest()
        random_value = int.from_bytes(random_hash[:8], 'big')
        
        # Weighted random selection
        selection_point = random_value % total_stake
        current_sum = 0
        
        for validator in active_validators:
            current_sum += validator.stake
            if current_sum > selection_point:
                logger.debug(
                    f"Slot {slot}: Selected validator {validator.address.hex()[:8]}... "
                    f"(stake: {validator.stake}, total: {total_stake})"
                )
                return validator.address
        
        # Fallback (should not reach here)
        return active_validators[0].address
    
    def validate_block_producer(self, block: Block, expected_slot: int) -> bool:
        """
        Verify block was produced by the correct validator for the slot.
        
        Args:
            block: Block to validate
            expected_slot: Expected slot number
            
        Returns:
            bool: True if block producer is valid
        """
        expected_validator = self.select_validator(expected_slot)
        if expected_validator is None:
            return False
        
        # Check if block validator matches expected
        # Map public key to address using keccak256 (Ethereum-style)
        # Note: block.header.validator contains validator identifier
        # In production, should derive address from public key properly
        actual_validator = block.header.validator[:20]  # Use first 20 bytes as address
        
        if actual_validator != expected_validator:
            logger.error(
                f"Block producer mismatch. Expected {expected_validator.hex()[:8]}..., "
                f"got {actual_validator.hex()[:8]}..."
            )
            return False
        
        return True
    
    def process_epoch(self, epoch: int) -> None:
        """
        Process epoch boundary - update validator set, distribute rewards.
        
        Args:
            epoch: Epoch number
        """
        logger.info(f"Processing epoch {epoch}...")
        
        # 1. Calculate and distribute rewards
        self._distribute_rewards(epoch)
        
        # 2. Process any pending slashing
        self._process_slashing(epoch)
        
        # 3. Update validator set for next epoch
        self._update_validator_set(epoch)
        
        self.current_epoch = epoch
        logger.info(f"Epoch {epoch} processing complete")
    
    def _distribute_rewards(self, epoch: int) -> None:
        """
        Distribute block rewards to validators for the epoch.
        
        Args:
            epoch: Epoch number
        """
        # Calculate total rewards for the epoch
        # Reward distribution based on blocks produced and stake
        blocks_in_epoch = self.config.epoch_length
        
        # Implement reward calculation based on:
        # - Blocks produced by each validator
        # - Validator stake (proportional rewards)
        # - Performance metrics (uptime, missed blocks)
        
        total_stake = self.validator_set.get_total_stake()
        base_reward = 100  # Base reward per epoch (configurable)
        
        if total_stake > 0:
            for validator in self.validator_set.get_active_validators():
                # Proportional reward based on stake
                stake_ratio = validator.stake / total_stake
                
                # Performance factor (based on blocks produced)
                # In full implementation, track actual blocks produced
                performance = 1.0 - (validator.missed_blocks / blocks_in_epoch)
                performance = max(0, performance)  # Ensure non-negative
                
                # Calculate final reward
                reward = int(base_reward * stake_ratio * performance)
                
                # Distribute reward (would update state in production)
                logger.debug(f"Validator {validator.address.hex()[:8]} reward: {reward}")
        
        logger.info(f"Distributed rewards for epoch {epoch}")
    
    def _process_slashing(self, epoch: int) -> None:
        """
        Process slashing for misbehaving validators.
        
        Args:
            epoch: Epoch number
        """
        validators_to_slash = []
        
        # Check for validators that missed too many blocks
        for validator in self.validator_set.get_active_validators():
            if validator.missed_blocks >= self.config.max_missed_blocks:
                validators_to_slash.append(validator.address)
                logger.warning(
                    f"Validator {validator.address.hex()[:8]}... missed {validator.missed_blocks} blocks, "
                    f"will be slashed"
                )
        
        # Apply slashing
        for address in validators_to_slash:
            validator = self.validator_set.get_validator(address)
            if validator:
                # Slash stake
                slash_amount = int(validator.stake * self.config.slash_rate)
                validator.stake -= slash_amount
                
                # Jail validator if stake too low
                if validator.stake < self.config.min_stake:
                    self.validator_set.jail_validator(address)
                    logger.warning(f"Validator {address.hex()[:8]}... jailed due to insufficient stake")
                
                # Reset missed blocks counter
                validator.missed_blocks = 0
        
        if validators_to_slash:
            logger.info(f"Slashed {len(validators_to_slash)} validators in epoch {epoch}")
    
    def _update_validator_set(self, epoch: int) -> None:
        """
        Update the active validator set for the next epoch.
        
        Args:
            epoch: Epoch number
        """
        # Select top validators by stake
        selected = self.validator_set.select_validators_for_epoch(self.config.validator_count)
        
        # Update active set
        self.validator_set.active_validators.clear()
        for validator in selected:
            self.validator_set.active_validators.add(validator.address)
        
        logger.info(
            f"Updated validator set for epoch {epoch + 1}: "
            f"{len(selected)} active validators"
        )
    
    def register_validator_from_info(self, validator_info: 'ValidatorInfo') -> bool:
        """
        Register a validator from ValidatorInfo (existing system).
        
        Integrates with existing ModernTensor validator system.
        
        Args:
            validator_info: Validator information from existing system
            
        Returns:
            bool: True if successful
        """
        try:
            # Convert hex UID to address (20 bytes)
            address = bytes.fromhex(validator_info.uid)[:20]
            
            # Use wallet_addr_hash as public key if available
            public_key = validator_info.wallet_addr_hash or address
            if len(public_key) < 32:
                # Pad to 32 bytes if needed
                public_key = public_key + b'\x00' * (32 - len(public_key))
            
            # Add to validator set
            return self.validator_set.add_validator(
                address=address,
                public_key=public_key[:32],
                stake=int(validator_info.stake),
            )
        except Exception as e:
            logger.error(f"Failed to register validator from info: {e}")
            return False
    
    def sync_validators_from_state(self, validators_info: Dict[str, 'ValidatorInfo']) -> int:
        """
        Synchronize validators from existing state system.
        
        Args:
            validators_info: Dictionary of validator info from existing system
            
        Returns:
            int: Number of validators synchronized
        """
        count = 0
        for uid, info in validators_info.items():
            if info.status == STATUS_ACTIVE:
                if self.register_validator_from_info(info):
                    count += 1
        
        logger.info(f"Synchronized {count} validators from existing state")
        return count

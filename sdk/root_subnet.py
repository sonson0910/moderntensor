"""
Root Subnet (Subnet 0) Management

This module implements the Root Subnet functionality for ModernTensor,
managing all registered subnets and root validator weight voting.

Synced with Luxtensor Rust backend:
- luxtensor-core/src/subnet.rs
- luxtensor-node/src/root_subnet.rs
"""

import logging
from typing import Dict, List, Optional
from dataclasses import dataclass, field

from .models.subnet import (
    SubnetInfo,
    RootConfig,
    RootValidatorInfo,
    SubnetWeights,
    EmissionShare,
    SubnetRegistrationResult,
)

logger = logging.getLogger(__name__)


@dataclass
class RootSubnet:
    """
    Root Subnet (Subnet 0) state management.

    The Root Subnet manages:
    - All registered subnets (netuid 1, 2, 3, ...)
    - Root validators (top 64 stakers)
    - Weight voting for emission distribution
    - Emission share calculation

    Synced with Rust: luxtensor-node/src/root_subnet.rs::RootSubnet
    """

    # Registered subnets: netuid -> SubnetInfo
    subnets: Dict[int, SubnetInfo] = field(default_factory=dict)

    # Root validators (auto-selected from top stakers)
    root_validators: List[RootValidatorInfo] = field(default_factory=list)

    # Weight votes: validator_address -> SubnetWeights
    weight_matrix: Dict[str, SubnetWeights] = field(default_factory=dict)

    # Computed emission shares: netuid -> share (0.0 - 1.0)
    emission_shares: Dict[int, float] = field(default_factory=dict)

    # Configuration
    config: RootConfig = field(default_factory=RootConfig)

    # Current state
    next_netuid: int = 1  # Next available subnet ID
    last_weight_update_block: int = 0

    def register_subnet(
        self,
        name: str,
        owner: str,
        block_number: int,
        metadata: Optional[Dict] = None
    ) -> SubnetRegistrationResult:
        """
        Register a new subnet.

        Args:
            name: Human-readable subnet name
            owner: Owner address
            block_number: Current block number
            metadata: Optional metadata dict

        Returns:
            SubnetRegistrationResult with success status and assigned netuid
        """
        # Check max subnets
        if len(self.subnets) >= self.config.max_subnets:
            return SubnetRegistrationResult(
                success=False,
                error=f"Maximum subnets ({self.config.max_subnets}) reached"
            )

        # Assign netuid
        netuid = self.next_netuid
        self.next_netuid += 1

        # Create subnet info
        subnet = SubnetInfo(
            id=netuid,
            netuid=netuid,
            subnet_uid=netuid,
            name=name,
            owner=owner,
            created_at=block_number,
            active=True,
        )

        # Register
        self.subnets[netuid] = subnet

        # Initialize emission share to 0
        self.emission_shares[netuid] = 0.0

        logger.info(f"Registered subnet {netuid}: {name} (owner: {owner})")

        return SubnetRegistrationResult(
            success=True,
            netuid=netuid,
            cost_burned=self.config.subnet_registration_cost
        )

    def deregister_subnet(self, netuid: int, caller: str) -> bool:
        """
        Deregister a subnet.

        Args:
            netuid: Subnet ID to deregister
            caller: Address attempting deregistration

        Returns:
            True if successful
        """
        if netuid not in self.subnets:
            logger.warning(f"Subnet {netuid} not found")
            return False

        subnet = self.subnets[netuid]
        if subnet.owner != caller:
            logger.warning(f"Caller {caller} is not owner of subnet {netuid}")
            return False

        # Remove subnet
        del self.subnets[netuid]
        if netuid in self.emission_shares:
            del self.emission_shares[netuid]

        # Remove from weight matrix
        for weights in self.weight_matrix.values():
            if netuid in weights.weights:
                del weights.weights[netuid]

        logger.info(f"Deregistered subnet {netuid}")
        return True

    def update_root_validators(self, stakers: List[tuple]) -> None:
        """
        Update root validators based on top stakers.

        Args:
            stakers: List of (address, stake) tuples sorted by stake DESC
        """
        # Take top N stakers
        top_stakers = stakers[:self.config.max_root_validators]

        # Filter by minimum stake
        self.root_validators = []
        for rank, (address, stake) in enumerate(top_stakers, 1):
            if stake >= self.config.min_stake_for_root:
                self.root_validators.append(RootValidatorInfo(
                    address=address,
                    stake=stake,
                    rank=rank,
                    is_active=True
                ))

        logger.info(f"Updated root validators: {len(self.root_validators)} validators")

    def is_root_validator(self, address: str) -> bool:
        """Check if address is a root validator."""
        return any(v.address.lower() == address.lower() for v in self.root_validators)

    def set_weights(
        self,
        validator: str,
        weights: Dict[int, float],
        block_number: int
    ) -> bool:
        """
        Set subnet weights for a root validator.

        Args:
            validator: Validator address
            weights: Dict of netuid -> weight (must sum to <= 1.0)
            block_number: Current block number

        Returns:
            True if successful
        """
        if not self.is_root_validator(validator):
            logger.warning(f"{validator} is not a root validator")
            return False

        # Validate weights
        if sum(weights.values()) > 1.0 + 1e-6:  # Allow small floating point error
            logger.warning(f"Weights sum to {sum(weights.values())}, must be <= 1.0")
            return False

        # Validate netuids exist
        for netuid in weights.keys():
            if netuid not in self.subnets:
                logger.warning(f"Subnet {netuid} does not exist")
                return False

        # Store weights
        self.weight_matrix[validator] = SubnetWeights(
            validator=validator,
            weights=weights,
            block_updated=block_number
        )

        # Recalculate emission shares
        self._calculate_emission_shares()

        logger.info(f"Updated weights for {validator}: {weights}")
        return True

    def _calculate_emission_shares(self) -> None:
        """
        Calculate emission shares based on stake-weighted votes.

        Formula: share[subnet] = Σ(validator_stake × validator_vote[subnet]) / total_stake
        """
        if not self.root_validators or not self.weight_matrix:
            return

        # Get total stake
        total_stake = sum(v.stake for v in self.root_validators)
        if total_stake == 0:
            return

        # Calculate weighted average
        shares: Dict[int, float] = {}
        for netuid in self.subnets.keys():
            weighted_sum = 0.0
            for validator in self.root_validators:
                if validator.address in self.weight_matrix:
                    vote = self.weight_matrix[validator.address].weights.get(netuid, 0.0)
                    weighted_sum += validator.stake * vote
            shares[netuid] = weighted_sum / total_stake

        # Normalize to sum to 1.0
        total_shares = sum(shares.values())
        if total_shares > 0:
            self.emission_shares = {k: v / total_shares for k, v in shares.items()}
        else:
            self.emission_shares = {k: 0.0 for k in shares.keys()}

    def get_emission_distribution(self, total_emission: int) -> List[EmissionShare]:
        """
        Get emission distribution for all subnets.

        Args:
            total_emission: Total emission amount for this epoch

        Returns:
            List of EmissionShare with amounts
        """
        result = []
        for netuid, share in self.emission_shares.items():
            amount = int(total_emission * share)
            result.append(EmissionShare(
                netuid=netuid,
                share=share,
                amount=amount
            ))
        return result

    def get_subnet(self, netuid: int) -> Optional[SubnetInfo]:
        """Get subnet info by netuid."""
        return self.subnets.get(netuid)

    def get_all_subnets(self) -> List[SubnetInfo]:
        """Get all registered subnets."""
        return list(self.subnets.values())

    def get_root_validator_count(self) -> int:
        """Get number of root validators."""
        return len(self.root_validators)

    def to_dict(self) -> Dict:
        """Serialize to dict."""
        return {
            "subnets": {k: v.model_dump() for k, v in self.subnets.items()},
            "root_validators": [v.model_dump() for v in self.root_validators],
            "weight_matrix": {k: v.model_dump() for k, v in self.weight_matrix.items()},
            "emission_shares": self.emission_shares,
            "config": self.config.model_dump(),
            "next_netuid": self.next_netuid,
        }

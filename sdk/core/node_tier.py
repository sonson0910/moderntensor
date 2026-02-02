"""
Node Tier System for Progressive Staking (Model C).

Matches luxtensor-consensus/src/node_tier.rs for SDK compatibility.

4-Tier System:
    Tier 0: Light Node  - No stake, tx relay fees only
    Tier 1: Full Node   - 10 MDT, 2% infrastructure emission
    Tier 2: Validator   - 100 MDT, 28% validator emission
    Tier 3: Super Validator - 1000 MDT, priority fees + delegation
"""

from dataclasses import dataclass, field
from enum import Enum, IntEnum
from typing import Dict, List, Optional, Tuple
from decimal import Decimal
import time


# Minimum stake requirements (in base units - 18 decimals)
MIN_STAKE_LIGHT_NODE = 0
MIN_STAKE_FULL_NODE = 10 * 10**18        # 10 MDT
MIN_STAKE_VALIDATOR = 100 * 10**18       # 100 MDT
MIN_STAKE_SUPER_VALIDATOR = 1000 * 10**18  # 1000 MDT


class NodeTier(IntEnum):
    """Node tier levels - matches Rust NodeTier enum."""
    LIGHT_NODE = 0       # Tier 0: Light client, tx relay only
    FULL_NODE = 1        # Tier 1: Full node, infrastructure provider
    VALIDATOR = 2        # Tier 2: Validator, AI quality validation
    SUPER_VALIDATOR = 3  # Tier 3: Super validator, block production priority

    @property
    def min_stake(self) -> int:
        """Get minimum stake for this tier."""
        return {
            NodeTier.LIGHT_NODE: MIN_STAKE_LIGHT_NODE,
            NodeTier.FULL_NODE: MIN_STAKE_FULL_NODE,
            NodeTier.VALIDATOR: MIN_STAKE_VALIDATOR,
            NodeTier.SUPER_VALIDATOR: MIN_STAKE_SUPER_VALIDATOR,
        }[self]

    @classmethod
    def from_stake(cls, stake: int) -> 'NodeTier':
        """Determine tier from stake amount."""
        if stake >= MIN_STAKE_SUPER_VALIDATOR:
            return cls.SUPER_VALIDATOR
        elif stake >= MIN_STAKE_VALIDATOR:
            return cls.VALIDATOR
        elif stake >= MIN_STAKE_FULL_NODE:
            return cls.FULL_NODE
        else:
            return cls.LIGHT_NODE

    @property
    def emission_share(self) -> float:
        """Get emission share for this tier."""
        return {
            NodeTier.LIGHT_NODE: 0.0,       # No emission, only tx fee relay
            NodeTier.FULL_NODE: 0.02,       # 2% infrastructure
            NodeTier.VALIDATOR: 0.28,       # 28% validator
            NodeTier.SUPER_VALIDATOR: 0.28, # Same + priority fees
        }[self]

    @property
    def name(self) -> str:
        """Get tier display name."""
        return {
            NodeTier.LIGHT_NODE: "Light Node",
            NodeTier.FULL_NODE: "Full Node",
            NodeTier.VALIDATOR: "Validator",
            NodeTier.SUPER_VALIDATOR: "Super Validator",
        }[self]

    @property
    def can_produce_blocks(self) -> bool:
        """Check if this tier can produce blocks."""
        return self >= NodeTier.VALIDATOR

    @property
    def receives_infrastructure_rewards(self) -> bool:
        """Check if this tier receives infrastructure rewards."""
        return self >= NodeTier.FULL_NODE

    @property
    def receives_validator_rewards(self) -> bool:
        """Check if this tier receives validator rewards."""
        return self >= NodeTier.VALIDATOR


@dataclass
class NodeInfo:
    """Registered node information - matches Rust NodeInfo."""
    address: str  # 0x prefixed address
    stake: int
    tier: NodeTier
    registered_at: int  # block height
    blocks_produced: int = 0
    last_block: int = 0
    tx_relayed: int = 0
    uptime_score: float = 1.0

    @classmethod
    def new(cls, address: str, stake: int, block_height: int) -> 'NodeInfo':
        """Create new NodeInfo."""
        return cls(
            address=address,
            stake=stake,
            tier=NodeTier.from_stake(stake),
            registered_at=block_height,
        )

    def update_stake(self, new_stake: int) -> NodeTier:
        """Update stake and recalculate tier."""
        self.stake = new_stake
        self.tier = NodeTier.from_stake(new_stake)
        return self.tier

    def record_block(self, block_height: int):
        """Record block production."""
        self.blocks_produced += 1
        self.last_block = block_height

    def record_tx_relay(self, count: int = 1):
        """Record transaction relay."""
        self.tx_relayed += count

    def update_uptime(self, score: float):
        """Update uptime score (0.0 - 1.0)."""
        self.uptime_score = max(0.0, min(1.0, score))

    def to_dict(self) -> Dict:
        """Convert to dictionary."""
        return {
            "address": self.address,
            "stake": str(self.stake),
            "stake_mdt": str(Decimal(self.stake) / Decimal(10**18)),
            "tier": self.tier.name,
            "tier_level": int(self.tier),
            "registered_at": self.registered_at,
            "blocks_produced": self.blocks_produced,
            "last_block": self.last_block,
            "tx_relayed": self.tx_relayed,
            "uptime_score": self.uptime_score,
            "can_produce_blocks": self.tier.can_produce_blocks,
            "receives_infrastructure_rewards": self.tier.receives_infrastructure_rewards,
            "receives_validator_rewards": self.tier.receives_validator_rewards,
        }


class NodeRegistry:
    """
    Node Registry - tracks all registered nodes.

    Matches luxtensor-consensus NodeRegistry struct.
    """

    def __init__(self):
        self._nodes: Dict[str, NodeInfo] = {}

    def register(self, address: str, stake: int, block_height: int) -> Tuple[NodeTier, Optional[str]]:
        """
        Register a new node.

        Args:
            address: Node address
            stake: Initial stake amount
            block_height: Current block height

        Returns:
            Tuple of (tier, error_message)
        """
        address = address.lower()
        if address in self._nodes:
            return (NodeTier.LIGHT_NODE, "Node already registered")

        node = NodeInfo.new(address, stake, block_height)
        self._nodes[address] = node
        return (node.tier, None)

    def update_stake(self, address: str, new_stake: int) -> Optional[NodeTier]:
        """Update node stake and return new tier."""
        address = address.lower()
        if address not in self._nodes:
            return None

        old_tier = self._nodes[address].tier
        new_tier = self._nodes[address].update_stake(new_stake)
        return new_tier

    def unregister(self, address: str) -> Optional[NodeInfo]:
        """Unregister a node."""
        address = address.lower()
        return self._nodes.pop(address, None)

    def get(self, address: str) -> Optional[NodeInfo]:
        """Get node info."""
        return self._nodes.get(address.lower())

    def get_tier(self, address: str) -> Optional[NodeTier]:
        """Get node tier."""
        node = self.get(address)
        return node.tier if node else None

    def get_by_tier(self, tier: NodeTier) -> List[NodeInfo]:
        """Get all nodes in a tier."""
        return [n for n in self._nodes.values() if n.tier == tier]

    def get_infrastructure_nodes(self) -> List[NodeInfo]:
        """Get all full nodes (infrastructure providers)."""
        return [n for n in self._nodes.values() if n.tier >= NodeTier.FULL_NODE]

    def get_validators(self) -> List[NodeInfo]:
        """Get all validators."""
        return [n for n in self._nodes.values() if n.tier >= NodeTier.VALIDATOR]

    def get_super_validators(self) -> List[NodeInfo]:
        """Get super validators only."""
        return self.get_by_tier(NodeTier.SUPER_VALIDATOR)

    def count_by_tier(self) -> Dict[NodeTier, int]:
        """Count nodes by tier."""
        counts = {tier: 0 for tier in NodeTier}
        for node in self._nodes.values():
            counts[node.tier] += 1
        return counts

    @property
    def total_nodes(self) -> int:
        """Total registered nodes."""
        return len(self._nodes)

    @property
    def total_stake(self) -> int:
        """Total stake across all nodes."""
        return sum(n.stake for n in self._nodes.values())

    def record_block_production(self, address: str, block_height: int):
        """Record block production for a node."""
        node = self.get(address)
        if node:
            node.record_block(block_height)


# Node tier configuration
@dataclass
class NodeTierConfig:
    """
    Configuration for progressive staking tiers.

    Matches SDK tokenomics NodeTierConfig for consistency.
    """
    # Minimum stake requirements
    light_node_stake: int = MIN_STAKE_LIGHT_NODE
    full_node_stake: int = MIN_STAKE_FULL_NODE
    validator_stake: int = MIN_STAKE_VALIDATOR
    super_validator_stake: int = MIN_STAKE_SUPER_VALIDATOR

    # Emission shares
    light_node_emission_share: float = 0.0
    full_node_emission_share: float = 0.02
    validator_emission_share: float = 0.28
    super_validator_emission_share: float = 0.28

    # Tier benefits
    light_node_tx_fee_share: float = 0.005  # 0.5% of relayed tx fees
    full_node_block_proposal: bool = False
    validator_block_proposal: bool = True
    super_validator_priority: bool = True

    def get_tier_from_stake(self, stake: int) -> NodeTier:
        """Get tier from stake amount."""
        return NodeTier.from_stake(stake)

    def get_min_stake_for_tier(self, tier: NodeTier) -> int:
        """Get minimum stake for a tier."""
        return tier.min_stake


# Module exports
__all__ = [
    "NodeTier",
    "NodeInfo",
    "NodeRegistry",
    "NodeTierConfig",
    "MIN_STAKE_LIGHT_NODE",
    "MIN_STAKE_FULL_NODE",
    "MIN_STAKE_VALIDATOR",
    "MIN_STAKE_SUPER_VALIDATOR",
]

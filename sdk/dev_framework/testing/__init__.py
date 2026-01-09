"""
Testing Utilities

Provides mock objects and testing harness for subnet development.
"""

import logging
from typing import Optional, Dict, Any, List
from unittest.mock import Mock


logger = logging.getLogger(__name__)


class MockClient:
    """
    Mock blockchain client for testing.
    
    Simulates LuxtensorClient behavior without requiring
    a real blockchain connection.
    
    Example:
        ```python
        from sdk.dev_framework import MockClient
        
        client = MockClient()
        client.set_block_number(12345)
        
        neuron = client.get_neuron(0, 1)
        print(neuron.stake)
        ```
    """
    
    def __init__(self):
        """Initialize mock client."""
        self._block_number = 0
        self._neurons: Dict[tuple, Any] = {}
        self._subnets: Dict[int, Any] = {}
        self._balances: Dict[str, float] = {}
        
        logger.info("Initialized MockClient")
    
    def set_block_number(self, block_number: int):
        """Set current block number."""
        self._block_number = block_number
    
    def get_block_number(self) -> int:
        """Get current block number."""
        return self._block_number
    
    def add_neuron(self, uid: int, netuid: int, **kwargs):
        """Add a mock neuron."""
        from sdk.models import NeuronInfo
        
        neuron = NeuronInfo(
            uid=uid,
            hotkey=kwargs.get("hotkey", f"hotkey_{uid}"),
            coldkey=kwargs.get("coldkey", f"coldkey_{uid}"),
            subnet_uid=netuid,
            stake=kwargs.get("stake", 0.0),
            **kwargs
        )
        
        self._neurons[(uid, netuid)] = neuron
    
    def get_neuron(self, uid: int, netuid: int):
        """Get mock neuron."""
        return self._neurons.get((uid, netuid))
    
    def get_neurons(self, netuid: int) -> List[Any]:
        """Get all neurons in subnet."""
        return [
            n for (u, net), n in self._neurons.items()
            if net == netuid
        ]
    
    def add_subnet(self, netuid: int, **kwargs):
        """Add a mock subnet."""
        from sdk.models import SubnetInfo
        
        subnet = SubnetInfo(
            subnet_uid=netuid,
            netuid=netuid,
            owner=kwargs.get("owner", f"owner_{netuid}"),
            **kwargs
        )
        
        self._subnets[netuid] = subnet
    
    def get_subnet_info(self, netuid: int):
        """Get mock subnet info."""
        return self._subnets.get(netuid)
    
    def set_balance(self, address: str, balance: float):
        """Set account balance."""
        self._balances[address] = balance
    
    def get_balance(self, address: str) -> float:
        """Get account balance."""
        return self._balances.get(address, 0.0)
    
    def __repr__(self) -> str:
        return f"MockClient(block={self._block_number})"


class TestHarness:
    """
    Test harness for subnet development.
    
    Provides utilities for testing subnet logic with mock data.
    
    Example:
        ```python
        from sdk.dev_framework import TestHarness
        
        harness = TestHarness()
        harness.setup_subnet(netuid=1, n_validators=5, n_miners=20)
        
        # Test subnet logic
        results = harness.simulate_epoch()
        ```
    """
    
    def __init__(self):
        """Initialize test harness."""
        self.client = MockClient()
        self.subnet_uid: Optional[int] = None
        
        logger.info("Initialized TestHarness")
    
    def setup_subnet(
        self,
        netuid: int = 1,
        n_validators: int = 5,
        n_miners: int = 20,
    ):
        """
        Setup a test subnet with validators and miners.
        
        Args:
            netuid: Subnet UID
            n_validators: Number of validators
            n_miners: Number of miners
        """
        self.subnet_uid = netuid
        
        # Create subnet
        self.client.add_subnet(
            netuid,
            name=f"Test Subnet {netuid}",
            n=n_validators + n_miners,
            max_n=4096
        )
        
        # Create validators
        for i in range(n_validators):
            self.client.add_neuron(
                uid=i,
                netuid=netuid,
                hotkey=f"validator_hotkey_{i}",
                coldkey=f"validator_coldkey_{i}",
                stake=1000.0 * (i + 1),
                validator_permit=True,
                active=True
            )
        
        # Create miners
        for i in range(n_miners):
            uid = n_validators + i
            self.client.add_neuron(
                uid=uid,
                netuid=netuid,
                hotkey=f"miner_hotkey_{i}",
                coldkey=f"miner_coldkey_{i}",
                stake=100.0,
                validator_permit=False,
                active=True
            )
        
        logger.info(
            f"Setup test subnet {netuid} with "
            f"{n_validators} validators and {n_miners} miners"
        )
    
    def simulate_epoch(self) -> Dict[str, Any]:
        """
        Simulate an epoch (validation round).
        
        Returns:
            Dictionary with simulation results
        """
        if self.subnet_uid is None:
            raise ValueError("Subnet not setup. Call setup_subnet() first.")
        
        validators = [
            n for n in self.client.get_neurons(self.subnet_uid)
            if n.validator_permit
        ]
        
        miners = [
            n for n in self.client.get_neurons(self.subnet_uid)
            if not n.validator_permit
        ]
        
        logger.info(
            f"Simulating epoch: {len(validators)} validators, "
            f"{len(miners)} miners"
        )
        
        return {
            "subnet_uid": self.subnet_uid,
            "validators": len(validators),
            "miners": len(miners),
            "status": "completed"
        }
    
    def __repr__(self) -> str:
        return f"TestHarness(subnet={self.subnet_uid})"


__all__ = [
    "MockClient",
    "TestHarness",
]

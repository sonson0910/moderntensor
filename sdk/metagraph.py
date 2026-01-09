"""
Unified Metagraph Interface

Provides a unified interface for accessing and managing network state
from the ModernTensor blockchain (Luxtensor).
"""

import logging
from typing import List, Dict, Optional, Any
import numpy as np
from datetime import datetime

from sdk.luxtensor_client import LuxtensorClient
from sdk.models import (
    NeuronInfo,
    SubnetInfo,
    StakeInfo,
)


logger = logging.getLogger(__name__)


class Metagraph:
    """
    Unified network state interface for ModernTensor.
    
    The Metagraph class provides a convenient interface to access
    the current state of a subnet in the ModernTensor network,
    including neurons, weights, stakes, and other network parameters.
    
    Example:
        ```python
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.metagraph import Metagraph
        
        client = LuxtensorClient("http://localhost:9933")
        metagraph = Metagraph(client, subnet_uid=1)
        
        # Sync from blockchain
        metagraph.sync()
        
        # Access network state
        neurons = metagraph.get_neurons()
        weights = metagraph.get_weights()
        print(f"Subnet has {len(neurons)} neurons")
        ```
    """
    
    def __init__(
        self,
        client: LuxtensorClient,
        subnet_uid: int,
        cache_ttl: int = 60,
    ):
        """
        Initialize Metagraph for a specific subnet.
        
        Args:
            client: LuxtensorClient instance for blockchain queries
            subnet_uid: Subnet UID to track
            cache_ttl: Cache time-to-live in seconds (default: 60)
        """
        self.client = client
        self.subnet_uid = subnet_uid
        self.cache_ttl = cache_ttl
        
        # Cached data
        self._neurons: List[NeuronInfo] = []
        self._subnet_info: Optional[SubnetInfo] = None
        self._weights: Optional[np.ndarray] = None
        self._stake_distribution: Dict[str, float] = {}
        self._last_sync: Optional[datetime] = None
        
        # State tracking
        self._synced = False
        self._version = 0
        
        logger.info(f"Initialized Metagraph for subnet {subnet_uid}")
    
    def sync(self, force: bool = False) -> None:
        """
        Synchronize metagraph state from blockchain.
        
        Args:
            force: Force sync even if cache is valid
        """
        # Check cache validity
        if not force and self._is_cache_valid():
            logger.debug("Using cached metagraph data")
            return
        
        logger.info(f"Syncing metagraph for subnet {self.subnet_uid}")
        
        try:
            # Fetch subnet info
            self._subnet_info = self.client.get_subnet_info(self.subnet_uid)
            
            # Fetch all neurons in subnet
            self._neurons = self.client.get_neurons(self.subnet_uid)
            
            # Fetch weight matrix
            self._weights = self._fetch_weight_matrix()
            
            # Calculate stake distribution
            self._stake_distribution = self._calculate_stake_distribution()
            
            # Update sync state
            self._last_sync = datetime.now()
            self._synced = True
            self._version += 1
            
            logger.info(
                f"Synced metagraph: {len(self._neurons)} neurons, "
                f"version {self._version}"
            )
            
        except Exception as e:
            logger.error(f"Failed to sync metagraph: {e}")
            raise
    
    def get_neurons(self, refresh: bool = False) -> List[NeuronInfo]:
        """
        Get list of all neurons in the subnet.
        
        Args:
            refresh: Force refresh from blockchain
            
        Returns:
            List of NeuronInfo objects
        """
        if refresh or not self._synced:
            self.sync()
        return self._neurons.copy()
    
    def get_neuron(self, uid: int, refresh: bool = False) -> Optional[NeuronInfo]:
        """
        Get specific neuron by UID.
        
        Args:
            uid: Neuron UID
            refresh: Force refresh from blockchain
            
        Returns:
            NeuronInfo object or None if not found
        """
        neurons = self.get_neurons(refresh=refresh)
        for neuron in neurons:
            if neuron.uid == uid:
                return neuron
        return None
    
    def get_weights(self, refresh: bool = False) -> np.ndarray:
        """
        Get weight matrix for the subnet.
        
        The weight matrix is a 2D numpy array where:
        - Rows represent validators (sources)
        - Columns represent miners (targets)
        - Values are normalized weights [0, 1]
        
        Args:
            refresh: Force refresh from blockchain
            
        Returns:
            Weight matrix as numpy array
        """
        if refresh or not self._synced:
            self.sync()
        return self._weights.copy() if self._weights is not None else np.array([])
    
    def get_stake_distribution(self, refresh: bool = False) -> Dict[str, float]:
        """
        Get stake distribution across neurons.
        
        Args:
            refresh: Force refresh from blockchain
            
        Returns:
            Dictionary mapping hotkey to stake amount
        """
        if refresh or not self._synced:
            self.sync()
        return self._stake_distribution.copy()
    
    def get_subnet_info(self, refresh: bool = False) -> Optional[SubnetInfo]:
        """
        Get subnet metadata and configuration.
        
        Args:
            refresh: Force refresh from blockchain
            
        Returns:
            SubnetInfo object
        """
        if refresh or not self._synced:
            self.sync()
        return self._subnet_info
    
    def get_total_stake(self) -> float:
        """
        Get total stake in the subnet.
        
        Returns:
            Total stake amount
        """
        return sum(self._stake_distribution.values())
    
    def get_validators(self, min_stake: float = 0.0) -> List[NeuronInfo]:
        """
        Get list of validators in the subnet.
        
        Args:
            min_stake: Minimum stake required to be considered a validator
            
        Returns:
            List of validator NeuronInfo objects
        """
        neurons = self.get_neurons()
        return [
            n for n in neurons
            if n.validator_permit and n.stake >= min_stake
        ]
    
    def get_miners(self) -> List[NeuronInfo]:
        """
        Get list of miners in the subnet.
        
        Returns:
            List of miner NeuronInfo objects
        """
        neurons = self.get_neurons()
        return [n for n in neurons if not n.validator_permit]
    
    def get_top_neurons(self, n: int = 10, by: str = "stake") -> List[NeuronInfo]:
        """
        Get top N neurons sorted by a metric.
        
        Args:
            n: Number of neurons to return
            by: Metric to sort by ('stake', 'emission', 'rank', 'trust')
            
        Returns:
            List of top NeuronInfo objects
        """
        neurons = self.get_neurons()
        
        if by not in ['stake', 'emission', 'rank', 'trust', 'incentive']:
            raise ValueError(f"Invalid sort metric: {by}")
        
        sorted_neurons = sorted(
            neurons,
            key=lambda x: getattr(x, by),
            reverse=True
        )
        
        return sorted_neurons[:n]
    
    def is_synced(self) -> bool:
        """Check if metagraph is synced with blockchain."""
        return self._synced
    
    def get_version(self) -> int:
        """Get current metagraph version (increments on each sync)."""
        return self._version
    
    def get_last_sync_time(self) -> Optional[datetime]:
        """Get timestamp of last successful sync."""
        return self._last_sync
    
    # Private methods
    
    def _is_cache_valid(self) -> bool:
        """Check if cached data is still valid."""
        if not self._synced or self._last_sync is None:
            return False
        
        elapsed = (datetime.now() - self._last_sync).total_seconds()
        return elapsed < self.cache_ttl
    
    def _fetch_weight_matrix(self) -> np.ndarray:
        """
        Fetch and construct weight matrix from blockchain.
        
        Returns:
            Weight matrix as numpy array
        """
        try:
            n_neurons = len(self._neurons)
            
            if n_neurons == 0:
                return np.array([])
            
            # Initialize weight matrix
            weights = np.zeros((n_neurons, n_neurons), dtype=np.float32)
            
            # Fetch weights for each validator
            for i, neuron in enumerate(self._neurons):
                if neuron.validator_permit:
                    try:
                        # Query weights set by this validator
                        neuron_weights = self.client.get_weights(
                            self.subnet_uid,
                            neuron.uid
                        )
                        
                        if neuron_weights:
                            # Populate row in weight matrix
                            for uid, weight in neuron_weights.items():
                                if uid < n_neurons:
                                    weights[i, uid] = weight
                    except Exception as e:
                        logger.warning(
                            f"Failed to fetch weights for neuron {neuron.uid}: {e}"
                        )
            
            return weights
            
        except Exception as e:
            logger.error(f"Failed to construct weight matrix: {e}")
            return np.array([])
    
    def _calculate_stake_distribution(self) -> Dict[str, float]:
        """
        Calculate stake distribution from neurons.
        
        Returns:
            Dictionary mapping hotkey to stake
        """
        distribution = {}
        
        for neuron in self._neurons:
            distribution[neuron.hotkey] = neuron.total_stake
        
        return distribution
    
    def __str__(self) -> str:
        return (
            f"Metagraph(subnet={self.subnet_uid}, "
            f"neurons={len(self._neurons)}, "
            f"synced={self._synced}, "
            f"version={self._version})"
        )
    
    def __repr__(self) -> str:
        return self.__str__()

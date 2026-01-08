Metagraph API
=============

The Metagraph represents the network state and topology in ModernTensor. It provides access to subnet information, neuron (node) details, stake distributions, weights, and consensus data stored on the Luxtensor blockchain.

Overview
--------

The Metagraph provides:

- **Network Topology**: Complete view of subnet structure and participants
- **Neuron Information**: Details about miners and validators (stake, trust, performance)
- **Weight Matrices**: Consensus weights set by validators
- **Stake Distribution**: Token stakes across all participants
- **Aggregated State**: Efficient subnet-wide metrics from blockchain
- **Real-time Sync**: Stay updated with blockchain state
- **Query Interface**: Efficient data retrieval and filtering

Architecture
------------

The Metagraph consists of several components:

1. **Metagraph Data (metagraph_data.py)**: Fetch and decode data from blockchain
2. **Aggregated State (aggregated_state.py)**: Subnet-level aggregated metrics
3. **Metagraph Datum (metagraph_datum.py)**: Data models for miners, validators, subnets
4. **Metagraph API (metagraph_api.py)**: High-level query interface
5. **Metagraph Utils (metagraph_utils.py)**: Utility functions

Data Models
-----------

MinerDatum
~~~~~~~~~~

Represents miner information on the blockchain:

.. code-block:: python

   @dataclass
   class MinerDatum:
       uid: bytes                    # Unique miner identifier
       hotkey: bytes                 # Miner's hotkey address
       stake: int                    # Staked tokens (scaled)
       trust: int                    # Trust score (scaled)
       consensus: int                # Consensus weight (scaled)
       incentive: int                # Incentive score (scaled)
       dividends: int                # Dividend earnings (scaled)
       emission: int                 # Token emissions (scaled)
       vtrust: int                   # Validator trust (scaled)
       last_update: int              # Last update slot
       validator_permit: bool        # Has validator permit
       weights: List[Tuple[int, int]]  # Weight assignments [(uid, weight)]
       bonds: List[Tuple[int, int]]    # Bond connections [(uid, bond)]
       active: bool                  # Currently active

ValidatorDatum
~~~~~~~~~~~~~~

Represents validator information:

.. code-block:: python

   @dataclass
   class ValidatorDatum:
       uid: bytes                    # Unique validator identifier
       hotkey: bytes                 # Validator's hotkey address
       stake: int                    # Staked tokens (scaled)
       trust: int                    # Trust score (scaled)
       validator_trust: int          # Validator-specific trust (scaled)
       consensus: int                # Consensus weight (scaled)
       incentive: int                # Incentive score (scaled)
       dividends: int                # Dividend earnings (scaled)
       emission: int                 # Token emissions (scaled)
       last_update: int              # Last update slot
       weights_version: int          # Weight matrix version
       weights: List[Tuple[int, int]]  # Weight assignments
       active: bool                  # Currently active

SubnetAggregatedDatum
~~~~~~~~~~~~~~~~~~~~~

Aggregated subnet-level state (more efficient than querying individual UTXOs):

.. code-block:: python

   @dataclass
   class SubnetAggregatedDatum:
       # Identification
       subnet_uid: int
       current_epoch: int
       
       # Participant counts
       total_miners: int
       total_validators: int
       active_miners: int
       active_validators: int
       
       # Economic metrics
       total_stake: int
       total_miner_stake: int
       total_validator_stake: int
       
       # Consensus data (off-chain with hash on-chain)
       weight_matrix_hash: bytes
       consensus_scores_root: bytes
       emission_schedule_root: bytes
       
       # Economic data
       total_emission_this_epoch: int
       miner_reward_pool: int
       validator_reward_pool: int
       
       # Performance metrics (scaled)
       scaled_avg_miner_performance: int
       scaled_avg_validator_performance: int
       scaled_subnet_performance: int
       
       # Update tracking
       last_update_slot: int
       last_consensus_slot: int
       last_emission_slot: int

Basic Usage
-----------

Fetching Miner Data
~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.metagraph import get_all_miner_data
   from sdk.compat.pycardano import BlockFrostChainContext, ScriptHash, Network
   
   async def fetch_miners():
       # Connect to blockchain
       context = BlockFrostChainContext(
           project_id="your_blockfrost_project_id",
           network=Network.TESTNET
       )
       
       # Script hash of miner contract
       script_hash = ScriptHash.from_primitive(
           bytes.fromhex("your_script_hash_here")
       )
       
       # Fetch all miner data
       miner_data = await get_all_miner_data(
           context=context,
           script_hash=script_hash,
           network=Network.TESTNET
       )
       
       # Process miner data
       for utxo, miner_dict in miner_data:
           print(f"Miner UID: {miner_dict['uid']}")
           print(f"  Hotkey: {miner_dict['hotkey']}")
           print(f"  Stake: {miner_dict['stake']}")
           print(f"  Active: {miner_dict['active']}")

Fetching Validator Data
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.metagraph import get_all_validator_data
   
   async def fetch_validators():
       context = BlockFrostChainContext(
           project_id="your_project_id",
           network=Network.TESTNET
       )
       
       script_hash = ScriptHash.from_primitive(
           bytes.fromhex("validator_script_hash")
       )
       
       # Fetch all validator data
       validator_data = await get_all_validator_data(
           context=context,
           script_hash=script_hash,
           network=Network.TESTNET
       )
       
       for utxo, validator_dict in validator_data:
           print(f"Validator UID: {validator_dict['uid']}")
           print(f"  Stake: {validator_dict['stake']}")
           print(f"  Weights: {validator_dict['weights']}")

Fetching Aggregated Subnet State
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   from sdk.metagraph import get_subnet_aggregated_data
   
   async def fetch_subnet_state():
       context = BlockFrostChainContext(
           project_id="your_project_id",
           network=Network.TESTNET
       )
       
       script_hash = ScriptHash.from_primitive(
           bytes.fromhex("subnet_script_hash")
       )
       
       # Fetch aggregated subnet data (single UTXO query)
       subnet_state = await get_subnet_aggregated_data(
           context=context,
           script_hash=script_hash,
           network=Network.TESTNET
       )
       
       if subnet_state:
           utxo, state_dict = subnet_state
           print(f"Subnet UID: {state_dict['subnet_uid']}")
           print(f"Total Miners: {state_dict['total_miners']}")
           print(f"Active Miners: {state_dict['active_miners']}")
           print(f"Total Stake: {state_dict['total_stake']}")
           print(f"Current Epoch: {state_dict['current_epoch']}")

Advanced Usage
--------------

Filtering Active Participants
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   async def get_active_miners():
       miner_data = await get_all_miner_data(context, script_hash, network)
       
       # Filter only active miners
       active_miners = [
           (utxo, miner)
           for utxo, miner in miner_data
           if miner.get('active', False)
       ]
       
       print(f"Found {len(active_miners)} active miners")
       return active_miners

Sorting by Stake
~~~~~~~~~~~~~~~~

.. code-block:: python

   async def get_top_staked_miners(limit=10):
       miner_data = await get_all_miner_data(context, script_hash, network)
       
       # Sort by stake (descending)
       sorted_miners = sorted(
           miner_data,
           key=lambda x: x[1].get('stake', 0),
           reverse=True
       )
       
       # Get top N miners
       top_miners = sorted_miners[:limit]
       
       for utxo, miner in top_miners:
           print(f"UID: {miner['uid']}, Stake: {miner['stake']}")
       
       return top_miners

Analyzing Weight Matrix
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   async def analyze_weights():
       validator_data = await get_all_validator_data(
           context, script_hash, network
       )
       
       # Extract weight assignments
       weight_matrix = {}
       
       for utxo, validator in validator_data:
           validator_uid = validator['uid']
           weights = validator.get('weights', [])
           
           weight_matrix[validator_uid] = {}
           for uid, weight in weights:
               weight_matrix[validator_uid][uid] = weight
       
       # Analyze consensus
       for val_uid, weights in weight_matrix.items():
           total_weight = sum(weights.values())
           print(f"Validator {val_uid}: {len(weights)} assignments, "
                 f"total weight: {total_weight}")

Building Network Graph
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   import networkx as nx
   
   async def build_network_graph():
       """Build network topology graph."""
       miner_data = await get_all_miner_data(context, script_hash, network)
       validator_data = await get_all_validator_data(
           context, script_hash, network
       )
       
       # Create directed graph
       G = nx.DiGraph()
       
       # Add miner nodes
       for utxo, miner in miner_data:
           uid = miner['uid']
           G.add_node(
               uid,
               type='miner',
               stake=miner['stake'],
               trust=miner['trust'],
               active=miner['active']
           )
       
       # Add validator nodes
       for utxo, validator in validator_data:
           uid = validator['uid']
           G.add_node(
               uid,
               type='validator',
               stake=validator['stake'],
               trust=validator['trust']
           )
           
           # Add weight edges
           for target_uid, weight in validator.get('weights', []):
               G.add_edge(uid, target_uid, weight=weight)
       
       # Analyze network
       print(f"Nodes: {G.number_of_nodes()}")
       print(f"Edges: {G.number_of_edges()}")
       print(f"Density: {nx.density(G):.4f}")
       
       return G

Computing Statistics
~~~~~~~~~~~~~~~~~~~~

.. code-block:: python

   async def compute_subnet_statistics():
       """Compute comprehensive subnet statistics."""
       miner_data = await get_all_miner_data(context, script_hash, network)
       validator_data = await get_all_validator_data(
           context, script_hash, network
       )
       
       # Miner statistics
       miner_stakes = [m['stake'] for _, m in miner_data]
       active_miners = sum(1 for _, m in miner_data if m['active'])
       
       # Validator statistics
       validator_stakes = [v['stake'] for _, v in validator_data]
       
       stats = {
           'total_miners': len(miner_data),
           'active_miners': active_miners,
           'total_validators': len(validator_data),
           'total_stake': sum(miner_stakes) + sum(validator_stakes),
           'avg_miner_stake': sum(miner_stakes) / len(miner_stakes) if miner_stakes else 0,
           'avg_validator_stake': sum(validator_stakes) / len(validator_stakes) if validator_stakes else 0,
           'max_stake': max(miner_stakes + validator_stakes),
           'min_stake': min(miner_stakes + validator_stakes),
       }
       
       return stats

Production Example: Metagraph Sync Service
-------------------------------------------

.. code-block:: python

   import asyncio
   from sdk.metagraph import get_all_miner_data, get_all_validator_data
   from sdk.compat.pycardano import BlockFrostChainContext, ScriptHash, Network
   import logging
   
   logging.basicConfig(level=logging.INFO)
   logger = logging.getLogger(__name__)
   
   class MetagraphSync:
       """Service to keep metagraph synchronized with blockchain."""
       
       def __init__(self, subnet_id: int, project_id: str):
           self.subnet_id = subnet_id
           self.project_id = project_id
           self.network = Network.TESTNET
           
           # Cached metagraph state
           self.miners = []
           self.validators = []
           self.last_sync = None
           
           # Initialize blockchain connection
           self.context = BlockFrostChainContext(
               project_id=project_id,
               network=self.network
           )
           
           # Script hashes (from config)
           self.miner_script_hash = ScriptHash.from_primitive(
               bytes.fromhex("miner_script_hash_here")
           )
           self.validator_script_hash = ScriptHash.from_primitive(
               bytes.fromhex("validator_script_hash_here")
           )
       
       async def sync(self):
           """Synchronize metagraph with blockchain."""
           logger.info(f"Syncing metagraph for subnet {self.subnet_id}...")
           
           try:
               # Fetch miner data
               self.miners = await get_all_miner_data(
                   context=self.context,
                   script_hash=self.miner_script_hash,
                   network=self.network
               )
               logger.info(f"Fetched {len(self.miners)} miners")
               
               # Fetch validator data
               self.validators = await get_all_validator_data(
                   context=self.context,
                   script_hash=self.validator_script_hash,
                   network=self.network
               )
               logger.info(f"Fetched {len(self.validators)} validators")
               
               self.last_sync = asyncio.get_event_loop().time()
               
               # Compute and log statistics
               stats = self.compute_stats()
               logger.info(f"Subnet stats: {stats}")
               
               return True
               
           except Exception as e:
               logger.error(f"Sync failed: {e}")
               return False
       
       def compute_stats(self):
           """Compute current statistics."""
           active_miners = sum(
               1 for _, m in self.miners if m.get('active', False)
           )
           
           total_stake = sum(
               m.get('stake', 0) for _, m in self.miners
           ) + sum(
               v.get('stake', 0) for _, v in self.validators
           )
           
           return {
               'total_miners': len(self.miners),
               'active_miners': active_miners,
               'total_validators': len(self.validators),
               'total_stake': total_stake
           }
       
       def get_active_miners(self):
           """Get list of active miners."""
           return [
               (utxo, miner)
               for utxo, miner in self.miners
               if miner.get('active', False)
           ]
       
       def get_miner_by_uid(self, uid: str):
           """Get specific miner by UID."""
           for utxo, miner in self.miners:
               if miner.get('uid') == uid:
                   return (utxo, miner)
           return None
       
       async def auto_sync_loop(self, interval: int = 60):
           """Automatically sync at regular intervals."""
           logger.info(f"Starting auto-sync loop (interval: {interval}s)...")
           
           while True:
               try:
                   await self.sync()
                   await asyncio.sleep(interval)
               except Exception as e:
                   logger.error(f"Auto-sync error: {e}")
                   await asyncio.sleep(10)  # Retry sooner on error
   
   # Usage
   async def main():
       metagraph = MetagraphSync(
           subnet_id=1,
           project_id="your_blockfrost_project_id"
       )
       
       # Initial sync
       await metagraph.sync()
       
       # Start auto-sync in background
       asyncio.create_task(metagraph.auto_sync_loop(interval=60))
       
       # Use metagraph data
       while True:
           active_miners = metagraph.get_active_miners()
           print(f"Active miners: {len(active_miners)}")
           await asyncio.sleep(10)
   
   if __name__ == "__main__":
       asyncio.run(main())

API Reference
-------------

.. automodule:: sdk.metagraph.metagraph_data
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.metagraph.aggregated_state
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.metagraph.metagraph_datum
   :members:
   :undoc-members:
   :show-inheritance:

.. automodule:: sdk.metagraph.metagraph_utils
   :members:
   :undoc-members:
   :show-inheritance:

See Also
--------

* :doc:`transactions` - Submit transactions to update metagraph
* :doc:`axon` - Server component for miners
* :doc:`dendrite` - Client component for validators

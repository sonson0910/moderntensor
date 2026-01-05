# examples/layer1_phase1_demo.py
"""
Demo of Layer 1 Phase 1 features: SubnetAggregatedState and WeightMatrixManager.

This example demonstrates:
1. Creating and managing aggregated subnet state
2. Storing and retrieving weight matrices
3. Integration with consensus rounds
4. Verification of consensus integrity
"""

import asyncio
import numpy as np
from dataclasses import dataclass

from sdk.metagraph.aggregated_state import (
    SubnetAggregatedDatum,
    SubnetAggregatedStateManager
)
from sdk.consensus.weight_matrix import WeightMatrixManager
from sdk.consensus.layer1_integration import Layer1ConsensusIntegrator
from sdk.core.datatypes import MinerInfo, ValidatorInfo


@dataclass
class MockMiner:
    """Mock miner for demo purposes."""
    uid: str
    address: str
    stake: float
    last_performance: float
    status: int = 1  # Active


@dataclass
class MockValidator:
    """Mock validator for demo purposes."""
    uid: str
    address: str
    stake: float
    last_performance: float
    status: int = 1  # Active


async def demo_aggregated_state():
    """Demonstrate SubnetAggregatedState functionality."""
    print("=" * 60)
    print("DEMO 1: SubnetAggregatedState Management")
    print("=" * 60)
    
    # Create a state manager
    manager = SubnetAggregatedStateManager()
    
    # Create a new subnet state
    subnet_uid = 1
    current_slot = 10000
    
    print(f"\n1. Creating subnet state for subnet {subnet_uid}...")
    state = manager.create_subnet_state(subnet_uid, current_slot)
    print(f"   ✓ Created empty state at slot {current_slot}")
    print(f"   - Epoch: {state.current_epoch}")
    print(f"   - Total miners: {state.total_miners}")
    print(f"   - Total validators: {state.total_validators}")
    
    # Update participant counts
    print("\n2. Updating participant counts...")
    manager.update_participant_counts(
        subnet_uid=subnet_uid,
        total_miners=50,
        total_validators=20,
        active_miners=48,
        active_validators=19
    )
    state = manager.get_state(subnet_uid)
    print(f"   ✓ Updated counts:")
    print(f"   - Total miners: {state.total_miners}")
    print(f"   - Active miners: {state.active_miners}")
    print(f"   - Total validators: {state.total_validators}")
    print(f"   - Active validators: {state.active_validators}")
    
    # Update economic metrics
    print("\n3. Updating economic metrics...")
    manager.update_economic_metrics(
        subnet_uid=subnet_uid,
        total_stake=10000000,  # 10M tokens
        miner_stake=6000000,   # 6M from miners
        validator_stake=4000000  # 4M from validators
    )
    state = manager.get_state(subnet_uid)
    print(f"   ✓ Updated stakes:")
    print(f"   - Total stake: {state.total_stake:,} tokens")
    print(f"   - Miner stake: {state.total_miner_stake:,} tokens")
    print(f"   - Validator stake: {state.total_validator_stake:,} tokens")
    
    # Update performance metrics
    print("\n4. Updating performance metrics...")
    manager.update_performance_metrics(
        subnet_uid=subnet_uid,
        avg_miner_performance=0.85,
        avg_validator_performance=0.92,
        subnet_performance=0.88
    )
    state = manager.get_state(subnet_uid)
    print(f"   ✓ Updated performance:")
    print(f"   - Avg miner performance: {state.avg_miner_performance:.2f}")
    print(f"   - Avg validator performance: {state.avg_validator_performance:.2f}")
    print(f"   - Subnet performance: {state.subnet_performance:.2f}")
    
    # Increment epoch
    print("\n5. Incrementing epoch...")
    manager.increment_epoch(subnet_uid, current_slot + 1000)
    state = manager.get_state(subnet_uid)
    print(f"   ✓ New epoch: {state.current_epoch}")
    print(f"   ✓ Last update slot: {state.last_update_slot}")
    
    # Get state as dictionary
    print("\n6. Exporting state to dictionary...")
    state_dict = state.to_dict()
    print(f"   ✓ Exported {len(state_dict)} fields")
    print(f"   Sample fields:")
    for key in list(state_dict.keys())[:5]:
        print(f"   - {key}: {state_dict[key]}")


async def demo_weight_matrix():
    """Demonstrate WeightMatrixManager functionality."""
    print("\n" + "=" * 60)
    print("DEMO 2: WeightMatrixManager")
    print("=" * 60)
    
    # Create a weight matrix manager
    manager = WeightMatrixManager()
    
    # Create a dense weight matrix
    print("\n1. Creating and storing dense weight matrix...")
    num_validators = 5
    num_miners = 10
    weights = np.random.rand(num_validators, num_miners)
    
    # Normalize each row to sum to 1.0
    weights = weights / weights.sum(axis=1, keepdims=True)
    
    merkle_root, ipfs_hash = await manager.store_weight_matrix(
        subnet_uid=1,
        epoch=1,
        weights=weights,
        upload_to_ipfs=False
    )
    
    print(f"   ✓ Stored weight matrix ({num_validators}x{num_miners})")
    print(f"   - Merkle root: {merkle_root.hex()[:16]}...")
    print(f"   - IPFS hash: {ipfs_hash}")
    
    # Retrieve the matrix
    print("\n2. Retrieving weight matrix...")
    retrieved = await manager.get_weight_matrix(1, 1)
    print(f"   ✓ Retrieved matrix shape: {retrieved.shape}")
    print(f"   ✓ Matrices match: {np.allclose(weights, retrieved)}")
    
    # Verify the matrix
    print("\n3. Verifying weight matrix integrity...")
    is_valid = await manager.verify_weight_matrix(
        subnet_uid=1,
        epoch=1,
        weights=retrieved,
        merkle_root=merkle_root
    )
    print(f"   ✓ Verification result: {'PASSED' if is_valid else 'FAILED'}")
    
    # Create a sparse matrix
    print("\n4. Creating and storing sparse weight matrix...")
    sparse_weights = np.zeros((20, 50))
    # Add a few non-zero values
    sparse_weights[0, 0] = 1.0
    sparse_weights[5, 10] = 0.8
    sparse_weights[10, 20] = 0.6
    sparse_weights[15, 40] = 0.9
    
    merkle_root2, ipfs_hash2 = await manager.store_weight_matrix(
        subnet_uid=2,
        epoch=1,
        weights=sparse_weights,
        upload_to_ipfs=False
    )
    
    print(f"   ✓ Stored sparse matrix (20x50)")
    print(f"   - Sparsity: {1.0 - np.count_nonzero(sparse_weights) / sparse_weights.size:.1%}")
    
    # Get metadata
    print("\n5. Getting weight matrix metadata...")
    metadata = await manager.get_metadata(2, 1)
    print(f"   ✓ Metadata:")
    print(f"   - Subnet UID: {metadata.subnet_uid}")
    print(f"   - Epoch: {metadata.epoch}")
    print(f"   - Validators: {metadata.num_validators}")
    print(f"   - Miners: {metadata.num_miners}")
    print(f"   - Is sparse: {metadata.is_sparse}")
    print(f"   - Compression ratio: {metadata.compression_ratio:.2f}")
    
    # Get storage statistics
    print("\n6. Getting storage statistics...")
    stats = manager.get_storage_stats()
    print(f"   ✓ Storage stats:")
    print(f"   - Total matrices: {stats['total_matrices']}")
    print(f"   - Total size: {stats['total_size_bytes']:,} bytes")
    print(f"   - Cache size: {stats['cache_size']}")
    print(f"   - Avg compression: {stats['avg_compression_ratio']:.2f}")


async def demo_consensus_integration():
    """Demonstrate integration with consensus system."""
    print("\n" + "=" * 60)
    print("DEMO 3: Layer 1 Consensus Integration")
    print("=" * 60)
    
    # Create integrator
    integrator = Layer1ConsensusIntegrator()
    
    # Create mock miners and validators
    print("\n1. Setting up subnet participants...")
    miners = [
        MockMiner(uid=f"miner_{i}", address=f"addr_m_{i}", 
                 stake=100000 + i * 10000, last_performance=0.7 + i * 0.05)
        for i in range(10)
    ]
    
    validators = [
        MockValidator(uid=f"validator_{i}", address=f"addr_v_{i}",
                     stake=500000 + i * 50000, last_performance=0.8 + i * 0.03)
        for i in range(5)
    ]
    
    print(f"   ✓ Created {len(miners)} miners")
    print(f"   ✓ Created {len(validators)} validators")
    
    # Create validator scores for miners
    print("\n2. Generating validator scores...")
    validator_scores = {}
    for validator in validators:
        # Each validator scores all miners
        scores = [0.5 + np.random.rand() * 0.5 for _ in miners]
        validator_scores[validator.uid] = scores
    
    print(f"   ✓ Generated scores from {len(validator_scores)} validators")
    
    # Process consensus round
    print("\n3. Processing consensus round...")
    subnet_uid = 1
    current_epoch = 1
    current_slot = 20000
    
    aggregated_state = await integrator.process_consensus_round(
        subnet_uid=subnet_uid,
        current_epoch=current_epoch,
        current_slot=current_slot,
        miners=miners,
        validators=validators,
        validator_scores=validator_scores
    )
    
    print(f"   ✓ Consensus round completed")
    print(f"   - Epoch: {aggregated_state.current_epoch}")
    print(f"   - Total participants: {aggregated_state.total_miners + aggregated_state.total_validators}")
    print(f"   - Total emission: {aggregated_state.total_emission_this_epoch:,} tokens")
    print(f"   - Miner reward pool: {aggregated_state.miner_reward_pool:,} tokens")
    print(f"   - Validator reward pool: {aggregated_state.validator_reward_pool:,} tokens")
    
    # Verify consensus integrity
    print("\n4. Verifying consensus integrity...")
    is_valid, message = await integrator.verify_consensus_integrity(
        subnet_uid=subnet_uid,
        epoch=current_epoch,
        miners=miners,
        validators=validators,
        validator_scores=validator_scores
    )
    
    print(f"   ✓ Verification: {'PASSED' if is_valid else 'FAILED'}")
    print(f"   - Message: {message}")
    
    # Get subnet summary
    print("\n5. Getting subnet summary...")
    summary = integrator.get_subnet_summary(subnet_uid)
    print(f"   ✓ Subnet summary:")
    print(f"   - Subnet UID: {summary['subnet_uid']}")
    print(f"   - Current epoch: {summary['current_epoch']}")
    print(f"   - Total stake: {summary['total_stake']:,} tokens")
    print(f"   - Subnet performance: {summary['subnet_performance']:.2f}")
    print(f"   - Last update slot: {summary['last_update_slot']}")


async def main():
    """Run all demos."""
    print("\n")
    print("╔" + "=" * 58 + "╗")
    print("║" + " " * 5 + "Layer 1 Phase 1 Features - Demo" + " " * 20 + "║")
    print("╚" + "=" * 58 + "╝")
    
    await demo_aggregated_state()
    await demo_weight_matrix()
    await demo_consensus_integration()
    
    print("\n" + "=" * 60)
    print("✓ All demos completed successfully!")
    print("=" * 60)
    print()


if __name__ == "__main__":
    asyncio.run(main())

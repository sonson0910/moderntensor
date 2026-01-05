#!/usr/bin/env python3
"""
Demo: Adaptive Tokenomics System

This demo shows how the ModernTensor adaptive tokenomics system works,
demonstrating its superiority over fixed emission models.
"""

from sdk.tokenomics import (
    TokenomicsIntegration,
    ConsensusData,
    NetworkMetrics,
    TokenomicsConfig,
    DistributionConfig
)


def print_section(title: str):
    """Print a formatted section header."""
    print(f"\n{'='*70}")
    print(f"  {title}")
    print(f"{'='*70}\n")


def demo_basic_epoch():
    """Demonstrate a basic epoch with good network activity."""
    print_section("Demo 1: Basic Epoch Processing")
    
    # Initialize tokenomics
    tokenomics = TokenomicsIntegration()
    
    # Simulate network activity
    consensus_data = ConsensusData(
        miner_scores={
            'miner1': 0.8,
            'miner2': 0.6,
            'miner3': 0.4,
        },
        validator_stakes={
            'validator1': 100000,
            'validator2': 50000,
        },
        quality_score=0.9
    )
    
    network_metrics = NetworkMetrics(
        task_count=5000,
        avg_difficulty=0.8,
        validator_ratio=1.0
    )
    
    # Process epoch
    result = tokenomics.process_epoch_tokenomics(
        epoch=0,
        consensus_data=consensus_data,
        network_metrics=network_metrics
    )
    
    # Display results
    print(f"Epoch: {result.epoch}")
    print(f"Utility Score: {result.utility_score:.4f}")
    print(f"Total Emission: {result.emission_amount} tokens")
    print(f"\nDistribution:")
    print(f"  Miners:     {result.miner_pool} tokens (40%)")
    print(f"  Validators: {result.validator_pool} tokens (40%)")
    print(f"  DAO:        {result.dao_allocation} tokens (20%)")
    print(f"\nToken Sourcing:")
    print(f"  From Pool:  {result.from_pool} tokens")
    print(f"  From Mint:  {result.from_mint} tokens")
    print(f"  Burned:     {result.burned_amount} tokens")


def demo_adaptive_emission():
    """Demonstrate how emission adapts to network activity."""
    print_section("Demo 2: Adaptive Emission Based on Network Activity")
    
    tokenomics = TokenomicsIntegration()
    
    scenarios = [
        ("High Activity", 10000, 0.9, 1.0),
        ("Medium Activity", 5000, 0.5, 0.8),
        ("Low Activity", 1000, 0.3, 0.5),
    ]
    
    for name, tasks, difficulty, participation in scenarios:
        consensus_data = ConsensusData(
            miner_scores={'miner1': 0.8},
            validator_stakes={'validator1': 100000},
            quality_score=0.9
        )
        
        network_metrics = NetworkMetrics(
            task_count=tasks,
            avg_difficulty=difficulty,
            validator_ratio=participation
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        print(f"{name}:")
        print(f"  Tasks: {tasks}, Difficulty: {difficulty}, Participation: {participation}")
        print(f"  → Utility Score: {result.utility_score:.4f}")
        print(f"  → Emission: {result.emission_amount} tokens\n")


def demo_recycling_pool():
    """Demonstrate token recycling mechanism."""
    print_section("Demo 3: Token Recycling Pool")
    
    tokenomics = TokenomicsIntegration()
    
    # Add tokens from various sources
    print("Adding tokens to recycling pool:")
    tokenomics.add_to_recycling_pool(2000, 'registration_fees')
    print("  + 2000 from registration fees")
    
    tokenomics.add_to_recycling_pool(1500, 'slashing_penalties')
    print("  + 1500 from slashing penalties")
    
    tokenomics.add_to_recycling_pool(500, 'task_fees')
    print("  + 500 from task fees")
    
    stats = tokenomics.pool.get_pool_stats()
    print(f"\nPool Balance: {stats['total_balance']} tokens")
    
    # Process epoch
    consensus_data = ConsensusData(
        miner_scores={'miner1': 0.8},
        validator_stakes={'validator1': 100000},
        quality_score=0.9
    )
    
    network_metrics = NetworkMetrics(
        task_count=5000,
        avg_difficulty=0.7,
        validator_ratio=1.0
    )
    
    result = tokenomics.process_epoch_tokenomics(
        epoch=0,
        consensus_data=consensus_data,
        network_metrics=network_metrics
    )
    
    print(f"\nEpoch Processing:")
    print(f"  Required: {result.emission_amount} tokens")
    print(f"  From Pool: {result.from_pool} tokens")
    print(f"  From Mint: {result.from_mint} tokens")
    print(f"  → {result.from_pool / result.emission_amount * 100:.1f}% recycled!")


def demo_quality_burning():
    """Demonstrate token burning for low quality."""
    print_section("Demo 4: Quality-Based Token Burning")
    
    tokenomics = TokenomicsIntegration()
    
    scenarios = [
        ("High Quality (0.9)", 0.9, "No burn expected"),
        ("Medium Quality (0.6)", 0.6, "No burn expected"),
        ("Low Quality (0.3)", 0.3, "Burn expected"),
    ]
    
    for name, quality, expectation in scenarios:
        consensus_data = ConsensusData(
            miner_scores={'miner1': 0.5},
            validator_stakes={'validator1': 100000},
            quality_score=quality
        )
        
        network_metrics = NetworkMetrics(
            task_count=5000,
            avg_difficulty=0.5,
            validator_ratio=1.0
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=0,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        print(f"{name}:")
        print(f"  Quality Score: {quality}")
        print(f"  Expected Emission: {result.emission_amount} tokens")
        print(f"  Burned: {result.burned_amount} tokens")
        print(f"  → {expectation}\n")


def demo_halving_schedule():
    """Demonstrate emission halving over time."""
    print_section("Demo 5: Emission Halving Schedule")
    
    tokenomics = TokenomicsIntegration()
    
    epochs = [0, 210000, 420000, 630000]  # Halving intervals
    
    print("Full utility scenario across halvings:\n")
    
    for epoch in epochs:
        consensus_data = ConsensusData(
            miner_scores={'miner1': 1.0},
            validator_stakes={'validator1': 100000},
            quality_score=1.0
        )
        
        network_metrics = NetworkMetrics(
            task_count=10000,
            avg_difficulty=1.0,
            validator_ratio=1.0
        )
        
        result = tokenomics.process_epoch_tokenomics(
            epoch=epoch,
            consensus_data=consensus_data,
            network_metrics=network_metrics
        )
        
        halvings = epoch // 210000
        print(f"Epoch {epoch:7d} (Halving #{halvings}): {result.emission_amount:4d} tokens")


def demo_claim_system():
    """Demonstrate Merkle proof-based claiming."""
    print_section("Demo 6: Merkle Proof-Based Claiming")
    
    tokenomics = TokenomicsIntegration()
    
    # Process epoch
    consensus_data = ConsensusData(
        miner_scores={
            'miner1': 0.8,
            'miner2': 0.6,
        },
        validator_stakes={
            'validator1': 100000,
        },
        quality_score=0.9
    )
    
    network_metrics = NetworkMetrics(
        task_count=5000,
        avg_difficulty=0.7,
        validator_ratio=1.0
    )
    
    result = tokenomics.process_epoch_tokenomics(
        epoch=0,
        consensus_data=consensus_data,
        network_metrics=network_metrics
    )
    
    print(f"Epoch {result.epoch} processed")
    print(f"Claim Root: {result.claim_root.hex()[:16]}...")
    
    # Check claims
    print("\nClaim Status:")
    for address in ['miner1', 'miner2', 'validator1']:
        status = tokenomics.claims.get_claim_status(0, address)
        if status['exists']:
            print(f"  {address}: {status['reward']} tokens (claimed: {status['claimed']})")
            
            # Get proof
            proof = tokenomics.get_claim_proof(0, address)
            print(f"    → Proof: {len(proof)} hashes")


def main():
    """Run all demos."""
    print("\n" + "="*70)
    print("  ModernTensor Adaptive Tokenomics Demo")
    print("  Superior to Fixed Emission Models")
    print("="*70)
    
    demo_basic_epoch()
    demo_adaptive_emission()
    demo_recycling_pool()
    demo_quality_burning()
    demo_halving_schedule()
    demo_claim_system()
    
    print("\n" + "="*70)
    print("  Demo Complete!")
    print("="*70 + "\n")


if __name__ == '__main__':
    main()

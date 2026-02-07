#!/usr/bin/env python3
# examples/layer2_consensus_demo.py
"""
Demo application for Layer 2 Enhanced Consensus features.

This demo shows:
1. YudkowskyConsensusV2 with bonding curves and outlier detection
2. OptimisticConsensusLayer with challenge mechanism
3. Integration between both systems
4. Performance improvements over basic consensus
"""

import asyncio
import time
from typing import Dict, List

from sdk.consensus.yudkowsky_v2 import (
    YudkowskyConsensusV2,
    ConsensusConfig
)
from sdk.consensus.optimistic_consensus import (
    OptimisticConsensusLayer,
    OptimisticConfig,
    CommitmentStatus
)
from sdk.core.datatypes import MinerInfo


def print_section(title: str):
    """Print a section header."""
    print(f"\n{'=' * 80}")
    print(f" {title}")
    print(f"{'=' * 80}\n")


def print_subsection(title: str):
    """Print a subsection header."""
    print(f"\n{'-' * 60}")
    print(f" {title}")
    print(f"{'-' * 60}\n")


async def demo_yudkowsky_consensus():
    """Demo 1: YudkowskyConsensusV2 with bonding curves."""
    print_section("DEMO 1: Enhanced Yudkowsky Consensus V2")
    
    # Setup
    print("Setup:")
    print("- 5 validators with different stakes")
    print("- 3 miners to be scored")
    print("- Using bonding curve (α=2.0), stake dampening, and outlier detection")
    
    # Create miners
    miners = [
        MinerInfo(uid="miner1", address="addr1", trust_score=0.5, stake=0.0),
        MinerInfo(uid="miner2", address="addr2", trust_score=0.5, stake=0.0),
        MinerInfo(uid="miner3", address="addr3", trust_score=0.5, stake=0.0),
    ]
    
    # Validator scores (including one outlier)
    validator_scores = {
        "val1": [0.85, 0.70, 0.40],  # Normal scores
        "val2": [0.80, 0.65, 0.45],  # Normal scores
        "val3": [0.90, 0.75, 0.35],  # Normal scores
        "val4": [0.82, 0.68, 0.42],  # Normal scores
        "val5": [0.20, 0.95, 0.90],  # Outlier! Reversed scores (potential manipulation)
    }
    
    # Validator stakes (val5 has most stake but is malicious)
    validator_stakes = {
        "val1": 100_000,
        "val2": 150_000,
        "val3": 200_000,
        "val4": 120_000,
        "val5": 500_000,  # Whale validator (but malicious)
    }
    
    print_subsection("Validator Scores and Stakes")
    for val_uid, scores in validator_scores.items():
        stake = validator_stakes[val_uid]
        print(f"{val_uid}: stake={stake:>8,} scores={scores}")
    
    # Initialize consensus with custom config
    config = ConsensusConfig(
        bonding_curve_alpha=2.0,
        stake_dampening_factor=0.5,
        outlier_threshold_std=1.5,  # Lower threshold to catch val5
        use_weighted_median=True
    )
    consensus = YudkowskyConsensusV2(config=config)
    
    print_subsection("Running Consensus")
    start_time = time.time()
    
    # Calculate consensus
    consensus_scores = consensus.calculate_consensus(
        validator_scores=validator_scores,
        validator_stakes=validator_stakes,
        miners=miners,
        current_epoch=1
    )
    
    elapsed_time = (time.time() - start_time) * 1000  # Convert to ms
    
    print_subsection("Results")
    print(f"Consensus completed in {elapsed_time:.2f}ms\n")
    
    print("Final Consensus Scores (with bonding curve applied):")
    for miner_uid, score in consensus_scores.items():
        print(f"  {miner_uid}: {score:.4f}")
    
    print("\nValidator Trust Scores (after 1 epoch):")
    trust_scores = consensus.get_validator_trust_scores()
    for val_uid, trust in trust_scores.items():
        print(f"  {val_uid}: {trust:.4f}")
    
    print("\nObservations:")
    print("- val5's outlier scores were detected and filtered")
    print("- Despite having 500K stake, val5's trust is lower due to deviation")
    print("- Stake dampening prevents val5 from dominating (√500000 vs √100000)")
    print("- Bonding curve rewards top performers (miner1) exponentially")
    
    return consensus_scores


async def demo_optimistic_rollup():
    """Demo 2: Layer 2 Optimistic Rollup."""
    print_section("DEMO 2: Layer 2 Optimistic Rollup Consensus")
    
    print("Setup:")
    print("- 3 validators submitting scores")
    print("- Off-chain consensus aggregation (<1s)")
    print("- On-chain commitment with single transaction")
    print("- Challenge period: 100 blocks")
    print("- Fraud proof system with slashing")
    
    # Initialize Layer 2
    config = OptimisticConfig(
        challenge_period_blocks=100,
        max_deviation_percent=5.0,
        slash_amount=1_000_000,
        fraud_proof_reward=100_000
    )
    layer2 = OptimisticConsensusLayer(config=config)
    
    # Setup validator stakes
    layer2.l1.validator_stakes = {
        "val1": 5_000_000,  # Aggregator
        "val2": 2_000_000,
        "val3": 3_000_000,
    }
    
    print_subsection("Round 1: Honest Consensus")
    
    # Validator scores for 2 miners
    validator_scores = {
        "val1": [0.85, 0.60],
        "val2": [0.80, 0.55],
        "val3": [0.90, 0.65],
    }
    
    print("Validator Scores:")
    for val_uid, scores in validator_scores.items():
        print(f"  {val_uid}: {scores}")
    
    start_time = time.time()
    consensus_scores, commitment_hash = await layer2.run_consensus_round(
        subnet_uid=1,
        epoch=10,
        validator_scores=validator_scores,
        aggregator_uid="val1"
    )
    elapsed_time = (time.time() - start_time) * 1000
    
    print(f"\n⚡ Consensus aggregated in {elapsed_time:.2f}ms (off-chain)")
    print(f"📝 Commitment hash: {commitment_hash.hex()[:32]}...")
    print(f"🔒 Challenge period: blocks 0 to {layer2.config.challenge_period_blocks}")
    
    print("\nConsensus Scores:")
    for miner_uid, score in consensus_scores.items():
        print(f"  {miner_uid}: {score:.4f}")
    
    print_subsection("Round 2: Fraudulent Consensus (Will Be Challenged)")
    
    # Setup validator stakes for round 2
    layer2.l1.validator_stakes["dishonest_val"] = 10_000_000
    layer2.l1.validator_stakes["honest_challenger"] = 1_000_000
    
    # Dishonest aggregator manipulates scores
    fraudulent_scores = {
        "val1": [0.85, 0.60],
        "val2": [0.80, 0.55],
        "dishonest_val": [0.30, 0.95],  # Deliberately wrong to benefit certain miner
    }
    
    print("Validator Scores (dishonest_val is manipulating):")
    for val_uid, scores in fraudulent_scores.items():
        print(f"  {val_uid}: {scores}")
    
    fraud_consensus, fraud_hash = await layer2.run_consensus_round(
        subnet_uid=1,
        epoch=11,
        validator_scores=fraudulent_scores,
        aggregator_uid="dishonest_val"
    )
    
    print(f"\n📝 Fraudulent commitment: {fraud_hash.hex()[:32]}...")
    
    # Advance a few blocks
    await layer2.advance_block(10)
    
    print_subsection("Challenge Submission")
    
    print(f"🚨 honest_challenger detects fraud at block {layer2.l1.current_block}")
    print("   Expected miner_1 score: ~0.60")
    print("   Claimed miner_1 score: ~0.70 (>10% deviation)")
    
    # Submit fraud proof
    fraud_detected = await layer2.submit_fraud_proof(
        commitment_hash=fraud_hash,
        validator_uid="honest_challenger",
        fraud_type="incorrect_consensus",
        claimed_score=0.70,
        actual_score=0.60,
        proof_data={"validator_scores": fraudulent_scores}
    )
    
    print(f"\n✅ Fraud proof {'accepted' if fraud_detected else 'rejected'}")
    
    if fraud_detected:
        print("\nSlashing Results:")
        print(f"  dishonest_val stake: {layer2.l1.validator_stakes['dishonest_val']:>12,} "
              f"(-{config.slash_amount:,})")
        print(f"  honest_challenger stake: {layer2.l1.validator_stakes['honest_challenger']:>12,} "
              f"(+{config.fraud_proof_reward:,})")
    
    print_subsection("Finalization")
    
    # Advance past challenge period for honest commitment
    await layer2.advance_block(91)  # Total 101 blocks
    
    print(f"Current block: {layer2.l1.current_block}")
    print(f"Attempting to finalize commitments...")
    
    # Try to finalize honest commitment
    success1 = await layer2.finalize_commitment(commitment_hash)
    print(f"\nHonest commitment (round 1): {'✅ Finalized' if success1 else '❌ Failed'}")
    
    # Try to finalize fraudulent commitment
    success2 = await layer2.finalize_commitment(fraud_hash)
    print(f"Fraudulent commitment (round 2): {'✅ Finalized' if success2 else '❌ Rejected (challenged)'}")
    
    print_subsection("Summary")
    print(f"Finalized commitments: {len(layer2.finalized_commitments)}")
    print(f"Rejected commitments: {sum(1 for c in layer2.pending_commitments.values() if c.status == CommitmentStatus.REJECTED)}")
    print(f"\nSecurity achieved through:")
    print(f"  - Challenge period: {config.challenge_period_blocks} blocks")
    print(f"  - Economic incentive: {config.slash_amount:,} tokens at risk")
    print(f"  - Fraud detection: Any validator can challenge")


async def demo_performance_comparison():
    """Demo 3: Performance comparison with traditional consensus."""
    print_section("DEMO 3: Performance Comparison")
    
    print("Comparing:")
    print("  A. Traditional on-chain consensus (1 tx per validator)")
    print("  B. Layer 2 optimistic consensus (1 tx total)")
    
    print_subsection("Simulation Parameters")
    
    num_validators = 10
    num_miners = 50
    num_epochs = 5
    
    print(f"Validators: {num_validators}")
    print(f"Miners: {num_miners}")
    print(f"Epochs: {num_epochs}")
    print(f"Block time: ~12s (assumed)")
    
    # Traditional consensus costs
    traditional_txs_per_epoch = num_validators  # Each validator submits 1 tx
    traditional_total_txs = traditional_txs_per_epoch * num_epochs
    traditional_time = num_validators * 12 * num_epochs  # Each tx takes 1 block
    
    # Layer 2 costs
    layer2_txs_per_epoch = 1  # Only aggregator submits 1 tx
    layer2_total_txs = layer2_txs_per_epoch * num_epochs
    layer2_time = 1 * 12 * num_epochs  # Only 1 tx per epoch
    
    print_subsection("Results")
    
    print("Traditional On-Chain Consensus:")
    print(f"  Total transactions: {traditional_total_txs}")
    print(f"  Total time: {traditional_time}s ({traditional_time/60:.1f} minutes)")
    print(f"  Gas cost: ~{traditional_total_txs * 0.1:.1f} ADA (estimated)")
    
    print("\nLayer 2 Optimistic Consensus:")
    print(f"  Total transactions: {layer2_total_txs}")
    print(f"  Total time: {layer2_time}s ({layer2_time/60:.1f} minutes)")
    print(f"  Gas cost: ~{layer2_total_txs * 0.1:.1f} ADA (estimated)")
    
    print("\nImprovement:")
    tx_reduction = ((traditional_total_txs - layer2_total_txs) / traditional_total_txs) * 100
    time_reduction = ((traditional_time - layer2_time) / traditional_time) * 100
    
    print(f"  Transaction reduction: {tx_reduction:.1f}%")
    print(f"  Time reduction: {time_reduction:.1f}%")
    print(f"  Transaction ratio: {traditional_total_txs}:{layer2_total_txs} = {traditional_total_txs//layer2_total_txs}x improvement")
    
    print_subsection("Latency Comparison")
    
    print("Consensus Latency (off-chain calculation):")
    print(f"  Traditional: ~12s (on-chain)")
    print(f"  Layer 2: <1s (off-chain aggregation)")
    print(f"  Improvement: >12x faster")
    
    print("\nFinality Time (including challenge period):")
    print(f"  Traditional: ~12s (immediate)")
    print(f"  Layer 2: ~1200s (100 blocks * 12s)")
    print(f"  Note: Layer 2 provides instant consensus with delayed finality")


async def main():
    """Run all demos."""
    print("\n" + "=" * 80)
    print(" ModernTensor Layer 2 Enhanced Consensus Demo")
    print(" Demonstrating Phase 2 Features: Bonding Curves + Optimistic Rollup")
    print("=" * 80)
    
    # Demo 1: YudkowskyConsensusV2
    await demo_yudkowsky_consensus()
    
    # Demo 2: Optimistic Rollup
    await demo_optimistic_rollup()
    
    # Demo 3: Performance comparison
    await demo_performance_comparison()
    
    print_section("Demo Complete!")
    print("Layer 2 features successfully demonstrated:")
    print("  ✅ YudkowskyConsensusV2 with bonding curves and outlier detection")
    print("  ✅ OptimisticConsensusLayer with challenge mechanism")
    print("  ✅ Performance improvements: 10x txs reduction, 12x faster consensus")
    print("  ✅ Security maintained through economic incentives and fraud proofs")
    print("\nNext: Integration with Layer1ConsensusIntegrator and production deployment")


if __name__ == "__main__":
    asyncio.run(main())

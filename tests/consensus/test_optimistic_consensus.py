# tests/consensus/test_optimistic_consensus.py
"""
Tests for Optimistic Consensus Layer 2 implementation.
"""

import pytest
import asyncio

from sdk.consensus.optimistic_consensus import (
    OptimisticConsensusLayer,
    OptimisticConfig,
    L1Interface,
    CommitmentStatus,
    ConsensusCommitment,
    FraudProof
)


class TestOptimisticConfig:
    """Test OptimisticConfig dataclass."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = OptimisticConfig()
        
        assert config.challenge_period_blocks == 100
        assert config.min_validators == 3
        assert config.max_deviation_percent == 5.0
        assert config.slash_amount == 1000000
        assert config.fraud_proof_reward == 100000
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = OptimisticConfig(
            challenge_period_blocks=50,
            max_deviation_percent=10.0
        )
        
        assert config.challenge_period_blocks == 50
        assert config.max_deviation_percent == 10.0


class TestL1Interface:
    """Test L1Interface mock implementation."""
    
    def test_initialization(self):
        """Test L1 interface initialization."""
        l1 = L1Interface()
        
        assert l1.current_block == 0
        assert len(l1.commitments_on_chain) == 0
        assert len(l1.validator_stakes) == 0
    
    @pytest.mark.asyncio
    async def test_publish_commitment(self):
        """Test publishing commitment to L1."""
        l1 = L1Interface()
        
        commitment_hash = b"test_hash"
        tx_hash = await l1.publish_commitment(
            subnet_uid=1,
            epoch=10,
            commitment_hash=commitment_hash,
            aggregator_uid="val1"
        )
        
        assert tx_hash is not None
        assert commitment_hash in l1.commitments_on_chain
        assert l1.commitments_on_chain[commitment_hash]['subnet_uid'] == 1
        assert l1.commitments_on_chain[commitment_hash]['epoch'] == 10
    
    @pytest.mark.asyncio
    async def test_slash_validator(self):
        """Test slashing validator stakes."""
        l1 = L1Interface()
        l1.validator_stakes["val1"] = 1000000
        
        await l1.slash_validator("val1", 100000)
        
        assert l1.validator_stakes["val1"] == 900000
    
    @pytest.mark.asyncio
    async def test_reward_validator(self):
        """Test rewarding validators."""
        l1 = L1Interface()
        
        await l1.reward_validator("val1", 100000)
        
        assert l1.validator_stakes["val1"] == 100000


class TestOptimisticConsensusLayer:
    """Test OptimisticConsensusLayer implementation."""
    
    def test_initialization(self):
        """Test optimistic consensus layer initialization."""
        layer = OptimisticConsensusLayer()
        
        assert layer.l1 is not None
        assert layer.config is not None
        assert len(layer.pending_commitments) == 0
        assert len(layer.finalized_commitments) == 0
    
    def test_initialization_with_custom_config(self):
        """Test initialization with custom config."""
        config = OptimisticConfig(challenge_period_blocks=50)
        layer = OptimisticConsensusLayer(config=config)
        
        assert layer.config.challenge_period_blocks == 50
    
    @pytest.mark.asyncio
    async def test_run_consensus_round(self):
        """Test running a complete consensus round."""
        layer = OptimisticConsensusLayer()
        
        # Validator scores for 3 miners
        validator_scores = {
            "val1": [0.8, 0.6, 0.4],
            "val2": [0.7, 0.5, 0.5],
            "val3": [0.9, 0.7, 0.3],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Check consensus scores calculated
        assert len(consensus_scores) == 3
        assert "miner_0" in consensus_scores
        assert "miner_1" in consensus_scores
        assert "miner_2" in consensus_scores
        
        # Check commitment created and pending
        assert commitment_hash in layer.pending_commitments
        commitment = layer.pending_commitments[commitment_hash]
        assert commitment.subnet_uid == 1
        assert commitment.epoch == 10
        assert commitment.status == CommitmentStatus.PENDING
        assert commitment.finalize_at_block == 100  # 0 + 100
    
    @pytest.mark.asyncio
    async def test_consensus_calculation(self):
        """Test consensus calculation (simple average)."""
        layer = OptimisticConsensusLayer()
        
        validator_scores = {
            "val1": [0.8, 0.6],
            "val2": [0.6, 0.4],
            "val3": [1.0, 0.8],
        }
        
        consensus = layer._calculate_consensus(validator_scores)
        
        # miner_0: (0.8 + 0.6 + 1.0) / 3 = 0.8
        # miner_1: (0.6 + 0.4 + 0.8) / 3 = 0.6
        assert abs(consensus["miner_0"] - 0.8) < 0.001
        assert abs(consensus["miner_1"] - 0.6) < 0.001
    
    @pytest.mark.asyncio
    async def test_finalize_commitment_success(self):
        """Test successful commitment finalization."""
        layer = OptimisticConsensusLayer()
        
        # Run consensus
        validator_scores = {
            "val1": [0.8, 0.6],
            "val2": [0.7, 0.5],
            "val3": [0.9, 0.7],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Advance blocks past challenge period
        await layer.advance_block(100)
        
        # Finalize
        success = await layer.finalize_commitment(commitment_hash)
        
        assert success
        assert commitment_hash not in layer.pending_commitments
        assert commitment_hash in layer.finalized_commitments
        assert layer.finalized_commitments[commitment_hash].status == CommitmentStatus.FINALIZED
    
    @pytest.mark.asyncio
    async def test_finalize_before_challenge_period_fails(self):
        """Test that finalization fails before challenge period expires."""
        layer = OptimisticConsensusLayer()
        
        validator_scores = {
            "val1": [0.8, 0.6],
            "val2": [0.7, 0.5],
            "val3": [0.9, 0.7],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Try to finalize immediately (should fail)
        success = await layer.finalize_commitment(commitment_hash)
        
        assert not success
        assert commitment_hash in layer.pending_commitments
    
    @pytest.mark.asyncio
    async def test_fraud_proof_submission(self):
        """Test submitting fraud proof during challenge period."""
        config = OptimisticConfig(max_deviation_percent=10.0)
        layer = OptimisticConsensusLayer(config=config)
        
        # Setup validator stakes
        layer.l1.validator_stakes["val1"] = 2000000  # Aggregator
        layer.l1.validator_stakes["val2"] = 1000000  # Challenger
        
        # Run consensus with intentionally wrong scores
        validator_scores = {
            "val1": [0.8],
            "val2": [0.7],
            "val3": [0.9],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Submit fraud proof (claiming score was wrong by >10%)
        success = await layer.submit_fraud_proof(
            commitment_hash=commitment_hash,
            validator_uid="val2",
            fraud_type="incorrect_consensus",
            claimed_score=0.8,
            actual_score=0.5,  # 37.5% deviation - should trigger fraud
            proof_data={"original_scores": validator_scores}
        )
        
        assert success
        
        # Check commitment marked as challenged
        commitment = layer.pending_commitments[commitment_hash]
        assert commitment.status == CommitmentStatus.CHALLENGED
        assert commitment.challenged_by == "val2"
        
        # Check aggregator slashed
        assert layer.l1.validator_stakes["val1"] == 1000000  # 2M - 1M
        
        # Check challenger rewarded
        assert layer.l1.validator_stakes["val2"] == 1100000  # 1M + 100K
    
    @pytest.mark.asyncio
    async def test_fraud_proof_after_challenge_period_fails(self):
        """Test that fraud proof fails after challenge period."""
        layer = OptimisticConsensusLayer()
        
        validator_scores = {
            "val1": [0.8],
            "val2": [0.7],
            "val3": [0.9],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Advance past challenge period
        await layer.advance_block(101)
        
        # Try to submit fraud proof (should fail)
        success = await layer.submit_fraud_proof(
            commitment_hash=commitment_hash,
            validator_uid="val2",
            fraud_type="incorrect_consensus",
            claimed_score=0.8,
            actual_score=0.5,
            proof_data={}
        )
        
        assert not success
    
    @pytest.mark.asyncio
    async def test_challenged_commitment_cannot_finalize(self):
        """Test that challenged commitments cannot be finalized."""
        config = OptimisticConfig(max_deviation_percent=10.0)
        layer = OptimisticConsensusLayer(config=config)
        
        layer.l1.validator_stakes["val1"] = 2000000
        
        validator_scores = {
            "val1": [0.8],
            "val2": [0.7],
            "val3": [0.9],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Submit fraud proof
        await layer.submit_fraud_proof(
            commitment_hash=commitment_hash,
            validator_uid="val2",
            fraud_type="incorrect_consensus",
            claimed_score=0.8,
            actual_score=0.5,
            proof_data={}
        )
        
        # Advance past challenge period
        await layer.advance_block(101)
        
        # Try to finalize (should fail because challenged)
        success = await layer.finalize_commitment(commitment_hash)
        
        assert not success
        assert layer.pending_commitments[commitment_hash].status == CommitmentStatus.REJECTED
    
    @pytest.mark.asyncio
    async def test_get_commitment_status(self):
        """Test getting commitment status."""
        layer = OptimisticConsensusLayer()
        
        validator_scores = {
            "val1": [0.8],
            "val2": [0.7],
            "val3": [0.9],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Check pending status
        status = layer.get_commitment_status(commitment_hash)
        assert status == CommitmentStatus.PENDING
        
        # Finalize
        await layer.advance_block(101)
        await layer.finalize_commitment(commitment_hash)
        
        # Check finalized status
        status = layer.get_commitment_status(commitment_hash)
        assert status == CommitmentStatus.FINALIZED
    
    @pytest.mark.asyncio
    async def test_get_finalized_consensus(self):
        """Test retrieving finalized consensus scores."""
        layer = OptimisticConsensusLayer()
        
        validator_scores = {
            "val1": [0.8, 0.6],
            "val2": [0.7, 0.5],
            "val3": [0.9, 0.7],
        }
        
        consensus_scores, commitment_hash = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores,
            aggregator_uid="val1"
        )
        
        # Should be None before finalization
        scores = layer.get_finalized_consensus(commitment_hash)
        assert scores is None
        
        # Finalize
        await layer.advance_block(101)
        await layer.finalize_commitment(commitment_hash)
        
        # Should return scores after finalization
        scores = layer.get_finalized_consensus(commitment_hash)
        assert scores is not None
        assert len(scores) == 2
        assert "miner_0" in scores
        assert "miner_1" in scores
    
    @pytest.mark.asyncio
    async def test_multiple_consensus_rounds(self):
        """Test multiple consecutive consensus rounds."""
        layer = OptimisticConsensusLayer()
        
        # Round 1
        validator_scores1 = {
            "val1": [0.8],
            "val2": [0.7],
            "val3": [0.9],
        }
        
        scores1, hash1 = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=10,
            validator_scores=validator_scores1,
            aggregator_uid="val1"
        )
        
        # Round 2
        validator_scores2 = {
            "val1": [0.6],
            "val2": [0.5],
            "val3": [0.7],
        }
        
        scores2, hash2 = await layer.run_consensus_round(
            subnet_uid=1,
            epoch=11,
            validator_scores=validator_scores2,
            aggregator_uid="val2"
        )
        
        # Both should be pending
        assert hash1 in layer.pending_commitments
        assert hash2 in layer.pending_commitments
        
        # Finalize both
        await layer.advance_block(101)
        await layer.finalize_commitment(hash1)
        await layer.finalize_commitment(hash2)
        
        # Both should be finalized
        assert hash1 in layer.finalized_commitments
        assert hash2 in layer.finalized_commitments


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

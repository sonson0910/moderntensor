"""
Comprehensive Tests for Tokenomics Module (Month 1 Implementation)

Test Coverage:
- Unit tests for all tokenomics modules
- Integration tests for end-to-end flows
- Stress testing for high load scenarios
- Edge case testing

Target: 90%+ test coverage
Implementation Date: January 2026 (Month 1 Roadmap)
"""

import pytest
import asyncio
from unittest.mock import Mock, AsyncMock, patch, MagicMock
from sdk.tokenomics.enhanced_rpc_integration import (
    EnhancedRPCIntegration,
    RPCConfig,
    ConnectionState,
    CircuitBreaker
)
from sdk.tokenomics.emission_controller import EmissionController
from sdk.tokenomics.reward_distributor import RewardDistributor, DistributionResult
from sdk.tokenomics.burn_manager import BurnManager
from sdk.tokenomics.recycling_pool import RecyclingPool
from sdk.tokenomics.config import TokenomicsConfig, DistributionConfig


class TestEnhancedRPCIntegration:
    """Tests for Enhanced RPC Integration."""
    
    @pytest.fixture
    def rpc_config(self):
        """Create test RPC configuration."""
        return RPCConfig(
            url="http://localhost:9944",
            max_retry_attempts=3,
            retry_delay=0.1,  # Short delay for tests
            failure_threshold=3,
            recovery_timeout=5
        )
    
    @pytest.fixture
    async def rpc_client(self, rpc_config):
        """Create RPC client for tests."""
        client = EnhancedRPCIntegration(rpc_config)
        yield client
        await client.close()
    
    @pytest.mark.asyncio
    async def test_connection_establishment(self, rpc_client):
        """Test connection pool establishment."""
        await rpc_client.connect()
        assert rpc_client._session is not None
        assert rpc_client._health_check_task is not None
    
    @pytest.mark.asyncio
    async def test_connection_closure(self, rpc_client):
        """Test connection pool closure."""
        await rpc_client.connect()
        await rpc_client.close()
        assert rpc_client._session is None
    
    @pytest.mark.asyncio
    async def test_retry_mechanism_success_after_failure(self, rpc_client):
        """Test retry succeeds after initial failures."""
        call_count = 0
        
        async def mock_post(*args, **kwargs):
            nonlocal call_count
            call_count += 1
            if call_count < 3:
                raise Exception("Temporary failure")
            
            # Success on 3rd attempt
            mock_response = AsyncMock()
            mock_response.json = AsyncMock(return_value={"result": "success"})
            mock_response.raise_for_status = Mock()
            return mock_response
        
        await rpc_client.connect()
        rpc_client._session.post = Mock(return_value=mock_post())
        
        with patch.object(rpc_client._session, 'post', side_effect=mock_post):
            result = await rpc_client.execute_rpc_call("test_method", [])
            assert result == "success"
            assert call_count == 3
            assert rpc_client.metrics.retry_count >= 2
    
    @pytest.mark.asyncio
    async def test_retry_mechanism_all_fail(self, rpc_client):
        """Test all retry attempts fail."""
        async def mock_post(*args, **kwargs):
            raise Exception("Persistent failure")
        
        await rpc_client.connect()
        
        with patch.object(rpc_client._session, 'post', side_effect=mock_post):
            with pytest.raises(Exception, match="All .* attempts failed"):
                await rpc_client.execute_rpc_call("test_method", [])
    
    @pytest.mark.asyncio
    async def test_circuit_breaker_opens(self, rpc_client):
        """Test circuit breaker opens after threshold failures."""
        rpc_client.circuit_breaker.failure_threshold = 2
        
        # Simulate failures
        for _ in range(3):
            rpc_client.circuit_breaker.record_failure()
        
        assert rpc_client.circuit_breaker.state == ConnectionState.OPEN
        assert not rpc_client.circuit_breaker.can_execute()
    
    @pytest.mark.asyncio
    async def test_circuit_breaker_recovery(self, rpc_client):
        """Test circuit breaker recovers after timeout."""
        rpc_client.circuit_breaker.failure_threshold = 2
        rpc_client.circuit_breaker.recovery_timeout = 0.1  # Short timeout
        
        # Open circuit
        for _ in range(3):
            rpc_client.circuit_breaker.record_failure()
        
        assert rpc_client.circuit_breaker.state == ConnectionState.OPEN
        
        # Wait for recovery
        await asyncio.sleep(0.2)
        
        # Should enter HALF_OPEN
        assert rpc_client.circuit_breaker.can_execute()
    
    @pytest.mark.asyncio
    async def test_batch_execution(self, rpc_client):
        """Test batch RPC execution."""
        requests = [
            {"method": "eth_blockNumber", "params": []},
            {"method": "eth_getBalance", "params": ["0x123"]},
            {"method": "eth_gasPrice", "params": []}
        ]
        
        mock_response = [
            {"id": 0, "result": "0x1"},
            {"id": 1, "result": "0x100"},
            {"id": 2, "result": "0x10"}
        ]
        
        await rpc_client.connect()
        
        async def mock_post(*args, **kwargs):
            mock_resp = AsyncMock()
            mock_resp.json = AsyncMock(return_value=mock_response)
            mock_resp.raise_for_status = Mock()
            return mock_resp
        
        with patch.object(rpc_client._session, 'post', side_effect=mock_post):
            results = await rpc_client.batch_execute(requests)
            assert len(results) == 3
            assert results == ["0x1", "0x100", "0x10"]
    
    @pytest.mark.asyncio
    async def test_metrics_tracking(self, rpc_client):
        """Test metrics are properly tracked."""
        await rpc_client.connect()
        
        # Mock successful request
        async def mock_post_success(*args, **kwargs):
            mock_resp = AsyncMock()
            mock_resp.json = AsyncMock(return_value={"result": "success"})
            mock_resp.raise_for_status = Mock()
            return mock_resp
        
        with patch.object(rpc_client._session, 'post', side_effect=mock_post_success):
            await rpc_client.execute_rpc_call("test", [])
        
        metrics = rpc_client.get_metrics()
        assert metrics["total_requests"] == 1
        assert metrics["successful_requests"] == 1
        assert metrics["failed_requests"] == 0
        assert metrics["success_rate"] == 100.0
    
    @pytest.mark.asyncio
    async def test_context_manager(self, rpc_config):
        """Test async context manager."""
        async with EnhancedRPCIntegration(rpc_config) as client:
            assert client._session is not None
        
        # Session should be closed after exit
        assert client._session is None


class TestEmissionController:
    """Tests for Emission Controller."""
    
    @pytest.fixture
    def config(self):
        """Create test configuration."""
        return TokenomicsConfig(
            max_supply=21_000_000,
            base_reward=1000,
            halving_interval=210_000,
            max_expected_tasks=10_000,
            utility_weights=(0.5, 0.3, 0.2)
        )
    
    @pytest.fixture
    def controller(self, config):
        """Create emission controller."""
        return EmissionController(config)
    
    def test_emission_calculation_full_utility(self, controller):
        """Test emission with 100% utility."""
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=1000
        )
        assert emission == 1000  # base_reward * 1.0 * 1.0 (no halvings yet)
    
    def test_emission_calculation_half_utility(self, controller):
        """Test emission with 50% utility."""
        emission = controller.calculate_epoch_emission(
            utility_score=0.5,
            epoch=1000
        )
        assert emission == 500  # base_reward * 0.5 * 1.0
    
    def test_emission_calculation_with_halving(self, controller):
        """Test emission after first halving."""
        # After first halving interval
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=210_000
        )
        assert emission == 500  # base_reward * 1.0 * 0.5 (first halving)
    
    def test_emission_respects_max_supply(self, controller):
        """Test emission caps at max supply."""
        controller.current_supply = 20_999_000
        
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=1000
        )
        assert emission == 1000  # Should cap at remaining supply
        assert emission + controller.current_supply <= controller.config.max_supply
    
    def test_utility_score_calculation(self, controller):
        """Test utility score calculation."""
        utility = controller.calculate_utility_score(
            task_volume=5000,      # 50% of max
            avg_task_difficulty=0.8,
            validator_participation=0.9
        )
        
        # Expected: 0.5 * 0.5 + 0.3 * 0.8 + 0.2 * 0.9 = 0.25 + 0.24 + 0.18 = 0.67
        assert 0.66 <= utility <= 0.68
    
    def test_utility_score_validation(self, controller):
        """Test utility score input validation."""
        with pytest.raises(ValueError, match="must be between 0.0 and 1.0"):
            controller.calculate_utility_score(
                task_volume=1000,
                avg_task_difficulty=1.5,  # Invalid
                validator_participation=0.9
            )
    
    def test_supply_tracking(self, controller):
        """Test supply is tracked correctly."""
        initial_supply = controller.current_supply
        controller.update_supply(1000)
        assert controller.current_supply == initial_supply + 1000


class TestRewardDistributor:
    """Tests for Reward Distributor."""
    
    @pytest.fixture
    def config(self):
        """Create distribution configuration."""
        return DistributionConfig(
            miner_share=0.40,
            validator_share=0.40,
            dao_share=0.20
        )
    
    @pytest.fixture
    def distributor(self, config):
        """Create reward distributor."""
        return RewardDistributor(config)
    
    @pytest.fixture
    def recycling_pool(self):
        """Create recycling pool."""
        return RecyclingPool()
    
    def test_reward_distribution_by_miner_scores(self, distributor, recycling_pool):
        """Test rewards distributed proportionally to miner scores."""
        miner_scores = {
            "miner1": 0.9,
            "miner2": 0.7,
            "miner3": 0.4
        }
        validator_stakes = {
            "validator1": 10000
        }
        
        result = distributor.distribute_epoch_rewards(
            epoch=1,
            total_emission=1000,
            miner_scores=miner_scores,
            validator_stakes=validator_stakes,
            recycling_pool=recycling_pool
        )
        
        # Miner pool should be 40% of 1000 = 400
        total_miner_rewards = sum(result.miner_rewards.values())
        assert total_miner_rewards == 400
        
        # Rewards should be proportional to scores
        total_score = sum(miner_scores.values())
        for miner, score in miner_scores.items():
            expected = int((score / total_score) * 400)
            assert result.miner_rewards[miner] == expected
    
    def test_reward_distribution_by_validator_stakes(self, distributor, recycling_pool):
        """Test rewards distributed proportionally to validator stakes."""
        miner_scores = {"miner1": 1.0}
        validator_stakes = {
            "validator1": 5000,
            "validator2": 3000,
            "validator3": 2000
        }
        
        result = distributor.distribute_epoch_rewards(
            epoch=1,
            total_emission=1000,
            miner_scores=miner_scores,
            validator_stakes=validator_stakes,
            recycling_pool=recycling_pool
        )
        
        # Validator pool should be 40% of 1000 = 400
        total_validator_rewards = sum(result.validator_rewards.values())
        assert total_validator_rewards == 400
        
        # Rewards should be proportional to stakes
        total_stake = sum(validator_stakes.values())
        for validator, stake in validator_stakes.items():
            expected = int((stake / total_stake) * 400)
            assert result.validator_rewards[validator] == expected
    
    def test_dao_allocation(self, distributor, recycling_pool):
        """Test DAO receives correct allocation."""
        result = distributor.distribute_epoch_rewards(
            epoch=1,
            total_emission=1000,
            miner_scores={"miner1": 1.0},
            validator_stakes={"validator1": 10000},
            recycling_pool=recycling_pool
        )
        
        # DAO should receive 20% of 1000 = 200
        assert result.dao_allocation == 200
    
    def test_empty_scores_handled(self, distributor, recycling_pool):
        """Test handling of empty miner scores."""
        result = distributor.distribute_epoch_rewards(
            epoch=1,
            total_emission=1000,
            miner_scores={},
            validator_stakes={"validator1": 10000},
            recycling_pool=recycling_pool
        )
        
        assert result.miner_rewards == {}
        assert sum(result.validator_rewards.values()) == 400


class TestStressScenarios:
    """Stress tests for high load scenarios."""
    
    @pytest.mark.asyncio
    @pytest.mark.slow
    async def test_concurrent_rpc_requests(self):
        """Test handling of many concurrent RPC requests."""
        config = RPCConfig(
            url="http://localhost:9944",
            max_connections=50,
            retry_delay=0.01
        )
        
        async with EnhancedRPCIntegration(config) as client:
            # Mock responses
            async def mock_post(*args, **kwargs):
                await asyncio.sleep(0.01)  # Simulate latency
                mock_resp = AsyncMock()
                mock_resp.json = AsyncMock(return_value={"result": "ok"})
                mock_resp.raise_for_status = Mock()
                return mock_resp
            
            with patch.object(client._session, 'post', side_effect=mock_post):
                # Execute 100 concurrent requests
                tasks = [
                    client.execute_rpc_call("test", [])
                    for _ in range(100)
                ]
                results = await asyncio.gather(*tasks)
                
                assert len(results) == 100
                assert all(r == "ok" for r in results)
                assert client.metrics.total_requests == 100
    
    @pytest.mark.slow
    def test_large_emission_calculation(self):
        """Test emission calculation with large numbers."""
        controller = EmissionController()
        
        # Test with large utility scores and epochs
        for epoch in range(0, 1_000_000, 100_000):
            emission = controller.calculate_epoch_emission(
                utility_score=0.75,
                epoch=epoch
            )
            assert emission >= 0
            assert isinstance(emission, int)


class TestEdgeCases:
    """Tests for edge cases and boundary conditions."""
    
    def test_zero_emission(self):
        """Test handling of zero emission."""
        controller = EmissionController()
        emission = controller.calculate_epoch_emission(
            utility_score=0.0,
            epoch=1000
        )
        assert emission == 0
    
    def test_max_supply_reached(self):
        """Test behavior when max supply is reached."""
        controller = EmissionController()
        controller.current_supply = controller.config.max_supply
        
        emission = controller.calculate_epoch_emission(
            utility_score=1.0,
            epoch=1000
        )
        assert emission == 0  # No more can be minted
    
    def test_negative_values_rejected(self):
        """Test negative values are rejected."""
        distributor = RewardDistributor()
        pool = RecyclingPool()
        
        with pytest.raises(ValueError):
            distributor.distribute_epoch_rewards(
                epoch=1,
                total_emission=-1000,  # Negative
                miner_scores={"m1": 1.0},
                validator_stakes={"v1": 1000},
                recycling_pool=pool
            )
    
    def test_invalid_scores_rejected(self):
        """Test invalid scores are rejected."""
        distributor = RewardDistributor()
        pool = RecyclingPool()
        
        with pytest.raises(ValueError):
            distributor.distribute_epoch_rewards(
                epoch=1,
                total_emission=1000,
                miner_scores={"m1": 1.5},  # > 1.0
                validator_stakes={"v1": 1000},
                recycling_pool=pool
            )


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--cov=sdk/tokenomics", "--cov-report=html"])

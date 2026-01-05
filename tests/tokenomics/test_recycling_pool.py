"""
Unit tests for RecyclingPool.
"""

import pytest
from sdk.tokenomics.recycling_pool import RecyclingPool


class TestRecyclingPool:
    """Test cases for RecyclingPool."""
    
    def test_initialization(self):
        """Test pool initialization."""
        pool = RecyclingPool()
        
        assert pool.pool_balance == 0
        assert pool.total_recycled == 0
        assert pool.total_allocated == 0
        assert all(v == 0 for v in pool.sources.values())
    
    def test_add_to_pool(self):
        """Test adding tokens to pool."""
        pool = RecyclingPool()
        
        pool.add_to_pool(1000, 'registration_fees')
        
        assert pool.pool_balance == 1000
        assert pool.total_recycled == 1000
        assert pool.sources['registration_fees'] == 1000
    
    def test_add_to_pool_multiple_sources(self):
        """Test adding from multiple sources."""
        pool = RecyclingPool()
        
        pool.add_to_pool(1000, 'registration_fees')
        pool.add_to_pool(500, 'slashing_penalties')
        pool.add_to_pool(300, 'task_fees')
        
        assert pool.pool_balance == 1800
        assert pool.total_recycled == 1800
        assert pool.sources['registration_fees'] == 1000
        assert pool.sources['slashing_penalties'] == 500
        assert pool.sources['task_fees'] == 300
    
    def test_add_to_pool_invalid_amount(self):
        """Test that negative amounts raise error."""
        pool = RecyclingPool()
        
        with pytest.raises(ValueError, match="Amount must be non-negative"):
            pool.add_to_pool(-100, 'registration_fees')
    
    def test_add_to_pool_invalid_source(self):
        """Test that invalid source raises error."""
        pool = RecyclingPool()
        
        with pytest.raises(ValueError, match="Invalid source"):
            pool.add_to_pool(100, 'invalid_source')
    
    def test_allocate_rewards_from_pool_only(self):
        """Test allocating rewards entirely from pool."""
        pool = RecyclingPool()
        pool.add_to_pool(1000, 'registration_fees')
        
        from_pool, from_mint = pool.allocate_rewards(500)
        
        assert from_pool == 500
        assert from_mint == 0
        assert pool.pool_balance == 500
        assert pool.total_allocated == 500
    
    def test_allocate_rewards_from_pool_and_mint(self):
        """Test allocating rewards from pool and minting."""
        pool = RecyclingPool()
        pool.add_to_pool(300, 'registration_fees')
        
        from_pool, from_mint = pool.allocate_rewards(1000)
        
        assert from_pool == 300
        assert from_mint == 700
        assert pool.pool_balance == 0
        assert pool.total_allocated == 300
    
    def test_allocate_rewards_from_mint_only(self):
        """Test allocating rewards with empty pool."""
        pool = RecyclingPool()
        
        from_pool, from_mint = pool.allocate_rewards(1000)
        
        assert from_pool == 0
        assert from_mint == 1000
        assert pool.pool_balance == 0
    
    def test_allocate_rewards_invalid_amount(self):
        """Test that negative amounts raise error."""
        pool = RecyclingPool()
        
        with pytest.raises(ValueError, match="Required amount must be non-negative"):
            pool.allocate_rewards(-100)
    
    def test_get_pool_stats(self):
        """Test getting pool statistics."""
        pool = RecyclingPool()
        pool.add_to_pool(1000, 'registration_fees')
        pool.add_to_pool(500, 'slashing_penalties')
        pool.allocate_rewards(800)
        
        stats = pool.get_pool_stats()
        
        assert stats['total_balance'] == 700
        assert stats['total_recycled'] == 1500
        assert stats['total_allocated'] == 800
        assert abs(stats['utilization_rate'] - (800/1500)) < 0.001
    
    def test_utilization_rate_empty_pool(self):
        """Test utilization rate with empty pool."""
        pool = RecyclingPool()
        
        stats = pool.get_pool_stats()
        
        assert stats['utilization_rate'] == 0.0

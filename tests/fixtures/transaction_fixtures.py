"""
Test Fixtures and Utilities for ModernTensor SDK

Provides reusable test fixtures and helper functions.
"""

import pytest
from typing import Dict, Any, List
from sdk.transactions.types import (
    TransferTransaction,
    StakeTransaction,
    WeightTransaction,
)


# Transaction fixtures
@pytest.fixture
def sample_transfer_tx():
    """Create a sample transfer transaction."""
    return TransferTransaction(
        from_address="addr1234567890abcdef",
        to_address="addr0987654321fedcba",
        amount=100.0,
        fee=0.01,
        memo="Test transfer"
    )


@pytest.fixture
def sample_stake_tx():
    """Create a sample stake transaction."""
    return StakeTransaction(
        from_address="addr1234567890abcdef",
        hotkey="hotkey1234567890abcdef",
        amount=50.0,
        subnet_id=1,
        fee=0.005
    )


@pytest.fixture
def sample_weight_tx():
    """Create a sample weight transaction."""
    return WeightTransaction(
        from_address="addr1234567890abcdef",
        subnet_id=1,
        uids=[1, 2, 3, 4, 5],
        weights=[0.3, 0.25, 0.2, 0.15, 0.1],
        version_key=1
    )


@pytest.fixture
def sample_transactions(sample_transfer_tx, sample_stake_tx, sample_weight_tx):
    """Get a list of sample transactions."""
    return [sample_transfer_tx, sample_stake_tx, sample_weight_tx]


# Address fixtures
@pytest.fixture
def valid_addresses():
    """Get list of valid test addresses."""
    return [
        "addr1234567890abcdef",
        "addr0987654321fedcba",
        "addrabcdef1234567890",
    ]


@pytest.fixture
def valid_hotkeys():
    """Get list of valid test hotkeys."""
    return [
        "hotkey1234567890abcdef",
        "hotkey0987654321fedcba",
        "hotkeyabcdef1234567890",
    ]


# Helper functions
class TransactionTestHelper:
    """Helper class for transaction testing."""
    
    @staticmethod
    def create_transfer(amount: float = 100.0, **kwargs) -> TransferTransaction:
        """Create a transfer transaction with defaults."""
        defaults = {
            "from_address": "addr1234567890",
            "to_address": "addr0987654321",
            "amount": amount,
        }
        defaults.update(kwargs)
        return TransferTransaction(**defaults)
    
    @staticmethod
    def create_stake(amount: float = 50.0, **kwargs) -> StakeTransaction:
        """Create a stake transaction with defaults."""
        defaults = {
            "from_address": "addr1234567890",
            "hotkey": "hotkey1234567890",
            "amount": amount,
        }
        defaults.update(kwargs)
        return StakeTransaction(**defaults)
    
    @staticmethod
    def create_weights(num_neurons: int = 5, **kwargs) -> WeightTransaction:
        """Create a weight transaction with defaults."""
        uids = list(range(num_neurons))
        weights = [1.0 / num_neurons] * num_neurons
        
        defaults = {
            "from_address": "addr1234567890",
            "subnet_id": 1,
            "uids": uids,
            "weights": weights,
            "version_key": 1,
        }
        defaults.update(kwargs)
        return WeightTransaction(**defaults)


@pytest.fixture
def tx_helper():
    """Get transaction test helper."""
    return TransactionTestHelper()


# Mock data generators
class MockDataGenerator:
    """Generate mock data for testing."""
    
    @staticmethod
    def generate_address(prefix: str = "addr") -> str:
        """Generate a mock address."""
        import random
        import string
        suffix = ''.join(random.choices(string.hexdigits.lower(), k=16))
        return f"{prefix}{suffix}"
    
    @staticmethod
    def generate_hotkey(prefix: str = "hotkey") -> str:
        """Generate a mock hotkey."""
        import random
        import string
        suffix = ''.join(random.choices(string.hexdigits.lower(), k=16))
        return f"{prefix}{suffix}"
    
    @staticmethod
    def generate_weights(n: int, normalize: bool = True) -> List[float]:
        """Generate random weights."""
        import random
        weights = [random.random() for _ in range(n)]
        if normalize:
            total = sum(weights)
            weights = [w / total for w in weights]
        return weights


@pytest.fixture
def mock_data():
    """Get mock data generator."""
    return MockDataGenerator()

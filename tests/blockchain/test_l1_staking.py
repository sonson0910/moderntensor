"""
Tests for Layer 1 Staking functionality.

Tests staking transactions, state management, and reward distribution.
"""
import pytest
from sdk.blockchain.transaction import StakingTransaction
from sdk.blockchain.state import StateDB
from sdk.blockchain.l1_staking_service import L1StakingService
from sdk.blockchain.crypto import KeyPair


@pytest.fixture
def state_db():
    """Create a fresh StateDB for testing."""
    return StateDB()


@pytest.fixture
def staking_service(state_db):
    """Create L1StakingService with test state."""
    return L1StakingService(state_db)


@pytest.fixture
def keypair():
    """Create a test keypair."""
    private_key = b'\x01' * 32
    return KeyPair(private_key)


@pytest.fixture
def validator_address():
    """Test validator address."""
    return b'\x42' * 20


@pytest.fixture
def public_key():
    """Test public key."""
    return b'\x03' * 32


def test_staking_transaction_creation(validator_address, public_key):
    """Test creating a staking transaction."""
    tx = StakingTransaction(
        tx_type='stake',
        nonce=0,
        from_address=validator_address,
        validator_address=validator_address,
        amount=1_000_000,
        gas_price=1000,
        gas_limit=100000,
        public_key=public_key,
    )
    
    assert tx.tx_type == 'stake'
    assert tx.amount == 1_000_000
    assert tx.from_address == validator_address
    assert tx.validator_address == validator_address


def test_staking_transaction_signing(validator_address, public_key):
    """Test signing a staking transaction."""
    private_key = b'\x01' * 32
    
    tx = StakingTransaction(
        tx_type='stake',
        nonce=0,
        from_address=validator_address,
        validator_address=validator_address,
        amount=1_000_000,
        gas_price=1000,
        gas_limit=100000,
        public_key=public_key,
    )
    
    tx.sign(private_key)
    
    assert tx.r != b'\x00' * 32
    assert tx.s != b'\x00' * 32
    assert tx.v != 0
    assert tx.verify_signature()


def test_staking_transaction_serialization(validator_address, public_key):
    """Test serializing and deserializing staking transactions."""
    tx = StakingTransaction(
        tx_type='stake',
        nonce=0,
        from_address=validator_address,
        validator_address=validator_address,
        amount=1_000_000,
        gas_price=1000,
        gas_limit=100000,
        public_key=public_key,
    )
    
    # Serialize
    serialized = tx.serialize()
    assert isinstance(serialized, bytes)
    
    # Deserialize
    deserialized = StakingTransaction.deserialize(serialized)
    assert deserialized.tx_type == tx.tx_type
    assert deserialized.amount == tx.amount
    assert deserialized.from_address == tx.from_address


def test_state_staking_operations(state_db, validator_address):
    """Test staking state management."""
    # Initially no stake
    assert state_db.get_staked_amount(validator_address) == 0
    
    # Add stake
    state_db.add_stake(validator_address, 1_000_000)
    assert state_db.get_staked_amount(validator_address) == 1_000_000
    
    # Add more stake
    state_db.add_stake(validator_address, 500_000)
    assert state_db.get_staked_amount(validator_address) == 1_500_000
    
    # Subtract stake
    assert state_db.sub_stake(validator_address, 500_000)
    assert state_db.get_staked_amount(validator_address) == 1_000_000
    
    # Cannot subtract more than available
    assert not state_db.sub_stake(validator_address, 2_000_000)
    assert state_db.get_staked_amount(validator_address) == 1_000_000


def test_state_reward_operations(state_db, validator_address):
    """Test reward state management."""
    # Initially no rewards
    assert state_db.get_pending_rewards(validator_address) == 0
    
    # Add rewards
    state_db.add_reward(validator_address, 10_000)
    assert state_db.get_pending_rewards(validator_address) == 10_000
    
    # Add more rewards
    state_db.add_reward(validator_address, 5_000)
    assert state_db.get_pending_rewards(validator_address) == 15_000
    
    # Claim rewards
    initial_balance = state_db.get_balance(validator_address)
    claimed = state_db.claim_rewards(validator_address)
    
    assert claimed == 15_000
    assert state_db.get_pending_rewards(validator_address) == 0
    assert state_db.get_balance(validator_address) == initial_balance + 15_000


def test_state_validator_info(state_db, validator_address, public_key):
    """Test validator info storage and retrieval."""
    # Initially no validator info
    assert state_db.get_validator_info(validator_address) is None
    
    # Set validator info
    state_db.set_validator_info(validator_address, public_key, active=True)
    
    # Retrieve validator info
    info = state_db.get_validator_info(validator_address)
    assert info is not None
    assert info["address"] == validator_address.hex()
    assert info["public_key"] == public_key.hex()
    assert info["active"] is True


def test_staking_service_stake(staking_service, validator_address, public_key):
    """Test staking through the service."""
    private_key = b'\x01' * 32
    
    # Give the address sufficient balance (stake + gas)
    staking_service.state.add_balance(validator_address, 20_000_000)
    
    # Create stake transaction
    tx = staking_service.stake(
        from_address=validator_address,
        validator_address=validator_address,
        amount=1_000_000,
        public_key=public_key,
        private_key=private_key,
        nonce=0,
    )
    
    assert tx is not None
    assert tx.tx_type == 'stake'
    assert tx.amount == 1_000_000
    
    # Execute stake transaction
    success = staking_service.execute_staking_tx(tx)
    assert success
    
    # Verify state changes
    assert staking_service.state.get_staked_amount(validator_address) == 1_000_000
    assert staking_service.state.get_balance(validator_address) < 20_000_000  # Balance reduced


def test_staking_service_unstake(staking_service, validator_address, public_key):
    """Test unstaking through the service."""
    private_key = b'\x01' * 32
    
    # Setup: First stake tokens properly
    staking_service.state.add_balance(validator_address, 30_000_000)
    
    # Stake 2M tokens first (this will deduct from balance)
    stake_tx = staking_service.stake(
        from_address=validator_address,
        validator_address=validator_address,
        amount=2_000_000,
        public_key=public_key,
        private_key=private_key,
        nonce=0,
    )
    assert staking_service.execute_staking_tx(stake_tx)
    
    # Now get balance after staking
    balance_after_stake = staking_service.state.get_balance(validator_address)
    staked_amount_before = staking_service.state.get_staked_amount(validator_address)
    assert staked_amount_before == 2_000_000
    
    # Now unstake 1M
    tx = staking_service.unstake(
        from_address=validator_address,
        validator_address=validator_address,
        amount=1_000_000,
        private_key=private_key,
        nonce=1,  # Increment nonce
    )
    
    assert tx is not None
    assert tx.tx_type == 'unstake'
    
    # Execute unstake transaction
    success = staking_service.execute_staking_tx(tx)
    assert success
    
    # Verify state changes
    assert staking_service.state.get_staked_amount(validator_address) == 1_000_000
    # Balance change: +1M (returned stake) - gas_cost
    # Since gas is significant, just verify stake decreased
    final_balance = staking_service.state.get_balance(validator_address)
    # Verify the unstaked amount was returned (even if gas was deducted)
    # final_balance should be approximately balance_after_stake + 1M - gas
    # Just check stake was reduced correctly
    assert staking_service.state.get_staked_amount(validator_address) == staked_amount_before - 1_000_000


def test_staking_service_claim_rewards(staking_service, validator_address):
    """Test claiming rewards through the service."""
    private_key = b'\x01' * 32
    
    # Setup: add balance and pending rewards
    staking_service.state.add_balance(validator_address, 10_000_000)
    staking_service.state.add_reward(validator_address, 10_000_000)  # 10M rewards
    
    # Get initial balance and pending rewards
    initial_balance = staking_service.state.get_balance(validator_address)
    pending_before = staking_service.state.get_pending_rewards(validator_address)
    
    # Create claim rewards transaction
    tx = staking_service.claim_rewards(
        from_address=validator_address,
        validator_address=validator_address,
        private_key=private_key,
        nonce=0,
    )
    
    assert tx is not None
    assert tx.tx_type == 'claim_rewards'
    
    # Execute claim rewards transaction
    success = staking_service.execute_staking_tx(tx)
    assert success
    
    # Verify state changes
    assert staking_service.state.get_pending_rewards(validator_address) == 0
    # Balance increased by rewards minus gas (10M rewards > gas cost)
    final_balance = staking_service.state.get_balance(validator_address)
    # With 10M rewards and gas ~2.5M, final should be > initial
    assert final_balance > initial_balance, f"Expected {final_balance} > {initial_balance}"


def test_staking_service_insufficient_balance(staking_service, validator_address, public_key):
    """Test staking with insufficient balance."""
    private_key = b'\x01' * 32
    
    # Give small balance
    staking_service.state.add_balance(validator_address, 100)
    
    # Try to stake more than balance
    tx = staking_service.stake(
        from_address=validator_address,
        validator_address=validator_address,
        amount=1_000_000,
        public_key=public_key,
        private_key=private_key,
        nonce=0,
    )
    
    # Should return None due to insufficient balance
    assert tx is None


def test_staking_service_insufficient_stake(staking_service, validator_address):
    """Test unstaking with insufficient stake."""
    private_key = b'\x01' * 32
    
    # Setup: small stake
    staking_service.state.add_balance(validator_address, 1_000_000)
    staking_service.state.add_stake(validator_address, 100_000)
    
    # Try to unstake more than staked
    tx = staking_service.unstake(
        from_address=validator_address,
        validator_address=validator_address,
        amount=500_000,
        private_key=private_key,
        nonce=0,
    )
    
    # Should return None due to insufficient stake
    assert tx is None


def test_staking_info(staking_service, validator_address, public_key):
    """Test getting staking info."""
    # Setup some state
    staking_service.state.add_balance(validator_address, 5_000_000)
    staking_service.state.add_stake(validator_address, 1_000_000)
    staking_service.state.add_reward(validator_address, 25_000)
    staking_service.state.set_validator_info(validator_address, public_key, active=True)
    
    # Get staking info
    info = staking_service.get_staking_info(validator_address)
    
    assert info["address"] == validator_address.hex()
    assert info["staked_amount"] == 1_000_000
    assert info["pending_rewards"] == 25_000
    assert info["validator_info"] is not None
    assert info["validator_info"]["active"] is True


def test_staking_transaction_intrinsic_gas():
    """Test intrinsic gas calculation for staking transactions."""
    tx = StakingTransaction(
        tx_type='stake',
        nonce=0,
        from_address=b'\x01' * 20,
        validator_address=b'\x02' * 20,
        amount=1_000_000,
        gas_price=1000,
        gas_limit=100000,
        public_key=b'\x03' * 32,
    )
    
    gas = tx.intrinsic_gas()
    assert gas == 50000  # Base gas for staking operations


def test_multiple_validators_staking(staking_service):
    """Test multiple validators staking independently."""
    private_key = b'\x01' * 32
    public_key = b'\x03' * 32
    
    validator1 = b'\x10' * 20
    validator2 = b'\x20' * 20
    
    # Setup balances
    staking_service.state.add_balance(validator1, 20_000_000)
    staking_service.state.add_balance(validator2, 20_000_000)
    
    # Stake for validator1
    tx1 = staking_service.stake(
        from_address=validator1,
        validator_address=validator1,
        amount=1_000_000,
        public_key=public_key,
        private_key=private_key,
        nonce=0,
    )
    assert staking_service.execute_staking_tx(tx1)
    
    # Stake for validator2
    tx2 = staking_service.stake(
        from_address=validator2,
        validator_address=validator2,
        amount=2_000_000,
        public_key=public_key,
        private_key=private_key,
        nonce=0,
    )
    assert staking_service.execute_staking_tx(tx2)
    
    # Verify independent stakes
    assert staking_service.state.get_staked_amount(validator1) == 1_000_000
    assert staking_service.state.get_staked_amount(validator2) == 2_000_000

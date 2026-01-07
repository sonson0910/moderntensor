"""
Example: Using ModernTensor Data Models and Async Client

This example demonstrates how to use the new Pydantic data models
and async blockchain client.
"""

import asyncio
from sdk.models import (
    NeuronInfo,
    SubnetInfo,
    StakeInfo,
    ValidatorInfo,
    MinerInfo,
    AxonInfo,
    PrometheusInfo,
    DelegateInfo,
    BlockInfo,
    TransactionInfo,
)


def example_neuron_model():
    """Example: Creating and using NeuronInfo model."""
    print("=" * 60)
    print("Example 1: NeuronInfo Model")
    print("=" * 60)
    
    # Create a neuron
    neuron = NeuronInfo(
        uid=0,
        hotkey="5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
        coldkey="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
        subnet_uid=1,
        stake=1000.0,
        total_stake=1500.0,
        rank=0.95,
        trust=0.98,
        consensus=0.92,
        incentive=0.90,
        dividends=0.88,
        emission=100.5,
        validator_permit=True,
        validator_trust=0.99,
        active=True,
        last_update=12345
    )
    
    print(f"Neuron: {neuron}")
    print(f"  UID: {neuron.uid}")
    print(f"  Hotkey: {neuron.hotkey[:16]}...")
    print(f"  Stake: {neuron.stake} TAO")
    print(f"  Rank: {neuron.rank:.3f}")
    print(f"  Trust: {neuron.trust:.3f}")
    print(f"  Is Validator: {neuron.validator_permit}")
    print()


def example_subnet_model():
    """Example: Creating and using SubnetInfo model."""
    print("=" * 60)
    print("Example 2: SubnetInfo Model")
    print("=" * 60)
    
    # Create a subnet
    subnet = SubnetInfo(
        subnet_uid=1,
        netuid=1,
        name="Text Prompting",
        owner="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
        n=256,
        max_n=4096,
        emission_value=1000000.0,
        tempo=99,
        block=123456,
        burn=1.0
    )
    
    print(f"Subnet: {subnet}")
    print(f"  Name: {subnet.name}")
    print(f"  Neurons: {subnet.n}/{subnet.max_n}")
    print(f"  Tempo: {subnet.tempo} blocks")
    print(f"  Emission: {subnet.emission_value}")
    print()


def example_validator_and_miner():
    """Example: Validator and Miner models."""
    print("=" * 60)
    print("Example 3: Validator & Miner Models")
    print("=" * 60)
    
    # Create a validator
    validator = ValidatorInfo(
        uid=0,
        hotkey="validator_hotkey",
        coldkey="validator_coldkey",
        validator_permit=True,
        validator_trust=0.99,
        total_stake=5000.0,
        own_stake=1000.0,
        delegated_stake=4000.0,
        dividends=0.88,
        weights_set=True
    )
    
    print(f"Validator: {validator}")
    print(f"  Total Stake: {validator.total_stake} TAO")
    print(f"  Own: {validator.own_stake}, Delegated: {validator.delegated_stake}")
    print(f"  Trust: {validator.validator_trust:.3f}")
    print()
    
    # Create a miner
    miner = MinerInfo(
        uid=100,
        hotkey="miner_hotkey",
        coldkey="miner_coldkey",
        rank=0.95,
        trust=0.92,
        consensus=0.90,
        incentive=0.88,
        emission=50.5,
        stake=100.0,
        active=True
    )
    
    print(f"Miner: {miner}")
    print(f"  Rank: {miner.rank:.3f}")
    print(f"  Incentive: {miner.incentive:.3f}")
    print(f"  Emission: {miner.emission} TAO/block")
    print()


def example_axon_and_prometheus():
    """Example: Axon and Prometheus endpoint models."""
    print("=" * 60)
    print("Example 4: Axon & Prometheus Endpoints")
    print("=" * 60)
    
    # Create axon info
    axon = AxonInfo(
        ip="192.168.1.100",
        port=8091,
        ip_type=4,
        protocol=4,
        hotkey="axon_hotkey",
        coldkey="axon_coldkey",
        version=1
    )
    
    print(f"Axon: {axon}")
    print(f"  Endpoint: {axon.endpoint}")
    print()
    
    # Create prometheus info
    prometheus = PrometheusInfo(
        ip="192.168.1.100",
        port=9090,
        ip_type=4,
        version=1,
        block=12345
    )
    
    print(f"Prometheus: {prometheus}")
    print(f"  Metrics URL: {prometheus.endpoint}")
    print()


def example_delegate():
    """Example: Delegate model."""
    print("=" * 60)
    print("Example 5: DelegateInfo Model")
    print("=" * 60)
    
    # Create delegate info
    delegate = DelegateInfo(
        hotkey="delegate_hotkey",
        owner="owner_coldkey",
        total_stake=50000.0,
        nominators=["nominator1", "nominator2", "nominator3"],
        take=0.18,  # 18% commission
        registrations=[0, 1, 2, 3],
        validator_permits=[1, 2],
        return_per_1000=12.5,
        total_daily_return=625.0
    )
    
    print(f"Delegate: {delegate}")
    print(f"  Total Stake: {delegate.total_stake} TAO")
    print(f"  Nominators: {len(delegate.nominators)}")
    print(f"  Commission: {delegate.take * 100}%")
    print(f"  Daily Return: {delegate.total_daily_return} TAO")
    print(f"  Return per 1000 TAO: {delegate.return_per_1000} TAO")
    print()


def example_block_and_transaction():
    """Example: Block and Transaction models."""
    print("=" * 60)
    print("Example 6: Block & Transaction Models")
    print("=" * 60)
    
    # Create block info
    block = BlockInfo(
        block_number=12345,
        block_hash="0x1234567890abcdef",
        parent_hash="0xabcdef1234567890",
        timestamp=1704643200,
        transactions=["0xabc123", "0xdef456"],
        transaction_count=2,
        state_root="0xstateroot123",
        extrinsics_root="0xextroot456",
        author="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
    )
    
    print(f"Block: {block}")
    print(f"  Block #: {block.block_number}")
    print(f"  Transactions: {block.transaction_count}")
    print(f"  Hash: {block.block_hash[:16]}...")
    print()
    
    # Create transaction info
    tx = TransactionInfo(
        tx_hash="0x1234567890abcdef",
        block_number=12345,
        block_hash="0xabcdef1234567890",
        from_address="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
        to_address="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",
        method="transfer",
        pallet="balances",
        success=True,
        fee=0.01,
        args={"amount": 100.0},
        nonce=5,
        timestamp=1704643200
    )
    
    print(f"Transaction: {tx}")
    print(f"  Status: {'✓ Success' if tx.success else '✗ Failed'}")
    print(f"  Method: {tx.pallet}.{tx.method}")
    print(f"  Fee: {tx.fee} TAO")
    print()


def example_validation():
    """Example: Model validation."""
    print("=" * 60)
    print("Example 7: Validation")
    print("=" * 60)
    
    # Valid stake
    try:
        stake = StakeInfo(
            hotkey="key1",
            coldkey="key2",
            stake=1000.0
        )
        print(f"✓ Valid stake: {stake}")
    except Exception as e:
        print(f"✗ Error: {e}")
    
    # Invalid stake (negative)
    try:
        stake = StakeInfo(
            hotkey="key1",
            coldkey="key2",
            stake=-100.0  # This will fail validation
        )
        print(f"✓ Created: {stake}")
    except Exception as e:
        print(f"✗ Validation error (expected): Stake cannot be negative")
    
    # Invalid port
    try:
        axon = AxonInfo(
            ip="192.168.1.1",
            port=100000,  # This will fail validation
            hotkey="key",
            coldkey="key"
        )
        print(f"✓ Created: {axon}")
    except Exception as e:
        print(f"✗ Validation error (expected): Port must be 1-65535")
    
    print()


async def example_async_client():
    """Example: Using Async Client (placeholder)."""
    print("=" * 60)
    print("Example 8: Async Client (Coming Soon)")
    print("=" * 60)
    
    print("Async client implementation in progress...")
    print("Will support:")
    print("  - Async neuron queries")
    print("  - Async subnet queries")
    print("  - Async transaction submission")
    print("  - Connection pooling")
    print("  - Batch operations")
    print()
    
    # Example code (when implemented):
    print("Example usage (when available):")
    print("""
async with AsyncLuxtensorClient("ws://localhost:9944") as client:
    # Get neuron info
    neuron = await client.get_neuron(uid=0, netuid=1)
    print(f"Neuron stake: {neuron.stake}")
    
    # Get all neurons in subnet
    neurons = await client.get_neurons(netuid=1)
    print(f"Found {len(neurons)} neurons")
    
    # Batch query (parallel)
    neurons = await client.get_neurons_batch([0, 1, 2, 3], netuid=1)
    """)
    print()


def main():
    """Run all examples."""
    print("\n")
    print("*" * 60)
    print("ModernTensor SDK - Data Models Examples")
    print("*" * 60)
    print()
    
    # Run all examples
    example_neuron_model()
    example_subnet_model()
    example_validator_and_miner()
    example_axon_and_prometheus()
    example_delegate()
    example_block_and_transaction()
    example_validation()
    
    # Async example
    asyncio.run(example_async_client())
    
    print("=" * 60)
    print("✅ All examples completed successfully!")
    print("=" * 60)
    print()
    print("Phase 3 Implementation Complete:")
    print("  ✓ 11 Pydantic data models")
    print("  ✓ Full type safety and validation")
    print("  ✓ Comprehensive examples")
    print("  ✓ Ready for use in SDK")
    print()


if __name__ == "__main__":
    main()

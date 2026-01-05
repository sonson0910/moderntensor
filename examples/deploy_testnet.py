#!/usr/bin/env python3
"""
ModernTensor Testnet Deployment Example

This script demonstrates how to deploy a ModernTensor testnet.
"""

import asyncio
from pathlib import Path
from sdk.testnet import GenesisGenerator, Faucet, BootstrapNode
from sdk.testnet.deployment import deploy_testnet


def main():
    """Main deployment function"""
    print("=" * 70)
    print("ModernTensor Testnet Deployment")
    print("=" * 70)
    print()
    
    # Option 1: Quick deployment with defaults
    print("Option 1: Quick Deployment")
    print("-" * 70)
    print("Deploy a testnet with default settings:")
    print()
    print("  deploy_testnet(")
    print("      network_name='moderntensor-testnet',")
    print("      chain_id=9999,")
    print("      num_validators=5")
    print("  )")
    print()
    
    # Option 2: Custom genesis configuration
    print("Option 2: Custom Genesis Configuration")
    print("-" * 70)
    print("Create a custom genesis configuration:")
    print()
    
    generator = GenesisGenerator()
    config = generator.create_testnet_config(
        chain_id=9999,
        network_name="moderntensor-testnet",
        validator_count=5,
        validator_stake=10_000_000,
        faucet_balance=1_000_000_000_000
    )
    
    print(f"✅ Created genesis config:")
    print(f"   Chain ID: {config.chain_id}")
    print(f"   Network: {config.network_name}")
    print(f"   Validators: {len(config.initial_validators)}")
    print(f"   Total Supply: {config.total_supply:,}")
    print()
    
    # Validate configuration
    errors = generator.validate_config()
    if errors:
        print("⚠️  Validation warnings:")
        for error in errors:
            print(f"   - {error}")
    else:
        print("✅ Configuration is valid")
    print()
    
    # Option 3: Run individual services
    print("Option 3: Individual Services")
    print("-" * 70)
    print()
    
    # Faucet example
    print("🚰 Token Faucet:")
    print("   - Distributes test tokens to users")
    print("   - Rate limiting: 3 requests per address per hour")
    print("   - Daily limit: 1000 requests")
    print()
    
    # Bootstrap node example
    print("🌐 Bootstrap Node:")
    print("   - Helps new nodes discover peers")
    print("   - Tracks up to 1000 active peers")
    print("   - Automatic stale peer cleanup")
    print()
    
    # Monitoring example
    print("📊 Monitoring:")
    print("   - Real-time node health tracking")
    print("   - Network metrics calculation")
    print("   - Alert system for issues")
    print()
    
    # Deployment instructions
    print("=" * 70)
    print("Deployment Instructions")
    print("=" * 70)
    print()
    print("1. Generate configuration:")
    print("   python examples/deploy_testnet.py")
    print()
    print("2. Start the testnet:")
    print("   docker-compose -f docker-compose.testnet.yml up -d")
    print()
    print("3. Check status:")
    print("   docker-compose -f docker-compose.testnet.yml ps")
    print()
    print("4. View logs:")
    print("   docker-compose -f docker-compose.testnet.yml logs -f validator-1")
    print()
    print("5. Stop the testnet:")
    print("   docker-compose -f docker-compose.testnet.yml down")
    print()
    
    # Access information
    print("=" * 70)
    print("Access Information")
    print("=" * 70)
    print()
    print("Validators:")
    for i in range(5):
        print(f"  Validator {i+1}: http://localhost:{8545 + i}")
    print()
    print("Services:")
    print("  Faucet:    http://localhost:8080")
    print("  Explorer:  http://localhost:3000")
    print("  Bootstrap: tcp://localhost:30303")
    print()
    
    print("=" * 70)
    print("For detailed documentation, see PHASE8_SUMMARY.md")
    print("=" * 70)


async def demo_async_features():
    """Demonstrate async features"""
    print("\nAsync Features Demo")
    print("-" * 70)
    
    # Test faucet
    print("\n🚰 Testing Faucet...")
    faucet = Faucet()
    
    test_address = "0x1234567890123456789012345678901234567890"
    result = await faucet.request_tokens(test_address)
    
    if result['success']:
        print(f"✅ Sent {result['amount']:,} tokens")
        print(f"   TX Hash: {result['tx_hash']}")
    
    stats = faucet.get_stats()
    print(f"✅ Faucet Stats:")
    print(f"   Total Requests: {stats['total_requests']}")
    print(f"   Successful: {stats['successful_requests']}")
    
    # Test bootstrap node
    print("\n🌐 Testing Bootstrap Node...")
    bootstrap = BootstrapNode()
    
    # Register some peers
    for i in range(3):
        bootstrap.register_peer(
            node_id=f"validator-{i+1}",
            address=f"192.168.1.{i+1}",
            port=30303
        )
    
    peers = bootstrap.get_peers(max_count=10)
    print(f"✅ Registered {len(peers)} peers")
    
    stats = bootstrap.get_stats()
    print(f"✅ Bootstrap Stats:")
    print(f"   Total Peers Seen: {stats['total_peers_seen']}")
    print(f"   Active Peers: {stats['active_peers']}")


if __name__ == "__main__":
    # Run sync demo
    main()
    
    # Run async demo
    print("\n" + "=" * 70)
    asyncio.run(demo_async_features())
    print("=" * 70)
    print("\n✅ Demo complete!")

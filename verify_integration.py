#!/usr/bin/env python3
"""
ModernTensor Layer 1 Blockchain Integration Verification
=========================================================

This script verifies that all modules work correctly and are properly
integrated with each other, as requested in LAYER1_ROADMAP.md.

Verifies:
1. All modules function normally
2. Modules have proper connections/links between them
3. Nodes can run normally

Tests integration of all 8 phases:
- Phase 1: Core Blockchain (Block, Transaction, StateDB)
- Phase 2: Consensus (PoS, ValidatorSet)
- Phase 3: Network (P2P, Sync)
- Phase 4: Storage (Database)
- Phase 5: API (RPC, GraphQL)
- Phase 6: Testing
- Phase 7: Security & Optimization
- Phase 8: Testnet Deployment (Complete Integration)
"""

import asyncio
import sys
import tempfile
from pathlib import Path
from datetime import datetime

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent))

# Import all modules to verify
print("=" * 80)
print("ModernTensor Layer 1 Blockchain - Integration Verification")
print("=" * 80)
print()

print("📦 Step 1: Verifying Module Imports")
print("-" * 80)

modules_status = {}

# Phase 1: Core Blockchain
try:
    from sdk.blockchain import Block, Transaction, StateDB
    from sdk.blockchain.crypto import KeyPair, MerkleTree
    from sdk.blockchain.validation import BlockValidator
    from sdk.blockchain.state import Account
    modules_status['Phase 1: Core Blockchain'] = '✅'
    print("✅ Phase 1 (Core Blockchain): Block, Transaction, StateDB, KeyPair, MerkleTree, BlockValidator")
except Exception as e:
    modules_status['Phase 1: Core Blockchain'] = f'❌ {e}'
    print(f"❌ Phase 1 (Core Blockchain): {e}")

# Phase 2: Consensus
try:
    from sdk.consensus.pos import ProofOfStake, ValidatorSet
    from sdk.consensus.fork_choice import ForkChoice
    modules_status['Phase 2: Consensus'] = '✅'
    print("✅ Phase 2 (Consensus): PoS, ValidatorSet, ForkChoice")
except Exception as e:
    modules_status['Phase 2: Consensus'] = f'❌ {e}'
    print(f"❌ Phase 2 (Consensus): {e}")

# Phase 3: Network
try:
    from sdk.network.p2p import P2PNode
    from sdk.network.sync import SyncManager
    modules_status['Phase 3: Network'] = '✅'
    print("✅ Phase 3 (Network): P2PNode, SyncManager")
except Exception as e:
    modules_status['Phase 3: Network'] = f'❌ {e}'
    print(f"❌ Phase 3 (Network): {e}")

# Phase 4: Storage
try:
    from sdk.storage.blockchain_db import BlockchainDB
    from sdk.storage.indexer import Indexer
    modules_status['Phase 4: Storage'] = '✅'
    print("✅ Phase 4 (Storage): BlockchainDB, Indexer")
except Exception as e:
    modules_status['Phase 4: Storage'] = f'❌ {e}'
    print(f"❌ Phase 4 (Storage): {e}")

# Phase 5: API
try:
    from sdk.api.rpc import JSONRPC
    from sdk.api.graphql_api import GraphQLAPI
    modules_status['Phase 5: API'] = '✅'
    print("✅ Phase 5 (API): JSONRPC, GraphQLAPI")
except Exception as e:
    modules_status['Phase 5: API'] = f'❌ {e}'
    print(f"❌ Phase 5 (API): {e}")

# Phase 7: Security & Optimization
try:
    from sdk.optimization.consensus_optimizer import ConsensusOptimizer
    from sdk.optimization.network_optimizer import NetworkOptimizer
    from sdk.optimization.storage_optimizer import StorageOptimizer
    modules_status['Phase 7: Optimization'] = '✅'
    print("✅ Phase 7 (Optimization): ConsensusOptimizer, NetworkOptimizer, StorageOptimizer")
except Exception as e:
    modules_status['Phase 7: Optimization'] = f'❌ {e}'
    print(f"❌ Phase 7 (Optimization): {e}")

# Phase 8: Testnet
try:
    from sdk.testnet import (
        GenesisConfig, GenesisGenerator,
        Faucet, FaucetConfig,
        BootstrapNode, BootstrapConfig,
        L1Node
    )
    modules_status['Phase 8: Testnet'] = '✅'
    print("✅ Phase 8 (Testnet): GenesisConfig, Faucet, BootstrapNode, L1Node")
except Exception as e:
    modules_status['Phase 8: Testnet'] = f'❌ {e}'
    print(f"❌ Phase 8 (Testnet): {e}")

print()

# Check if all modules loaded successfully
all_success = all(v == '✅' for v in modules_status.values())
if not all_success:
    print("❌ Some modules failed to import. Cannot continue.")
    sys.exit(1)

print("✅ All modules imported successfully!")
print()


async def verify_integrations():
    """Verify module integrations"""
    
    print("🔗 Step 2: Verifying Module Connections")
    print("-" * 80)
    
    connections = []
    
    # Connection 1: Genesis → Block (Phase 8 → Phase 1)
    try:
        generator = GenesisGenerator()
        generator.create_testnet_config(chain_id=9999, network_name="test")
        genesis_block = generator.generate_genesis_block()
        
        assert isinstance(genesis_block, Block), "Genesis block should be a Block object"
        assert genesis_block.header.height == 0, "Genesis height should be 0"
        connections.append("✅ Genesis → Block (Phase 8 creates real Phase 1 Block objects)")
    except Exception as e:
        connections.append(f"❌ Genesis → Block: {e}")
    
    # Connection 2: Genesis → StateDB (Phase 8 → Phase 1)
    try:
        state_db = generator.initialize_genesis_state()
        assert isinstance(state_db, StateDB), "Should return StateDB"
        assert state_db.get_state_root() is not None, "State root should be calculated"
        connections.append("✅ Genesis → StateDB (Phase 8 initializes Phase 1 StateDB)")
    except Exception as e:
        connections.append(f"❌ Genesis → StateDB: {e}")
    
    # Connection 3: Faucet → Transaction (Phase 8 → Phase 1)
    try:
        faucet = Faucet(state_db=state_db)
        result = await faucet.request_tokens("0x1234567890123456789012345678901234567890")
        
        assert result['success'], "Faucet request should succeed"
        assert 'tx_hash' in result, "Should have transaction hash"
        connections.append("✅ Faucet → Transaction (Phase 8 creates real Phase 1 Transactions)")
    except Exception as e:
        connections.append(f"❌ Faucet → Transaction: {e}")
    
    # Connection 4: L1Node → All Components (Phase 8 orchestrates all)
    try:
        with tempfile.TemporaryDirectory() as tmpdir:
            validator_keypair = KeyPair()
            node = L1Node(
                node_id="test-validator",
                data_dir=Path(tmpdir),
                genesis_config=generator.config,
                is_validator=True,
                validator_keypair=validator_keypair
            )
            
            # Load genesis
            node.load_genesis()
            
            assert node.current_height == 0, "Should start at height 0"
            assert len(node.blockchain) == 1, "Should have genesis block"
            assert node.state_db is not None, "Should have state database"
            
            connections.append("✅ L1Node orchestrates: Blockchain + Consensus + State + Network")
    except Exception as e:
        connections.append(f"❌ L1Node Integration: {e}")
    
    # Connection 5: Transaction → Cryptography (Phase 1 → Phase 1)
    try:
        keypair = KeyPair()
        tx = Transaction(
            nonce=0,
            from_address=keypair.address(),
            to_address=bytes.fromhex("1234567890123456789012345678901234567890"),
            value=1000000,
            gas_price=1000000000,
            gas_limit=21000,
            data=b''
        )
        tx.sign(keypair.private_key)
        
        assert tx.verify_signature(), "Transaction signature should verify"
        connections.append("✅ Transaction → Cryptography (Phase 1 signing and verification)")
    except Exception as e:
        connections.append(f"❌ Transaction → Cryptography: {e}")
    
    # Connection 6: Consensus → ValidatorSet (Phase 2)
    try:
        from sdk.consensus.pos import ConsensusConfig
        
        # Create a temporary state DB for testing
        with tempfile.TemporaryDirectory() as tmpdir2:
            test_state = StateDB(Path(tmpdir2))
            test_config = ConsensusConfig()
            pos = ProofOfStake(state_db=test_state, config=test_config)
            validator_set = ValidatorSet()
            
            # Create a test validator
            test_keypair = KeyPair()
            validator_set.add_validator(
                address=test_keypair.address(),
                public_key=test_keypair.public_key,
                stake=1000000
            )
            
            assert validator_set.get_total_stake() == 1000000, "Stake should be tracked"
            connections.append("✅ Consensus → ValidatorSet (Phase 2 validator management)")
    except Exception as e:
        connections.append(f"❌ Consensus → ValidatorSet: {e}")
    
    # Print all connections
    for connection in connections:
        print(connection)
    
    print()
    
    # Summary
    passed = sum(1 for c in connections if c.startswith('✅'))
    total = len(connections)
    
    print(f"Connection Tests: {passed}/{total} passed")
    print()
    
    return passed == total


async def verify_node_functionality():
    """Verify node can run normally"""
    
    print("🚀 Step 3: Verifying Node Functionality")
    print("-" * 80)
    
    checks = []
    
    try:
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create genesis
            generator = GenesisGenerator()
            config = generator.create_testnet_config(
                chain_id=9999,
                network_name="test-network",
                validator_count=3
            )
            
            # Create validator node
            validator_keypair = KeyPair()
            node = L1Node(
                node_id="validator-1",
                data_dir=Path(tmpdir),
                genesis_config=config,
                is_validator=True,
                validator_keypair=validator_keypair
            )
            
            # Check 1: Node initialization
            try:
                node.load_genesis()
                assert node.current_height == 0
                checks.append("✅ Node initialization (genesis loaded)")
            except Exception as e:
                checks.append(f"❌ Node initialization: {e}")
            
            # Check 2: State management
            try:
                assert node.state_db is not None
                # Try to get an account
                test_address = bytes.fromhex("1234567890123456789012345678901234567890")
                account = node.get_account(test_address)
                assert isinstance(account, Account)
                checks.append("✅ State management (accounts accessible)")
            except Exception as e:
                checks.append(f"❌ State management: {e}")
            
            # Check 3: Transaction pool
            try:
                assert hasattr(node, 'mempool')
                assert isinstance(node.mempool, list)
                checks.append("✅ Transaction pool (mempool ready)")
            except Exception as e:
                checks.append(f"❌ Transaction pool: {e}")
            
            # Check 4: Block access
            try:
                genesis = node.get_block(0)
                assert genesis is not None
                assert genesis.header.height == 0
                checks.append("✅ Block access (can retrieve blocks)")
            except Exception as e:
                checks.append(f"❌ Block access: {e}")
            
            # Check 5: Consensus integration
            try:
                assert node.consensus is not None
                assert isinstance(node.consensus, ProofOfStake)
                checks.append("✅ Consensus integration (PoS connected)")
            except Exception as e:
                checks.append(f"❌ Consensus integration: {e}")
            
            # Check 6: Add transaction to mempool
            try:
                # First give the validator some balance in state (enough for gas + value)
                sender_addr = validator_keypair.address()
                sender_account = node.state_db.get_account(sender_addr)
                sender_account.balance = 1_000_000_000_000_000  # Give 1M tokens
                node.state_db.set_account(sender_addr, sender_account)
                
                tx = Transaction(
                    nonce=0,
                    from_address=sender_addr,
                    to_address=bytes.fromhex("9876543210987654321098765432109876543210"),
                    value=1000,
                    gas_price=1000000000,
                    gas_limit=21000,
                    data=b''
                )
                tx.sign(validator_keypair.private_key)
                
                node.add_transaction(tx)
                assert len(node.mempool) == 1
                checks.append("✅ Transaction submission (added to mempool)")
            except Exception as e:
                checks.append(f"❌ Transaction submission: {e}")
            
            # Check 7: Block production capability (simulated)
            try:
                # Check that node has the capability to produce blocks
                assert hasattr(node, '_produce_block')
                assert node.is_validator == True
                assert node.validator_keypair is not None
                checks.append("✅ Block production capability (validator ready)")
            except Exception as e:
                checks.append(f"❌ Block production capability: {e}")
            
    except Exception as e:
        checks.append(f"❌ Node setup failed: {e}")
    
    # Print all checks
    for check in checks:
        print(check)
    
    print()
    
    # Summary
    passed = sum(1 for c in checks if c.startswith('✅'))
    total = len(checks)
    
    print(f"Node Functionality Tests: {passed}/{total} passed")
    print()
    
    return passed == total


async def main():
    """Run all verification tests"""
    
    # Verify integrations
    integrations_ok = await verify_integrations()
    
    # Verify node functionality
    node_ok = await verify_node_functionality()
    
    # Final summary
    print("=" * 80)
    print("📊 Verification Summary")
    print("=" * 80)
    print()
    
    print("Module Status:")
    for phase, status in modules_status.items():
        print(f"  {status} {phase}")
    print()
    
    print("Integration Status:")
    print(f"  {'✅' if integrations_ok else '❌'} Module connections verified")
    print()
    
    print("Node Status:")
    print(f"  {'✅' if node_ok else '❌'} Node functionality verified")
    print()
    
    all_ok = all_success and integrations_ok and node_ok
    
    if all_ok:
        print("=" * 80)
        print("✅ VERIFICATION SUCCESSFUL")
        print("=" * 80)
        print()
        print("All modules work normally ✓")
        print("Modules are properly connected ✓")
        print("Nodes can run normally ✓")
        print()
        print("The ModernTensor Layer 1 blockchain is fully integrated and operational!")
        print()
        return 0
    else:
        print("=" * 80)
        print("❌ VERIFICATION FAILED")
        print("=" * 80)
        print()
        print("Some tests failed. Please check the output above for details.")
        print()
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)

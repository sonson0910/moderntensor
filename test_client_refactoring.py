"""
Test script for LuxtensorClient with new mixin architecture.

Tests:
1. Import verification
2. Client instantiation
3. MRO (Method Resolution Order)
4. Method availability
5. Consensus integration
6. Utils methods
"""

import sys
sys.path.insert(0, '.')

def test_imports():
    """Test all imports work correctly."""
    print("=" * 60)
    print("TEST 1: Import Verification")
    print("=" * 60)

    try:
        from sdk.client import (
            LuxtensorClient,
            BaseClient,
            ConsensusMixin,
            UtilsMixin,
            BlockchainMixin,
            AccountMixin,
            StakingMixin,
            TransactionMixin,
            SubnetMixin,
            NeuronMixin,
        )
        print("✅ All imports successful")
        return True
    except ImportError as e:
        print(f"❌ Import failed: {e}")
        return False


def test_client_instantiation():
    """Test client can be instantiated."""
    print("\n" + "=" * 60)
    print("TEST 2: Client Instantiation")
    print("=" * 60)

    try:
        from sdk.client import LuxtensorClient

        # Basic client
        client = LuxtensorClient()
        print(f"✅ Basic client created: {type(client).__name__}")

        # Client with consensus
        client_with_consensus = LuxtensorClient(enable_consensus=False)
        print(f"✅ Client with consensus param created")

        return True
    except Exception as e:
        print(f"❌ Instantiation failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_mro():
    """Test Method Resolution Order."""
    print("\n" + "=" * 60)
    print("TEST 3: Method Resolution Order (MRO)")
    print("=" * 60)

    try:
        from sdk.client import LuxtensorClient

        mro = [c.__name__ for c in LuxtensorClient.__mro__]
        print("MRO Chain:")
        for i, cls in enumerate(mro, 1):
            print(f"  {i}. {cls}")

        # Expected order
        expected = [
            'LuxtensorClient',
            'ConsensusMixin',
            'UtilsMixin',
            'SubnetMixin',
            'NeuronMixin',
            'StakingMixin',
            'TransactionMixin',
            'AccountMixin',
            'BlockchainMixin',
            'BaseClient',
        ]

        actual = mro[:len(expected)]
        if actual == expected:
            print("✅ MRO order is correct")
            return True
        else:
            print(f"⚠️ MRO differs from expected")
            print(f"Expected: {expected}")
            print(f"Actual:   {actual}")
            return False
    except Exception as e:
        print(f"❌ MRO test failed: {e}")
        return False


def test_method_availability():
    """Test all expected methods are available."""
    print("\n" + "=" * 60)
    print("TEST 4: Method Availability")
    print("=" * 60)

    try:
        from sdk.client import LuxtensorClient
        client = LuxtensorClient()

        # Get all public methods
        methods = [m for m in dir(client) if not m.startswith('_') and callable(getattr(client, m))]
        print(f"Total public methods: {len(methods)}")

        # Check for key methods from each mixin
        key_methods = {
            'ConsensusMixin': ['verify_block', 'check_finality', 'get_consensus_state'],
            'UtilsMixin': ['health_check', 'validate_address', 'wait_for_block'],
            'BlockchainMixin': ['get_block_number', 'get_block', 'get_chain_id'],
            'AccountMixin': ['get_account', 'get_balance', 'get_nonce'],
            'StakingMixin': ['get_stake', 'get_total_stake', 'get_delegates'],
            'SubnetMixin': ['get_all_subnets', 'register_subnet'],
            'NeuronMixin': ['get_neurons', 'get_neuron'],
            'TransactionMixin': ['send_transaction'],
        }

        all_present = True
        for mixin, expected_methods in key_methods.items():
            print(f"\n{mixin}:")
            for method in expected_methods:
                present = method in methods
                status = "✅" if present else "❌"
                print(f"  {status} {method}")
                if not present:
                    all_present = False

        if all_present:
            print(f"\n✅ All key methods available ({len(methods)} total)")
            return True
        else:
            print(f"\n⚠️ Some methods missing")
            return False

    except Exception as e:
        print(f"❌ Method availability test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_consensus_integration():
    """Test consensus methods."""
    print("\n" + "=" * 60)
    print("TEST 5: Consensus Integration")
    print("=" * 60)

    try:
        from sdk.client import LuxtensorClient
        client = LuxtensorClient()

        # Check consensus methods exist
        assert hasattr(client, 'init_consensus'), "Missing init_consensus"
        assert hasattr(client, 'verify_block'), "Missing verify_block"
        assert hasattr(client, 'check_finality'), "Missing check_finality"
        assert hasattr(client, 'get_consensus_state'), "Missing get_consensus_state"
        assert hasattr(client, 'calculate_block_reward'), "Missing calculate_block_reward"
        assert hasattr(client, 'is_circuit_broken'), "Missing is_circuit_broken"

        print("✅ All consensus methods present")

        # Try to initialize consensus (will fail without network, but should not crash)
        print("Testing consensus initialization...")
        try:
            client.init_consensus()
            print("✅ Consensus initialization works")
        except Exception as e:
            print(f"⚠️ Consensus init raised (expected without network): {type(e).__name__}")

        return True
    except AssertionError as e:
        print(f"❌ Consensus integration test failed: {e}")
        return False
    except Exception as e:
        print(f"❌ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_utils_methods():
    """Test utility methods."""
    print("\n" + "=" * 60)
    print("TEST 6: Utils Methods")
    print("=" * 60)

    try:
        from sdk.client import LuxtensorClient
        client = LuxtensorClient()

        # Test address validation
        assert client.validate_address("0x" + "00" * 20) == True
        assert client.validate_address("invalid") == False
        print("✅ validate_address works")

        # Test wei conversion
        assert client.to_wei(1.0) == 10**18
        assert client.from_wei(10**18) == 1.0
        print("✅ to_wei/from_wei works")

        # Health check (will fail without network)
        try:
            health = client.health_check()
            print(f"⚠️ health_check returned: {health}")
        except Exception:
            print("⚠️ health_check failed (expected without network)")

        print("✅ Utils methods functional")
        return True
    except Exception as e:
        print(f"❌ Utils test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    """Run all tests."""
    print("\n" + "=" * 60)
    print("LUXTENSOR CLIENT REFACTORING TEST SUITE")
    print("=" * 60)

    tests = [
        test_imports,
        test_client_instantiation,
        test_mro,
        test_method_availability,
        test_consensus_integration,
        test_utils_methods,
    ]

    results = []
    for test in tests:
        try:
            result = test()
            results.append(result)
        except Exception as e:
            print(f"\n❌ Test crashed: {e}")
            import traceback
            traceback.print_exc()
            results.append(False)

    # Summary
    print("\n" + "=" * 60)
    print("TEST SUMMARY")
    print("=" * 60)
    passed = sum(results)
    total = len(results)
    print(f"Passed: {passed}/{total}")
    print(f"Failed: {total - passed}/{total}")

    if passed == total:
        print("\n✅ ALL TESTS PASSED! Client refactoring successful!")
        return 0
    else:
        print(f"\n⚠️ {total - passed} test(s) failed. Review errors above.")
        return 1


if __name__ == "__main__":
    sys.exit(main())

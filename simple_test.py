"""Simple client test - ASCII only"""
import sys
sys.path.insert(0, '.')

from sdk.client import LuxtensorClient

print("=" * 60)
print("Testing Client Refactoring")
print("=" * 60)

tests_passed = 0
tests_failed = 0

try:
    # Test 1: Import
    print("\n[1/6] Import check...")
    client = LuxtensorClient()
    print("  PASS: Client created")
    tests_passed += 1
except Exception as e:
    print(f"  FAIL: {e}")
    tests_failed += 1

try:
    # Test 2: MRO
    print("\n[2/6] MRO check...")
    mro_names = [c.__name__ for c in LuxtensorClient.__mro__]
    assert 'ConsensusMixin' in mro_names
    assert 'UtilsMixin' in mro_names
    print("  PASS: Mixins in MRO")
    tests_passed += 1
except Exception as e:
    print(f"  FAIL: {e}")
    tests_failed += 1

try:
    # Test 3: Method availability
    print("\n[3/6] Method availability...")
    assert hasattr(client, 'verify_block')
    assert hasattr(client, 'health_check')
    assert hasattr(client, 'get_stake')
    print("  PASS: Key methods available")
    tests_passed += 1
except Exception as e:
    print(f"  FAIL: {e}")
    tests_failed += 1

try:
    # Test 4: Consensus integration
    print("\n[4/6] Consensus integration...")
    client.init_consensus()
    assert client._consensus_initialized
    print("  PASS: Consensus initialized")
    tests_passed += 1
except Exception as e:
    print(f"  FAIL: {e}")
    tests_failed += 1

try:
    # Test 5: Utility methods
    print("\n[5/6] Utility methods...")
    result = client.validate_address("0x" + "a" * 40)
    print(f"  Address validation: {result}")
    print("  PASS: Utility methods functional")
    tests_passed += 1
except Exception as e:
    print(f"  FAIL: {e}")
    tests_failed += 1

try:
    # Test 6: Consensus methods
    print("\n[6/6] Consensus methods...")
    state = client.get_consensus_state()
    print(f"  Consensus state: {state}")
    print("  PASS: Consensus methods functional")
    tests_passed += 1
except Exception as e:
    print(f"  FAIL: {e}")
    import traceback
    traceback.print_exc()
    tests_failed += 1

print("\n" + "=" * 60)
print(f"Results: {tests_passed} passed, {tests_failed} failed")
print("=" * 60)

sys.exit(0 if tests_failed == 0 else 1)

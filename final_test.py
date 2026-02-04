"""Final test - ASCII safe"""
import sys
sys.path.insert(0, '.')

from sdk.client import LuxtensorClient

def test_imports():
    """Test 1: Clean imports"""
    try:
        client = LuxtensorClient()
        return True, "Client created successfully"
    except Exception as e:
        return False, f"Import failed: {e}"

def test_mro():
    """Test 2: MRO correctness"""
    try:
        mro_names = [c.__name__ for c in LuxtensorClient.__mro__]
        assert 'ConsensusMixin' in mro_names, "ConsensusMixin not in MRO"
        assert 'UtilsMixin' in mro_names, "UtilsMixin not in MRO"
        return True, f"MRO correct: {len(mro_names)} classes"
    except Exception as e:
        return False, f"MRO check failed: {e}"

def test_attributes():
    """Test 3: Method availability"""
    try:
        client = LuxtensorClient()
        methods = ['verify_block', 'health_check', 'get_stake', 'check_finality']
        missing = [m for m in methods if not hasattr(client, m)]
        if missing:
            return False, f"Missing methods: {missing}"
        return True, f"All {len(methods)} methods available"
    except Exception as e:
        return False, f"Attribute check failed: {e}"

def test_consensus_init():
    """Test 4: Consensus initialization"""
    try:
        client = LuxtensorClient()
        client.init_consensus()
        assert client._consensus_initialized, "Consensus not initialized"
        return True, "Consensus initialized successfully"
    except Exception as e:
        import traceback
        traceback.print_exc()
        return False, f"Consensus init failed: {e}"

def test_utils():
    """Test 5: Utility methods"""
    try:
        client = LuxtensorClient()
        result = client.validate_address("0x" + "a" * 40)
        assert isinstance(result, bool), "validate_address returned non-bool"
        return True, f"Utils functional (validate_address={result})"
    except Exception as e:
        return False, f"Utils test failed: {e}"

def test_consensus_methods():
    """Test 6: Consensus methods"""
    try:
        client = LuxtensorClient()
        client.init_consensus()
        state = client.get_consensus_state()
        assert state is not None, "get_consensus_state returned None"
        return True, f"Consensus state: epoch={state.current_epoch}, validators={state.active_validators}"
    except Exception as e:
        import traceback
        traceback.print_exc()
        return False, f"Consensus methods failed: {e}"

# Run all tests
tests = [
    ("Imports", test_imports),
    ("MRO", test_mro),
    ("Attributes", test_attributes),
    ("Consensus Init", test_consensus_init),
    ("Utils", test_utils),
    ("Consensus Methods", test_consensus_methods),
]

print("=" * 70)
print(" CLIENT REFACTORING TEST SUITE")
print("=" * 70)

passed = 0
failed = 0
results = []

for name, test_func in tests:
    print(f"\n[{name}]", end=" ")
    success, msg = test_func()
    if success:
        print(f"PASS - {msg}")
        passed += 1
        results.append((name, "PASS", msg))
    else:
        print(f"FAIL - {msg}")
        failed += 1
        results.append((name, "FAIL", msg))

print("\n" + "=" * 70)
print(f" SUMMARY: {passed} passed, {failed} failed")
print("=" * 70)

if failed > 0:
    print("\nFailed tests:")
    for name, status, msg in results:
        if status == "FAIL":
            print(f"  - {name}: {msg}")

sys.exit(0 if failed == 0 else 1)

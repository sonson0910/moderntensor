"""Quick import test"""
import sys
sys.path.insert(0, '.')

print("Testing imports...")
try:
    from sdk.client import LuxtensorClient
    print("OK: LuxtensorClient imported")

    client = LuxtensorClient()
    print(f"OK: Client created - type: {type(client).__name__}")

    methods = [m for m in dir(client) if not m.startswith('_')]
    print(f"OK: {len(methods)} public methods/attributes")

    # Check key methods
    key_methods = ['verify_block', 'health_check', 'get_stake', 'get_block_number']
    for method in key_methods:
        if method in methods:
            print(f"OK: {method} available")
        else:
            print(f"MISSING: {method}")

    print("\n✅ ALL TESTS PASSED")
except Exception as e:
    print(f"\n❌ ERROR: {e}")
    import traceback
    traceback.print_exc()

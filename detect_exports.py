"""Auto-detect correct consensus module exports"""
import sys
import importlib
sys.path.insert(0, '.')

modules = [
    'pos',
    'fork_choice',
    'liveness',
    'rotation',
    'slashing',
    'halving',
    'long_range_protection',
    'fast_finality',
    'circuit_breaker',
    'fork_resolution',
]

print("Scanning consensus modules...")
print("="*60)

for mod_name in modules:
    try:
        mod = importlib.import_module(f'sdk.consensus.{mod_name}')
        exports = [x for x in dir(mod) if not x.startswith('_') and x[0].isupper()]
        print(f"\n{mod_name}:")
        for exp in exports:
            print(f"  - {exp}")
    except Exception as e:
        print(f"\n{mod_name}: ❌ {e}")

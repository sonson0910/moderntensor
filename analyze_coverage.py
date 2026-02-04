"""Count methods in mixins for coverage analysis"""
import os
import ast

mixins = [
    'blockchain_mixin.py',
    'account_mixin.py',
    'staking_mixin.py',
    'transaction_mixin.py',
    'subnet_mixin.py',
    'neuron_mixin.py',
    'consensus_mixin.py',
    'utils_mixin.py'
]

print("Mixin Coverage Analysis:")
print("=" * 50)

total = 0
for mixin in mixins:
    path = f'sdk/client/{mixin}'
    if os.path.exists(path):
        with open(path, 'r', encoding='utf-8') as f:
            tree = ast.parse(f.read())
            methods = [n.name for n in ast.walk(tree)
                      if isinstance(n, ast.FunctionDef)
                      and not n.name.startswith('_')]
            count = len(methods)
            total += count
            print(f"{mixin:25} {count:3} methods")

print("=" * 50)
print(f"Total mixin methods:      {total}")
print(f"Monolith methods:         115")
print(f"Coverage:                 {total/115*100:.1f}%")
print(f"Missing:                  {115-total} methods")

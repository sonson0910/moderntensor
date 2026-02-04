"""Find missing methods - what's in monolith but not in mixins"""
import os
import ast

def get_methods(filepath):
    """Extract public method names from Python file"""
    if not os.path.exists(filepath):
        return set()
    with open(filepath, 'r', encoding='utf-8') as f:
        tree = ast.parse(f.read())
        return {n.name for n in ast.walk(tree)
                if isinstance(n, ast.FunctionDef)
                and not n.name.startswith('_')
                and n.name not in ['connect', 'async_connect']}  # Exclude helpers

# Get monolith methods
monolith_methods = get_methods('sdk/luxtensor_client.py')

# Get mixin methods
mixin_files = [
    'sdk/client/blockchain_mixin.py',
    'sdk/client/account_mixin.py',
    'sdk/client/staking_mixin.py',
    'sdk/client/transaction_mixin.py',
    'sdk/client/subnet_mixin.py',
    'sdk/client/neuron_mixin.py',
    'sdk/client/consensus_mixin.py',
    'sdk/client/utils_mixin.py'
]

mixin_methods = set()
for mixin in mixin_files:
    mixin_methods.update(get_methods(mixin))

# Find missing
missing = sorted(monolith_methods - mixin_methods)

print(f"Total in monolith: {len(monolith_methods)}")
print(f"Total in mixins:   {len(mixin_methods)}")
print(f"Missing:           {len(missing)}\n")

# Categorize by name patterns
categories = {
    'Subnet': [],
    'Staking': [],
    'Neuron': [],
    'Weights': [],
    'Network': [],
    'Delegate': [],
    'AI/Oracle': [],
    'Utility': [],
    'Other': []
}

for method in missing:
    if 'subnet' in method.lower():
        categories['Subnet'].append(method)
    elif 'stake' in method.lower() or 'delegate' in method.lower():
        categories['Staking'].append(method)
    elif 'neuron' in method.lower():
        categories['Neuron'].append(method)
    elif 'weight' in method.lower():
        categories['Weights'].append(method)
    elif 'delegate' in method.lower():
        categories['Delegate'].append(method)
    elif'ai' in method.lower() or 'oracle' in method.lower() or 'task' in method.lower():
        categories['AI/Oracle'].append(method)
    elif any(x in method.lower() for x in ['burn', 'difficulty', 'emission', 'tempo', 'activity', 'consensus']):
        categories['Network'].append(method)
    elif any(x in method.lower() for x in ['health', 'validate', 'wait']):
        categories['Utility'].append(method)
    else:
        categories['Other'].append(method)

print("Missing Methods by Category:")
print("=" * 60)
for cat, methods in categories.items():
    if methods:
        print(f"\n{cat} ({len(methods)} methods):")
        for m in sorted(methods):
            print(f"  - {m}")

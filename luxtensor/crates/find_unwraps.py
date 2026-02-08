import os

results = []
for root, dirs, files in os.walk('.'):
    if '/tests/' in root or 'luxtensor-tests' in root:
        continue
    for f in files:
        if not f.endswith('.rs') or f.startswith('test_') or f.endswith('_test.rs'):
            continue
        path = os.path.join(root, f)
        with open(path) as fh:
            lines = fh.readlines()

        in_test_module = False
        brace_depth = 0
        test_start_depth = 0

        for i, line in enumerate(lines, 1):
            stripped = line.strip()

            if '#[cfg(test)]' in stripped:
                in_test_module = True
                test_start_depth = brace_depth

            brace_depth += line.count('{') - line.count('}')

            if in_test_module and brace_depth <= test_start_depth:
                in_test_module = False

            if in_test_module:
                continue

            if 'unwrap_or' in stripped or 'unwrap_or_else' in stripped or 'unwrap_or_default' in stripped:
                continue

            if '.unwrap()' in stripped:
                results.append((path, i, stripped))

for path, line, code in results:
    print(f'{path}:{line}: {code}')

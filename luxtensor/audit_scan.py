#!/usr/bin/env python3
"""Scan for dangerous patterns in production Rust code."""
import os
import re

CRATES_DIR = "crates"
RESULTS = {"unwrap_expect": [], "panic_todo": [], "println": [], "let_ignore": [], "sensitive_log": []}

def is_test_context(lines, target_idx):
    """Check if a line is inside a #[cfg(test)] module or #[test] function."""
    # Walk backwards to find #[cfg(test)] or #[test]
    brace_depth = 0
    for i in range(target_idx, -1, -1):
        line = lines[i].strip()
        # Count braces going backwards
        brace_depth += lines[i].count('}') - lines[i].count('{')
        if '#[cfg(test)]' in line:
            return True
        if '#[test]' in line and brace_depth >= 0:
            return True
        # If we've exited more scopes than entered, we're outside
        if brace_depth > 2:
            break
    return False

def scan_file(fpath):
    with open(fpath, 'r', errors='replace') as f:
        lines = f.readlines()

    # Find where #[cfg(test)] modules start
    test_regions = set()
    in_test = False
    brace_count = 0
    for i, line in enumerate(lines):
        if '#[cfg(test)]' in line:
            in_test = True
            brace_count = 0
            test_regions.add(i)
            continue
        if in_test:
            brace_count += line.count('{') - line.count('}')
            test_regions.add(i)
            if brace_count <= 0 and i > 0 and '{' in ''.join(lines[max(0,i-5):i+1]):
                # Check if we've closed the test module
                pass
            # Simple heuristic: once we enter test module, everything after is test
            # until brace_count goes to 0 after being positive

    # Better approach: find line of #[cfg(test)] and mark everything from there to EOF as test
    # (since #[cfg(test)] mod tests is typically at the bottom of the file)
    test_start = None
    for i, line in enumerate(lines):
        if '#[cfg(test)]' in line:
            test_start = i
            break

    for i, line in enumerate(lines):
        # Skip test code
        if test_start is not None and i >= test_start:
            continue

        stripped = line.strip()
        # Skip comments
        if stripped.startswith('//') or stripped.startswith('*') or stripped.startswith('/*'):
            continue

        # 1. unwrap() and expect()
        if '.unwrap()' in line:
            RESULTS["unwrap_expect"].append((fpath, i+1, stripped[:150], "unwrap"))
        if '.expect(' in line:
            RESULTS["unwrap_expect"].append((fpath, i+1, stripped[:150], "expect"))

        # 2. panic!, todo!, unimplemented!
        if re.search(r'\bpanic!\s*\(', line) and not stripped.startswith('//'):
            RESULTS["panic_todo"].append((fpath, i+1, stripped[:150], "panic!"))
        if re.search(r'\btodo!\s*\(', line) and not stripped.startswith('//'):
            RESULTS["panic_todo"].append((fpath, i+1, stripped[:150], "todo!"))
        if re.search(r'\bunimplemented!\s*\(', line) and not stripped.startswith('//'):
            RESULTS["panic_todo"].append((fpath, i+1, stripped[:150], "unimplemented!"))

        # 3. println! (should use tracing)
        if re.search(r'\bprintln!\s*\(', line) and not stripped.startswith('//'):
            RESULTS["println"].append((fpath, i+1, stripped[:150]))
        if re.search(r'\beprintln!\s*\(', line) and not stripped.startswith('//'):
            RESULTS["println"].append((fpath, i+1, stripped[:150]))

        # 4. let _ = (ignoring Results)
        if re.search(r'let\s+_\s*=', line):
            RESULTS["let_ignore"].append((fpath, i+1, stripped[:150]))

        # 5. Sensitive data logging
        if re.search(r'(private_key|secret_key|seed_phrase|mnemonic|password)', line, re.IGNORECASE):
            if re.search(r'(info!|warn!|error!|debug!|trace!|println!|log::)', line):
                RESULTS["sensitive_log"].append((fpath, i+1, stripped[:150]))

for root, dirs, files in os.walk(CRATES_DIR):
    for fname in files:
        if not fname.endswith('.rs'):
            continue
        fpath = os.path.join(root, fname)
        # Skip test crates and fuzz
        if 'luxtensor-tests/' in fpath or 'luxtensor-fuzz/' in fpath:
            continue
        scan_file(fpath)

# Output
print("=" * 100)
print(f"UNWRAP/EXPECT IN PRODUCTION CODE: {len(RESULTS['unwrap_expect'])} findings")
print("=" * 100)
for fpath, line, code, kind in RESULTS["unwrap_expect"]:
    print(f"  {fpath}:{line}: [{kind}] {code}")

print()
print("=" * 100)
print(f"PANIC/TODO/UNIMPLEMENTED IN PRODUCTION CODE: {len(RESULTS['panic_todo'])} findings")
print("=" * 100)
for fpath, line, code, kind in RESULTS["panic_todo"]:
    print(f"  {fpath}:{line}: [{kind}] {code}")

print()
print("=" * 100)
print(f"PRINTLN/EPRINTLN IN PRODUCTION CODE: {len(RESULTS['println'])} findings")
print("=" * 100)
for fpath, line, code in RESULTS["println"]:
    print(f"  {fpath}:{line}: {code}")

print()
print("=" * 100)
print(f"LET _ = (IGNORED RESULTS): {len(RESULTS['let_ignore'])} findings")
print("=" * 100)
for fpath, line, code in RESULTS["let_ignore"]:
    print(f"  {fpath}:{line}: {code}")

print()
print("=" * 100)
print(f"SENSITIVE DATA LOGGING: {len(RESULTS['sensitive_log'])} findings")
print("=" * 100)
for fpath, line, code in RESULTS["sensitive_log"]:
    print(f"  {fpath}:{line}: {code}")

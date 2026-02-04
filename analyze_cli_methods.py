"""
Analyze CLI commands to extract all LuxtensorClient method calls
"""
import re
import os
from pathlib import Path
from collections import defaultdict

CLI_COMMANDS_DIR = Path("sdk/cli/commands")

def extract_client_methods(file_path):
    """Extract all client method calls from a Python file"""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # Pattern: client.method_name(
    pattern = r'client\.(\w+)\('
    matches = re.findall(pattern, content)

    return set(matches)

def main():
    all_methods = defaultdict(list)

    # Scan all CLI command files
    for py_file in CLI_COMMANDS_DIR.glob("*.py"):
        if py_file.name == "__init__.py":
            continue

        print(f"\n📄 Analyzing {py_file.name}...")
        methods = extract_client_methods(py_file)

        for method in sorted(methods):
            all_methods[method].append(py_file.name)
            print(f"  - {method}")

    print("\n" + "="*60)
    print("📊 SUMMARY: All Client Methods Used by CLI")
    print("="*60)

    for method in sorted(all_methods.keys()):
        files = ", ".join(all_methods[method])
        print(f"✓ {method:40} | Used in: {files}")

    print(f"\n🎯 Total unique methods: {len(all_methods)}")

    # Check if all methods exist in new client
    print("\n" + "="*60)
    print("🔍 Checking compatibility with new mixin client...")
    print("="*60)

    try:
        from sdk.client import LuxtensorClient

        client_methods = set([
            m for m in dir(LuxtensorClient)
            if not m.startswith('_') and callable(getattr(LuxtensorClient, m))
        ])

        cli_methods = set(all_methods.keys())
        missing = cli_methods - client_methods

        if missing:
            print(f"\n⚠️  MISSING METHODS ({len(missing)}):")
            for m in sorted(missing):
                files = ", ".join(all_methods[m])
                print(f"  ❌ {m:40} (used in: {files})")
        else:
            print(f"\n✅ ALL {len(cli_methods)} CLI METHODS ARE AVAILABLE IN NEW CLIENT!")

        # Show extra methods in client not used by CLI
        extra = client_methods - cli_methods
        print(f"\n💡 Client has {len(extra)} additional methods not currently used by CLI")

    except Exception as e:
        print(f"\n❌ Error importing new client: {e}")

if __name__ == "__main__":
    main()

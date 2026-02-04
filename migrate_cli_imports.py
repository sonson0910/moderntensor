"""
Automated CLI Import Migration Script
Migrates all CLI commands from monolithic client to mixin-based client
"""
import os
import shutil
from pathlib import Path
from datetime import datetime

# Configuration
CLI_COMMANDS_DIR = Path("sdk/cli/commands")
OLD_IMPORT = "from sdk.luxtensor_client import LuxtensorClient"
NEW_IMPORT = "from sdk.client import LuxtensorClient"
BACKUP_SUFFIX = ".backup"

def backup_file(file_path):
    """Create backup of original file"""
    backup_path = str(file_path) + BACKUP_SUFFIX
    shutil.copy2(file_path, backup_path)
    return backup_path

def update_imports(file_path):
    """Update imports in a single file"""
    # Read file
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # Check if old import exists
    if OLD_IMPORT not in content:
        return False, "No old import found"

    # Count occurrences
    count = content.count(OLD_IMPORT)

    # Replace
    new_content = content.replace(OLD_IMPORT, NEW_IMPORT)

    # Write back
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(new_content)

    return True, f"Replaced {count} occurrence(s)"

def verify_syntax(file_path):
    """Verify Python syntax after update"""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            compile(f.read(), str(file_path), 'exec')
        return True, "Syntax OK"
    except SyntaxError as e:
        return False, f"Syntax error: {e}"

def main():
    print("="*70)
    print("CLI IMPORT MIGRATION SCRIPT")
    print("="*70)
    print(f"Target: {CLI_COMMANDS_DIR}")
    print(f"Old import: {OLD_IMPORT}")
    print(f"New import: {NEW_IMPORT}")
    print("="*70)

    # Get all Python files
    py_files = list(CLI_COMMANDS_DIR.glob("*.py"))
    py_files = [f for f in py_files if f.name != "__init__.py"]

    print(f"\nFound {len(py_files)} command files to process\n")

    results = []

    for py_file in sorted(py_files):
        print(f"📄 Processing {py_file.name}...")

        # Backup
        backup_path = backup_file(py_file)
        print(f"   ✓ Backup created: {Path(backup_path).name}")

        # Update imports
        updated, msg = update_imports(py_file)

        if not updated:
            print(f"   ⚠️  {msg}")
            results.append((py_file.name, "SKIPPED", msg))
            # Remove backup if no changes
            os.remove(backup_path)
            continue

        print(f"   ✓ {msg}")

        # Verify syntax
        syntax_ok, syntax_msg = verify_syntax(py_file)

        if syntax_ok:
            print(f"   ✓ {syntax_msg}")
            results.append((py_file.name, "SUCCESS", msg))
        else:
            print(f"   ❌ {syntax_msg}")
            print(f"   ⚠️  Restoring from backup...")
            shutil.copy2(backup_path, py_file)
            results.append((py_file.name, "FAILED", syntax_msg))

        print()

    # Summary
    print("="*70)
    print("MIGRATION SUMMARY")
    print("="*70)

    success = sum(1 for _, status, _ in results if status == "SUCCESS")
    skipped = sum(1 for _, status, _ in results if status == "SKIPPED")
    failed = sum(1 for _, status, _ in results if status == "FAILED")

    print(f"\n✅ Success: {success}")
    print(f"⚠️  Skipped: {skipped}")
    print(f"❌ Failed: {failed}")

    if success > 0:
        print(f"\n📋 Updated files:")
        for name, status, msg in results:
            if status == "SUCCESS":
                print(f"   • {name}: {msg}")

    if failed > 0:
        print(f"\n❌ Failed files:")
        for name, status, msg in results:
            if status == "FAILED":
                print(f"   • {name}: {msg}")

    # Verify imports work
    print("\n" + "="*70)
    print("IMPORT VERIFICATION")
    print("="*70)

    try:
        print("Testing new import: from sdk.client import LuxtensorClient")
        from sdk.client import LuxtensorClient
        methods = len([m for m in dir(LuxtensorClient) if not m.startswith('_') and callable(getattr(LuxtensorClient, m))])
        print(f"✅ Import successful! Client has {methods} methods")
    except Exception as e:
        print(f"❌ Import test failed: {e}")
        return 1

    print("\n" + "="*70)
    print("✅ MIGRATION COMPLETE!")
    print("="*70)

    if failed == 0:
        print("\nNext steps:")
        print("1. Review changes: git diff sdk/cli/commands/")
        print("2. Run CLI tests: pytest tests/cli/")
        print("3. Manual smoke test: mtcli --help")
        print("4. Remove backups if satisfied: rm sdk/cli/commands/*.backup")

    return 0 if failed == 0 else 1

if __name__ == "__main__":
    exit(main())

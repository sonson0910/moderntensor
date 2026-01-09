# tests/conftest.py
"""
Test configuration and fixtures for ModernTensor SDK tests.

REMOVED: Cardano-specific hotkey/coldkey fixtures (decode_hotkey_skeys, hotkey_config, hotkey_skey_fixture)
These were tightly coupled with removed keymanager and compat modules.

For AI/ML and core functionality tests, see individual test files for their fixtures.
"""

import pytest

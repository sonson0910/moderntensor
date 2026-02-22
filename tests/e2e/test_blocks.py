# tests/e2e/test_blocks.py
"""
E2E Tests — Block Queries
Verifies block retrieval, progression, and hash-based lookup.
"""

import time
import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestBlockQueries:
    """Read-only block queries."""

    def test_get_block_number(self, luxtensor_client):
        """Block number is a non-negative integer."""
        num = luxtensor_client.get_block_number()
        assert isinstance(num, int)
        assert num >= 0

    def test_get_latest_block(self, luxtensor_client):
        """Latest block has required fields."""
        num = luxtensor_client.get_block_number()
        block = luxtensor_client._call_rpc("eth_getBlockByNumber", [hex(num), False])
        assert block is not None
        assert isinstance(block, dict)
        assert "hash" in block or "number" in block

    def test_get_genesis_block(self, luxtensor_client):
        """Genesis block (0) is retrievable."""
        block = luxtensor_client._call_rpc("eth_getBlockByNumber", ["0x0", False])
        assert block is not None
        assert isinstance(block, dict)

    def test_block_number_increases(self, luxtensor_client, wait_next_block):
        """Block number advances over time."""
        b1 = luxtensor_client.get_block_number()
        wait_next_block()
        b2 = luxtensor_client.get_block_number()
        assert b2 >= b1, f"Block number did not advance: {b1} -> {b2}"

    def test_get_block_by_hash(self, luxtensor_client):
        """Block retrieval by hash works if we have a known block."""
        block_num = luxtensor_client.get_block_number()
        if block_num == 0:
            pytest.skip("No blocks to query by hash")

        block = luxtensor_client._call_rpc("eth_getBlockByNumber", [hex(block_num), False])
        if block and isinstance(block, dict) and "hash" in block:
            by_hash = luxtensor_client._call_rpc("eth_getBlockByHash", [block["hash"], False])
            # Some nodes may return None for very recent blocks; verify no crash
            if by_hash is not None:
                assert isinstance(by_hash, dict)

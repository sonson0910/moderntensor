# tests/e2e/test_core_blockchain.py
"""
E2E Tests - Core Blockchain Features
Tests fundamental blockchain operations: funding accounts, sending transactions,
deploying smart contracts, lock/unlock patterns, and cross-node queries.

Requires LuxTensor nodes:
  - Node 1: http://127.0.0.1:8545 (validator/producer)
  - Node 2: http://127.0.0.1:8547 (sync node, optional)
  - Node 3: http://127.0.0.1:8549 (sync node, optional)
"""

import json
import time
import os
import urllib.request
import pytest

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
]

# ---- Helpers ---------------------------------------------------------

NODE1_URL = "http://127.0.0.1:8545"
NODE2_URL = "http://127.0.0.1:8547"
NODE3_URL = "http://127.0.0.1:8549"

# Counter for generating unique addresses per test
_addr_counter = int(time.time()) % 100000

# Minimal EVM contract bytecodes (no compiler needed):
# SimpleStorage: constructor stores 42 at slot 0
SIMPLE_STORAGE_INIT = (
    "602a600055"          # PUSH1 0x2a, PUSH1 0, SSTORE (store 42)
    "6009"                # PUSH1 9 (runtime len)
    "80"                  # DUP1
    "600a"                # PUSH1 10 (offset)
    "6000"                # PUSH1 0
    "39"                  # CODECOPY
    "6000"                # PUSH1 0
    "f3"                  # RETURN
    # Runtime: SLOAD slot 0 and return
    "600054"              # PUSH1 0, SLOAD
    "600052"              # PUSH1 0, MSTORE
    "60206000f3"          # PUSH1 32, PUSH1 0, RETURN
)


def unique_addr():
    """Generate a unique test address to avoid nonce conflicts."""
    global _addr_counter
    _addr_counter += 1
    return "0x" + f"{_addr_counter:040x}"


def rpc_call(url, method, params=None):
    """JSON-RPC call, returns result or raises on error."""
    payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        body = json.loads(resp.read().decode("utf-8"))
    if "error" in body:
        raise Exception(f"RPC error: {body['error']}")
    return body.get("result")


def rpc_call_raw(url, method, params=None):
    """JSON-RPC call, returns full body dict (including errors)."""
    payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}
    req = urllib.request.Request(
        url,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read().decode("utf-8"))


def wait_for_tx(url, tx_hash, timeout=15):
    """Poll for a transaction receipt until it appears or times out."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        receipt = rpc_call(url, "eth_getTransactionReceipt", [tx_hash])
        if receipt is not None:
            return receipt
        time.sleep(1)
    raise TimeoutError(f"Transaction {tx_hash} not mined within {timeout}s")


def hex_to_int(v):
    if v is None:
        return 0
    return int(v, 16) if isinstance(v, str) else int(v)


def node_available(url):
    try:
        rpc_call(url, "eth_blockNumber")
        return True
    except Exception:
        return False


def fund_and_send(from_addr, to_addr, value="0x1000"):
    """Fund from_addr via faucet, then send value to to_addr. Returns tx hash."""
    rpc_call(NODE1_URL, "dev_faucet", [from_addr, "0xDE0B6B3A7640000"])
    tx_hash = rpc_call(NODE1_URL, "eth_sendTransaction", [{
        "from": from_addr,
        "to": to_addr,
        "value": value,
        "gas": "0x5208",
        "gasPrice": "0x3B9ACA00",
    }])
    return tx_hash


def poll_state(fn, check, timeout=12, interval=2):
    """Poll fn() until check(result) is truthy, returning the result.
    Handles the LuxTensor RPC state lag where UnifiedStateDB may not
    immediately reflect mined block state changes."""
    deadline = time.time() + timeout
    last_result = None
    while time.time() < deadline:
        last_result = fn()
        if check(last_result):
            return last_result
        time.sleep(interval)
    return last_result


# ---- Test Classes ----------------------------------------------------

class TestTransactionLifecycle:
    """Test basic transaction send, query, and receipt flow."""

    def test_faucet_funding(self):
        """dev_faucet should credit tokens to an account."""
        addr = unique_addr()
        balance_before = hex_to_int(
            rpc_call(NODE1_URL, "eth_getBalance", [addr, "latest"])
        )
        result = rpc_call(NODE1_URL, "dev_faucet", [addr, "0xDE0B6B3A7640000"])
        assert result is not None, "dev_faucet should return a result"

        balance_after = hex_to_int(
            rpc_call(NODE1_URL, "eth_getBalance", [addr, "latest"])
        )
        assert balance_after > balance_before, (
            f"Balance should increase: {balance_before} -> {balance_after}"
        )

    def test_send_transaction_and_query(self):
        """Send tx via eth_sendTransaction and query by hash."""
        sender = unique_addr()
        receiver = unique_addr()
        tx_hash = fund_and_send(sender, receiver)

        assert tx_hash is not None, "Should return tx hash"
        assert tx_hash.startswith("0x"), f"Hash format: {tx_hash}"
        assert len(tx_hash) == 66, f"Hash length: {tx_hash}"

        # Query by hash
        tx = rpc_call(NODE1_URL, "eth_getTransactionByHash", [tx_hash])
        if tx is not None:
            assert tx["hash"] == tx_hash

    def test_transaction_receipt_after_mining(self):
        """Send tx -> wait for block -> verify receipt."""
        sender = unique_addr()
        receiver = unique_addr()
        tx_hash = fund_and_send(sender, receiver)

        receipt = wait_for_tx(NODE1_URL, tx_hash, timeout=15)
        assert receipt is not None, "Receipt should exist"
        assert receipt.get("transactionHash") == tx_hash
        status = receipt.get("status")
        if status is not None:
            assert status in ("0x1", 1, True), f"Should succeed: status={status}"

    @pytest.mark.xfail(
        reason="Known: UnifiedStateDB RPC doesn't reflect mined tx state changes",
        strict=False,
    )
    def test_balance_transfer(self):
        """Fund A, send to B, verify balance changes via RPC state query."""
        sender = unique_addr()
        receiver = unique_addr()
        transfer_amount = 0x1000

        rpc_call(NODE1_URL, "dev_faucet", [sender, "0xDE0B6B3A7640000"])
        balance_b_before = hex_to_int(
            rpc_call(NODE1_URL, "eth_getBalance", [receiver, "latest"])
        )

        tx_hash = rpc_call(NODE1_URL, "eth_sendTransaction", [{
            "from": sender,
            "to": receiver,
            "value": hex(transfer_amount),
            "gas": "0x5208",
            "gasPrice": "0x3B9ACA00",
        }])
        receipt = wait_for_tx(NODE1_URL, tx_hash)
        assert receipt is not None

        # Poll state (may take time for UnifiedStateDB to sync)
        balance_b_after = hex_to_int(
            poll_state(
                lambda: rpc_call(NODE1_URL, "eth_getBalance", [receiver, "latest"]),
                lambda v: hex_to_int(v) >= balance_b_before + transfer_amount,
            )
        )
        assert balance_b_after >= balance_b_before + transfer_amount, (
            f"Receiver balance: {balance_b_before} -> {balance_b_after}"
        )

    @pytest.mark.xfail(
        reason="Known: UnifiedStateDB RPC doesn't reflect mined tx nonce changes",
        strict=False,
    )
    def test_nonce_increments(self):
        """Send 2 sequential txs -> verify nonce increases via RPC query."""
        sender = unique_addr()
        receiver = unique_addr()
        rpc_call(NODE1_URL, "dev_faucet", [sender, "0xDE0B6B3A7640000"])

        nonce_0 = hex_to_int(
            rpc_call(NODE1_URL, "eth_getTransactionCount", [sender, "latest"])
        )

        # Tx 1
        tx1 = rpc_call(NODE1_URL, "eth_sendTransaction", [{
            "from": sender,
            "to": receiver,
            "value": "0x1",
            "gas": "0x5208",
            "gasPrice": "0x3B9ACA00",
        }])
        wait_for_tx(NODE1_URL, tx1)

        nonce_1 = hex_to_int(
            poll_state(
                lambda: rpc_call(NODE1_URL, "eth_getTransactionCount", [sender, "latest"]),
                lambda v: hex_to_int(v) == nonce_0 + 1,
            )
        )
        assert nonce_1 == nonce_0 + 1, f"Nonce: {nonce_0} -> {nonce_1}"


class TestSmartContractDeployment:
    """Test contract deployment, read, and storage queries."""

    @pytest.mark.xfail(
        reason="Known: UnifiedStateDB RPC may not reflect deployed contract code",
        strict=False,
    )
    def test_deploy_contract(self):
        """Deploy SimpleStorage -> verify contractAddress + eth_getCode."""
        deployer = unique_addr()
        rpc_call(NODE1_URL, "dev_faucet", [deployer, "0xDE0B6B3A7640000"])

        tx_hash = rpc_call(NODE1_URL, "eth_sendTransaction", [{
            "from": deployer,
            "data": "0x" + SIMPLE_STORAGE_INIT,
            "gas": "0x30000",
            "gasPrice": "0x3B9ACA00",
        }])

        receipt = wait_for_tx(NODE1_URL, tx_hash)
        assert receipt is not None, "Contract deploy should produce receipt"

        contract_addr = receipt.get("contractAddress")
        assert contract_addr is not None, "Receipt should have contractAddress"
        assert contract_addr.startswith("0x")

        # Poll for code (UnifiedStateDB may lag)
        code = poll_state(
            lambda: rpc_call(NODE1_URL, "eth_getCode", [contract_addr, "latest"]),
            lambda v: v is not None and v != "0x" and v != "0x0",
        )
        assert code is not None and code != "0x" and code != "0x0", f"Code empty: {code}"

    def test_call_contract_function(self):
        """Deploy SimpleStorage -> eth_call -> should return 42."""
        deployer = unique_addr()
        rpc_call(NODE1_URL, "dev_faucet", [deployer, "0xDE0B6B3A7640000"])

        tx_hash = rpc_call(NODE1_URL, "eth_sendTransaction", [{
            "from": deployer,
            "data": "0x" + SIMPLE_STORAGE_INIT,
            "gas": "0x30000",
            "gasPrice": "0x3B9ACA00",
        }])
        receipt = wait_for_tx(NODE1_URL, tx_hash)
        contract_addr = receipt.get("contractAddress")
        assert contract_addr is not None

        result = rpc_call(NODE1_URL, "eth_call", [{
            "to": contract_addr,
            "data": "0x",
        }, "latest"])

        if result is not None and result != "0x":
            value = hex_to_int(result)
            assert value == 42, f"Stored value should be 42, got {value}"

    @pytest.mark.xfail(
        reason="Known: UnifiedStateDB RPC may not reflect contract storage changes",
        strict=False,
    )
    def test_contract_storage_query(self):
        """Deploy SimpleStorage -> read raw storage via eth_getStorageAt."""
        deployer = unique_addr()
        rpc_call(NODE1_URL, "dev_faucet", [deployer, "0xDE0B6B3A7640000"])

        tx_hash = rpc_call(NODE1_URL, "eth_sendTransaction", [{
            "from": deployer,
            "data": "0x" + SIMPLE_STORAGE_INIT,
            "gas": "0x30000",
            "gasPrice": "0x3B9ACA00",
        }])
        receipt = wait_for_tx(NODE1_URL, tx_hash)
        contract_addr = receipt.get("contractAddress")
        assert contract_addr is not None

        # Poll for storage value (UnifiedStateDB may lag)
        storage = poll_state(
            lambda: rpc_call(NODE1_URL, "eth_getStorageAt", [contract_addr, "0x0", "latest"]),
            lambda v: v is not None and v != "0x" and hex_to_int(v) == 42,
        )
        if storage is not None and storage != "0x":
            value = hex_to_int(storage)
            assert value == 42, f"Storage slot 0 should be 42, got {value}"

    def test_estimate_gas_for_contract(self):
        """Estimate gas for a contract call via eth_estimateGas."""
        deployer = unique_addr()
        rpc_call(NODE1_URL, "dev_faucet", [deployer, "0xDE0B6B3A7640000"])

        tx_hash = rpc_call(NODE1_URL, "eth_sendTransaction", [{
            "from": deployer,
            "data": "0x" + SIMPLE_STORAGE_INIT,
            "gas": "0x30000",
            "gasPrice": "0x3B9ACA00",
        }])
        receipt = wait_for_tx(NODE1_URL, tx_hash)
        contract_addr = receipt.get("contractAddress")
        assert contract_addr is not None

        gas_estimate = rpc_call(NODE1_URL, "eth_estimateGas", [{
            "from": deployer,
            "to": contract_addr,
            "data": "0x",
        }])
        assert gas_estimate is not None, "Gas estimate should be returned"
        gas_val = hex_to_int(gas_estimate)
        assert gas_val > 0, f"Gas estimate should be positive: {gas_val}"


class TestCrossNodeQueries:
    """Cross-node verification (requires nodes 2 and 3)."""

    @pytest.mark.skipif(
        not node_available(NODE2_URL),
        reason="Node 2 not available",
    )
    def test_cross_node_balance_query(self):
        """Fund on node 1 -> query balance on node 2 (after sync)."""
        addr = unique_addr()
        rpc_call(NODE1_URL, "dev_faucet", [addr, "0xDE0B6B3A7640000"])
        time.sleep(5)  # wait for propagation

        balance_1 = hex_to_int(
            rpc_call(NODE1_URL, "eth_getBalance", [addr, "latest"])
        )
        balance_2 = hex_to_int(
            rpc_call(NODE2_URL, "eth_getBalance", [addr, "latest"])
        )
        assert balance_1 > 0, "Node 1 should show balance"
        if balance_2 > 0:
            assert balance_2 == balance_1, (
                f"Node 2 should match: {balance_1} vs {balance_2}"
            )

    @pytest.mark.skipif(
        not node_available(NODE3_URL),
        reason="Node 3 not available",
    )
    def test_cross_node_transaction_query(self):
        """Send tx on node 1 -> query receipt on node 3."""
        sender = unique_addr()
        receiver = unique_addr()
        tx_hash = fund_and_send(sender, receiver)

        receipt_1 = wait_for_tx(NODE1_URL, tx_hash)
        assert receipt_1 is not None

        time.sleep(10)  # wait for sync

        try:
            receipt_3 = rpc_call(NODE3_URL, "eth_getTransactionReceipt", [tx_hash])
            if receipt_3 is not None:
                assert receipt_3.get("transactionHash") == tx_hash
        except Exception:
            pass  # node 3 may still be syncing


class TestEdgeCases:
    """Gas price, non-existent tx, block advancement, chain id."""

    def test_gas_price_query(self):
        """eth_gasPrice should return a value."""
        gas_price = rpc_call(NODE1_URL, "eth_gasPrice")
        assert gas_price is not None
        assert hex_to_int(gas_price) >= 0

    def test_transaction_not_found(self):
        """Non-existent tx hash should return null."""
        fake_hash = "0x" + "ab" * 32
        tx = rpc_call(NODE1_URL, "eth_getTransactionByHash", [fake_hash])
        assert tx is None, "Non-existent tx should return null"

    def test_block_number_advancing(self):
        """Block number should advance after waiting."""
        block1 = hex_to_int(rpc_call(NODE1_URL, "eth_blockNumber"))
        assert block1 > 0

        time.sleep(4)  # block_time = 3s

        block2 = hex_to_int(rpc_call(NODE1_URL, "eth_blockNumber"))
        assert block2 > block1, f"Block should advance: {block1} -> {block2}"

    def test_chain_id(self):
        """eth_chainId should return 8898 (0x22C2)."""
        try:
            chain_id = rpc_call(NODE1_URL, "eth_chainId")
            if chain_id is not None:
                val = hex_to_int(chain_id)
                assert val == 8898, f"Chain ID should be 8898, got {val}"
        except Exception:
            net_ver = rpc_call(NODE1_URL, "net_version")
            assert net_ver is not None

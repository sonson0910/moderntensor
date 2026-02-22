"""
Test signed TX mining with EIP-155 aligned signing_message().
Verifies that externally signed transactions are properly mined.
"""
import json
import urllib.request
import time
import sys
from eth_account import Account
from eth_utils import to_checksum_address


def rpc(method, params=None, port=8545):
    if params is None:
        params = []
    data = json.dumps({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).encode()
    req = urllib.request.Request(
        "http://127.0.0.1:{}".format(port),
        data=data,
        headers={"Content-Type": "application/json"},
    )
    resp = urllib.request.urlopen(req, timeout=10)
    return json.loads(resp.read())


def wait_for_receipt(tx_hash, timeout=60):
    for _ in range(timeout // 2):
        time.sleep(2)
        try:
            receipt = rpc("eth_getTransactionReceipt", [tx_hash])
            if receipt.get("result"):
                return receipt["result"]
        except Exception:
            pass
    return None


def main():
    print("=" * 60)
    print("EIP-155 Signed Transaction Mining Test")
    print("=" * 60)

    # 1. Create a new wallet
    acct = Account.create()
    print("New wallet:  {}".format(acct.address))

    # 2. Dev account (for funding) — Hardhat default #0, funded 10000 ETH in genesis
    dev_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    dev_acct = Account.from_key(dev_key)
    print("Dev account: {}".format(dev_acct.address))

    # 3. Get dev nonce
    nonce_r = rpc("eth_getTransactionCount", [dev_acct.address, "latest"])
    if "error" in nonce_r:
        print("ERROR getting nonce: {}".format(nonce_r["error"]))
        sys.exit(1)
    nonce = int(nonce_r["result"], 16)
    print("Dev nonce:   {}".format(nonce))

    # 4. Fund the new wallet with 10 LUX
    fund_tx = {
        "nonce": nonce,
        "to": acct.address,
        "value": 10 * 10**18,
        "gas": 21000,
        "gasPrice": 1000000000,
        "chainId": 8898,
    }
    signed_fund = dev_acct.sign_transaction(fund_tx)
    raw_hex = "0x" + signed_fund.raw_transaction.hex()
    result = rpc("eth_sendRawTransaction", [raw_hex])
    if "error" in result:
        print("ERROR sending fund TX: {}".format(result["error"]))
        sys.exit(1)
    fund_hash = result["result"]
    print("Fund TX hash: {}".format(fund_hash))

    # 5. Wait for fund TX to be mined
    print("Waiting for fund TX to be mined...")
    receipt = wait_for_receipt(fund_hash)
    if receipt is None:
        print("FAILED: Fund TX not mined after 60s!")
        sys.exit(1)
    print("Fund TX mined in block {}".format(receipt["blockNumber"]))

    # 6. Check balance
    bal = rpc("eth_getBalance", [acct.address, "latest"])
    balance_lux = int(bal["result"], 16) / 10**18
    print("New wallet balance: {} LUX".format(balance_lux))
    assert balance_lux >= 10.0, "Balance should be >= 10 LUX"

    # 7. Send TX from new wallet back to dev
    nonce2_r = rpc("eth_getTransactionCount", [acct.address, "latest"])
    nonce2 = int(nonce2_r["result"], 16)
    send_tx = {
        "nonce": nonce2,
        "to": dev_acct.address,
        "value": 1 * 10**18,
        "gas": 21000,
        "gasPrice": 1000000000,
        "chainId": 8898,
    }
    signed2 = acct.sign_transaction(send_tx)
    raw2 = "0x" + signed2.raw_transaction.hex()
    result2 = rpc("eth_sendRawTransaction", [raw2])
    if "error" in result2:
        print("ERROR sending TX from new wallet: {}".format(result2["error"]))
        sys.exit(1)
    send_hash = result2["result"]
    print("Send TX hash: {}".format(send_hash))

    # 8. Wait for send TX to be mined
    print("Waiting for send TX to be mined...")
    receipt2 = wait_for_receipt(send_hash)
    if receipt2 is None:
        print("FAILED: Send TX not mined after 60s!")
        sys.exit(1)
    print("Send TX mined in block {}".format(receipt2["blockNumber"]))

    # 9. Verify final balances
    bal_after = rpc("eth_getBalance", [acct.address, "latest"])
    balance_after = int(bal_after["result"], 16) / 10**18
    print("New wallet balance after send: {} LUX".format(balance_after))

    print()
    print("=" * 60)
    print("SUCCESS: Both signed transactions mined correctly!")
    print("EIP-155 signing format alignment is working!")
    print("=" * 60)


if __name__ == "__main__":
    main()

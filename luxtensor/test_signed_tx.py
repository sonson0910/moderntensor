"""Quick test: Send a signed EIP-155 transaction and check if it gets mined."""
import json
import time
import requests
from eth_account import Account
from eth_account.signers.local import LocalAccount

RPC_URL = "http://127.0.0.1:8545"
CHAIN_ID = 8898  # LuxTensor chain ID from config.toml

# Genesis funded account (from config.toml prefunded_accounts)
FUNDED_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
funded: LocalAccount = Account.from_key(FUNDED_KEY)

RECIPIENT = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

def rpc(method, params=None):
    r = requests.post(RPC_URL, json={
        "jsonrpc": "2.0", "id": 1,
        "method": method, "params": params or []
    }, timeout=10)
    data = r.json()
    if "error" in data:
        raise Exception(f"RPC error: {data['error']}")
    return data.get("result")

def main():
    print(f"=== Signed TX Mining Test ===")
    print(f"Sender:    {funded.address}")
    print(f"Recipient: {RECIPIENT}")
    print(f"Chain ID:  {CHAIN_ID}")

    # Get current block number
    block_before = int(rpc("eth_blockNumber"), 16)
    print(f"\nCurrent block: {block_before}")

    # Get nonce
    nonce = int(rpc("eth_getTransactionCount", [funded.address, "latest"]), 16)
    print(f"Nonce: {nonce}")

    # Get balance
    balance = int(rpc("eth_getBalance", [funded.address, "latest"]), 16)
    print(f"Balance: {balance / 1e18:.4f} ETH")

    # Build and sign transaction
    tx = {
        "nonce": nonce,
        "to": RECIPIENT,
        "value": 1_000_000_000_000_000_000,  # 1 ETH
        "gas": 21000,
        "gasPrice": 1_000_000_000,  # 1 Gwei
        "chainId": CHAIN_ID,
    }

    signed = Account.sign_transaction(tx, FUNDED_KEY)
    raw_tx = signed.raw_transaction.hex()
    # Ensure 0x prefix
    if not raw_tx.startswith("0x"):
        raw_tx = "0x" + raw_tx

    print(f"\nSigned TX hash: {signed.hash.hex()}")
    print(f"Raw TX length:  {len(raw_tx)} chars")

    # Send raw transaction
    print("\nSending eth_sendRawTransaction...")
    tx_hash = rpc("eth_sendRawTransaction", [raw_tx])
    print(f"TX hash returned: {tx_hash}")

    # Wait for the transaction to be mined
    print("\nWaiting for TX to be mined...")
    for i in range(30):  # Wait up to 30 * 3 = 90 seconds
        time.sleep(3)
        receipt = rpc("eth_getTransactionReceipt", [tx_hash])
        block_now = int(rpc("eth_blockNumber"), 16)
        print(f"  [{i+1}] Block: {block_now}, Receipt: {'YES' if receipt else 'not yet'}")
        if receipt:
            print(f"\n✅ TX MINED in block {int(receipt['blockNumber'], 16)}!")
            print(f"   Status: {receipt.get('status', 'N/A')}")
            print(f"   Gas used: {int(receipt.get('gasUsed', '0x0'), 16)}")

            # Verify balance changed
            new_balance = int(rpc("eth_getBalance", [funded.address, "latest"]), 16)
            recipient_balance = int(rpc("eth_getBalance", [RECIPIENT, "latest"]), 16)
            print(f"\nSender balance:    {new_balance / 1e18:.4f} ETH (was {balance / 1e18:.4f})")
            print(f"Recipient balance: {recipient_balance / 1e18:.4f} ETH")
            return True

    print("\n❌ TX NOT MINED after 90 seconds!")
    return False

if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)

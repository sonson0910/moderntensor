#!/usr/bin/env python3
"""
Simple Smart Contract Deployment và Interaction trên LuxTensor
"""

import sys
import json
import time
sys.path.insert(0, 'd:/venera/cardano/moderntensor/moderntensor')

from sdk.luxtensor_client import LuxtensorClient

# Simple Storage Contract Bytecode (no constructor)
# contract SimpleStorage { uint256 public value; function set(uint256 v) public { value = v; } }
SIMPLE_STORAGE_BYTECODE = "0x608060405234801561001057600080fd5b50610150806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80633fa4f2451461003b57806360fe47b114610059575b600080fd5b610043610075565b60405161005091906100a1565b60405180910390f35b610073600480360381019061006e91906100ed565b61007b565b005b60005481565b8060008190555050565b6000819050919050565b61009b81610088565b82525050565b60006020820190506100b66000830184610092565b92915050565b600080fd5b6100ca81610088565b81146100d557600080fd5b50565b6000813590506100e7816100c1565b92915050565b600060208284031215610103576101026100bc565b5b6000610111848285016100d8565b9150509291505056fea2646970667358221220"

# Full MDTToken bytecode
with open('d:/venera/cardano/moderntensor/moderntensor/luxtensor/contracts/artifacts/src/MDTToken.sol/MDTToken.json', 'r') as f:
    MDT_TOKEN_ARTIFACT = json.load(f)
    MDT_TOKEN_BYTECODE = MDT_TOKEN_ARTIFACT['bytecode']

DEPLOYER = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

def main():
    print("=" * 60)
    print("🚀 LuxTensor Smart Contract Demo")
    print("=" * 60)

    client = LuxtensorClient("http://localhost:8545")

    # Check connection
    block = client.get_block_number()
    print(f"\n📊 Current Block: {block}")

    # Get balance
    balance_wei = client.get_balance(DEPLOYER)
    balance_lux = balance_wei / 1e18
    print(f"💰 Deployer Balance: {balance_lux:,.2f} LUX")

    # Get nonce
    nonce = client.get_nonce(DEPLOYER)
    print(f"🔢 Deployer Nonce: {nonce}")

    # Deploy MDTToken
    print("\n📝 Deploying MDTToken...")
    print(f"   Bytecode size: {len(MDT_TOKEN_BYTECODE)} chars")

    tx_hash = client._call_rpc("eth_sendTransaction", [{
        "from": DEPLOYER,
        "data": MDT_TOKEN_BYTECODE,
        "gas": "0x2000000",  # 33M gas
        "gasPrice": "0x3B9ACA00"  # 1 gwei
    }])
    print(f"   TX Hash: {tx_hash}")

    # Wait for receipt
    print("   Waiting for mining...")
    receipt = None
    for i in range(60):  # 60 attempts, 1 second each
        time.sleep(1)
        receipt = client._call_rpc("eth_getTransactionReceipt", [tx_hash])
        if receipt:
            break
        if i % 10 == 0:
            curr_block = client.get_block_number()
            print(f"     Block {curr_block}... ({i}s elapsed)")

    if not receipt:
        print("   ⚠️ Transaction still pending after 60s")
        print(f"   TX Hash: {tx_hash}")

        # Check if tx exists
        tx = client._call_rpc("eth_getTransactionByHash", [tx_hash])
        if tx:
            print("   Transaction exists in mempool")
            print(f"   Gas: {tx.get('gas', 'N/A')}")
        return

    contract_address = receipt.get('contractAddress')
    print(f"\n✅ MDTToken deployed at: {contract_address}")
    print(f"   Block: {int(receipt.get('blockNumber', '0x0'), 16)}")
    print(f"   Gas Used: {int(receipt.get('gasUsed', '0x0'), 16):,}")

    # Interact with contract
    print("\n🔗 Interacting with MDTToken...")

    # Call name()
    name_selector = "0x06fdde03"  # keccak256("name()")[:4]
    name_result = client._call_rpc("eth_call", [{
        "to": contract_address,
        "data": name_selector
    }, "latest"])
    print(f"   name(): {name_result}")

    # Call symbol()
    symbol_selector = "0x95d89b41"  # keccak256("symbol()")[:4]
    symbol_result = client._call_rpc("eth_call", [{
        "to": contract_address,
        "data": symbol_selector
    }, "latest"])
    print(f"   symbol(): {symbol_result}")

    # Call totalSupply()
    total_supply_selector = "0x18160ddd"  # keccak256("totalSupply()")[:4]
    total_supply = client._call_rpc("eth_call", [{
        "to": contract_address,
        "data": total_supply_selector
    }, "latest"])
    if total_supply:
        supply = int(total_supply, 16) / 1e18
        print(f"   totalSupply(): {supply:,.2f} MDT")

    # Call balanceOf(deployer)
    balance_of_selector = "0x70a08231"
    balance_of_data = balance_of_selector + "000000000000000000000000" + DEPLOYER[2:]
    deployer_balance = client._call_rpc("eth_call", [{
        "to": contract_address,
        "data": balance_of_data
    }, "latest"])
    if deployer_balance:
        balance = int(deployer_balance, 16) / 1e18
        print(f"   balanceOf(deployer): {balance:,.2f} MDT")

    # Transfer tokens
    print("\n💸 Transferring 1000 MDT...")
    recipient = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
    transfer_selector = "0xa9059cbb"  # keccak256("transfer(address,uint256)")[:4]
    amount = hex(1000 * 10**18)[2:].zfill(64)
    recipient_padded = "000000000000000000000000" + recipient[2:]
    transfer_data = transfer_selector + recipient_padded + amount

    transfer_tx = client._call_rpc("eth_sendTransaction", [{
        "from": DEPLOYER,
        "to": contract_address,
        "data": transfer_data,
        "gas": "0x100000"
    }])
    print(f"   TX Hash: {transfer_tx}")

    # Wait for transfer
    time.sleep(5)
    transfer_receipt = client._call_rpc("eth_getTransactionReceipt", [transfer_tx])
    if transfer_receipt:
        print(f"   ✅ Transfer mined in block {int(transfer_receipt.get('blockNumber', '0x0'), 16)}")

        # Check recipient balance
        recipient_balance_data = balance_of_selector + "000000000000000000000000" + recipient[2:]
        recipient_balance = client._call_rpc("eth_call", [{
            "to": contract_address,
            "data": recipient_balance_data
        }, "latest"])
        if recipient_balance:
            balance = int(recipient_balance, 16) / 1e18
            print(f"   Recipient balance: {balance:,.2f} MDT")

    # Save deployment
    deployment = {
        "network": "luxtensor_local",
        "chainId": 1337,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "deployer": DEPLOYER,
        "contracts": {
            "MDTToken": contract_address
        }
    }
    with open('d:/venera/cardano/moderntensor/moderntensor/luxtensor/contracts/deployment-python.json', 'w') as f:
        json.dump(deployment, f, indent=2)
    print("\n📋 Saved to deployment-python.json")

    print("\n" + "=" * 60)
    print("✅ Smart Contract Demo Complete!")
    print("=" * 60)

if __name__ == "__main__":
    main()

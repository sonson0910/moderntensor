"""
Example: Using Luxtensor Python Client

This example shows how to interact with Luxtensor blockchain using the new client.
"""

import asyncio
from sdk import LuxtensorClient, AsyncLuxtensorClient, connect, async_connect


def sync_example():
    """Synchronous example"""
    print("=== Synchronous Luxtensor Client Example ===\n")
    
    # Connect to Luxtensor
    client = connect(url="http://localhost:9944", network="testnet")
    
    # Check connection
    if client.is_connected():
        print("✓ Connected to Luxtensor")
    else:
        print("✗ Failed to connect")
        return
    
    # Get blockchain info
    try:
        chain_info = client.get_chain_info()
        print(f"\nChain Info:")
        print(f"  Network: {chain_info.network}")
        print(f"  Block Height: {chain_info.block_height}")
        print(f"  Version: {chain_info.version}")
    except Exception as e:
        print(f"Error getting chain info: {e}")
    
    # Get current block number
    try:
        block_number = client.get_block_number()
        print(f"\nCurrent Block: {block_number}")
    except Exception as e:
        print(f"Error getting block number: {e}")
    
    # Get account info
    try:
        address = "0x1234567890abcdef1234567890abcdef12345678"
        account = client.get_account(address)
        print(f"\nAccount Info:")
        print(f"  Address: {account.address}")
        print(f"  Balance: {account.balance}")
        print(f"  Nonce: {account.nonce}")
        print(f"  Stake: {account.stake}")
    except Exception as e:
        print(f"Error getting account: {e}")
    
    # Get validators
    try:
        validators = client.get_validators()
        print(f"\nActive Validators: {len(validators)}")
    except Exception as e:
        print(f"Error getting validators: {e}")


async def async_example():
    """Asynchronous example"""
    print("\n\n=== Asynchronous Luxtensor Client Example ===\n")
    
    # Connect to Luxtensor (async)
    client = async_connect(url="http://localhost:9944", network="testnet")
    
    # Check connection
    if await client.is_connected():
        print("✓ Connected to Luxtensor (async)")
    else:
        print("✗ Failed to connect")
        return
    
    # Get block number
    try:
        block_number = await client.get_block_number()
        print(f"\nCurrent Block: {block_number}")
    except Exception as e:
        print(f"Error: {e}")
    
    # Batch query example - query multiple things concurrently
    try:
        print("\nBatch Query Example:")
        calls = [
            ("chain_getBlockNumber", []),
            ("validators_getActive", []),
            ("staking_getTotalStake", []),
        ]
        results = await client.batch_call(calls)
        print(f"  Block Number: {results[0]}")
        print(f"  Validators: {len(results[1])}")
        print(f"  Total Stake: {results[2]}")
    except Exception as e:
        print(f"Error in batch query: {e}")
    
    # Query multiple accounts concurrently
    try:
        print("\nConcurrent Account Queries:")
        addresses = [
            "0x1234567890abcdef1234567890abcdef12345678",
            "0xabcdef1234567890abcdef1234567890abcdef12",
        ]
        
        tasks = [client.get_account(addr) for addr in addresses]
        accounts = await asyncio.gather(*tasks, return_exceptions=True)
        
        for addr, account in zip(addresses, accounts):
            if isinstance(account, Exception):
                print(f"  {addr[:10]}...: Error - {account}")
            else:
                print(f"  {addr[:10]}...: Balance={account.balance}, Stake={account.stake}")
    except Exception as e:
        print(f"Error: {e}")


def main():
    """Run examples"""
    # Sync example
    sync_example()
    
    # Async example
    asyncio.run(async_example())
    
    print("\n" + "="*50)
    print("Examples complete!")
    print("\nNote: These examples require a running Luxtensor node.")
    print("Start Luxtensor with: cd luxtensor && cargo run --release")


if __name__ == "__main__":
    main()

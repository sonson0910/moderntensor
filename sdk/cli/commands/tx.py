"""
Transaction commands for ModernTensor CLI

Commands for creating and sending transactions.
"""

import click
from typing import Optional
import json
import os
import getpass
from pathlib import Path

from rich.console import Console
from rich.table import Table
from rich.panel import Panel

from sdk.cli.utils import (
    print_error, print_info, print_success, print_warning,
    create_table, console, get_default_wallet_path
)
from sdk.cli.config import get_network_config
from sdk.client import LuxtensorClient
from sdk.keymanager import decrypt_data
from sdk.transactions import create_transfer_transaction, encode_transaction_for_rpc

console = Console()


@click.group(name='tx', short_help='Transaction operations')
def tx():
    """
    Transaction commands
    
    Create and send transactions on the Luxtensor network.
    """
    pass


@tx.command('send')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--to', 'recipient', required=True, help='Recipient address')
@click.option('--amount', required=True, type=float, help='Amount to send (in MDT)')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name (mainnet, testnet, local)')
@click.option('--gas-price', type=int, default=50, help='Gas price (default: 50)')
@click.option('--gas-limit', type=int, default=21000, help='Gas limit (default: 21000)')
def send_tx(
    coldkey: str,
    hotkey: str,
    recipient: str,
    amount: float,
    base_dir: Optional[str],
    network: str,
    gas_price: int,
    gas_limit: int
):
    """
    Send MDT tokens to an address
    
    Examples:
        mtcli tx send --coldkey my_coldkey --hotkey miner_hk1 --to 0x1234... --amount 10.5 --network testnet
        mtcli tx send --coldkey my_coldkey --hotkey miner_hk1 --to 0x1234... --amount 100 --gas-price 100
    """
    try:
        # Get network configuration
        network_config = get_network_config(network)
        
        print_info(f"Preparing transaction on {network}...")
        print_info(f"Recipient: {recipient}")
        print_info(f"Amount: {amount} MDT")
        
        # Convert MDT to base units (1 MDT = 1e9 base units)
        value_base_units = int(amount * 1_000_000_000)
        
        # Load wallet
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        hotkeys_file = coldkey_path / "hotkeys.json"
        
        if not hotkeys_file.exists():
            print_error(f"No hotkeys found for coldkey '{coldkey}'")
            return
        
        # Load hotkey info
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        if hotkey not in hotkeys_data:
            print_error(f"Hotkey '{hotkey}' not found for coldkey '{coldkey}'")
            return
        
        hotkey_info = hotkeys_data[hotkey]
        from_address = hotkey_info['address']
        
        print_info(f"From: {from_address}")
        
        # Get password to decrypt coldkey
        password = getpass.getpass("Enter coldkey password: ")
        
        # Load and decrypt coldkey mnemonic
        coldkey_file = coldkey_path / "coldkey.enc"
        if not coldkey_file.exists():
            print_error(f"Coldkey file not found at {coldkey_file}")
            return
        
        with open(coldkey_file, 'rb') as f:
            encrypted_data = f.read()
        
        try:
            mnemonic = decrypt_data(encrypted_data, password)
        except Exception as e:
            print_error("Failed to decrypt coldkey. Incorrect password?")
            return
        
        # Derive private key from mnemonic
        from sdk.keymanager import KeyGenerator
        kg = KeyGenerator()
        hotkey_index = hotkey_info['index']
        derived_key = kg.derive_hotkey(mnemonic, hotkey_index)
        private_key = derived_key['private_key']
        
        # Verify address matches
        if derived_key['address'].lower() != from_address.lower():
            print_error("Derived address does not match stored address!")
            return
        
        # Initialize Luxtensor client
        client = LuxtensorClient(network_config.rpc_url)
        
        # Get current nonce
        print_info("Getting account nonce...")
        nonce = client.get_nonce(from_address)
        print_info(f"Nonce: {nonce}")
        
        # Check balance
        balance = client.get_balance(from_address)
        balance_mdt = balance / 1_000_000_000
        print_info(f"Current balance: {balance_mdt} MDT")
        
        # Calculate total cost
        total_fee = gas_limit * gas_price
        total_cost = value_base_units + total_fee
        
        if balance < total_cost:
            print_error(f"Insufficient balance. Need {total_cost / 1_000_000_000} MDT, have {balance_mdt} MDT")
            return
        
        # Show transaction details
        print_info("\nTransaction details:")
        details_table = create_table("Transaction Details", ["Field", "Value"])
        details_table.add_row("From", from_address)
        details_table.add_row("To", recipient)
        details_table.add_row("Amount", f"{amount} MDT ({value_base_units} base units)")
        details_table.add_row("Gas Price", str(gas_price))
        details_table.add_row("Gas Limit", str(gas_limit))
        details_table.add_row("Max Fee", f"{total_fee / 1_000_000_000} MDT ({total_fee} base units)")
        details_table.add_row("Total Cost", f"{total_cost / 1_000_000_000} MDT")
        details_table.add_row("Network", network)
        details_table.add_row("Chain ID", str(network_config.chain_id))
        console.print(details_table)
        
        # Confirm transaction
        confirm = click.confirm("\nDo you want to send this transaction?", default=False)
        if not confirm:
            print_warning("Transaction cancelled")
            return
        
        # Build and sign transaction using Luxtensor format
        print_info("\nSigning transaction...")
        
        # Create and sign transaction using Luxtensor transaction format
        from sdk.transactions import LuxtensorTransaction
        tx = LuxtensorTransaction(
            nonce=nonce,
            from_address=from_address,
            to_address=recipient,
            value=value_base_units,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=b''
        )
        
        # Sign the transaction
        from sdk.transactions import sign_transaction
        signed_tx_obj = sign_transaction(tx, private_key)
        
        # Encode for RPC submission
        signed_tx = encode_transaction_for_rpc(signed_tx_obj)
        
        # Submit transaction
        print_info("Submitting transaction...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"\n✅ Transaction submitted successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Status: {result.status}")
        
        if network_config.explorer_url:
            explorer_link = f"{network_config.explorer_url}/tx/{result.tx_hash}"
            print_info(f"Explorer: {explorer_link}")
        
        print_info("\nℹ️  Use 'mtcli tx status <tx_hash>' to check transaction status")
        
    except Exception as e:
        print_error(f"Failed to send transaction: {str(e)}")
        import traceback
        traceback.print_exc()


@tx.command('status')
@click.argument('tx_hash')
@click.option('--network', default='testnet', help='Network name (mainnet, testnet, local)')
def tx_status(tx_hash: str, network: str):
    """
    Query transaction status by hash
    
    Examples:
        mtcli tx status 0x1234567890abcdef... --network testnet
        mtcli tx status 0xabcd... --network mainnet
    """
    try:
        # Get network configuration
        network_config = get_network_config(network)
        
        print_info(f"Querying transaction {tx_hash} on {network}...")
        
        # Initialize Luxtensor client
        client = LuxtensorClient(network_config.rpc_url)
        
        # Get transaction details
        tx_data = client.get_transaction(tx_hash)
        
        if not tx_data:
            print_warning(f"Transaction not found: {tx_hash}")
            print_info("The transaction may not exist or hasn't been indexed yet.")
            return
        
        # Get transaction receipt
        try:
            receipt = client.get_transaction_receipt(tx_hash)
        except Exception:
            receipt = None
        
        # Display transaction information
        console.print()
        
        # Basic info
        info_table = create_table("Transaction Information", ["Field", "Value"])
        info_table.add_row("Transaction Hash", tx_hash)
        info_table.add_row("From", tx_data.get('from', 'N/A'))
        info_table.add_row("To", tx_data.get('to', 'N/A'))
        
        # Value
        value = int(tx_data.get('value', 0), 16) if isinstance(tx_data.get('value'), str) else tx_data.get('value', 0)
        value_mdt = value / 1_000_000_000
        info_table.add_row("Value", f"{value_mdt} MDT ({value} base units)")
        
        # Gas info
        gas_price = int(tx_data.get('gasPrice', 0), 16) if isinstance(tx_data.get('gasPrice'), str) else tx_data.get('gasPrice', 0)
        gas_limit = int(tx_data.get('gas', 0), 16) if isinstance(tx_data.get('gas'), str) else tx_data.get('gas', 0)
        info_table.add_row("Gas Price", str(gas_price))
        info_table.add_row("Gas Limit", str(gas_limit))
        
        # Nonce
        nonce = int(tx_data.get('nonce', 0), 16) if isinstance(tx_data.get('nonce'), str) else tx_data.get('nonce', 0)
        info_table.add_row("Nonce", str(nonce))
        
        # Block info
        block_number = tx_data.get('blockNumber')
        if block_number:
            block_num = int(block_number, 16) if isinstance(block_number, str) else block_number
            info_table.add_row("Block Number", str(block_num))
            info_table.add_row("Status", "✅ Confirmed")
        else:
            info_table.add_row("Block Number", "Pending")
            info_table.add_row("Status", "⏳ Pending")
        
        console.print(info_table)
        
        # Receipt information if available
        if receipt:
            console.print()
            receipt_table = create_table("Transaction Receipt", ["Field", "Value"])
            
            # Status
            status = receipt.get('status', 0)
            status_text = "✅ Success" if status == 1 or status == '0x1' else "❌ Failed"
            receipt_table.add_row("Status", status_text)
            
            # Gas used
            gas_used = int(receipt.get('gasUsed', 0), 16) if isinstance(receipt.get('gasUsed'), str) else receipt.get('gasUsed', 0)
            receipt_table.add_row("Gas Used", str(gas_used))
            
            # Fee
            fee = gas_used * gas_price
            fee_mdt = fee / 1_000_000_000
            receipt_table.add_row("Fee Paid", f"{fee_mdt} MDT ({fee} base units)")
            
            # Block hash
            block_hash = receipt.get('blockHash', 'N/A')
            receipt_table.add_row("Block Hash", block_hash)
            
            console.print(receipt_table)
        
        # Explorer link
        if network_config.explorer_url:
            console.print()
            explorer_link = f"{network_config.explorer_url}/tx/{tx_hash}"
            console.print(f"🔗 Explorer: {explorer_link}")
        
        print_success("\n✅ Transaction query completed")
        
    except Exception as e:
        print_error(f"Failed to query transaction: {str(e)}")
        import traceback
        traceback.print_exc()


@tx.command('history')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name (mainnet, testnet, local)')
@click.option('--limit', default=10, type=int, help='Number of transactions to show')
def tx_history(coldkey: str, hotkey: str, base_dir: Optional[str], network: str, limit: int):
    """
    Show transaction history for a wallet
    
    Examples:
        mtcli tx history --coldkey my_coldkey --hotkey miner_hk1 --network testnet
        mtcli tx history --coldkey my_coldkey --hotkey miner_hk1 --limit 20
    """
    try:
        # Get network configuration
        network_config = get_network_config(network)
        
        # Load wallet
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        hotkeys_file = coldkey_path / "hotkeys.json"
        
        if not hotkeys_file.exists():
            print_error(f"No hotkeys found for coldkey '{coldkey}'")
            return
        
        # Load hotkey info
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        if hotkey not in hotkeys_data:
            print_error(f"Hotkey '{hotkey}' not found for coldkey '{coldkey}'")
            return
        
        hotkey_info = hotkeys_data[hotkey]
        address = hotkey_info['address']
        
        print_info(f"Querying transaction history for {coldkey}/{hotkey} on {network}...")
        print_info(f"Address: {address}")
        
        # Initialize Luxtensor client
        client = LuxtensorClient(network_config.rpc_url)
        
        # Get transaction history
        try:
            transactions = client.get_transactions_for_address(address, limit=limit)
        except Exception as e:
            print_warning(f"Failed to get transaction history: {str(e)}")
            print_info("Transaction history may not be available on this network.")
            print_info("Try querying individual transactions with 'mtcli tx status <tx_hash>'")
            return
        
        if not transactions:
            print_info(f"\nNo transactions found for {address}")
            return
        
        # Display transactions
        console.print()
        tx_table = create_table(f"Transaction History ({len(transactions)} transactions)", [
            "Hash",
            "Block",
            "From",
            "To",
            "Value (MDT)",
            "Status"
        ])
        
        for tx in transactions:
            tx_hash = tx.get('hash', 'N/A')
            # Shorten hash for display
            tx_hash_short = tx_hash[:10] + '...' if len(tx_hash) > 10 else tx_hash
            
            block = str(tx.get('blockNumber', 'Pending'))
            
            from_addr = tx.get('from', 'N/A')
            from_short = from_addr[:8] + '...' if len(from_addr) > 8 else from_addr
            
            to_addr = tx.get('to', 'N/A')
            to_short = to_addr[:8] + '...' if len(to_addr) > 8 else to_addr
            
            value = tx.get('value', 0)
            if isinstance(value, str):
                value = int(value, 16) if value.startswith('0x') else int(value)
            value_mdt = value / 1_000_000_000
            
            status = tx.get('status', 'unknown')
            if status == 1 or status == '0x1' or status == 'success':
                status_icon = "✅"
            elif status == 0 or status == '0x0' or status == 'failed':
                status_icon = "❌"
            else:
                status_icon = "⏳"
            
            tx_table.add_row(
                tx_hash_short,
                block,
                from_short,
                to_short,
                f"{value_mdt:.6f}",
                status_icon
            )
        
        console.print(tx_table)
        
        print_success(f"\n✅ Found {len(transactions)} transaction(s)")
        
        if network_config.explorer_url:
            explorer_link = f"{network_config.explorer_url}/address/{address}"
            print_info(f"Explorer: {explorer_link}")
        
    except Exception as e:
        print_error(f"Failed to get transaction history: {str(e)}")
        import traceback
        traceback.print_exc()

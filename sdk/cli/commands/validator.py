"""
Validator commands for ModernTensor CLI

Commands for validator operations.
"""

import click
from typing import Optional
from pathlib import Path
import json

from sdk.cli.utils import (
    print_warning, print_error, print_success, print_info,
    confirm_action, console, create_table, format_balance,
    format_address, get_default_wallet_path
)
from sdk.cli.config import get_network_config

# Constants
DEFAULT_GAS_PRICE = 1_000_000_000


@click.group(name='validator', short_help='Validator operations')
def validator():
    """
    Validator commands
    
    Manage validator operations and queries.
    """
    pass


@validator.command('start')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID to validate on')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def start_validator(ctx, coldkey: str, hotkey: str, subnet_uid: int,
                   base_dir: Optional[str], network: str):
    """
    Start validator node
    
    Starts a validator node for the specified subnet. This is a placeholder
    command - actual validator nodes run as separate processes.
    
    Examples:
        mtcli validator start --coldkey my_coldkey --hotkey validator_hk --subnet-uid 1
    
    Note:
        - Validator nodes typically run as separate services
        - This command helps configure and check validator setup
        - Actual node software may be separate from mtcli
    """
    print_info(f"Validator node management for subnet {subnet_uid}")
    print_info("")
    print_warning("Note: Validator nodes run as separate processes/services")
    print_info("")
    print_info("To run a validator:")
    print_info("1. Ensure your hotkey is registered on the subnet:")
    print_info(f"   mtcli wallet register-hotkey --coldkey {coldkey} --hotkey {hotkey} --subnet-uid {subnet_uid}")
    print_info("")
    print_info("2. Ensure you have sufficient stake:")
    print_info(f"   mtcli stake add --coldkey {coldkey} --hotkey {hotkey} --amount <amount>")
    print_info("")
    print_info("3. Run the validator node software (separate from mtcli):")
    print_info("   python -m sdk.validator.node --coldkey {coldkey} --hotkey {hotkey} --subnet {subnet_uid}")
    print_info("")
    print_info("4. Check validator status:")
    print_info(f"   mtcli validator status --coldkey {coldkey} --hotkey {hotkey}")


@validator.command('stop')
@click.pass_context
def stop_validator(ctx):
    """
    Stop validator node
    
    Stops a running validator node. This is a placeholder command.
    
    Note:
        - Validator nodes run as separate processes
        - Use your system's process manager to stop validator nodes
        - Or use the validator node software's stop command
    """
    print_info("To stop a validator node:")
    print_info("1. If running as a service: sudo systemctl stop moderntensor-validator")
    print_info("2. If running in terminal: Press Ctrl+C")
    print_info("3. Or kill the process: pkill -f validator.node")
    print_info("")
    print_warning("Note: This does not un-register your hotkey from the subnet")
    print_info("Your stake and registration remain active after stopping the node")


@validator.command('status')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', type=int, help='Subnet UID (optional)')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def validator_status(ctx, coldkey: str, hotkey: str, subnet_uid: Optional[int],
                    base_dir: Optional[str], network: str):
    """
    Show validator status
    
    Shows the current status of a validator including stake, registration,
    and performance metrics.
    
    Examples:
        mtcli validator status --coldkey my_coldkey --hotkey validator_hk
        mtcli validator status --coldkey my_coldkey --hotkey validator_hk --subnet-uid 1
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.cli.wallet_utils import get_hotkey_address
        
        # Get network config
        network_config = get_network_config(network)
        
        # Get hotkey address
        hotkey_address = get_hotkey_address(coldkey, hotkey, base_dir)
        
        print_info(f"Querying validator status for {coldkey}/{hotkey}")
        print_info(f"Address: {hotkey_address}")
        
        # Initialize client
        client = LuxtensorClient(network_config.rpc_url)
        
        # Get basic info
        balance = client.get_balance(hotkey_address)
        stake = client.get_stake(hotkey_address)
        
        balance_formatted = format_balance(balance, decimals=9)
        stake_formatted = format_balance(stake, decimals=9)
        
        # Check if registered on subnet
        is_registered = False
        if subnet_uid is not None:
            try:
                is_registered = client.is_hotkey_registered(subnet_uid, hotkey_address)
            except:
                pass
        
        # Display status
        console.print("\n[bold cyan]Validator Status[/bold cyan]\n")
        
        table = create_table()
        table.add_column("Field", style="bold")
        table.add_column("Value")
        
        table.add_row("Coldkey", coldkey)
        table.add_row("Hotkey", hotkey)
        table.add_row("Address", hotkey_address)
        table.add_row("Network", network)
        table.add_row("", "")
        table.add_row("Balance", f"{balance_formatted} MDT ({balance} base)")
        table.add_row("Stake", f"{stake_formatted} MDT ({stake} base)")
        
        if subnet_uid is not None:
            table.add_row("", "")
            table.add_row("Subnet UID", str(subnet_uid))
            status_text = "✅ Registered" if is_registered else "❌ Not Registered"
            table.add_row("Registration Status", status_text)
        
        console.print(table)
        
        # Additional info
        console.print()
        if stake == 0:
            print_warning("⚠️  No stake detected. Validators require stake to participate.")
            print_info("   Add stake with: mtcli stake add --coldkey {coldkey} --hotkey {hotkey} --amount <amount>")
        
        if subnet_uid and not is_registered:
            print_warning(f"⚠️  Not registered on subnet {subnet_uid}")
            print_info(f"   Register with: mtcli wallet register-hotkey --coldkey {coldkey} --hotkey {hotkey} --subnet-uid {subnet_uid}")
        
        print_success("\n✅ Status query completed")
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to query validator status: {str(e)}")


@validator.command('set-weights')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--weights-file', required=True, type=click.Path(exists=True), help='Weights JSON file')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def set_weights(ctx, coldkey: str, hotkey: str, subnet_uid: int, weights_file: str,
               base_dir: Optional[str], network: str, yes: bool):
    """
    Set validator weights
    
    Sets weights for miners in the subnet. Weights determine reward distribution.
    
    Examples:
        mtcli validator set-weights --coldkey my_coldkey --hotkey validator_hk --subnet-uid 1 --weights-file weights.json
    
    Weights file format (JSON):
        {
            "weights": [
                {"uid": 0, "weight": 0.5},
                {"uid": 1, "weight": 0.3},
                {"uid": 2, "weight": 0.2}
            ]
        }
    
    Note:
        - Weights must sum to 1.0
        - Only registered validators can set weights
        - Transaction encoding not yet implemented
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        from sdk.cli.wallet_utils import derive_hotkey_from_coldkey
        from rich.table import Table
        
        # Load weights file
        print_info(f"Loading weights from {weights_file}")
        with open(weights_file, 'r') as f:
            weights_data = json.load(f)
        
        weights_list = weights_data.get('weights', [])
        if not weights_list:
            print_error("No weights found in file")
            return
        
        # Validate weights
        total_weight = sum(w.get('weight', 0) for w in weights_list)
        if abs(total_weight - 1.0) > 0.001:
            print_error(f"Weights must sum to 1.0 (current sum: {total_weight})")
            return
        
        print_info(f"Setting weights for {len(weights_list)} miners on subnet {subnet_uid}")
        
        # Get network config
        network_config = get_network_config(network)
        rpc_url = network_config.rpc_url
        chain_id = network_config.chain_id
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Check if registered
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']
        
        try:
            is_registered = client.is_hotkey_registered(subnet_uid, from_address)
            if not is_registered:
                print_error(f"Hotkey not registered on subnet {subnet_uid}")
                return
        except:
            print_warning("Could not verify registration status")
        
        # Get nonce
        print_info("Fetching account nonce...")
        nonce = client.get_nonce(from_address)
        
        # Estimate gas
        gas_limit = TransactionSigner.estimate_gas('set_weights')
        gas_price = DEFAULT_GAS_PRICE
        
        # Build set weights transaction data using Luxtensor pallet encoding
        from sdk.luxtensor_pallets import encode_set_weights
        
        # Extract UIDs and weights from the weights list
        # Convert float weights (0.0-1.0) to u32 by scaling to 0-1000000 range
        neuron_uids = [w.get('uid') for w in weights_list]
        weight_values = [int(w.get('weight') * 1_000_000) for w in weights_list]
        
        encoded_call = encode_set_weights(subnet_uid, neuron_uids, weight_values)
        weights_tx_data = encoded_call.data
        
        print_info(f"Transaction: {encoded_call.description}")
        print_info(f"Estimated gas: {encoded_call.gas_estimate}")
        
        # Create transaction signer
        signer = TransactionSigner(private_key)
        
        # Build and sign transaction
        print_info("Building and signing transaction...")
        signed_tx = signer.build_and_sign_transaction(
            to=from_address,  # Weights might go to a specific contract
            value=0,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=weights_tx_data,
            chain_id=chain_id
        )
        
        # Display summary
        console.print("\n[bold yellow]Set Weights Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Subnet UID:[/bold]", str(subnet_uid))
        table.add_row("[bold]Validator:[/bold]", f"{coldkey}/{hotkey}")
        table.add_row("[bold]Address:[/bold]", from_address)
        table.add_row("[bold]Number of Weights:[/bold]", str(len(weights_list)))
        table.add_row("[bold]Network:[/bold]", network)
        console.print(table)
        
        # Show weights
        console.print("\n[bold]Weights:[/bold]")
        weights_table = create_table()
        weights_table.add_column("UID", justify="right")
        weights_table.add_column("Weight", justify="right")
        for w in weights_list[:10]:  # Show first 10
            weights_table.add_row(str(w.get('uid')), f"{w.get('weight'):.4f}")
        if len(weights_list) > 10:
            weights_table.add_row("...", "...")
        console.print(weights_table)
        console.print()
        
        # Confirm
        if not yes and not confirm_action("Submit weights transaction?"):
            print_warning("Transaction cancelled")
            return
        
        # Submit transaction
        print_info("Submitting transaction to network...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"Weights set successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Block: {result.block_number}")
        
    except FileNotFoundError as e:
        print_error(f"File not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except json.JSONDecodeError as e:
        print_error(f"Invalid JSON in weights file: {str(e)}")
    except Exception as e:
        print_error(f"Failed to set weights: {str(e)}")

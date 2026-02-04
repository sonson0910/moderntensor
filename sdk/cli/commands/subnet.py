"""
Subnet commands for ModernTensor CLI

Commands for managing subnets.
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
MDT_TO_BASE_UNITS = 1_000_000_000
DEFAULT_GAS_PRICE = 1_000_000_000


@click.group(name='subnet', short_help='Manage subnets')
def subnet():
    """
    Subnet commands
    
    Manage and query subnets on the network.
    """
    pass


@subnet.command('create')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--name', required=True, help='Subnet name')
@click.option('--registration-cost', type=float, default=1000, help='Registration cost in MDT')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def create_subnet(ctx, coldkey: str, name: str, registration_cost: float, 
                 base_dir: Optional[str], network: str, yes: bool):
    """
    Create a new subnet
    
    Creates a new subnet on the Luxtensor network. Requires payment of
    registration cost.
    
    Examples:
        mtcli subnet create --coldkey my_coldkey --name "AI Subnet" --registration-cost 1000
        mtcli subnet create --coldkey my_coldkey --name "Research Subnet" --network testnet
    
    Note:
        - Creating a subnet requires burning MDT tokens
        - You become the owner of the subnet
        - Subnet creation transaction encoding not yet implemented
    """
    try:
        from sdk.client import LuxtensorClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        from sdk.cli.wallet_utils import derive_hotkey_from_coldkey
        from rich.table import Table
        
        print_info(f"Creating subnet: {name}")
        
        # Get network config
        network_config = get_network_config(network)
        rpc_url = network_config.rpc_url
        chain_id = network_config.chain_id
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Load coldkey info - use first hotkey as transaction sender
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        hotkeys_file = coldkey_path / "hotkeys.json"
        
        if not hotkeys_file.exists():
            print_error(f"No hotkeys found for coldkey '{coldkey}'")
            print_info("Please generate a hotkey first with: mtcli wallet generate-hotkey")
            return
        
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        hotkeys_list = hotkeys_data.get('hotkeys', [])
        if not hotkeys_list:
            print_error("No hotkeys available")
            return
        
        # Use first hotkey
        first_hotkey = hotkeys_list[0]
        hotkey_name = first_hotkey['name']
        
        print_info(f"Using hotkey '{hotkey_name}' for transaction")
        
        # Derive hotkey to get private key
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey_name, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']
        
        # Convert cost to base units
        cost_base = int(registration_cost * MDT_TO_BASE_UNITS)
        
        # Get current nonce
        print_info("Fetching account nonce...")
        nonce = client.get_nonce(from_address)
        
        # Check balance
        balance = client.get_balance(from_address)
        if balance < cost_base:
            print_error(f"Insufficient balance. Need {registration_cost} MDT, have {balance / MDT_TO_BASE_UNITS} MDT")
            return
        
        # Estimate gas
        gas_limit = TransactionSigner.estimate_gas('complex')
        gas_price = DEFAULT_GAS_PRICE
        
        # Build subnet creation transaction data using Luxtensor pallet encoding
        from sdk.luxtensor_pallets import encode_subnet_create
        
        # Use registration_cost as initial emission rate for the subnet
        encoded_call = encode_subnet_create(name, cost_base)
        subnet_data = encoded_call.data
        
        print_info(f"Transaction: {encoded_call.description}")
        print_info(f"Estimated gas: {encoded_call.gas_estimate}")
        
        # Create transaction signer
        signer = TransactionSigner(private_key)
        
        # Build and sign transaction
        print_info("Building and signing transaction...")
        signed_tx = signer.build_and_sign_transaction(
            to=from_address,  # Subnet creation might go to a specific contract
            value=cost_base,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=subnet_data,
            chain_id=chain_id
        )
        
        # Display summary
        console.print("\n[bold yellow]Subnet Creation Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Subnet Name:[/bold]", name)
        table.add_row("[bold]Owner (Coldkey):[/bold]", coldkey)
        table.add_row("[bold]Transaction From:[/bold]", from_address)
        table.add_row("[bold]Network:[/bold]", network)
        table.add_row("", "")
        table.add_row("[bold yellow]Registration Cost:[/bold yellow]", f"{registration_cost} MDT ({cost_base} base units)")
        table.add_row("[bold]Gas Limit:[/bold]", str(gas_limit))
        table.add_row("[bold]Gas Price:[/bold]", f"{gas_price} ({gas_price / 1e9} Gwei)")
        table.add_row("[bold]Est. Fee:[/bold]", f"{gas_limit * gas_price} base units")
        console.print(table)
        console.print()
        
        # Confirm
        if not yes and not confirm_action("Create subnet and burn MDT tokens?"):
            print_warning("Subnet creation cancelled")
            return
        
        # Submit transaction
        print_info("Submitting transaction to network...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"Subnet created successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Block: {result.block_number}")
        print_info(f"Subnet name: {name}")
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to create subnet: {str(e)}")


@subnet.command('register')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def register_subnet(ctx, coldkey: str, hotkey: str, subnet_uid: int, 
                   base_dir: Optional[str], network: str, yes: bool):
    """
    Register on a subnet
    
    Registers your hotkey on an existing subnet to participate as a miner/validator.
    
    Examples:
        mtcli subnet register --coldkey my_coldkey --hotkey miner_hk1 --subnet-uid 1
    
    Note:
        - This is similar to wallet register-hotkey but specific to subnet registration
        - Use 'wallet register-hotkey' for full registration with optional stake
    """
    print_info(f"For registering hotkeys on subnets, please use:")
    print_info(f"  mtcli wallet register-hotkey --coldkey {coldkey} --hotkey {hotkey} --subnet-uid {subnet_uid}")
    print_info("")
    print_info("The wallet register-hotkey command provides full registration functionality")
    print_info("with optional initial stake and API endpoint configuration.")


@subnet.command('info')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def subnet_info(ctx, subnet_uid: int, network: str):
    """
    Show subnet information
    
    Displays detailed information about a specific subnet.
    
    Examples:
        mtcli subnet info --subnet-uid 1 --network testnet
    """
    print_info("For querying subnet information, please use:")
    print_info(f"  mtcli query subnet --subnet-uid {subnet_uid} --network {network}")
    print_info("")
    print_info("The query subnet command provides comprehensive subnet information")
    print_info("including hyperparameters, neuron count, and emission rates.")


@subnet.command('participants')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
@click.option('--limit', default=50, type=int, help='Max participants to show')
@click.pass_context
def subnet_participants(ctx, subnet_uid: int, network: str, limit: int):
    """
    List subnet participants
    
    Lists all miners and validators participating in the subnet.
    
    Examples:
        mtcli subnet participants --subnet-uid 1 --network testnet --limit 100
    """
    try:
        from sdk.client import LuxtensorClient
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying participants for subnet {subnet_uid} on {network_config.name}...")
        
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get subnet neurons/participants
            try:
                neurons = client.get_neurons(subnet_uid)
            except:
                print_error(f"Failed to get participants for subnet {subnet_uid}")
                print_info("Subnet may not exist or API not available")
                return
            
            if not neurons:
                print_warning(f"No participants found in subnet {subnet_uid}")
                return
            
            # Limit results
            neurons = neurons[:limit]
            
            # Display participants
            console.print(f"\n[bold cyan]Subnet {subnet_uid} Participants[/bold cyan]\n")
            
            table = create_table()
            table.add_column("UID", justify="right", style="cyan")
            table.add_column("Address", style="yellow")
            table.add_column("Type", justify="center")
            table.add_column("Stake", justify="right", style="green")
            table.add_column("Active", justify="center")
            
            for neuron in neurons:
                uid = str(neuron.get('uid', 'N/A'))
                address = neuron.get('hotkey', 'N/A')
                if len(address) > 20:
                    address = format_address(address)
                
                # Determine type (validator vs miner)
                is_validator = neuron.get('validator_permit', False)
                neuron_type = "Validator" if is_validator else "Miner"
                
                # Get stake
                stake = neuron.get('stake', 0)
                stake_formatted = format_balance(stake, decimals=9)
                
                # Active status
                is_active = neuron.get('active', False)
                status = "✅" if is_active else "❌"
                
                table.add_row(uid, address, neuron_type, stake_formatted, status)
            
            console.print(table)
            print_success(f"\nFound {len(neurons)} participant(s) in subnet {subnet_uid}")
            
        except Exception as e:
            print_error(f"Failed to query participants: {str(e)}")
            
    except Exception as e:
        print_error(f"Failed to list participants: {str(e)}")

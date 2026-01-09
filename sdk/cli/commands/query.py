"""
Query commands for ModernTensor CLI

Commands for querying blockchain information.
"""

import click
from typing import Optional
from pathlib import Path
import json

from sdk.cli.utils import (
    print_warning, print_error, print_success, print_info,
    console, create_table, format_balance, format_address,
    get_default_wallet_path
)
from sdk.cli.config import get_network_config


@click.group(name='query', short_help='Query blockchain information')
def query():
    """
    Query commands
    
    Query information from the Luxtensor blockchain.
    """
    pass


@query.command('address')
@click.argument('address')
@click.option('--network', default='testnet', help='Network name')
def query_address(address: str, network: str):
    """
    Query address information
    
    Query balance, nonce, and stake for any address on the network.
    
    Example:
        mtcli query address 0x1234567890abcdef... --network testnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from rich.panel import Panel
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying address {format_address(address)} on {network_config.name}...")
        
        # Create client and query
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get account info
            balance = client.get_balance(address)
            nonce = client.get_nonce(address)
            
            # Try to get stake info
            try:
                stake = client.get_stake(address)
            except:
                stake = 0
            
            # Format values
            balance_formatted = format_balance(balance, decimals=9)
            stake_formatted = format_balance(stake, decimals=9)
            
            # Display results
            info_text = f"""[bold cyan]Address:[/bold cyan] {address}
[bold cyan]Network:[/bold cyan] {network_config.name}

[bold green]Balance:[/bold green] {balance_formatted} MDT ({balance} base units)
[bold green]Stake:[/bold green] {stake_formatted} MDT ({stake} base units)
[bold yellow]Nonce:[/bold yellow] {nonce}"""
            
            if network_config.explorer_url:
                info_text += f"\n\n[bold cyan]Explorer:[/bold cyan] {network_config.explorer_url}/address/{address}"
            
            panel = Panel(info_text, title="Address Information", border_style="green")
            console.print(panel)
            
            print_success("Query completed successfully")
            
        except Exception as e:
            print_error(f"Failed to query blockchain: {str(e)}")
            print_info(f"Make sure the RPC endpoint is accessible: {network_config.rpc_url}")
            
    except Exception as e:
        print_error(f"Failed to query address: {str(e)}")


@query.command('balance')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
def query_balance(coldkey: str, hotkey: str, base_dir: Optional[str], network: str):
    """
    Query balance for hotkey
    
    Queries the balance for a specific hotkey on the network.
    
    Example:
        mtcli query balance --coldkey my_coldkey --hotkey miner_hk1 --network testnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        
        wallet_path = Path(base_dir) if base_dir else get_default_wallet_path()
        coldkey_path = wallet_path / coldkey
        
        # Load hotkeys
        hotkeys_file = coldkey_path / "hotkeys.json"
        if not hotkeys_file.exists():
            print_error(f"No hotkeys found for coldkey '{coldkey}'")
            return
        
        with open(hotkeys_file, 'r') as f:
            hotkeys_data = json.load(f)
        
        # Find the hotkey
        hotkey_info = None
        for hk in hotkeys_data.get('hotkeys', []):
            if hk['name'] == hotkey:
                hotkey_info = hk
                break
        
        if not hotkey_info:
            print_error(f"Hotkey '{hotkey}' not found")
            return
        
        address = hotkey_info['address']
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying balance for {coldkey}/{hotkey} on {network_config.name}...")
        
        # Query balance
        try:
            client = LuxtensorClient(network_config.rpc_url)
            balance = client.get_balance(address)
            
            balance_formatted = format_balance(balance, decimals=9)
            
            # Display result
            table = create_table("Balance Query", ["Field", "Value"])
            table.add_row("Wallet", f"{coldkey}/{hotkey}")
            table.add_row("Address", address)
            table.add_row("Network", network_config.name)
            table.add_row("Balance (MDT)", balance_formatted)
            table.add_row("Balance (base)", str(balance))
            
            console.print(table)
            print_success("Balance query completed")
            
        except Exception as e:
            print_error(f"Failed to query balance: {str(e)}")
            
    except Exception as e:
        print_error(f"Failed to query balance: {str(e)}")


@query.command('subnet')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
def query_subnet(subnet_uid: int, network: str):
    """
    Query subnet information
    
    Queries detailed information about a specific subnet.
    
    Example:
        mtcli query subnet --subnet-uid 1 --network testnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from rich.panel import Panel
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying subnet {subnet_uid} on {network_config.name}...")
        
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get subnet info
            subnet_info = client.get_subnet_info(subnet_uid)
            
            if not subnet_info:
                print_error(f"Subnet {subnet_uid} not found")
                return
            
            # Get neuron count
            try:
                neuron_count = client.get_neuron_count(subnet_uid)
            except:
                neuron_count = 0
            
            # Get subnet hyperparameters
            try:
                hyperparams = client.get_subnet_hyperparameters(subnet_uid)
            except:
                hyperparams = {}
            
            # Display subnet info
            table = create_table(f"Subnet {subnet_uid} Information", ["Field", "Value"])
            table.add_row("Subnet UID", str(subnet_uid))
            table.add_row("Network", network_config.name)
            table.add_row("Neuron Count", str(neuron_count))
            
            # Add hyperparameters if available
            if hyperparams:
                for key, value in hyperparams.items():
                    table.add_row(key.replace('_', ' ').title(), str(value))
            
            # Add subnet info fields
            for key, value in subnet_info.items():
                if key not in ['hyperparameters']:
                    table.add_row(key.replace('_', ' ').title(), str(value))
            
            console.print(table)
            print_success("Subnet query completed")
            
        except Exception as e:
            print_error(f"Failed to query subnet: {str(e)}")
            print_info(f"Make sure subnet {subnet_uid} exists on {network_config.name}")
            
    except Exception as e:
        print_error(f"Failed to query subnet: {str(e)}")


@query.command('list-subnets')
@click.option('--network', default='testnet', help='Network name')
def list_subnets(network: str):
    """
    List all subnets
    
    Lists all registered subnets on the network.
    
    Example:
        mtcli query list-subnets --network testnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying all subnets on {network_config.name}...")
        
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get all subnets
            subnets = client.get_all_subnets()
            
            if not subnets:
                print_warning("No subnets found")
                return
            
            # Display subnets
            table = create_table(f"Subnets on {network_config.name}", 
                               ["UID", "Owner", "Neurons", "Emission"])
            
            for subnet in subnets:
                uid = subnet.get('uid', 'N/A')
                owner = subnet.get('owner', 'N/A')
                if isinstance(owner, str) and len(owner) > 20:
                    owner = format_address(owner)
                
                # Get neuron count
                try:
                    neuron_count = client.get_neuron_count(uid)
                except:
                    neuron_count = 'N/A'
                
                # Get emission
                try:
                    emission = client.get_subnet_emission(uid)
                    emission = format_balance(emission, decimals=9)
                except:
                    emission = 'N/A'
                
                table.add_row(str(uid), owner, str(neuron_count), emission)
            
            console.print(table)
            print_success(f"Found {len(subnets)} subnet(s)")
            
        except Exception as e:
            print_error(f"Failed to query subnets: {str(e)}")
            
    except Exception as e:
        print_error(f"Failed to list subnets: {str(e)}")


@query.command('validator')
@click.argument('address')
@click.option('--network', default='testnet', help='Network name')
def query_validator(address: str, network: str):
    """
    Query validator information
    
    Queries detailed information about a validator.
    
    Example:
        mtcli query validator 0x1234567890abcdef... --network testnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying validator {format_address(address)} on {network_config.name}...")
        
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get validator status
            validator_info = client.get_validator_status(address)
            
            if not validator_info:
                print_warning(f"Address {address} is not a validator")
                return
            
            # Get stake
            try:
                stake = client.get_stake(address)
                stake_formatted = format_balance(stake, decimals=9)
            except:
                stake = 0
                stake_formatted = "0"
            
            # Display validator info
            table = create_table("Validator Information", ["Field", "Value"])
            table.add_row("Address", address)
            table.add_row("Network", network_config.name)
            table.add_row("Stake", f"{stake_formatted} MDT ({stake} base)")
            
            # Add validator info fields
            for key, value in validator_info.items():
                table.add_row(key.replace('_', ' ').title(), str(value))
            
            console.print(table)
            print_success("Validator query completed")
            
        except Exception as e:
            print_error(f"Failed to query validator: {str(e)}")
            
    except Exception as e:
        print_error(f"Failed to query validator: {str(e)}")


@query.command('miner')
@click.argument('address')
@click.option('--network', default='testnet', help='Network name')
def query_miner(address: str, network: str):
    """
    Query miner information
    
    Queries information about a miner (neuron) on the network.
    
    Example:
        mtcli query miner 0x1234567890abcdef... --network testnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        
        # Get network config
        network_config = get_network_config(network)
        
        print_info(f"Querying miner {format_address(address)} on {network_config.name}...")
        
        try:
            client = LuxtensorClient(network_config.rpc_url)
            
            # Get balance and stake
            balance = client.get_balance(address)
            try:
                stake = client.get_stake(address)
            except:
                stake = 0
            
            balance_formatted = format_balance(balance, decimals=9)
            stake_formatted = format_balance(stake, decimals=9)
            
            # Display miner info
            table = create_table("Miner Information", ["Field", "Value"])
            table.add_row("Address", address)
            table.add_row("Network", network_config.name)
            table.add_row("Balance", f"{balance_formatted} MDT ({balance} base)")
            table.add_row("Stake", f"{stake_formatted} MDT ({stake} base)")
            
            console.print(table)
            print_success("Miner query completed")
            
        except Exception as e:
            print_error(f"Failed to query miner: {str(e)}")
            
    except Exception as e:
        print_error(f"Failed to query miner: {str(e)}")

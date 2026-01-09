"""
Staking commands for ModernTensor CLI

Commands for managing stakes on the Luxtensor network.

Commands:
- add: Add stake to become a validator
- remove: Remove stake from validator (with unbonding period)
- claim: Claim staking rewards
- info: Show staking information for a hotkey
- list: List all validators and their stakes
"""

import click
from typing import Optional
from rich.table import Table

from sdk.cli.utils import (
    print_error, print_success, print_info, print_warning,
    confirm_action, console, create_table
)
from sdk.cli.config import get_network_config
from sdk.cli.wallet_utils import derive_hotkey_from_coldkey, get_hotkey_address


@click.group(name='stake', short_help='Manage staking operations')
def stake():
    """
    Staking commands
    
    Manage staking operations for validators and miners on the Luxtensor network.
    
    Operations include adding stake, removing stake, claiming rewards, and querying
    stake information.
    """
    pass


@stake.command('add')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--amount', required=True, type=float, help='Amount to stake (in MDT)')
@click.option('--base-dir', type=click.Path(), default=None, help='Base directory for wallets')
@click.option('--network', default='testnet', help='Network name (mainnet/testnet)')
def add_stake(coldkey: str, hotkey: str, amount: float, base_dir: Optional[str], network: str):
    """
    Add stake to become a validator
    
    Stakes the specified amount of MDT tokens to a hotkey, enabling it to participate
    as a validator in the network. The stake helps secure the network and earns rewards.
    
    Examples:
        mtcli stake add --coldkey my_coldkey --hotkey validator_hk --amount 10000
        mtcli stake add --coldkey my_coldkey --hotkey miner_hk --amount 5000 --network mainnet
    
    Note:
        - Amount is in MDT tokens (1 MDT = 1,000,000,000 base units)
        - Minimum stake may be required (check network requirements)
        - Transaction will require gas fees
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        
        print_info(f"Adding stake: {amount} MDT to hotkey '{hotkey}'")
        
        # Convert MDT to base units
        amount_base = int(amount * 1_000_000_000)
        
        # Get network config
        net_config = get_network_config(network)
        rpc_url = net_config.get('rpc_url', 'http://localhost:9944')
        chain_id = net_config.get('chain_id', 2)
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Derive hotkey to get private key
        print_info("Loading wallet keys...")
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']
        
        # Get current nonce
        print_info("Fetching account nonce...")
        nonce = client.get_nonce(from_address)
        
        # Estimate gas
        gas_limit = TransactionSigner.estimate_gas('stake')
        gas_price = 1000000000  # 1 Gwei - adjust based on network
        
        # Build stake transaction data
        # TODO (GitHub Issue): Implement actual stake transaction encoding based on Luxtensor pallet
        # Expected format: function_selector (4 bytes) + encoded_parameters
        # Example structure:
        #   stake_data = encode_stake_call(
        #       function_selector="0x12345678",  # stake function selector
        #       hotkey=hotkey_address,
        #       amount=amount_base
        #   )
        # Track: Create GitHub issue for transaction encoding implementation
        stake_data = b''  # Placeholder - implement when Luxtensor staking pallet is ready
        
        # Create transaction signer
        signer = TransactionSigner(private_key)
        
        # Build and sign transaction
        print_info("Building and signing transaction...")
        signed_tx = signer.build_and_sign_transaction(
            to=from_address,  # Staking to self
            value=amount_base,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=stake_data,
            chain_id=chain_id
        )
        
        # Confirm before submitting
        console.print("\n[bold yellow]Transaction Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("From:", from_address)
        table.add_row("Hotkey:", hotkey_data['address'])
        table.add_row("Amount:", f"{amount} MDT ({amount_base} base units)")
        table.add_row("Gas Limit:", str(gas_limit))
        table.add_row("Gas Price:", f"{gas_price} ({gas_price / 1e9} Gwei)")
        table.add_row("Est. Fee:", f"{gas_limit * gas_price} base units")
        console.print(table)
        console.print()
        
        if not confirm_action("Submit transaction?"):
            print_warning("Transaction cancelled")
            return
        
        # Submit transaction
        print_info("Submitting transaction to network...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"Stake added successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Block: {result.block_number}")
        
        if not result.success:
            print_warning(f"Transaction may have failed. Status: {result.status}")
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to add stake: {str(e)}")


@stake.command('remove')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--amount', required=True, type=float, help='Amount to unstake (in MDT)')
@click.option('--base-dir', type=click.Path(), default=None, help='Base directory for wallets')
@click.option('--network', default='testnet', help='Network name (mainnet/testnet)')
def remove_stake(coldkey: str, hotkey: str, amount: float, base_dir: Optional[str], network: str):
    """
    Remove stake from validator
    
    Unstakes the specified amount of MDT tokens from a hotkey. Note that there may be
    an unbonding period before the tokens become available.
    
    Examples:
        mtcli stake remove --coldkey my_coldkey --hotkey validator_hk --amount 5000
    
    Note:
        - Unbonding period may apply (typically 7-28 days)
        - You must maintain minimum stake to remain a validator
        - Transaction will require gas fees
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        
        print_info(f"Removing stake: {amount} MDT from hotkey '{hotkey}'")
        
        # Convert MDT to base units
        amount_base = int(amount * 1_000_000_000)
        
        # Get network config
        net_config = get_network_config(network)
        rpc_url = net_config.get('rpc_url', 'http://localhost:9944')
        chain_id = net_config.get('chain_id', 2)
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Get hotkey address
        hotkey_address = get_hotkey_address(coldkey, hotkey, base_dir)
        
        # Check current stake
        print_info("Checking current stake...")
        current_stake = client.get_stake(hotkey_address)
        
        if current_stake < amount_base:
            print_error(f"Insufficient stake. Current: {current_stake / 1e9} MDT, Requested: {amount} MDT")
            return
        
        # Derive hotkey to get private key
        print_info("Loading wallet keys...")
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']
        
        # Get current nonce
        print_info("Fetching account nonce...")
        nonce = client.get_nonce(from_address)
        
        # Estimate gas
        gas_limit = TransactionSigner.estimate_gas('unstake')
        gas_price = 1000000000  # 1 Gwei
        
        # Build unstake transaction data
        # TODO: Implement actual unstake transaction encoding based on Luxtensor pallet
        # Expected format: function_selector (4 bytes) + encoded_parameters
        # Example structure:
        #   unstake_data = encode_unstake_call(
        #       function_selector="0x87654321",  # unstake function selector
        #       hotkey=hotkey_address,
        #       amount=amount_base
        #   )
        unstake_data = b''  # Placeholder - implement when Luxtensor staking pallet is ready
        
        # Create transaction signer
        signer = TransactionSigner(private_key)
        
        # Build and sign transaction
        print_info("Building and signing transaction...")
        signed_tx = signer.build_and_sign_transaction(
            to=from_address,
            value=0,  # No value transfer for unstake
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=unstake_data,
            chain_id=chain_id
        )
        
        # Confirm before submitting
        console.print("\n[bold yellow]Unstake Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("From:", from_address)
        table.add_row("Hotkey:", hotkey_address)
        table.add_row("Amount:", f"{amount} MDT ({amount_base} base units)")
        table.add_row("Current Stake:", f"{current_stake / 1e9} MDT")
        table.add_row("Remaining:", f"{(current_stake - amount_base) / 1e9} MDT")
        console.print(table)
        console.print()
        
        print_warning("⚠️  Note: Unbonding period applies (tokens will be locked for 7-28 days)")
        
        if not confirm_action("Submit unstake transaction?"):
            print_warning("Transaction cancelled")
            return
        
        # Submit transaction
        print_info("Submitting transaction to network...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"Unstake initiated successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Block: {result.block_number}")
        print_warning("Tokens will be available after unbonding period")
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to remove stake: {str(e)}")


@stake.command('claim')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None, help='Base directory for wallets')
@click.option('--network', default='testnet', help='Network name (mainnet/testnet)')
def claim_rewards(coldkey: str, hotkey: str, base_dir: Optional[str], network: str):
    """
    Claim staking rewards
    
    Claims accumulated staking rewards for the specified hotkey. Rewards are earned
    by validators for participating in consensus and securing the network.
    
    Examples:
        mtcli stake claim --coldkey my_coldkey --hotkey validator_hk
        mtcli stake claim --coldkey my_coldkey --hotkey validator_hk --network mainnet
    
    Note:
        - Only accumulated rewards can be claimed
        - Transaction will require gas fees
        - Claimed rewards are sent to the hotkey address
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        
        print_info(f"Claiming rewards for hotkey '{hotkey}'")
        
        # Get network config
        net_config = get_network_config(network)
        rpc_url = net_config.get('rpc_url', 'http://localhost:9944')
        chain_id = net_config.get('chain_id', 2)
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Get hotkey address
        hotkey_address = get_hotkey_address(coldkey, hotkey, base_dir)
        
        # Note: Pending rewards query will be implemented when Luxtensor provides the API
        # For now, proceed with claim operation directly
        
        # Derive hotkey to get private key
        print_info("Loading wallet keys...")
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']
        
        # Get current nonce
        print_info("Fetching account nonce...")
        nonce = client.get_nonce(from_address)
        
        # Estimate gas
        gas_limit = 100000  # Gas for claim operation
        gas_price = 1000000000  # 1 Gwei
        
        # Build claim transaction data
        # TODO: Implement actual claim transaction encoding based on Luxtensor pallet
        # Expected format: function_selector (4 bytes) + encoded_parameters
        # Example structure:
        #   claim_data = encode_claim_call(
        #       function_selector="0xabcdef12",  # claim function selector
        #       hotkey=hotkey_address
        #   )
        claim_data = b''  # Placeholder - implement when Luxtensor staking pallet is ready
        
        # Create transaction signer
        signer = TransactionSigner(private_key)
        
        # Build and sign transaction
        print_info("Building and signing transaction...")
        signed_tx = signer.build_and_sign_transaction(
            to=from_address,
            value=0,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit,
            data=claim_data,
            chain_id=chain_id
        )
        
        # Confirm before submitting
        if not confirm_action("Submit claim transaction?"):
            print_warning("Transaction cancelled")
            return
        
        # Submit transaction
        print_info("Submitting transaction to network...")
        result = client.submit_transaction(signed_tx)
        
        print_success(f"Rewards claimed successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info(f"Block: {result.block_number}")
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to claim rewards: {str(e)}")


@stake.command('info')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--base-dir', type=click.Path(), default=None, help='Base directory for wallets')
@click.option('--network', default='testnet', help='Network name (mainnet/testnet)')
def stake_info(coldkey: str, hotkey: str, base_dir: Optional[str], network: str):
    """
    Show staking information
    
    Displays detailed staking information for the specified hotkey, including:
    - Current stake amount
    - Pending rewards
    - Validator status
    - Performance metrics
    
    Examples:
        mtcli stake info --coldkey my_coldkey --hotkey validator_hk
        mtcli stake info --coldkey my_coldkey --hotkey validator_hk --network mainnet
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        
        print_info(f"Fetching stake information for hotkey '{hotkey}'")
        
        # Get network config
        net_config = get_network_config(network)
        rpc_url = net_config.get('rpc_url', 'http://localhost:9944')
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Get hotkey address
        hotkey_address = get_hotkey_address(coldkey, hotkey, base_dir)
        
        # Query stake information
        print_info("Querying blockchain...")
        stake_amount = client.get_stake(hotkey_address)
        balance = client.get_balance(hotkey_address)
        
        # Create info table
        console.print("\n[bold cyan]Stake Information[/bold cyan]\n")
        
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Coldkey:[/bold]", coldkey)
        table.add_row("[bold]Hotkey:[/bold]", hotkey)
        table.add_row("[bold]Address:[/bold]", hotkey_address)
        table.add_row("[bold]Network:[/bold]", network)
        table.add_row("", "")
        table.add_row("[bold yellow]Current Stake:[/bold yellow]", f"{stake_amount / 1e9:.6f} MDT")
        table.add_row("[bold]Account Balance:[/bold]", f"{balance / 1e9:.6f} MDT")
        table.add_row("[bold]Total Holdings:[/bold]", f"{(stake_amount + balance) / 1e9:.6f} MDT")
        
        console.print(table)
        console.print()
        
        # Additional validator info (if available)
        try:
            # This is a placeholder - actual implementation depends on validator info methods
            print_info("Note: For detailed validator metrics, use 'mtcli query validator' command")
        except Exception:
            pass
        
    except FileNotFoundError as e:
        print_error(f"Wallet not found: {str(e)}")
    except KeyError as e:
        print_error(f"Hotkey not found: {str(e)}")
    except Exception as e:
        print_error(f"Failed to fetch stake info: {str(e)}")


@stake.command('list')
@click.option('--network', default='testnet', help='Network name (mainnet/testnet)')
@click.option('--limit', default=20, type=int, help='Number of validators to show')
def list_stakes(network: str, limit: int):
    """
    List all validators and their stakes
    
    Displays a list of all validators in the network along with their stake amounts,
    status, and performance metrics.
    
    Examples:
        mtcli stake list
        mtcli stake list --network mainnet --limit 50
    
    Note:
        - Shows top validators by stake amount
        - Use --limit to control number of results
    """
    try:
        from sdk.luxtensor_client import LuxtensorClient
        
        print_info(f"Fetching validators from {network}...")
        
        # Get network config
        net_config = get_network_config(network)
        rpc_url = net_config.get('rpc_url', 'http://localhost:9944')
        
        # Initialize client
        client = LuxtensorClient(rpc_url)
        
        # Get validators
        validators = client.get_validators()
        
        if not validators:
            print_warning("No validators found on network")
            return
        
        # Limit results
        validators = validators[:limit]
        
        # Create table
        console.print(f"\n[bold cyan]Validators on {network}[/bold cyan]\n")
        
        table = create_table()
        table.add_column("Rank", justify="right", style="cyan")
        table.add_column("Address", style="yellow")
        table.add_column("Stake", justify="right", style="green")
        table.add_column("Status", justify="center")
        
        for i, validator in enumerate(validators, 1):
            # Parse validator data (structure depends on actual API)
            address = validator.get('address', 'N/A')[:16] + "..."
            stake = validator.get('stake', 0)
            stake_mdt = stake / 1e9
            status = "🟢 Active" if validator.get('active', False) else "🔴 Inactive"
            
            table.add_row(
                str(i),
                address,
                f"{stake_mdt:.2f} MDT",
                status
            )
        
        console.print(table)
        console.print(f"\n[dim]Showing {len(validators)} validators[/dim]\n")
        
        # Total stake
        total_stake = sum(v.get('stake', 0) for v in validators)
        print_info(f"Total stake (top {len(validators)}): {total_stake / 1e9:.2f} MDT")
        
    except Exception as e:
        print_error(f"Failed to list validators: {str(e)}")

# sdk/cli/l1_stake_cli.py
"""
Layer 1 Staking CLI commands for ModernTensor blockchain.

Provides CLI commands for native Layer 1 staking operations.
"""
import click
from rich.console import Console
from rich.panel import Panel
from rich.table import Table
from typing import Optional

from sdk.config.settings import settings, logger
from sdk.blockchain.state import StateDB
from sdk.blockchain.l1_staking_service import L1StakingService
from sdk.blockchain.l1_keymanager import L1KeyManager


console = Console()


@click.group()
def l1_stake_cli():
    """
    🛡️ Layer 1 staking commands for native blockchain validators.
    """
    pass


def _get_l1_staking_service(state_path: Optional[str] = None) -> L1StakingService:
    """
    Initialize Layer 1 staking service.
    
    Args:
        state_path: Path to state database
        
    Returns:
        L1StakingService: Initialized service
    """
    if state_path is None:
        state_path = str(settings.HOTKEY_BASE_DIR / "l1_state")
    
    state_db = StateDB(storage_path=state_path)
    return L1StakingService(state_db)


@l1_stake_cli.command("add")
@click.option("--address", required=True, help="Validator address (hex)")
@click.option("--private-key", required=True, help="Private key for signing (hex)")
@click.option("--amount", required=True, type=int, help="Amount to stake")
@click.option("--public-key", required=True, help="Validator public key (hex)")
@click.option("--nonce", default=0, type=int, help="Transaction nonce")
@click.option("--gas-price", default=1000, type=int, help="Gas price")
@click.option("--gas-limit", default=100000, type=int, help="Gas limit")
@click.option("--state-path", default=None, help="Path to state database")
@click.option("--yes", is_flag=True, help="Skip confirmation prompt")
def add_stake_cmd(address, private_key, amount, public_key, nonce, gas_price, gas_limit, state_path, yes):
    """
    💰 Add stake to become a validator or increase validator stake.
    """
    try:
        # Parse addresses and keys
        from_address = bytes.fromhex(address)
        validator_address = from_address  # Self-staking
        priv_key = bytes.fromhex(private_key)
        pub_key = bytes.fromhex(public_key)
        
        # Validate inputs
        if len(from_address) != 20:
            console.print(f"[bold red]Error:[/bold red] Address must be 20 bytes (40 hex chars)")
            return
        if len(priv_key) != 32:
            console.print(f"[bold red]Error:[/bold red] Private key must be 32 bytes (64 hex chars)")
            return
        if len(pub_key) != 32:
            console.print(f"[bold red]Error:[/bold red] Public key must be 32 bytes (64 hex chars)")
            return
        
        # Initialize service
        service = _get_l1_staking_service(state_path)
        
        # Check current balance
        current_balance = service.state.get_balance(from_address)
        current_stake = service.state.get_staked_amount(validator_address)
        
        console.print(f"\n[bold]Staking Details:[/bold]")
        console.print(f"  Validator: [cyan]{validator_address.hex()}[/cyan]")
        console.print(f"  Current Balance: [yellow]{current_balance:,}[/yellow] tokens")
        console.print(f"  Current Stake: [yellow]{current_stake:,}[/yellow] tokens")
        console.print(f"  Amount to Stake: [green]{amount:,}[/green] tokens")
        console.print(f"  New Total Stake: [green]{current_stake + amount:,}[/green] tokens")
        console.print(f"  Gas Cost: [yellow]{gas_price * gas_limit:,}[/yellow] tokens\n")
        
        if not yes:
            click.confirm("Proceed with staking?", abort=True)
        
        # Create stake transaction
        console.print("⏳ Creating stake transaction...")
        tx = service.stake(
            from_address=from_address,
            validator_address=validator_address,
            amount=amount,
            public_key=pub_key,
            private_key=priv_key,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit
        )
        
        if tx is None:
            console.print("[bold red]Failed to create stake transaction.[/bold red]")
            return
        
        # Execute transaction
        console.print("⏳ Executing stake transaction...")
        success = service.execute_staking_tx(tx)
        
        if success:
            # Commit state
            service.state.commit()
            console.print(f"\n[bold green]✅ Stake transaction successful![/bold green]")
            console.print(f"  Transaction Hash: [blue]{tx.hash().hex()}[/blue]")
            console.print(f"  New Stake: [green]{service.state.get_staked_amount(validator_address):,}[/green] tokens")
        else:
            console.print("[bold red]❌ Stake transaction failed.[/bold red]")
    
    except ValueError as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
    except Exception as e:
        console.print(f"[bold red]Unexpected error:[/bold red] {e}")
        logger.exception("Add stake command failed")


@l1_stake_cli.command("remove")
@click.option("--address", required=True, help="Validator address (hex)")
@click.option("--private-key", required=True, help="Private key for signing (hex)")
@click.option("--amount", required=True, type=int, help="Amount to unstake")
@click.option("--nonce", default=0, type=int, help="Transaction nonce")
@click.option("--gas-price", default=1000, type=int, help="Gas price")
@click.option("--gas-limit", default=100000, type=int, help="Gas limit")
@click.option("--state-path", default=None, help="Path to state database")
@click.option("--yes", is_flag=True, help="Skip confirmation prompt")
def remove_stake_cmd(address, private_key, amount, nonce, gas_price, gas_limit, state_path, yes):
    """
    💸 Remove stake from validator and return tokens to balance.
    """
    try:
        # Parse addresses and keys
        from_address = bytes.fromhex(address)
        validator_address = from_address
        priv_key = bytes.fromhex(private_key)
        
        # Initialize service
        service = _get_l1_staking_service(state_path)
        
        # Check current stake
        current_stake = service.state.get_staked_amount(validator_address)
        
        if current_stake < amount:
            console.print(f"[bold red]Error:[/bold red] Insufficient stake. Current: {current_stake:,}, Requested: {amount:,}")
            return
        
        console.print(f"\n[bold]Unstaking Details:[/bold]")
        console.print(f"  Validator: [cyan]{validator_address.hex()}[/cyan]")
        console.print(f"  Current Stake: [yellow]{current_stake:,}[/yellow] tokens")
        console.print(f"  Amount to Unstake: [red]{amount:,}[/red] tokens")
        console.print(f"  Remaining Stake: [yellow]{current_stake - amount:,}[/yellow] tokens")
        console.print(f"  Gas Cost: [yellow]{gas_price * gas_limit:,}[/yellow] tokens\n")
        
        if not yes:
            click.confirm("Proceed with unstaking?", abort=True)
        
        # Create unstake transaction
        console.print("⏳ Creating unstake transaction...")
        tx = service.unstake(
            from_address=from_address,
            validator_address=validator_address,
            amount=amount,
            private_key=priv_key,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit
        )
        
        if tx is None:
            console.print("[bold red]Failed to create unstake transaction.[/bold red]")
            return
        
        # Execute transaction
        console.print("⏳ Executing unstake transaction...")
        success = service.execute_staking_tx(tx)
        
        if success:
            # Commit state
            service.state.commit()
            console.print(f"\n[bold green]✅ Unstake transaction successful![/bold green]")
            console.print(f"  Transaction Hash: [blue]{tx.hash().hex()}[/blue]")
            console.print(f"  Remaining Stake: [yellow]{service.state.get_staked_amount(validator_address):,}[/yellow] tokens")
            console.print(f"  New Balance: [green]{service.state.get_balance(from_address):,}[/green] tokens")
        else:
            console.print("[bold red]❌ Unstake transaction failed.[/bold red]")
    
    except ValueError as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
    except Exception as e:
        console.print(f"[bold red]Unexpected error:[/bold red] {e}")
        logger.exception("Remove stake command failed")


@l1_stake_cli.command("claim")
@click.option("--address", required=True, help="Validator address (hex)")
@click.option("--private-key", required=True, help="Private key for signing (hex)")
@click.option("--nonce", default=0, type=int, help="Transaction nonce")
@click.option("--gas-price", default=1000, type=int, help="Gas price")
@click.option("--gas-limit", default=50000, type=int, help="Gas limit")
@click.option("--state-path", default=None, help="Path to state database")
@click.option("--yes", is_flag=True, help="Skip confirmation prompt")
def claim_rewards_cmd(address, private_key, nonce, gas_price, gas_limit, state_path, yes):
    """
    💎 Claim accumulated staking rewards.
    """
    try:
        # Parse addresses and keys
        from_address = bytes.fromhex(address)
        validator_address = from_address
        priv_key = bytes.fromhex(private_key)
        
        # Initialize service
        service = _get_l1_staking_service(state_path)
        
        # Check pending rewards
        pending_rewards = service.state.get_pending_rewards(validator_address)
        
        if pending_rewards == 0:
            console.print(f"[bold yellow]No pending rewards to claim.[/bold yellow]")
            return
        
        current_balance = service.state.get_balance(from_address)
        
        console.print(f"\n[bold]Claim Rewards Details:[/bold]")
        console.print(f"  Validator: [cyan]{validator_address.hex()}[/cyan]")
        console.print(f"  Pending Rewards: [green]{pending_rewards:,}[/green] tokens")
        console.print(f"  Current Balance: [yellow]{current_balance:,}[/yellow] tokens")
        console.print(f"  New Balance: [green]{current_balance + pending_rewards:,}[/green] tokens")
        console.print(f"  Gas Cost: [yellow]{gas_price * gas_limit:,}[/yellow] tokens\n")
        
        if not yes:
            click.confirm("Proceed with claiming rewards?", abort=True)
        
        # Create claim rewards transaction
        console.print("⏳ Creating claim rewards transaction...")
        tx = service.claim_rewards(
            from_address=from_address,
            validator_address=validator_address,
            private_key=priv_key,
            nonce=nonce,
            gas_price=gas_price,
            gas_limit=gas_limit
        )
        
        if tx is None:
            console.print("[bold red]Failed to create claim rewards transaction.[/bold red]")
            return
        
        # Execute transaction
        console.print("⏳ Executing claim rewards transaction...")
        success = service.execute_staking_tx(tx)
        
        if success:
            # Commit state
            service.state.commit()
            console.print(f"\n[bold green]✅ Rewards claimed successfully![/bold green]")
            console.print(f"  Transaction Hash: [blue]{tx.hash().hex()}[/blue]")
            console.print(f"  Rewards Claimed: [green]{pending_rewards:,}[/green] tokens")
            console.print(f"  New Balance: [green]{service.state.get_balance(from_address):,}[/green] tokens")
        else:
            console.print("[bold red]❌ Claim rewards transaction failed.[/bold red]")
    
    except ValueError as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
    except Exception as e:
        console.print(f"[bold red]Unexpected error:[/bold red] {e}")
        logger.exception("Claim rewards command failed")


@l1_stake_cli.command("info")
@click.option("--address", required=True, help="Validator address (hex)")
@click.option("--state-path", default=None, help="Path to state database")
def staking_info_cmd(address, state_path):
    """
    ℹ️  Show staking information for a validator.
    """
    try:
        # Parse address
        validator_address = bytes.fromhex(address)
        
        # Initialize service
        service = _get_l1_staking_service(state_path)
        
        # Get staking info
        info = service.get_staking_info(validator_address)
        
        # Create info table
        table = Table(title=f"Staking Info for {validator_address.hex()[:16]}...")
        table.add_column("Property", style="cyan")
        table.add_column("Value", style="yellow")
        
        table.add_row("Address", info["address"])
        table.add_row("Staked Amount", f"{info['staked_amount']:,} tokens")
        table.add_row("Pending Rewards", f"{info['pending_rewards']:,} tokens")
        
        validator_info = info.get("validator_info")
        if validator_info:
            table.add_row("Public Key", validator_info.get("public_key", "N/A")[:32] + "...")
            table.add_row("Active", "Yes" if validator_info.get("active") else "No")
        else:
            table.add_row("Validator Status", "Not Registered")
        
        console.print()
        console.print(table)
        console.print()
        
        # Show current balance
        balance = service.state.get_balance(validator_address)
        console.print(f"  Account Balance: [green]{balance:,}[/green] tokens")
        
    except ValueError as e:
        console.print(f"[bold red]Error:[/bold red] {e}")
    except Exception as e:
        console.print(f"[bold red]Unexpected error:[/bold red] {e}")
        logger.exception("Staking info command failed")

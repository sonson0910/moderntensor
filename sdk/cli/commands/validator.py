"""
Validator commands for ModernTensor CLI

Commands for validator operations.
"""

import click
from typing import Optional
import json

from sdk.cli.utils import (
    print_warning, print_error, print_success, print_info,
    confirm_action, console, create_table, format_balance,
    format_address
)
from sdk.cli.config import get_network_config

# Constants
DEFAULT_GAS_PRICE = 1_000_000_000
WEIGHT_SCALE_FACTOR = 1_000_000  # Scale factor for converting float weights (0.0-1.0) to u32
U32_MAX = 4_294_967_295  # Maximum value for u32 type


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
        from sdk.client import LuxtensorClient
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
            except Exception:
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
        from sdk.client import LuxtensorClient
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
        except Exception:
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
        # Validate that all entries have required fields
        try:
            # Convert float weights (0.0-1.0) to u32 by scaling with WEIGHT_SCALE_FACTOR
            # Use round() for accurate representation and clamp to valid u32 range
            neuron_uids = []
            weight_values = []
            for w in weights_list:
                neuron_uids.append(w['uid'])
                scaled_weight = round(w['weight'] * WEIGHT_SCALE_FACTOR)
                # Clamp to valid u32 range (0 to U32_MAX)
                clamped_weight = max(0, min(scaled_weight, U32_MAX))
                weight_values.append(clamped_weight)
        except KeyError as e:
            print_error(f"Invalid weights file format: missing required field {e}")
            return
        except (TypeError, ValueError) as e:
            print_error(f"Invalid weight value: {e}")
            return

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

        print_success("Weights set successfully!")
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


@validator.command('commit-weights')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--weights-file', required=True, type=click.Path(exists=True), help='Weights JSON file')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def commit_weights(ctx, coldkey: str, hotkey: str, subnet_uid: int, weights_file: str,
                   base_dir: Optional[str], network: str, yes: bool):
    """
    Commit weights hash (commit-reveal mechanism)

    Commits a hash of weights for later reveal. This prevents weight manipulation
    by requiring validators to commit before knowing other validators' weights.

    Flow:
    1. mtcli validator commit-weights ... (during commit window)
    2. Wait for reveal window (~100 blocks)
    3. mtcli validator reveal-weights ... (reveals the actual weights)

    Examples:
        mtcli validator commit-weights --coldkey my_coldkey --hotkey validator_hk --subnet-uid 1 --weights-file weights.json

    Note:
        - SAVE THE SALT! It will be printed and needed for reveal
        - Once committed, weights cannot be changed
        - Must reveal before reveal window closes
    """
    try:
        from sdk.commit_reveal import compute_commit_hash, generate_salt
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

        # Convert to tuples
        weights = [(w['uid'], int(w['weight'] * WEIGHT_SCALE_FACTOR)) for w in weights_list]

        # Generate salt
        salt = generate_salt()
        salt_hex = salt.hex()

        # Compute commit hash
        commit_hash = compute_commit_hash(weights, salt)

        print_info(f"Commit hash: {commit_hash}")

        # Get network config
        network_config = get_network_config(network)

        # Get wallet
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']

        # Display summary
        console.print("\n[bold yellow]Commit Weights Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Subnet UID:[/bold]", str(subnet_uid))
        table.add_row("[bold]Validator:[/bold]", f"{coldkey}/{hotkey}")
        table.add_row("[bold]Number of Weights:[/bold]", str(len(weights)))
        table.add_row("[bold]Commit Hash:[/bold]", commit_hash[:20] + "...")
        table.add_row("", "")
        table.add_row("[bold red]🔐 SALT (SAVE THIS!):[/bold red]", salt_hex)
        console.print(table)

        console.print("\n[bold red]⚠️  IMPORTANT: Save the salt above! You will need it for reveal![/bold red]")
        console.print()

        # Confirm
        if not yes and not confirm_action("Submit commit transaction?"):
            print_warning("Transaction cancelled")
            return

        # Submit
        from sdk.client import LuxtensorClient
        from sdk.luxtensor_pallets import encode_commit_weights

        client = LuxtensorClient(network_config.rpc_url)
        signer = TransactionSigner(private_key)

        # Encode and sign
        encoded = encode_commit_weights(subnet_uid, commit_hash)
        nonce = client.get_nonce(from_address)

        signed_tx = signer.build_and_sign_transaction(
            to=encoded.contract_address or from_address,
            value=0,
            nonce=nonce,
            gas_price=DEFAULT_GAS_PRICE,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=network_config.chain_id
        )

        result = client.submit_transaction(signed_tx)

        print_success("Weights committed successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info("")
        print_info("To reveal weights, run:")
        print_info(f"  mtcli validator reveal-weights --coldkey {coldkey} --hotkey {hotkey} \\")
        print_info(f"    --subnet-uid {subnet_uid} --weights-file {weights_file} --salt {salt_hex}")

    except Exception as e:
        print_error(f"Failed to commit weights: {str(e)}")


@validator.command('reveal-weights')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--weights-file', required=True, type=click.Path(exists=True), help='Weights JSON file')
@click.option('--salt', required=True, help='Salt from commit (64 hex chars)')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def reveal_weights(ctx, coldkey: str, hotkey: str, subnet_uid: int, weights_file: str,
                   salt: str, base_dir: Optional[str], network: str, yes: bool):
    """
    Reveal weights (commit-reveal mechanism)

    Reveals the actual weights after a previous commit. Must use the same
    weights file and salt from the commit phase.

    Examples:
        mtcli validator reveal-weights --coldkey my_coldkey --hotkey validator_hk \\
          --subnet-uid 1 --weights-file weights.json --salt abc123...

    Note:
        - Must be called during reveal window (after commit window closes)
        - Weights and salt must match the original commit hash
        - If hash mismatch, transaction will fail
    """
    try:
        from sdk.commit_reveal import compute_commit_hash
        from sdk.keymanager.transaction_signer import TransactionSigner
        from sdk.cli.wallet_utils import derive_hotkey_from_coldkey
        from sdk.client import LuxtensorClient
        from sdk.luxtensor_pallets import encode_reveal_weights
        from rich.table import Table

        # Load weights file
        print_info(f"Loading weights from {weights_file}")
        with open(weights_file, 'r') as f:
            weights_data = json.load(f)

        weights_list = weights_data.get('weights', [])
        if not weights_list:
            print_error("No weights found in file")
            return

        # Convert to tuples
        weights = [(w['uid'], int(w['weight'] * WEIGHT_SCALE_FACTOR)) for w in weights_list]

        # Parse salt
        salt_bytes = bytes.fromhex(salt.replace('0x', ''))
        if len(salt_bytes) != 32:
            print_error(f"Invalid salt length: {len(salt_bytes)}, expected 32 bytes (64 hex chars)")
            return

        # Verify hash locally
        commit_hash = compute_commit_hash(weights, salt_bytes)
        print_info(f"Computed commit hash: {commit_hash}")

        # Get network config
        network_config = get_network_config(network)

        # Get wallet
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        from_address = hotkey_data['address']
        private_key = hotkey_data['private_key']

        # Display summary
        console.print("\n[bold yellow]Reveal Weights Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Subnet UID:[/bold]", str(subnet_uid))
        table.add_row("[bold]Validator:[/bold]", f"{coldkey}/{hotkey}")
        table.add_row("[bold]Number of Weights:[/bold]", str(len(weights)))
        table.add_row("[bold]Commit Hash:[/bold]", commit_hash[:20] + "...")
        console.print(table)
        console.print()

        # Confirm
        if not yes and not confirm_action("Submit reveal transaction?"):
            print_warning("Transaction cancelled")
            return

        # Submit
        client = LuxtensorClient(network_config.rpc_url)
        signer = TransactionSigner(private_key)

        # Encode and sign
        encoded = encode_reveal_weights(subnet_uid, weights, salt)
        nonce = client.get_nonce(from_address)

        signed_tx = signer.build_and_sign_transaction(
            to=encoded.contract_address or from_address,
            value=0,
            nonce=nonce,
            gas_price=DEFAULT_GAS_PRICE,
            gas_limit=encoded.gas_estimate,
            data=encoded.data,
            chain_id=network_config.chain_id
        )

        result = client.submit_transaction(signed_tx)

        print_success("Weights revealed successfully!")
        print_info(f"Transaction hash: {result.tx_hash}")
        print_info("Weights will be applied after finalization")

    except Exception as e:
        print_error(f"Failed to reveal weights: {str(e)}")


@validator.command('commit-status')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def commit_status(ctx, subnet_uid: int, network: str):
    """
    Check commit-reveal epoch status

    Shows current phase (committing/revealing/finalizing) and pending commits.

    Examples:
        mtcli validator commit-status --subnet-uid 1
    """
    try:
        from sdk.commit_reveal import CommitRevealClient

        network_config = get_network_config(network)
        client = CommitRevealClient(network_config.rpc_url)

        state = client.get_epoch_state(subnet_uid)

        if not state:
            print_warning(f"No active commit-reveal epoch for subnet {subnet_uid}")
            return

        console.print(f"\n[bold cyan]Commit-Reveal Status for Subnet {subnet_uid}[/bold cyan]\n")

        table = create_table()
        table.add_column("Field", style="bold")
        table.add_column("Value")

        phase_colors = {
            "committing": "green",
            "revealing": "yellow",
            "finalizing": "red",
            "finalized": "cyan",
        }
        phase_color = phase_colors.get(state.phase.value, "white")

        table.add_row("Epoch Number", str(state.epoch_number))
        table.add_row("Phase", f"[{phase_color}]{state.phase.value.upper()}[/{phase_color}]")
        table.add_row("Commit Start Block", str(state.commit_start_block))
        table.add_row("Reveal Start Block", str(state.reveal_start_block))
        table.add_row("Finalize Block", str(state.finalize_block))
        table.add_row("", "")
        table.add_row("Total Commits", str(len(state.commits)))
        table.add_row("Revealed", str(sum(1 for c in state.commits if c.revealed)))

        console.print(table)

        if state.commits:
            console.print("\n[bold]Pending Commits:[/bold]")
            commits_table = create_table()
            commits_table.add_column("Validator", style="yellow")
            commits_table.add_column("Committed At")
            commits_table.add_column("Revealed")

            console.print(commits_table)

    except Exception as e:
        print_error(f"Failed to get commit status: {str(e)}")


# =============================================================================
# Weight Consensus Commands (Multi-Validator)
# =============================================================================

@validator.command('propose-weights')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--weights-file', required=True, type=click.Path(exists=True), help='Weights JSON file')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def propose_weights(ctx, coldkey: str, hotkey: str, subnet_uid: int, weights_file: str,
                    base_dir: Optional[str], network: str, yes: bool):
    """
    Propose weights for multi-validator consensus

    Creates a weight proposal that other validators must vote on.
    Weights are only applied if enough validators approve (2/3 majority).

    Examples:
        mtcli validator propose-weights --coldkey my_coldkey --hotkey validator_hk \\
          --subnet-uid 1 --weights-file weights.json

    Note:
        - Other validators must vote on your proposal
        - Proposal expires after ~200 blocks if not enough votes
        - Use 'validator proposals' to check status
    """
    try:
        from sdk.weight_consensus import WeightConsensusClient
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

        # Convert to tuples
        weights = [(w['uid'], int(w['weight'] * WEIGHT_SCALE_FACTOR)) for w in weights_list]

        # Get network config
        network_config = get_network_config(network)

        # Get wallet
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        _ = hotkey_data['address']  # Validate address exists
        private_key = hotkey_data['private_key']

        # Display summary
        console.print("\n[bold yellow]Propose Weights Summary:[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Subnet UID:[/bold]", str(subnet_uid))
        table.add_row("[bold]Proposer:[/bold]", f"{coldkey}/{hotkey}")
        table.add_row("[bold]Number of Weights:[/bold]", str(len(weights)))
        console.print(table)

        console.print("\n[bold cyan]ℹ️  Other validators must vote to approve this proposal[/bold cyan]")
        console.print()

        # Confirm
        if not yes and not confirm_action("Submit proposal?"):
            print_warning("Proposal cancelled")
            return

        # Submit
        client = WeightConsensusClient(network_config.rpc_url)
        signer = TransactionSigner(private_key)

        proposal_id = client.propose_weights(subnet_uid, weights, signer)

        print_success("Weights proposed successfully!")
        print_info(f"Proposal ID: {proposal_id}")
        print_info("")
        print_info("Other validators can vote with:")
        print_info(f"  mtcli validator vote-proposal --proposal-id {proposal_id} --approve")

    except Exception as e:
        print_error(f"Failed to propose weights: {str(e)}")


@validator.command('vote-proposal')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--proposal-id', required=True, help='Proposal ID to vote on')
@click.option('--approve/--reject', default=True, help='Approve or reject the proposal')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.option('--yes', is_flag=True, help='Skip confirmation')
@click.pass_context
def vote_proposal(ctx, coldkey: str, hotkey: str, proposal_id: str, approve: bool,
                  base_dir: Optional[str], network: str, yes: bool):
    """
    Vote on a weight proposal

    As a validator, vote to approve or reject another validator's weight proposal.

    Examples:
        mtcli validator vote-proposal --coldkey my_coldkey --hotkey validator_hk \\
          --proposal-id 0x123... --approve

        mtcli validator vote-proposal --coldkey my_coldkey --hotkey validator_hk \\
          --proposal-id 0x123... --reject

    Note:
        - Cannot vote on your own proposal
        - Can only vote once per proposal
        - Proposal finalizes when 2/3 majority is reached
    """
    try:
        from sdk.weight_consensus import WeightConsensusClient
        from sdk.keymanager.transaction_signer import TransactionSigner
        from sdk.cli.wallet_utils import derive_hotkey_from_coldkey
        from rich.table import Table

        # Get network config
        network_config = get_network_config(network)

        # Get wallet
        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        private_key = hotkey_data['private_key']

        # Get proposal info
        client = WeightConsensusClient(network_config.rpc_url)
        proposal = client.get_proposal(proposal_id)

        if not proposal:
            print_error(f"Proposal {proposal_id} not found")
            return

        # Display summary
        vote_text = "APPROVE ✅" if approve else "REJECT ❌"
        console.print(f"\n[bold yellow]Vote {vote_text}[/bold yellow]")
        table = Table(show_header=False, box=None)
        table.add_row("[bold]Proposal ID:[/bold]", proposal_id[:20] + "...")
        table.add_row("[bold]Proposer:[/bold]", format_address(proposal.proposer))
        table.add_row("[bold]Subnet UID:[/bold]", str(proposal.subnet_uid))
        table.add_row("[bold]Current Votes:[/bold]", f"{proposal.approval_count()}/{len(proposal.votes)} approve")
        console.print(table)
        console.print()

        # Confirm
        if not yes and not confirm_action(f"Vote {vote_text} on this proposal?"):
            print_warning("Vote cancelled")
            return

        # Vote
        signer = TransactionSigner(private_key)
        result = client.vote_on_proposal(proposal_id, approve, signer)

        print_success(f"Voted {vote_text} successfully!")

        if result.reached:
            print_success("🎉 Consensus reached! Proposal is now approved.")
            print_info("Finalize with: mtcli validator finalize-proposal --proposal-id " + proposal_id)
        else:
            print_info(f"Approval: {result.approval_count}/{result.total_votes} ({result.approval_percentage}%)")
            print_info(f"Threshold: {result.threshold}%")

    except Exception as e:
        print_error(f"Failed to vote: {str(e)}")


@validator.command('finalize-proposal')
@click.option('--coldkey', required=True, help='Coldkey name')
@click.option('--hotkey', required=True, help='Hotkey name')
@click.option('--proposal-id', required=True, help='Proposal ID to finalize')
@click.option('--base-dir', type=click.Path(), default=None)
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def finalize_proposal(ctx, coldkey: str, hotkey: str, proposal_id: str,
                      base_dir: Optional[str], network: str):
    """
    Finalize an approved proposal

    After a proposal reaches consensus (2/3 majority), anyone can finalize it
    to apply the weights on-chain.

    Examples:
        mtcli validator finalize-proposal --coldkey my_coldkey --hotkey validator_hk \\
          --proposal-id 0x123...
    """
    try:
        from sdk.weight_consensus import WeightConsensusClient, ProposalStatus
        from sdk.keymanager.transaction_signer import TransactionSigner
        from sdk.cli.wallet_utils import derive_hotkey_from_coldkey

        network_config = get_network_config(network)

        hotkey_data = derive_hotkey_from_coldkey(coldkey, hotkey, base_dir)
        private_key = hotkey_data['private_key']

        client = WeightConsensusClient(network_config.rpc_url)

        # Check proposal status
        proposal = client.get_proposal(proposal_id)
        if not proposal:
            print_error(f"Proposal {proposal_id} not found")
            return

        if proposal.status != ProposalStatus.APPROVED:
            print_error(f"Proposal not approved. Status: {proposal.status.value}")
            return

        # Finalize
        signer = TransactionSigner(private_key)
        weights = client.finalize_proposal(proposal_id, signer)

        print_success("Proposal finalized! Weights applied on-chain.")
        print_info(f"Applied {len(weights)} weight entries")

    except Exception as e:
        print_error(f"Failed to finalize: {str(e)}")


@validator.command('proposals')
@click.option('--subnet-uid', required=True, type=int, help='Subnet UID')
@click.option('--network', default='testnet', help='Network name')
@click.pass_context
def list_proposals(ctx, subnet_uid: int, network: str):
    """
    List pending weight proposals

    Shows all active proposals waiting for votes.

    Examples:
        mtcli validator proposals --subnet-uid 1
    """
    try:
        from sdk.weight_consensus import WeightConsensusClient

        network_config = get_network_config(network)
        client = WeightConsensusClient(network_config.rpc_url)

        proposals = client.get_pending_proposals(subnet_uid)

        if not proposals:
            print_info(f"No pending proposals for subnet {subnet_uid}")
            return

        console.print(f"\n[bold cyan]Pending Proposals for Subnet {subnet_uid}[/bold cyan]\n")

        for p in proposals:
            status_icon = "⏳"
            if p.approval_percentage() >= 67:
                status_icon = "✅"

            console.print(f"{status_icon} [bold]Proposal:[/bold] {p.id[:20]}...")
            table = create_table()
            table.add_column("Field")
            table.add_column("Value")

            table.add_row("Proposer", format_address(p.proposer))
            table.add_row("Weights Count", str(len(p.weights)))
            table.add_row("Proposed At", str(p.proposed_at))
            table.add_row("Expires At", str(p.expires_at))
            table.add_row("Votes", f"{p.approval_count()}/{len(p.votes)} approve ({p.approval_percentage()}%)")

            console.print(table)
            console.print()

        print_info(f"Total: {len(proposals)} pending proposals")

    except Exception as e:
        print_error(f"Failed to list proposals: {str(e)}")



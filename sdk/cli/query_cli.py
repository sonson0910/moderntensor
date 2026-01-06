# sdk/cli/query_cli.py
import click
from rich.console import Console
from rich.table import Table
from sdk.compat.pycardano import Address, Network, UTxO, PlutusData, RawPlutusData
from pycardano.serialization import RawCBOR
from typing import Optional, Any
import asyncio
from collections import defaultdict

from sdk.config.settings import settings, logger
from sdk.service.context import get_chain_context
from sdk.service.query_service import get_address_info
from sdk.service.stake_service import Wallet  # For balance/utxo query
from sdk.service.utxos import get_utxo_from_str, get_utxo_with_lowest_performance
from sdk.metagraph.metagraph_datum import (
    MinerDatum,
    SubnetStaticDatum,
    SubnetDynamicDatum,
    DATUM_INT_DIVISOR,
)  # Assuming this is the default datum for contract queries
# REMOVED: Cardano smartcontract imports
# from sdk.smartcontract.validator import (
#     read_validator_static_subnet,
#     read_validator_dynamic_subnet,
# )
from rich.panel import Panel


# ------------------------------------------------------------------------------
# QUERY COMMAND GROUP
# ------------------------------------------------------------------------------
@click.group()
def query_cli():
    """
    🔍 Commands for querying blockchain information. 🔍
    """
    pass


# ------------------------------------------------------------------------------
# QUERY ADDRESS INFO
# ------------------------------------------------------------------------------
@query_cli.command("address")
@click.argument("address_str", type=str)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
def address_cmd(address_str, network):
    """
    ℹ️  Show detailed balance (ADA, Tokens) and UTxO count for an ADDRESS.
    """
    console = Console()
    net = Network.TESTNET if network == "testnet" else Network.MAINNET

    console.print(
        f"🔍 Querying information for address: [cyan]{address_str}[/cyan] on [yellow]{network.upper()}[/yellow]..."
    )

    try:
        context = get_chain_context()
        if not context:
            console.print(
                "[bold red]Error:[/bold red] Could not initialize Cardano chain context."
            )
            return

        info = get_address_info(address_str, context)

        table = Table(
            title=f"Address Information: {address_str}",
            show_header=True,
            header_style="bold magenta",
        )
        table.add_column("Item", style="dim")
        table.add_column("Value")

        table.add_row("Address", info["address"])
        table.add_row("ADA (Lovelace)", f"{info['lovelace']:,}")
        table.add_row("ADA", f"{info['lovelace'] / 1_000_000:.6f}")
        table.add_row("UTxO Count", str(info["utxo_count"]))

        console.print(table)

        if info["tokens"]:
            token_table = Table(
                title="Native Tokens", show_header=True, header_style="bold blue"
            )
            token_table.add_column("Policy ID")
            token_table.add_column("Asset Name (Hex)")
            token_table.add_column("Asset Name (Decoded)")
            token_table.add_column("Amount")

            for (policy, asset_name), amount in info["tokens"].items():
                try:
                    an_hex = (
                        asset_name.payload.hex()
                        if hasattr(asset_name, "payload")
                        else asset_name.hex()
                    )
                    decoded_name = (
                        asset_name.payload.decode("utf-8")
                        if hasattr(asset_name, "payload")
                        else bytes.fromhex(asset_name.hex()).decode(
                            "utf-8", errors="replace"
                        )
                    )
                except Exception:
                    an_hex = (
                        asset_name.hex()
                        if hasattr(asset_name, "hex")
                        else str(asset_name)
                    )
                    decoded_name = "[dim](Cannot decode)[/dim]"

                token_table.add_row(str(policy), an_hex, decoded_name, f"{amount:,}")
            console.print(token_table)
        else:
            console.print("  No native tokens found at this address.")

    except Exception as e:
        console.print(f":cross_mark: [bold red]Error querying address:[/bold red] {e}")
        logger.exception("Address query failed")


# ------------------------------------------------------------------------------
# QUERY HOTKEY BALANCE
# ------------------------------------------------------------------------------
@query_cli.command("balance")
@click.option("--coldkey", required=True, help="Coldkey name.")
@click.option("--hotkey", required=True, help="Hotkey name.")
@click.option(
    "--password",
    prompt=True,
    hide_input=True,
    help="Coldkey password.",
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
def balance_cmd(coldkey, hotkey, password, network):
    """
    💰 Show detailed balance (ADA, Tokens) for a specific Hotkey.
    """
    console = Console()
    net = Network.TESTNET if network == "testnet" else Network.MAINNET

    console.print(
        f"💰 Querying balance for: [magenta]{coldkey}[/magenta] / [cyan]{hotkey}[/cyan] on [yellow]{network.upper()}[/yellow]..."
    )
    try:
        wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
        console.print(f"  Address: [blue]{wallet.main_address}[/blue]")
        wallet.get_balances()

    except Exception as e:
        console.print(f":cross_mark: [bold red]Error querying balance:[/bold red] {e}")
        logger.exception("Hotkey balance query failed")


# ------------------------------------------------------------------------------
# QUERY HOTKEY UTXOS
# ------------------------------------------------------------------------------
@query_cli.command("utxos")
@click.option("--coldkey", required=True, help="Coldkey name.")
@click.option("--hotkey", required=True, help="Hotkey name.")
@click.option(
    "--password",
    prompt=True,
    hide_input=True,
    help="Coldkey password.",
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
def utxos_cmd(coldkey, hotkey, password, network):
    """
    📄 List UTxOs held by a specific Hotkey address.
    """
    console = Console()
    net = Network.TESTNET if network == "testnet" else Network.MAINNET

    console.print(
        f"📄 Querying UTxOs for: [magenta]{coldkey}[/magenta] / [cyan]{hotkey}[/cyan] on [yellow]{network.upper()}[/yellow]..."
    )

    try:
        wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
        console.print(f"  Address: [blue]{wallet.main_address}[/blue]")
        bf_utxos = wallet.get_utxos()

        if not bf_utxos:
            console.print("  No UTxOs found at this address.")
            return

        table = Table(
            title=f"UTxOs at {wallet.main_address}",
            show_header=True,
            header_style="bold green",
        )
        table.add_column("Tx Hash")
        table.add_column("Index", style="dim")
        table.add_column("ADA")
        table.add_column("Other Tokens")

        for item in bf_utxos:
            utxo_data: Any = item
            try:
                tx_hash = getattr(utxo_data, "tx_hash", utxo_data["tx_hash"])
                tx_index = getattr(utxo_data, "output_index", utxo_data["output_index"])
                amount_list = getattr(utxo_data, "amount", utxo_data["amount"])
            except (AttributeError, KeyError, TypeError) as access_err:
                logger.warning(
                    f"Could not access expected UTxO attributes: {access_err} for data: {utxo_data}"
                )
                continue

            ada_amt = 0
            tokens_str_list = []
            if isinstance(amount_list, list):
                for token_item in amount_list:
                    token_data: Any = token_item
                    try:
                        unit = getattr(token_data, "unit", token_data["unit"])
                        quantity = getattr(
                            token_data, "quantity", token_data["quantity"]
                        )
                        if unit == "lovelace":
                            ada_amt = int(quantity)
                        else:
                            policy_id = unit[:56] if len(unit) > 56 else unit
                            asset_hex = unit[56:] if len(unit) > 56 else ""
                            try:
                                asset_name = bytes.fromhex(asset_hex).decode(
                                    "utf-8", errors="replace"
                                )
                            except ValueError:
                                asset_name = f"(hex: {asset_hex})"
                            tokens_str_list.append(
                                f"{quantity} {policy_id[:6]}...{asset_name}"
                            )
                    except (AttributeError, KeyError, TypeError) as token_err:
                        logger.warning(
                            f"Could not process token data: {token_err} for token: {token_data}"
                        )
                        tokens_str_list.append("[Error processing token]")
            else:
                logger.warning(
                    f"Unexpected amount format for UTxO {tx_hash}#{tx_index}: {amount_list}"
                )

            tokens_display = ", ".join(tokens_str_list) if tokens_str_list else "-"
            table.add_row(
                str(tx_hash),
                str(tx_index),
                f"{ada_amt / 1_000_000:.6f}",
                tokens_display,
            )
        console.print(table)

    except Exception as e:
        console.print(f":cross_mark: [bold red]Error querying UTxOs:[/bold red] {e}")
        logger.exception("Hotkey UTxO query failed")


# ------------------------------------------------------------------------------
# QUERY CONTRACT UTXO BY UID (Advanced/Debug)
# ------------------------------------------------------------------------------
@query_cli.command("contract-utxo")
@click.option("--contract-address", required=True, help="Smart contract address.")
@click.option("--uid", required=True, help="UID (hex string) to search for in datum.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if str(settings.CARDANO_NETWORK).lower() == "mainnet" else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
def contract_utxo_cmd(contract_address, uid, network):
    """
    🔧 Find a contract UTxO by UID in its datum (assumes MinerDatum).
    """
    console = Console()
    net = Network.TESTNET if network == "testnet" else Network.MAINNET

    console.print(
        f"🔧 Searching for UTxO with UID [yellow]{uid}[/yellow] at contract [cyan]{contract_address}[/cyan] on [yellow]{network.upper()}[/yellow]..."
    )
    console.print("[dim](Assuming datum type: MinerDatum)[/dim]")

    try:
        context = get_chain_context()
        if not context:
            console.print(
                "[bold red]Error:[/bold red] Could not initialize Cardano chain context."
            )
            return

        address_obj = Address.from_primitive(contract_address)
        uid_bytes = bytes.fromhex(uid)

        found_utxo: Optional[UTxO] = get_utxo_from_str(
            contract_address=address_obj,
            datumclass=MinerDatum,
            context=context,
            search_uid=uid_bytes,
        )

        if found_utxo:
            console.print(f"✅ Found UTxO:")
            console.print(f"  Input: [green]{found_utxo.input}[/green]")
            console.print(f"  Output Address: {found_utxo.output.address}")
            console.print(f"  Amount: {found_utxo.output.amount}")

            output = found_utxo.output
            datum = output.datum
            datum_hash = output.datum_hash
            script = output.script

            if isinstance(datum, PlutusData):
                console.print(f"  Decoded Datum: {datum}")
                try:
                    datum_cbor_bytes = datum.to_cbor()
                    console.print(f"  Datum CBOR: {datum_cbor_bytes.hex()}")
                except Exception as cbor_err:
                    logger.warning(f"Could not get CBOR from decoded datum: {cbor_err}")
            elif datum_hash:
                console.print(f"  Datum Hash: {bytes(datum_hash).hex()}")
            if script:
                console.print(f"  Script: {script}")
        else:
            console.print(
                f"❌ No UTxO found with UID {uid} at the specified contract address."
            )

    except Exception as e:
        console.print(
            f":cross_mark: [bold red]Error querying contract UTxO:[/bold red] {e}"
        )
        logger.exception("Contract UTxO query failed")


# ------------------------------------------------------------------------------
# QUERY LOWEST PERFORMANCE UTXO (Advanced/Debug)
# ------------------------------------------------------------------------------
@query_cli.command("lowest-performance")
@click.option("--contract-address", required=True, help="Smart contract address.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if str(settings.CARDANO_NETWORK).lower() == "mainnet" else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
def lowest_perf_cmd(contract_address, network):
    """
    📉 Find the contract UTxO with the lowest scaled_last_performance (assumes MinerDatum).
    """
    console = Console()
    net = Network.TESTNET if network == "testnet" else Network.MAINNET

    console.print(
        f"📉 Searching for lowest performance UTxO at contract [cyan]{contract_address}[/cyan] on [yellow]{network.upper()}[/yellow]..."
    )
    console.print("[dim](Assuming datum type: MinerDatum)[/dim]")

    try:
        context = get_chain_context()
        if not context:
            console.print(
                "[bold red]Error:[/bold red] Could not initialize Cardano chain context."
            )
            return

        address_obj = Address.from_primitive(contract_address)

        lowest_utxo: Optional[UTxO] = get_utxo_with_lowest_performance(
            contract_address=address_obj,
            datumclass=MinerDatum,
            context=context,
        )

        if lowest_utxo:
            console.print(f"✅ Found UTxO with lowest performance:")
            console.print(f"  Input: [green]{lowest_utxo.input}[/green]")
            console.print(f"  Output Address: {lowest_utxo.output.address}")
            console.print(f"  Amount: {lowest_utxo.output.amount}")

            output = lowest_utxo.output
            datum = output.datum
            datum_hash = output.datum_hash
            script = output.script

            if isinstance(datum, PlutusData):
                console.print(f"  Decoded Datum: {datum}")
                if hasattr(datum, "scaled_last_performance"):
                    perf = getattr(datum, "scaled_last_performance")
                    console.print(f"  Scaled Performance: [yellow]{perf}[/yellow]")
                try:
                    datum_cbor_bytes = datum.to_cbor()
                    console.print(f"  Datum CBOR: {datum_cbor_bytes.hex()}")
                except Exception as cbor_err:
                    logger.warning(f"Could not get CBOR from decoded datum: {cbor_err}")
            elif datum_hash:
                console.print(f"  Datum Hash: {bytes(datum_hash).hex()}")
            if script:
                console.print(f"  Script: {script}")
        else:
            console.print(
                f"❌ No suitable UTXO found with MinerDatum at the specified contract address."
            )

    except Exception as e:
        console.print(
            f":cross_mark: [bold red]Error querying lowest performance UTxO:[/bold red] {e}"
        )
        logger.exception("Lowest performance UTxO query failed")


# ------------------------------------------------------------------------------
# QUERY SUBNET
# ------------------------------------------------------------------------------
@query_cli.command("subnet")
@click.option(
    "--subnet-uid",
    type=int,
    required=True,
    help="The numeric UID of the subnet to query.",
)
@click.pass_context
def query_subnet(ctx, subnet_uid):
    """Query and display the Static and Dynamic Datum for a specific Subnet UID."""
    console = Console()
    # Import and use the global logger from settings
    from sdk.config.settings import logger

    # --- Helper display functions (nested inside command) ---
    def _display_static_datum(datum: SubnetStaticDatum):
        title = (
            f"📊 [bold magenta]Subnet Static Data (UID: {datum.net_uid})[/bold magenta]"
        )
        table = Table(
            title=title,
            show_header=True,
            header_style="bold magenta",
            border_style="dim blue",
        )
        table.add_column("Field", style="dim cyan", width=25)
        table.add_column("Value", style="bright_white")
        try:
            table.add_row(
                "Name",
                datum.name.decode("utf-8", errors="replace") if datum.name else "N/A",
            )
        except AttributeError:
            table.add_row("Name", "[red]Error decoding[/red]")
        try:
            table.add_row(
                "Owner Address Hash",
                datum.owner_addr_hash.hex() if datum.owner_addr_hash else "N/A",
            )
        except AttributeError:
            table.add_row("Owner Address Hash", "[red]Invalid[/red]")
        table.add_row("Max Miners", str(datum.max_miners))
        table.add_row("Max Validators", str(datum.max_validators))
        table.add_row("Immunity Period (slots)", str(datum.immunity_period_slots))
        table.add_row("Creation Slot", str(datum.creation_slot))
        table.add_row(
            "Min Stake Miner (ADA)", f"{datum.min_stake_miner / 1_000_000:.2f}"
        )
        table.add_row(
            "Min Stake Validator (ADA)", f"{datum.min_stake_validator / 1_000_000:.2f}"
        )
        try:
            table.add_row(
                "Description",
                (
                    datum.description.decode("utf-8", errors="replace")
                    if datum.description
                    else "N/A"
                ),
            )
        except AttributeError:
            table.add_row("Description", "[red]Error decoding[/red]")
        table.add_row("Version", str(datum.version))
        console.print(table)

    def _display_dynamic_datum(datum: SubnetDynamicDatum):
        title = f"⚡ [bold cyan]Subnet Dynamic Data (UID: {datum.net_uid})[/bold cyan]"
        table = Table(
            title=title,
            show_header=True,
            header_style="bold cyan",
            border_style="dim blue",
        )
        table.add_column("Field", style="dim cyan", width=25)
        table.add_column("Value", style="bright_white")
        table.add_row(
            "Scaled Weight",
            f"{datum.scaled_weight} [dim](~{datum.scaled_weight / DATUM_INT_DIVISOR:.4f})[/dim]",
        )
        table.add_row(
            "Scaled Performance",
            f"{datum.scaled_performance} [dim](~{datum.scaled_performance / DATUM_INT_DIVISOR:.4f})[/dim]",
        )
        table.add_row("Current Epoch", str(datum.current_epoch))
        table.add_row(
            "Registration Open",
            "[green]✅ Yes[/green]" if datum.registration_open else "[red]❌ No[/red]",
        )
        table.add_row(
            "Registration Cost (ADA)",
            f"[yellow]{datum.reg_cost / 1_000_000:.2f}[/yellow]",
        )
        table.add_row(
            "Scaled Incentive Ratio",
            f"{datum.scaled_incentive_ratio} [dim](~{datum.scaled_incentive_ratio / DATUM_INT_DIVISOR:.4f})[/dim]",
        )
        table.add_row("Last Update Slot", str(datum.last_update_slot))
        table.add_row(
            "Total Stake (ADA)", f"[yellow]{datum.total_stake / 1_000_000:.2f}[/yellow]"
        )
        table.add_row("Validator Count", str(datum.validator_count))
        table.add_row("Miner Count", str(datum.miner_count))
        console.print(table)

    # --- Async query logic (nested inside command) ---
    async def _query_subnet_async(subnet_uid_to_query: int):
        console.print(
            f"[cyan]🔍 Attempting to query state for Subnet UID:[/cyan] [bold yellow]{subnet_uid_to_query}[/bold yellow]"
        )
        found_static = False
        found_dynamic = False

        try:
            context = get_chain_context()
            network = context.network

            # Load script addresses
            static_info = None
            dynamic_info = None
            try:
                static_info = read_validator_static_subnet()
                dynamic_info = read_validator_dynamic_subnet()
                if not static_info or not dynamic_info:
                    logger.error(
                        "Failed to load one or both subnet script definitions."
                    )
                    console.print(
                        "[bold red]Error:[/bold red] Could not load subnet script definitions. Check configuration."
                    )
                    return

                static_addr = Address(static_info["script_hash"], network=network)  # type: ignore[index]
                dynamic_addr = Address(dynamic_info["script_hash"], network=network)  # type: ignore[index]
                logger.info(f"Static script address: {static_addr}")
                logger.info(f"Dynamic script address: {dynamic_addr}")
                console.print(
                    f"  [dim]Static Script Address :[/dim] [blue]{static_addr}[/blue]"
                )
                console.print(
                    f"  [dim]Dynamic Script Address:[/dim] [blue]{dynamic_addr}[/blue]"
                )
            except Exception as script_err:
                logger.error(
                    f"Failed to load subnet script addresses: {script_err}",
                    exc_info=True,
                )
                console.print(
                    f"[bold red]Error:[/bold red] Could not load subnet script addresses. Check configuration."
                )
                return

            # Query Static Datum
            logger.info(f"Querying UTxOs at static address: {static_addr}")
            console.print(f"[cyan]🔎 Querying Static UTxOs at {static_addr}...[/cyan]")
            static_utxos = await asyncio.to_thread(context.utxos, str(static_addr))
            logger.info(f"Found {len(static_utxos)} UTxOs at static address.")
            for utxo in static_utxos:
                datum_field = utxo.output.datum
                datum_cbor: Optional[bytes] = None

                # Simplified check: If datum_field exists and has a .cbor attribute, use it.
                if datum_field is not None and hasattr(datum_field, "cbor"):
                    datum_cbor = datum_field.cbor  # type: ignore[attr-defined]
                elif datum_field is not None:
                    # Log only if datum_field exists but doesn't have .cbor (unexpected)
                    logger.warning(
                        f"Unexpected datum type or structure in static UTxO {utxo.input}: {type(datum_field)}. Lacks .cbor attribute."
                    )

                if datum_cbor:
                    logger.debug(
                        f"Found static UTxO {utxo.input} with datum CBOR: {datum_cbor.hex()}"
                    )
                    try:
                        datum = SubnetStaticDatum.from_cbor(datum_cbor)
                        if isinstance(datum, SubnetStaticDatum) and datum.net_uid == subnet_uid_to_query:  # type: ignore[attr-defined]
                            logger.info(
                                f"Found matching Static Datum in UTxO: {utxo.input}"
                            )
                            console.print(
                                f"✅ [green]Found matching Static Datum in UTxO:[/green] {utxo.input}"
                            )
                            _display_static_datum(datum)  # type: ignore[arg-type]
                            found_static = True
                            break
                    except Exception as e:
                        logger.warning(
                            f"Could not decode Static Datum from UTxO {utxo.input}: {e}",
                            exc_info=False,
                        )

            if not found_static:
                console.print(
                    Panel(
                        f"[yellow]Warning:[/yellow] No valid Static Datum found for Subnet UID {subnet_uid_to_query} at {static_addr}",
                        border_style="yellow",
                    )
                )

            # Query Dynamic Datum
            logger.info(f"Querying UTxOs at dynamic address: {dynamic_addr}")
            console.print(
                f"[cyan]🔎 Querying Dynamic UTxOs at {dynamic_addr}...[/cyan]"
            )
            dynamic_utxos = await asyncio.to_thread(context.utxos, str(dynamic_addr))
            logger.info(f"Found {len(dynamic_utxos)} UTxOs at dynamic address.")
            for utxo in dynamic_utxos:
                datum_field = utxo.output.datum
                datum_cbor: Optional[bytes] = None

                # Simplified check: If datum_field exists and has a .cbor attribute, use it.
                if datum_field is not None and hasattr(datum_field, "cbor"):
                    datum_cbor = datum_field.cbor  # type: ignore[attr-defined]
                elif datum_field is not None:
                    # Log only if datum_field exists but doesn't have .cbor (unexpected)
                    logger.warning(
                        f"Unexpected datum type or structure in dynamic UTxO {utxo.input}: {type(datum_field)}. Lacks .cbor attribute."
                    )

                if datum_cbor:
                    logger.debug(
                        f"Found dynamic UTxO {utxo.input} with datum CBOR: {datum_cbor.hex()}"
                    )
                    try:
                        datum = SubnetDynamicDatum.from_cbor(datum_cbor)
                        if isinstance(datum, SubnetDynamicDatum) and datum.net_uid == subnet_uid_to_query:  # type: ignore[attr-defined]
                            logger.info(
                                f"Found matching Dynamic Datum in UTxO: {utxo.input}"
                            )
                            console.print(
                                f"✅ [green]Found matching Dynamic Datum in UTxO:[/green] {utxo.input}"
                            )
                            _display_dynamic_datum(datum)  # type: ignore[arg-type]
                            found_dynamic = True
                            break
                    except Exception as e:
                        logger.warning(
                            f"Could not decode Dynamic Datum from UTxO {utxo.input}: {e}",
                            exc_info=False,
                        )

            if not found_dynamic:
                console.print(
                    Panel(
                        f"[yellow]Warning:[/yellow] No valid Dynamic Datum found for Subnet UID {subnet_uid_to_query} at {dynamic_addr}",
                        border_style="yellow",
                    )
                )

        except Exception as e:
            logger.error(f"Error during subnet query: {e}", exc_info=True)
            console.print(f"[bold red]Error querying subnet state:[/bold red] {e}")

    # --- Run the async function ---
    try:
        asyncio.run(_query_subnet_async(subnet_uid))
    except Exception as e:
        logger.error(f"Failed to run async query function: {e}", exc_info=True)
        console.print(f"[bold red]An unexpected error occurred:[/bold red] {e}")


# ------------------------------------------------------------------------------
# QUERY LIST SUBNETS
# ------------------------------------------------------------------------------
@query_cli.command("list-subnets")
@click.pass_context
def list_subnets_cmd(ctx):
    """📋 List summary information for all detected subnets."""
    console = Console()
    from sdk.config.settings import logger

    # --- Async query logic ---
    async def _list_subnets_async():
        console.print("[cyan]🔍 Querying all subnet datums...[/cyan]")
        subnet_data = defaultdict(dict)  # Use defaultdict for easier updates

        try:
            context = get_chain_context()
            network = context.network

            # Load script addresses
            static_info = None
            dynamic_info = None
            try:
                static_info = read_validator_static_subnet()
                dynamic_info = read_validator_dynamic_subnet()
                if not static_info or not dynamic_info:
                    logger.error(
                        "Failed to load one or both subnet script definitions."
                    )
                    console.print(
                        "[bold red]Error:[/bold red] Could not load subnet script definitions."
                    )
                    return

                static_addr = Address(static_info["script_hash"], network=network)  # type: ignore[index]
                dynamic_addr = Address(dynamic_info["script_hash"], network=network)  # type: ignore[index]
                logger.info(f"Static script address: {static_addr}")
                logger.info(f"Dynamic script address: {dynamic_addr}")
            except Exception as script_err:
                logger.error(
                    f"Failed to load subnet script addresses: {script_err}",
                    exc_info=True,
                )
                console.print(
                    f"[bold red]Error:[/bold red] Could not load subnet script addresses."
                )
                return

            # --- Query and process Static Datums ---
            console.print(
                f"[cyan]🔎 Querying all Static UTxOs at {static_addr}...[/cyan]"
            )
            static_utxos = await asyncio.to_thread(context.utxos, str(static_addr))
            logger.info(f"Found {len(static_utxos)} potential Static UTxOs.")
            processed_static = 0
            for utxo in static_utxos:
                datum_field = utxo.output.datum
                datum_cbor: Optional[bytes] = None
                if datum_field is not None and hasattr(datum_field, "cbor"):
                    datum_cbor = datum_field.cbor  # type: ignore[attr-defined]

                if datum_cbor:
                    try:
                        datum = SubnetStaticDatum.from_cbor(datum_cbor)
                        if isinstance(datum, SubnetStaticDatum):
                            uid = datum.net_uid
                            subnet_data[uid]["name"] = (
                                datum.name.decode("utf-8", errors="replace")
                                if datum.name
                                else "N/A"
                            )
                            processed_static += 1
                    except Exception as e:
                        logger.warning(
                            f"Could not decode Static Datum from UTxO {utxo.input}: {e}",
                            exc_info=False,
                        )

            logger.info(f"Successfully processed {processed_static} Static Datums.")

            # --- Query and process Dynamic Datums ---
            console.print(
                f"[cyan]🔎 Querying all Dynamic UTxOs at {dynamic_addr}...[/cyan]"
            )
            dynamic_utxos = await asyncio.to_thread(context.utxos, str(dynamic_addr))
            logger.info(f"Found {len(dynamic_utxos)} potential Dynamic UTxOs.")
            processed_dynamic = 0
            for utxo in dynamic_utxos:
                datum_field = utxo.output.datum
                datum_cbor: Optional[bytes] = None
                if datum_field is not None and hasattr(datum_field, "cbor"):
                    datum_cbor = datum_field.cbor  # type: ignore[attr-defined]

                if datum_cbor:
                    try:
                        datum = SubnetDynamicDatum.from_cbor(datum_cbor)
                        if isinstance(datum, SubnetDynamicDatum):
                            uid = datum.net_uid
                            subnet_data[uid]["scaled_weight"] = datum.scaled_weight
                            subnet_data[uid]["reg_cost"] = datum.reg_cost
                            subnet_data[uid][
                                "scaled_incentive_ratio"
                            ] = datum.scaled_incentive_ratio
                            subnet_data[uid][
                                "last_update_slot"
                            ] = datum.last_update_slot
                            subnet_data[uid]["total_stake"] = datum.total_stake
                            processed_dynamic += 1
                    except Exception as e:
                        logger.warning(
                            f"Could not decode Dynamic Datum from UTxO {utxo.input}: {e}",
                            exc_info=False,
                        )

            logger.info(f"Successfully processed {processed_dynamic} Dynamic Datums.")

            # --- Display Results ---
            num_subnets = len(subnet_data)
            console.print(
                f"\n[bold green]📊 Found {num_subnets} subnet(s) in total.[/bold green]"
            )

            if num_subnets > 0:
                table = Table(
                    title="Subnet Overview",
                    show_header=True,
                    header_style="bold blue",
                    border_style="dim green",
                )
                table.add_column("UID", style="dim", justify="right")
                table.add_column("Name", style="cyan")
                table.add_column("Weight (Scaled)", style="magenta", justify="right")
                table.add_column("Reg Cost (ADA)", style="yellow", justify="right")
                table.add_column("Incentive (Scaled)", style="green", justify="right")
                table.add_column("Last Update Slot", style="blue", justify="right")
                table.add_column("Total Stake (ADA)", style="yellow", justify="right")

                # Sort by UID for consistent display
                sorted_uids = sorted(subnet_data.keys())

                for uid in sorted_uids:
                    data = subnet_data[uid]
                    name = data.get("name", "[dim i]N/A[/dim i]")
                    weight = data.get("scaled_weight", None)
                    reg_cost_lovelace = data.get("reg_cost", None)
                    incentive = data.get("scaled_incentive_ratio", None)
                    last_update = data.get("last_update_slot", "[dim i]N/A[/dim i]")
                    stake_lovelace = data.get("total_stake", None)

                    # Format scaled values
                    weight_str = (
                        f"{weight} [dim](~{weight / DATUM_INT_DIVISOR:.3f})[/dim]"
                        if weight is not None
                        else "[dim i]N/A[/dim i]"
                    )
                    incentive_str = (
                        f"{incentive} [dim](~{incentive / DATUM_INT_DIVISOR:.3f})[/dim]"
                        if incentive is not None
                        else "[dim i]N/A[/dim i]"
                    )

                    # Format ADA values
                    reg_cost_ada = (
                        f"{reg_cost_lovelace / 1_000_000:.2f}"
                        if reg_cost_lovelace is not None
                        else "[dim i]N/A[/dim i]"
                    )
                    stake_ada = (
                        f"{stake_lovelace / 1_000_000:.2f}"
                        if stake_lovelace is not None
                        else "[dim i]N/A[/dim i]"
                    )

                    table.add_row(
                        str(uid),
                        name,
                        weight_str,
                        reg_cost_ada,
                        incentive_str,
                        str(last_update),
                        stake_ada,
                    )

                console.print(table)
            else:
                console.print(
                    "[yellow]No subnet information could be retrieved.[/yellow]"
                )

        except Exception as e:
            logger.error(f"Error during subnet listing: {e}", exc_info=True)
            console.print(f"[bold red]Error listing subnets:[/bold red] {e}")

    # --- Run the async function ---
    try:
        asyncio.run(_list_subnets_async())
    except Exception as e:
        logger.error(f"Failed to run async list function: {e}", exc_info=True)
        console.print(f"[bold red]An unexpected error occurred:[/bold red] {e}")

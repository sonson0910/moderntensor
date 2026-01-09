"""
Utility commands for ModernTensor CLI

General utility commands.
"""

import click
from sdk.cli.utils import print_warning, console


@click.group(name='utils', short_help='Utility commands')
def utils():
    """
    Utility commands
    
    General utility operations and tools.
    """
    pass


@utils.command('convert')
@click.option('--from-base', type=float, help='Convert from base unit')
@click.option('--from-mdt', type=float, help='Convert from MDT')
def convert_units(from_base: float, from_mdt: float):
    """
    Convert between MDT and base units
    
    Similar to TAO/RAO conversion in Bittensor.
    """
    if from_base is None and from_mdt is None:
        console.print("❌ Specify --from-base or --from-mdt", style="bold red")
        return
    
    # Assuming 9 decimals like most cryptocurrencies
    decimals = 9
    base_per_mdt = 10 ** decimals
    
    if from_base is not None:
        mdt_value = from_base / base_per_mdt
        console.print(f"{from_base} base units = {mdt_value} MDT", style="bold green")
    
    if from_mdt is not None:
        base_value = from_mdt * base_per_mdt
        console.print(f"{from_mdt} MDT = {base_value} base units", style="bold green")


@utils.command('latency')
@click.option('--network', multiple=True, help='Additional networks to test')
def test_latency(network: tuple):
    """Test network latency to various nodes"""
    print_warning("Command not yet implemented")
    # TODO: Implement latency test


@utils.command('generate-keypair')
def generate_keypair():
    """Generate a new keypair (for testing)"""
    try:
        from sdk.keymanager.key_generator import KeyGenerator
        
        kg = KeyGenerator()
        keypair = kg.generate_keypair()
        
        console.print("\n✅ Generated new keypair:", style="bold green")
        console.print(f"Address: {keypair['address']}")
        console.print(f"Public Key: {keypair['public_key']}")
        console.print(f"\n⚠️  Private Key: {keypair['private_key']}", style="bold red")
        console.print("⚠️  Keep this private key secure!", style="bold red")
        
    except Exception as e:
        console.print(f"❌ Error: {str(e)}", style="bold red")


@utils.command('version')
def show_version():
    """Show detailed version information"""
    from sdk.cli import __version__
    from sdk.version import __version__ as sdk_version
    
    console.print(f"\n📦 mtcli version: {__version__}", style="bold cyan")
    console.print(f"📦 SDK version: {sdk_version}", style="bold cyan")
    console.print("\n🔗 Luxtensor blockchain interface", style="bold green")
    console.print("🌐 ModernTensor - Decentralized AI Network\n")

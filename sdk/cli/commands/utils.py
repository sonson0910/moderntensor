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
@click.option('--network', default='testnet', help='Network to test (mainnet/testnet/local)')
@click.option('--count', default=5, help='Number of test requests')
def test_latency(network: str, count: int):
    """
    Test network latency to Luxtensor nodes
    
    Sends multiple test requests to the network RPC endpoint and measures response times.
    
    Example:
        mtcli utils latency --network testnet --count 10
    """
    from sdk.cli.config import get_network_config
    from sdk.client import LuxtensorClient
    from rich.table import Table
    import time
    import statistics
    
    try:
        # Get network configuration
        config = get_network_config(network)
        
        print_info(f"Testing latency to {config.name} network...")
        print_info(f"RPC URL: {config.rpc_url}")
        print_info(f"Running {count} test requests...\n")
        
        # Create client
        client = LuxtensorClient(config.rpc_url)
        
        # Perform latency tests
        latencies = []
        successful = 0
        failed = 0
        
        for i in range(count):
            try:
                start_time = time.time()
                block_num = client.get_block_number()
                end_time = time.time()
                
                latency_ms = (end_time - start_time) * 1000
                latencies.append(latency_ms)
                successful += 1
                
                print_info(f"Request {i+1}/{count}: {latency_ms:.2f} ms (block: {block_num})")
                
            except Exception as e:
                failed += 1
                print_warning(f"Request {i+1}/{count}: Failed - {str(e)}")
            
            # Small delay between requests
            if i < count - 1:
                time.sleep(0.1)
        
        # Calculate statistics
        if latencies:
            avg_latency = statistics.mean(latencies)
            min_latency = min(latencies)
            max_latency = max(latencies)
            
            if len(latencies) > 1:
                std_dev = statistics.stdev(latencies)
            else:
                std_dev = 0
            
            # Display results table
            table = Table(title="\nLatency Test Results", show_header=True)
            table.add_column("Metric", style="cyan")
            table.add_column("Value", style="green")
            
            table.add_row("Network", config.name)
            table.add_row("RPC URL", config.rpc_url)
            table.add_row("Total Requests", str(count))
            table.add_row("Successful", str(successful))
            table.add_row("Failed", str(failed))
            table.add_row("Success Rate", f"{(successful/count)*100:.1f}%")
            table.add_row("---", "---")
            table.add_row("Average Latency", f"{avg_latency:.2f} ms")
            table.add_row("Min Latency", f"{min_latency:.2f} ms")
            table.add_row("Max Latency", f"{max_latency:.2f} ms")
            table.add_row("Std Deviation", f"{std_dev:.2f} ms")
            
            console.print(table)
            
            # Quality assessment
            if avg_latency < 100:
                print_success("✓ Excellent connection quality")
            elif avg_latency < 300:
                print_info("○ Good connection quality")
            elif avg_latency < 1000:
                print_warning("⚠ Fair connection quality")
            else:
                print_error("✗ Poor connection quality")
        else:
            print_error("All requests failed. Check your network connection and RPC endpoint.")
            
    except Exception as e:
        print_error(f"Failed to test latency: {str(e)}")


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

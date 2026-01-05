import click
import importlib
import asyncio
from sdk.simulation.simulator import SubnetSimulator


@click.command(name="simulate")
@click.option(
    "--subnet",
    required=True,
    help="Path to the subnet module (e.g., sdk.subnets.text_gen.TextGenerationSubnet).",
)
@click.option("--miners", default=3, help="Number of mock miners.")
@click.option("--steps", default=5, help="Number of simulation steps.")
def simulate_subnet(subnet, miners, steps):
    """Simulate a Subnet Protocol locally."""

    try:
        module_path, class_name = subnet.rsplit(".", 1)
        module = importlib.import_module(module_path)
        SubnetClass = getattr(module, class_name)
    except Exception as e:
        click.echo(f"Error loading subnet protocol: {e}")
        return

    simulator = SubnetSimulator(SubnetClass, n_miners=miners)
    asyncio.run(simulator.run(n_steps=steps))

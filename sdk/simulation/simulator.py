import asyncio
import logging
import time
from typing import Type
from rich.console import Console
from rich.table import Table
from sdk.subnets.protocol import SubnetProtocol

console = Console()
logger = logging.getLogger(__name__)


class SubnetSimulator:
    """
    Simulates the interaction between a Validator and a Miner for a specific SubnetProtocol.
    Useful for developing and testing subnet logic without running a full blockchain network.
    """

    def __init__(self, subnet_protocol_class: Type[SubnetProtocol], n_miners: int = 1):
        self.subnet_protocol = subnet_protocol_class()
        self.n_miners = n_miners
        self.miners = [f"miner_{i}" for i in range(n_miners)]
        console.print(
            f"[bold green]Initialized Simulator for Subnet: {self.subnet_protocol.get_metadata()['name']}[/bold green]"
        )

    async def run(self, n_steps: int = 5):
        """
        Runs the simulation for a number of steps.
        """
        for step in range(1, n_steps + 1):
            console.print(
                f"\n[bold blue]--- Simulation Step {step}/{n_steps} ---[/bold blue]"
            )

            # 1. Validator creates tasks
            tasks = {}
            for miner_uid in self.miners:
                difficulty = 0.5  # Mock difficulty
                task_data = self.subnet_protocol.create_task(miner_uid, difficulty)
                tasks[miner_uid] = task_data
                # console.print(f"Validator -> {miner_uid}: Task created (keys: {list(task_data.keys())})")

            # 2. Miners solve tasks
            results = {}
            for miner_uid, task_data in tasks.items():
                start_time = time.time()
                try:
                    # Simulate network latency?
                    # await asyncio.sleep(0.1)
                    result_data = self.subnet_protocol.solve_task(task_data)
                    elapsed = time.time() - start_time
                    results[miner_uid] = (result_data, elapsed)
                    # console.print(f"{miner_uid} -> Validator: Solved in {elapsed:.4f}s")
                except Exception as e:
                    console.print(
                        f"[red]Miner {miner_uid} failed to solve task: {e}[/red]"
                    )
                    results[miner_uid] = (None, 0)

            # 3. Validator scores results
            scores = {}
            table = Table(title=f"Step {step} Results")
            table.add_column("Miner UID", style="cyan")
            table.add_column("Task Input (Summary)", style="magenta")
            table.add_column("Miner Output (Summary)", style="green")
            table.add_column("Score", style="bold yellow")
            table.add_column("Time (s)", style="dim")

            for miner_uid, (result_data, elapsed) in results.items():
                task_data = tasks[miner_uid]
                if result_data:
                    try:
                        score = self.subnet_protocol.score_result(
                            task_data, result_data
                        )
                    except Exception as e:
                        console.print(f"[red]Error scoring {miner_uid}: {e}[/red]")
                        score = 0.0
                else:
                    score = 0.0

                scores[miner_uid] = score

                # Format for table
                task_str = (
                    str(task_data)[:50] + "..."
                    if len(str(task_data)) > 50
                    else str(task_data)
                )
                res_str = (
                    str(result_data)[:50] + "..."
                    if len(str(result_data)) > 50
                    else str(result_data)
                )

                table.add_row(
                    miner_uid, task_str, res_str, f"{score:.4f}", f"{elapsed:.4f}"
                )

            console.print(table)

            # Optional: Validate zkML if applicable (Mock check)
            # if hasattr(self.subnet_protocol, 'verifier'): ...

            await asyncio.sleep(1)  # Pause between steps

        console.print("\n[bold green]Simulation Completed.[/bold green]")

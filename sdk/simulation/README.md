# Subnet Simulator

The Subnet Simulator allows developers to test their `SubnetProtocol` implementations locally without running a full blockchain network or multiple processes.

## Features
- **Mock Validator**: Generates tasks using your protocol's `create_task` method.
- **Mock Miners**: Solves tasks using your protocol's `solve_task` method.
- **Scoring**: Validates and scores results using `score_result`.
- **Instant Feedback**: Prints a table of results and scores for each step.

## Usage

Run the simulator via the CLI:

```bash
python -m sdk.cli.main simulate --subnet sdk.subnets.text_gen.TextGenerationSubnet --miners 5 --steps 10
```

## Arguments
- `--subnet`: The python path to your SubnetProtocol class.
- `--miners`: Number of mock miners to simulate (default: 3).
- `--steps`: Number of simulation cycles to run (default: 5).

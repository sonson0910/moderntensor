# Using the CLI

`mtcli` is the official command-line tool for interacting with the LuxTensor network.

## Installation

```bash
pip install moderntensor-cli
# OR from source
cd moderntensor/sdk
pip install -e .
```

## Wallet Management

```bash
# Create a new wallet
mtcli wallet create --name my_wallet

# List wallets
mtcli wallet list

# Show balance
mtcli query balance --wallet my_wallet
```

## Transactions

```bash
# Transfer MDT
mtcli tx send --to <DESTINATION_ADDRESS> --amount 10.5 --wallet my_wallet
```

## Staking

```bash
# Stake tokens
mtcli stake add --amount 1000 --wallet my_wallet

# Unstake
mtcli stake remove --amount 500 --wallet my_wallet
```

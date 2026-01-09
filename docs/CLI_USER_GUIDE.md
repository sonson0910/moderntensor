# ModernTensor CLI (mtcli) - User Guide

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Configuration](#configuration)
4. [Wallet Management](#wallet-management)
5. [Blockchain Queries](#blockchain-queries)
6. [Transactions](#transactions)
7. [Staking](#staking)
8. [Subnets](#subnets)
9. [Validator Operations](#validator-operations)
10. [Utilities](#utilities)
11. [Troubleshooting](#troubleshooting)

## Installation

### From Source

```bash
# Clone repository
git clone https://github.com/sonson0910/moderntensor.git
cd moderntensor

# Install dependencies
pip install -r requirements.txt

# Install CLI
pip install -e .

# Verify installation
mtcli --version
```

### Requirements

- Python 3.8+
- pip
- 50MB disk space

## Quick Start

### Create Your First Wallet

```bash
# Create a coldkey (master wallet)
mtcli wallet create-coldkey --name my_wallet

# Save your mnemonic phrase securely!
# Write it down on paper - NEVER share it

# Create a hotkey (operational wallet)
mtcli wallet generate-hotkey --coldkey my_wallet --hotkey-name my_hotkey

# List your wallets
mtcli wallet list
```

### Check Balance

```bash
# Query your balance
mtcli wallet query-address \
    --coldkey my_wallet \
    --hotkey my_hotkey \
    --network testnet
```

### Send Tokens

```bash
# Send MDT tokens
mtcli tx send \
    --coldkey my_wallet \
    --hotkey my_hotkey \
    --to <recipient_address> \
    --amount 1000000000 \
    --network testnet
```

## Configuration

### Default Configuration

Located at: `~/.moderntensor/config.yaml`

```yaml
network:
  name: testnet
  rpc_url: http://localhost:8545
  chain_id: 2
  explorer_url: https://testnet-explorer.luxtensor.io

wallet:
  path: ~/.moderntensor/wallets
  default_coldkey: my_wallet
  default_hotkey: my_hotkey

verbosity: 1
use_cache: true
cache_ttl: 300
```

### Custom Configuration

```bash
# Use custom config file
mtcli --config /path/to/config.yaml wallet list

# Override network
mtcli --network mainnet query list-subnets
```

## Wallet Management

### Coldkey Operations

**Create new coldkey:**
```bash
mtcli wallet create-coldkey --name my_coldkey
```

**Restore from mnemonic:**
```bash
mtcli wallet restore-coldkey --name restored_wallet
# You'll be prompted for your mnemonic phrase
```

**List all coldkeys:**
```bash
mtcli wallet list
```

### Hotkey Operations

**Generate new hotkey:**
```bash
mtcli wallet generate-hotkey \
    --coldkey my_coldkey \
    --hotkey-name miner_hotkey
```

**Import existing hotkey:**
```bash
mtcli wallet import-hotkey \
    --coldkey my_coldkey \
    --hotkey-name imported_hk \
    --hotkey-file /path/to/hotkey.enc
```

**Regenerate hotkey from index:**
```bash
mtcli wallet regen-hotkey \
    --coldkey my_coldkey \
    --hotkey-name recovered_hk \
    --index 5
```

**List hotkeys:**
```bash
mtcli wallet list-hotkeys --coldkey my_coldkey
```

**Show hotkey details:**
```bash
mtcli wallet show-hotkey \
    --coldkey my_coldkey \
    --hotkey miner_hotkey
```

### Address Management

**Show address:**
```bash
mtcli wallet show-address \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --network testnet
```

**Query address on blockchain:**
```bash
mtcli wallet query-address \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --network testnet
```

**Register hotkey on subnet:**
```bash
mtcli wallet register-hotkey \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --subnet-uid 1 \
    --network testnet
```

## Blockchain Queries

### Address Queries

**Query any address:**
```bash
mtcli query address <address> --network testnet
```

**Query balance:**
```bash
mtcli query balance \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --network testnet
```

### Subnet Queries

**Query subnet info:**
```bash
mtcli query subnet --subnet-uid 1 --network testnet
```

**List all subnets:**
```bash
mtcli query list-subnets --network testnet
```

### Validator/Miner Queries

**Query validator:**
```bash
mtcli query validator <address> --network testnet
```

**Query miner:**
```bash
mtcli query miner <address> --network testnet
```

## Transactions

### Sending Tokens

**Send MDT tokens:**
```bash
mtcli tx send \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --to 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb \
    --amount 1000000000 \
    --network testnet
```

**Check transaction status:**
```bash
mtcli tx status <tx_hash> --network testnet
```

**View transaction history:**
```bash
mtcli tx history \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --limit 10 \
    --network testnet
```

## Staking

### Add Stake

**Stake tokens to validator:**
```bash
mtcli stake add \
    --coldkey my_coldkey \
    --hotkey validator_hotkey \
    --amount 10000 \
    --network testnet
```

### Remove Stake

**Unstake tokens:**
```bash
mtcli stake remove \
    --coldkey my_coldkey \
    --hotkey validator_hotkey \
    --amount 5000 \
    --network testnet
```

### Claim Rewards

**Claim staking rewards:**
```bash
mtcli stake claim \
    --coldkey my_coldkey \
    --hotkey validator_hotkey \
    --network testnet
```

### Staking Info

**View staking information:**
```bash
mtcli stake info \
    --coldkey my_coldkey \
    --hotkey validator_hotkey \
    --network testnet
```

**List all validators:**
```bash
mtcli stake list \
    --network testnet \
    --limit 20
```

## Subnets

### Create Subnet

**Create new subnet:**
```bash
mtcli subnet create \
    --coldkey my_coldkey \
    --name "My AI Subnet" \
    --network testnet
```

### Register on Subnet

**Register your hotkey:**
```bash
mtcli subnet register \
    --coldkey my_coldkey \
    --hotkey miner_hotkey \
    --subnet-uid 1 \
    --network testnet
```

### Subnet Info

**View subnet details:**
```bash
mtcli subnet info --subnet-uid 1 --network testnet
```

**List subnet participants:**
```bash
mtcli subnet participants \
    --subnet-uid 1 \
    --network testnet
```

## Validator Operations

### Start Validator

**Start validator node:**
```bash
mtcli validator start \
    --coldkey my_coldkey \
    --hotkey validator_hotkey \
    --network testnet
```

### Stop Validator

**Stop validator node:**
```bash
mtcli validator stop
```

### Validator Status

**Check validator status:**
```bash
mtcli validator status --network testnet
```

**Check specific validator:**
```bash
mtcli validator status \
    --address <validator_address> \
    --network testnet
```

### Set Weights

**Set validator weights:**
```bash
mtcli validator set-weights \
    --coldkey my_coldkey \
    --hotkey validator_hotkey \
    --subnet-uid 1 \
    --weights '{"0": 0.3, "1": 0.4, "2": 0.3}' \
    --network testnet
```

## Utilities

### Unit Conversion

**Convert MDT to base units:**
```bash
mtcli utils convert --from-mdt 1.5
# Output: 1.5 MDT = 1500000000 base units
```

**Convert base units to MDT:**
```bash
mtcli utils convert --from-base 1000000000
# Output: 1000000000 base units = 1.0 MDT
```

### Network Testing

**Test network latency:**
```bash
mtcli utils latency --network testnet --count 5
```

### Test Keypair Generation

**Generate test keypair:**
```bash
mtcli utils generate-keypair
```

### Version Info

**Show version:**
```bash
mtcli --version
mtcli utils version
```

## Troubleshooting

### Common Issues

#### Issue: "Command not found: mtcli"

**Solution:**
```bash
# Make sure CLI is installed
pip install -e .

# Or use Python module directly
python -m sdk.cli.main --version
```

#### Issue: "Wallet not found"

**Solution:**
```bash
# Check wallet location
ls ~/.moderntensor/wallets/

# List wallets
mtcli wallet list

# Verify coldkey name
mtcli wallet list
```

#### Issue: "Network connection failed"

**Solution:**
```bash
# Check network is reachable
ping testnet.luxtensor.io

# Try different network
mtcli --network local query list-subnets

# Check RPC URL in config
cat ~/.moderntensor/config.yaml
```

#### Issue: "Insufficient balance"

**Solution:**
```bash
# Check your balance
mtcli query balance \
    --coldkey my_coldkey \
    --hotkey my_hotkey \
    --network testnet

# Get testnet tokens (if available)
# Visit: https://faucet.luxtensor.io
```

#### Issue: "Invalid password"

**Solution:**
- Make sure you're entering the correct password
- Password is case-sensitive
- If forgotten, restore from mnemonic

### Debug Mode

**Enable verbose output:**
```bash
# Set verbosity in config
verbosity: 2  # 0=quiet, 1=normal, 2=verbose

# Or use environment variable
export MTCLI_VERBOSITY=2
```

### Getting Help

**Command help:**
```bash
# Main help
mtcli --help

# Command group help
mtcli wallet --help

# Specific command help
mtcli wallet create-coldkey --help
```

### Support Resources

- **Documentation**: See MTCLI_IMPLEMENTATION_GUIDE.md
- **GitHub Issues**: https://github.com/sonson0910/moderntensor/issues
- **Discord**: [Join our community]
- **Telegram**: [Join our channel]

## Best Practices

### Security

1. **Never share your mnemonic phrase**
2. **Use strong passwords** (min 12 characters)
3. **Back up your wallet** securely
4. **Use testnet first** before mainnet
5. **Verify addresses** before sending

### Performance

1. **Use cache** for repeated queries
2. **Set shorter TTL** for real-time data
3. **Use local node** for better latency
4. **Batch operations** when possible

### Maintenance

1. **Keep CLI updated**: `git pull && pip install -e .`
2. **Back up regularly**: Copy `~/.moderntensor/wallets/`
3. **Monitor disk space**: Clean old logs if needed
4. **Update config**: Check for new settings in releases

## Advanced Usage

### Scripting

**Bash script example:**
```bash
#!/bin/bash
# Check balance and send if sufficient

BALANCE=$(mtcli query balance --coldkey my_coldkey --hotkey my_hotkey --network testnet | grep "Balance")
if [ $? -eq 0 ]; then
    echo "Balance: $BALANCE"
    # Add logic to parse and check balance
fi
```

### JSON Output (Future)

```bash
# Get JSON output for parsing
mtcli --output json query balance ...
```

### Multiple Networks

```bash
# Query mainnet and testnet
mtcli --network mainnet query balance ... > mainnet.txt
mtcli --network testnet query balance ... > testnet.txt
```

## Appendix

### Network Information

| Network | Chain ID | RPC URL | Explorer |
|---------|----------|---------|----------|
| Mainnet | 1 | TBD | TBD |
| Testnet | 2 | http://testnet.luxtensor.io | https://testnet-explorer.luxtensor.io |
| Local | 3 | http://127.0.0.1:8545 | N/A |

### Unit Conversion

- 1 MDT = 1,000,000,000 base units (9 decimals)
- Like ETH/Wei or TAO/RAO

### Default Paths

- Config: `~/.moderntensor/config.yaml`
- Wallets: `~/.moderntensor/wallets/`
- Logs: `~/.moderntensor/logs/`

### Environment Variables

- `MTCLI_CONFIG`: Custom config path
- `MTCLI_NETWORK`: Default network
- `MTCLI_VERBOSITY`: Logging level (0-2)

---

**Version**: 1.0.0  
**Last Updated**: January 2026  
**License**: MIT

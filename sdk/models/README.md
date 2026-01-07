# ModernTensor SDK - Data Models

Standardized Pydantic models for blockchain data structures.

## Overview

This module provides type-safe, validated data models for all ModernTensor blockchain entities using Pydantic v2.

## Models Implemented

### Core Models (5)

1. **NeuronInfo** - Complete neuron information including state, stake, and performance metrics
2. **SubnetInfo** - Subnet metadata and state information
3. **SubnetHyperparameters** - Subnet configuration parameters
4. **StakeInfo** - Staking information for neurons
5. **ValidatorInfo** - Validator-specific information and metrics

### Network Models (2)

6. **AxonInfo** - Axon server endpoint information
7. **PrometheusInfo** - Prometheus metrics endpoint information

### Economic Models (2)

8. **DelegateInfo** - Delegation information including stake and nominators
9. **MinerInfo** - Miner-specific information and metrics

### Blockchain Models (2)

10. **BlockInfo** - Blockchain block information
11. **TransactionInfo** - Transaction information

## Usage

```python
from sdk.models import NeuronInfo, SubnetInfo, StakeInfo

# Create a neuron
neuron = NeuronInfo(
    uid=0,
    hotkey="5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
    coldkey="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
    subnet_uid=1,
    stake=1000.0,
    rank=0.95,
    trust=0.98
)

print(neuron)  # NeuronInfo(uid=0, hotkey=5C4hrfjw..., stake=1000.0)
print(f"Neuron stake: {neuron.stake}")

# Create a subnet
subnet = SubnetInfo(
    subnet_uid=1,
    netuid=1,
    name="Text Prompting",
    owner="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
    n=256,
    max_n=4096
)

print(subnet)  # SubnetInfo(uid=1, name='Text Prompting', n=256/4096)

# Create stake info
stake = StakeInfo(
    hotkey="5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
    coldkey="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
    stake=1000.0,
    block=12345
)

print(stake)  # StakeInfo(hotkey=5C4hrfjw..., stake=1000.0)
```

## Features

### Validation

All models include comprehensive validation:

```python
# This will raise validation error (negative stake)
neuron = NeuronInfo(
    uid=0,
    hotkey="key",
    coldkey="key",
    subnet_uid=1,
    stake=-100.0  # ❌ ValidationError: stake must be >= 0
)

# This will raise validation error (invalid port)
axon = AxonInfo(
    ip="192.168.1.100",
    port=100000,  # ❌ ValidationError: port must be 1-65535
    hotkey="key",
    coldkey="key"
)
```

### Type Safety

All models have full type hints:

```python
from sdk.models import NeuronInfo

def process_neuron(neuron: NeuronInfo) -> float:
    # IDE autocomplete works perfectly
    return neuron.stake * neuron.rank

neuron = NeuronInfo(uid=0, hotkey="key", coldkey="key", subnet_uid=1)
score = process_neuron(neuron)  # Type checked!
```

### Examples

Each model includes example data:

```python
from sdk.models import NeuronInfo

# Get example schema
print(NeuronInfo.model_json_schema())

# Example output shows valid data structure
```

## Model Details

### NeuronInfo

Complete neuron information:

- **Identity**: uid, hotkey, coldkey, subnet_uid
- **State**: active, last_update
- **Stake**: stake, total_stake
- **Performance**: rank, trust, consensus, incentive, dividends, emission
- **Validator**: validator_permit, validator_trust
- **Endpoints**: axon_info, prometheus_info

### SubnetInfo

Subnet metadata:

- **Identity**: subnet_uid, netuid, name, owner
- **Network**: n (neurons), max_n, tempo, emission_value
- **Configuration**: hyperparameters
- **State**: block, burn, connect

### SubnetHyperparameters

Subnet configuration:

- **Network**: rho, kappa, immunity_period
- **Validator**: min_allowed_weights, max_weights_limit, tempo
- **Stake**: min_stake, max_stake
- **Adjustment**: adjustment_interval, adjustment_alpha
- **Other**: difficulty, serving_rate_limit, etc.

### ValidatorInfo

Validator-specific data:

- **Identity**: uid, hotkey, coldkey
- **Status**: validator_permit, validator_trust
- **Stake**: total_stake, own_stake, delegated_stake
- **Activity**: last_update, dividends, weights_set

### MinerInfo

Miner-specific data:

- **Identity**: uid, hotkey, coldkey
- **Performance**: rank, trust, consensus, incentive, emission
- **State**: active, stake, last_update
- **Endpoints**: axon_info, prometheus_info

### AxonInfo

Server endpoint:

- **Network**: ip, port, ip_type, protocol
- **Identity**: hotkey, coldkey
- **Metadata**: version

Property: `endpoint` returns full URL

### PrometheusInfo

Metrics endpoint:

- **Network**: ip, port, ip_type
- **Metadata**: version, block

Property: `endpoint` returns metrics URL

### DelegateInfo

Delegation data:

- **Identity**: hotkey, owner
- **Stake**: total_stake, nominators (list)
- **Commission**: take (commission rate)
- **Performance**: return_per_1000, total_daily_return
- **Registrations**: registrations, validator_permits

### BlockInfo

Block data:

- **Identity**: block_number, block_hash, parent_hash
- **Transactions**: transactions (list), transaction_count
- **State**: state_root, extrinsics_root
- **Metadata**: timestamp, author

### TransactionInfo

Transaction data:

- **Identity**: tx_hash, block_number, block_hash
- **Addresses**: from_address, to_address
- **Type**: method, pallet
- **Status**: success, fee
- **Data**: args, nonce, signature, timestamp

## Testing

```python
# Run model tests
python -m pytest tests/test_models.py -v

# Validate all models
python -c "from sdk.models import *; print('✓ All models imported')"
```

## Migration from Old Models

If you have existing code using old model formats, migration is straightforward:

```python
# Old way (dict)
neuron_dict = {
    "uid": 0,
    "hotkey": "...",
    "stake": 1000.0
}

# New way (Pydantic model)
from sdk.models import NeuronInfo

neuron = NeuronInfo(**neuron_dict)  # Validates and converts
print(neuron.stake)  # Type-safe access

# Convert back to dict if needed
neuron_dict = neuron.model_dump()  # or .dict() in Pydantic v1
```

## Performance

- Models use Pydantic v2 for fast validation
- Validation overhead is minimal (<1ms per model)
- Models are serializable to JSON efficiently

## Future Enhancements

Additional models planned for future phases:

- **ProposalInfo** - Governance proposals
- **IdentityInfo** - On-chain identity
- **CrowdloanInfo** - Crowdloan participation
- **LiquidityInfo** - Liquidity pool data
- **MEVInfo** - MEV protection data
- **NetworkInfo** - Network statistics
- **ConsensusInfo** - Consensus data

## Contributing

When adding new models:

1. Create model file in `sdk/models/`
2. Use Pydantic BaseModel
3. Add comprehensive field validation
4. Include example in `json_schema_extra`
5. Add `__str__` and `__repr__` methods
6. Export in `__init__.py`
7. Add tests
8. Update this README

## License

MIT License - See LICENSE file for details

# ModernTensor SDK - API Reference

**SDK Version:** 0.4.0
**Compatible Luxtensor RPC Version:** 0.4.0

## Version Compatibility

| SDK Version | Luxtensor RPC | Status |
|-------------|---------------|--------|
| 0.4.0 | 0.4.0 | ✅ Current |
| 0.3.x | 0.3.x | ⚠️ Legacy |
| < 0.3 | N/A | ❌ Deprecated |

---

## RPC Methods Reference

### Blockchain Methods (`eth_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `eth_blockNumber` | Get current block height | None | `hex string` |
| `eth_getBlockByNumber` | Get block by number | `block_number`, `full_txs` | Block object |
| `eth_getBlockByHash` | Get block by hash | `hash`, `full_txs` | Block object |
| `eth_getTransactionByHash` | Get transaction | `tx_hash` | Transaction object |
| `eth_getBalance` | Get account balance | `address`, `block` | `hex string` |
| `eth_getTransactionCount` | Get account nonce | `address`, `block` | `hex string` |
| `eth_sendRawTransaction` | Submit signed tx | `raw_tx` | `tx_hash` |

### Transaction Methods

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `tx_getReceipt` | Get transaction receipt | `tx_hash` | Receipt object |

### Staking Methods (`staking_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `staking_getTotalStake` | Total network stake | None | `hex string` |
| `staking_getStake` | Get stake for address | `address` | `hex string` |
| `staking_getValidators` | List all validators | None | Validator[] |
| `staking_addStake` | Add stake | `address`, `amount` | `bool` |
| `staking_removeStake` | Remove stake | `address`, `amount` | `bool` |
| `staking_claimRewards` | Claim rewards | `address` | Receipt |

### Subnet Methods (`subnet_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `subnet_getInfo` | Get subnet details | `subnet_id` | SubnetInfo |
| `subnet_listAll` | List all subnets | None | SubnetInfo[] |
| `subnet_create` | Create subnet | `name`, `owner`, `emission` | Receipt |
| `subnet_getCount` | Total subnet count | None | `number` |
| `subnet_getAll` | Get all subnets | None | Subnet[] |
| `subnet_register` | Register on subnet | Params | Receipt |
| `subnet_getRootValidators` | Root validators | None | Validator[] |
| `subnet_getEmissions` | Subnet emissions | `subnet_id` | Emissions |
| `subnet_getConfig` | Root config | None | Config |

### Neuron Methods (`neuron_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `neuron_getInfo` | Get neuron details | `subnet_id`, `uid` | NeuronInfo |
| `neuron_listBySubnet` | List neurons | `subnet_id` | NeuronInfo[] |
| `neuron_register` | Register neuron | `subnet_id`, `address`, `stake` | Receipt |
| `neuron_getCount` | Neuron count | `subnet_id` | `number` |

### Weight Methods (`weight_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `weight_getWeights` | Get neuron weights | `subnet_id`, `uid` | WeightInfo[] |
| `weight_setWeights` | Set weights | `subnet_id`, `uid`, `uids[]`, `weights[]` | Receipt |
| `weight_getAll` | All weights | `subnet_id` | Weights |

### Query Methods (`query_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `query_neuron` | Get neuron | `subnet_id`, `uid` | Neuron |
| `query_neuronCount` | Neuron count | `subnet_id` | `number` |
| `query_activeNeurons` | Active neurons | `subnet_id` | Neuron[] |
| `query_allSubnets` | All subnets | None | Subnet[] |
| `query_subnetExists` | Check exists | `subnet_id` | `bool` |
| `query_subnetOwner` | Owner address | `subnet_id` | `string` |
| `query_subnetEmission` | Emission rate | `subnet_id` | `number` |
| `query_subnetHyperparameters` | Hyperparams | `subnet_id` | Object |
| `query_subnetTempo` | Tempo | `subnet_id` | `number` |
| `query_rank` | Neuron rank | `subnet_id`, `uid` | `number` |
| `query_trust` | Neuron trust | `subnet_id`, `uid` | `float` |
| `query_incentive` | Neuron incentive | `subnet_id`, `uid` | `float` |
| `query_dividends` | Neuron dividends | `subnet_id`, `uid` | `float` |
| `query_consensus` | Neuron consensus | `subnet_id`, `uid` | `number` |
| `query_isHotkeyRegistered` | Check hotkey | `subnet_id`, `hotkey` | `bool` |
| `query_uidForHotkey` | UID for hotkey | `subnet_id`, `hotkey` | `number` |
| `query_hotkeyForUid` | Hotkey for UID | `subnet_id`, `uid` | `string` |
| `query_stakeForColdkeyAndHotkey` | Stake amount | `coldkey`, `hotkey` | `hex string` |
| `query_totalStakeForColdkey` | Total for coldkey | `coldkey` | `hex string` |
| `query_totalStakeForHotkey` | Total for hotkey | `hotkey` | `hex string` |
| `query_allStakeForColdkey` | All stakes | `coldkey` | Stakes[] |
| `query_allStakeForHotkey` | All stakes | `hotkey` | Stakes[] |
| `query_weightCommits` | Weight commits | `subnet_id` | Commits[] |
| `query_weightsVersion` | Weights version | `subnet_id` | `number` |
| `query_weightsRateLimit` | Rate limit | `subnet_id` | `number` |
| `query_hasValidatorPermit` | Has permit | `subnet_id`, `hotkey` | `bool` |
| `query_validatorTrust` | Validator trust | `subnet_id`, `uid` | `float` |
| `query_rho` | Rho param | `subnet_id` | `float` |
| `query_kappa` | Kappa param | `subnet_id` | `float` |
| `query_adjustmentInterval` | Interval | `subnet_id` | `number` |
| `query_activityCutoff` | Cutoff blocks | `subnet_id` | `number` |
| `query_rootNetworkValidators` | Root validators | None | Validator[] |
| `query_senateMembers` | Senate members | None | Member[] |

### AI Methods (`lux_*`)

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `lux_submitAITask` | Submit AI task | Task params | `task_id` |
| `lux_getAIResult` | Get task result | `task_id` | Result |
| `lux_getValidatorStatus` | Validator status | `address` | Status |

### System Methods

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `system_health` | Node health | None | Health |
| `system_version` | Node version | None | Version |
| `system_peerCount` | Peer count | None | `number` |
| `system_syncState` | Sync state | None | State |

### Governance Methods

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `governance_getProposals` | All proposals | None | Proposal[] |
| `governance_getProposal` | Get proposal | `proposal_id` | Proposal |

### Balance Methods

| Method | Description | Parameters | Returns |
|--------|-------------|------------|---------|
| `balances_free` | Free balance | `address` | `hex string` |
| `balances_reserved` | Reserved | `address` | `hex string` |

---

## Python SDK Usage

### Import Guide

```python
# ✅ After pip install
from moderntensor.sdk import LuxtensorClient

# ✅ Development mode
import sys; sys.path.insert(0, 'moderntensor')
from sdk import LuxtensorClient
```

### Basic Usage

```python
from moderntensor.sdk import LuxtensorClient

client = LuxtensorClient(\"http://localhost:8545\")

# Blockchain
block = client.get_block_number()

# Staking
stake = client.get_stake("0x...")
validators = client.get_validators()

# Subnets
subnets = client.get_all_subnets()
info = client.get_subnet_info(1)

# Neurons
neurons = client.get_neurons(subnet_id=1)
registered = client.is_hotkey_registered(1, "0x...")
```

### Advanced Features

```python
from sdk.commit_reveal import CommitRevealClient
from sdk.neuron_checker import NeuronChecker

# Commit-Reveal
cr_client = CommitRevealClient("http://localhost:8545")
salt = cr_client.generate_salt()
commit_hash = cr_client.compute_hash(weights, salt)

# Neuron Checker
checker = NeuronChecker("http://localhost:8545")
status = checker.check_registration(subnet_uid=1, hotkey="0x...")
```

---

## Error Codes

| Code | Description |
|------|-------------|
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 | Server error |

---

*Last updated: 2026-01-16*

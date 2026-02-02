# ModernTensor SDK - API Reference

**SDK Version:** 0.4.0
**Compatible Luxtensor RPC Version:** 0.4.0

## Version Compatibility

| SDK Version | Luxtensor RPC | Status |
| :--- | :--- | :--- |
| 0.4.0 | 0.4.0 | ✅ Current |
| 0.3.x | 0.3.x | ⚠️ Legacy |
| < 0.3 | N/A | ❌ Deprecated |

---

## RPC Methods Reference

### Blockchain Methods (`eth_*`)

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `eth_blockNumber` | Get current block height | None | `hex string` |
| `eth_getBlockByNumber` | Get block by number | `block_number`, `full_txs` | Block object |
| `eth_getBlockByHash` | Get block by hash | `hash`, `full_txs` | Block object |
| `eth_getTransactionByHash` | Get transaction | `tx_hash` | Transaction object |
| `eth_getBalance` | Get account balance | `address`, `block` | `hex string` |
| `eth_getTransactionCount` | Get account nonce | `address`, `block` | `hex string` |
| `eth_sendRawTransaction` | Submit signed tx | `raw_tx` | `tx_hash` |

### Transaction Methods

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `tx_getReceipt` | Get transaction receipt | `tx_hash` | Receipt object |

### Staking Methods (`staking_*`)

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `staking_getTotalStake` | Total network stake | None | `hex string` |
| `staking_getStake` | Get stake for address | `address` | `hex string` |
| `staking_getValidators` | List all validators | None | Validator[] |
| `staking_stake` | Add stake | `address`, `amount` | Receipt |
| `staking_unstake` | Remove stake | `address`, `amount` | Receipt |
| `staking_delegate` | Delegate to validator | `delegator`, `validator`, `amount` | Receipt |
| `staking_undelegate` | Remove delegation | `delegator` | Receipt |
| `staking_getDelegation` | Get delegation info | `delegator` | Delegation |
| `staking_getMinimums` | Min stake requirements | None | Object |
| `staking_getStakeForPair` | 🆕 Stake for coldkey-hotkey | `coldkey`, `hotkey` | Object |
| `staking_getAllStakesForColdkey` | 🆕 All coldkey stakes | `coldkey` | Stakes[] |
| `staking_getDelegates` | 🆕 List delegates | None | Delegate[] |

### Subnet Methods (`subnet_*`)

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `subnet_getInfo` | Get subnet details | `subnet_id` | SubnetInfo |
| `subnet_getAll` | Get all subnets | None | Subnet[] |
| `subnet_register` | Register subnet | `name`, `owner` | Receipt |
| `subnet_getCount` | 🆕 Total subnet count | None | `number` |
| `subnet_exists` | 🆕 Check subnet exists | `subnet_id` | `bool` |
| `subnet_getHyperparameters` | 🆕 Hyperparameters | `subnet_id` | Object |
| `subnet_getRootValidators` | Root validators | None | Validator[] |
| `subnet_getEmissions` | Subnet emissions | `subnet_id` | Emissions |
| `subnet_getConfig` | Root config | None | Config |

### Neuron Methods (`neuron_*`)

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `neuron_getInfo` | Get neuron details | `subnet_id`, `uid` | NeuronInfo |
| `neuron_listBySubnet` | List neurons | `subnet_id` | NeuronInfo[] |
| `neuron_register` | Register neuron | `subnet_id`, `address`, `stake` | Receipt |
| `neuron_getCount` | Neuron count | `subnet_id` | `number` |
| `neuron_get` | 🆕 Alias for getInfo | `subnet_id`, `uid` | NeuronInfo |
| `neuron_getAll` | 🆕 Alias for listBySubnet | `subnet_id` | NeuronInfo[] |
| `neuron_exists` | 🆕 Check neuron exists | `subnet_id`, `uid` | `bool` |
| `neuron_getByHotkey` | 🆕 Get by hotkey | `subnet_id`, `hotkey` | NeuronInfo |
| `neuron_getActive` | 🆕 Active neuron UIDs | `subnet_id` | uid[] |
| `neuron_count` | 🆕 Alias for getCount | `subnet_id` | `number` |
| `neuron_batchGet` | 🆕 Batch get neurons | `subnet_id`, `uids[]` | NeuronInfo[] |

### Weight Methods (`weight_*`)

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `weight_getWeights` | Get neuron weights | `subnet_id`, `uid` | WeightInfo[] |
| `weight_setWeights` | Set weights | `subnet_id`, `uid`, `uids[]`, `weights[]` | Receipt |
| `weight_getAll` | All weights | `subnet_id` | Weights |
| `weight_getCommits` | 🆕 Weight commits | `subnet_id` | Commits[] |

### System Methods

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `system_health` | Node health | None | Health |
| `system_version` | Node version | None | Version |
| `system_peerCount` | Peer count | None | `number` |
| `system_syncState` | Sync state | None | State |

### Governance Methods

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `governance_getProposals` | All proposals | None | Proposal[] |
| `governance_getProposal` | Get proposal | `proposal_id` | Proposal |

---

## SDK Module Reference

### Core Types (`sdk.core`)

#### NodeTier

| Enum Member | ID | Description |
| :--- | :--- | :--- |
| `LIGHT_NODE` | 0 | Light client, tx relay |
| `FULL_NODE` | 1 | Full node, infrastructure |
| `VALIDATOR` | 2 | Validator, AI quality check |
| `SUPER_VALIDATOR` | 3 | Block production priority |

#### ScoringManager

| Method | Description |
| :--- | :--- |
| `calculate_miner_score(metrics)` | Calc score based on latency/quality |
| `calculate_validator_score(metrics)` | Calc score based on blocks/uptime |
| `apply_decay(score)` | Apply time-based decay |

### Consensus Module (`sdk.consensus`)

#### Fork Choice (GHOST)

| Method | Description |
| :--- | :--- |
| `add_block(block)` | Add new block to fork choice |
| `get_head()` | Get canonical chain head |
| `get_canonical_chain()` | Get main chain blocks |

#### Fast Finality (BFT)

| Method | Description |
| :--- | :--- |
| `add_signature(block, validator)` | Add vote for block |
| `is_finalized(block)` | Check if >67% stake voted |
| `get_finality_progress(block)` | Get current vote % |

#### Slashing

| Method | Description |
| :--- | :--- |
| `check_offline(validator)` | Check for missed blocks |
| `check_double_signing(height)` | Check for conflicting sigs |
| `slash(evidence)` | Execute penalty & jail |

#### Circuit Breaker

| Method | Description |
| :--- | :--- |
| `allow_request()` | Check if op is allowed |
| `record_failure()` | Track error count |
| `execute(func)` | Run with protection |

#### Liveness

| Method | Description |
| :--- | :--- |
| `check_liveness()` | Check network health |
| `record_block(height)` | Track block production |

---

## Error Codes

| Code | Description |
| :--- | :--- |
| -32700 | Parse error |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 | Server error |

---

*Last updated: 2026-02-02*

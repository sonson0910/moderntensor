# Luxtensor RPC Reference

Complete reference for all JSON-RPC methods.

---

## System Methods

### system_health

Check node health status.

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"system_health","params":[],"id":1}'
```

**Response:**

```json
{
  "jsonrpc": "2.0",
  "result": {
    "is_syncing": false,
    "peers": 12,
    "should_have_peers": true
  },
  "id": 1
}
```

### system_peers

Get connected peers.

```bash
{"jsonrpc":"2.0","method":"system_peers","params":[],"id":1}
```

### system_chain

Get chain information.

```bash
{"jsonrpc":"2.0","method":"system_chain","params":[],"id":1}
```

---

## Ethereum-Compatible Methods

### eth_blockNumber

Get current block number.

```bash
{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}
```

### eth_getBalance

Get account balance.

```bash
{"jsonrpc":"2.0","method":"eth_getBalance","params":["0xADDRESS","latest"],"id":1}
```

### eth_sendRawTransaction

Send signed transaction.

```bash
{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0xSIGNED_TX"],"id":1}
```

### eth_getTransactionReceipt

Get transaction receipt.

```bash
{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0xTX_HASH"],"id":1}
```

### eth_call

Execute contract call (read-only).

```bash
{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0xCONTRACT","data":"0xDATA"},"latest"],"id":1}
```

---

## Staking Methods

### staking_registerValidator

Register as validator.

```bash
{"jsonrpc":"2.0","method":"staking_registerValidator","params":["STAKE_AMOUNT"],"id":1}
```

### staking_getActiveValidators

Get all active validators.

```bash
{"jsonrpc":"2.0","method":"staking_getActiveValidators","params":[],"id":1}
```

**Response:**

```json
{
  "result": {
    "validators": [
      {
        "address": "0x...",
        "stake": "10000000000000000000000",
        "activation_epoch": 5
      }
    ]
  }
}
```

### staking_getValidator

Get validator info.

```bash
{"jsonrpc":"2.0","method":"staking_getValidator","params":["0xADDRESS"],"id":1}
```

### staking_getConfig

Get staking configuration.

```bash
{"jsonrpc":"2.0","method":"staking_getConfig","params":[],"id":1}
```

---

## Subnet Methods

### subnet_list

List all subnets.

```bash
{"jsonrpc":"2.0","method":"subnet_list","params":[],"id":1}
```

### subnet_getInfo

Get subnet details.

```bash
{"jsonrpc":"2.0","method":"subnet_getInfo","params":[1],"id":1}
```

### subnet_getNeurons

Get neurons in subnet.

```bash
{"jsonrpc":"2.0","method":"subnet_getNeurons","params":[1],"id":1}
```

---

## Neuron Methods

### neuron_register

Register neuron in subnet.

```bash
{"jsonrpc":"2.0","method":"neuron_register","params":[1, "0xADDRESS"],"id":1}
```

### neuron_getInfo

Get neuron info.

```bash
{"jsonrpc":"2.0","method":"neuron_getInfo","params":[1, 0],"id":1}
```

---

## Weight Methods

### weight_setWeights

Set neuron weights.

```bash
{"jsonrpc":"2.0","method":"weight_setWeights","params":[1, [[0, 100], [1, 200]]],"id":1}
```

### weight_getWeights

Get neuron weights.

```bash
{"jsonrpc":"2.0","method":"weight_getWeights","params":[1, 0],"id":1}
```

---

## AI Layer Methods

### ai_getCircuitBreakerStatus

Get AI circuit breaker status.

```bash
{"jsonrpc":"2.0","method":"ai_getCircuitBreakerStatus","params":[],"id":1}
```

**Response:**

```json
{
  "result": {
    "state": "Closed",
    "failure_count": 0,
    "last_transition": 1706698000
  }
}
```

---

## Error Codes

| Code | Message |
|------|---------|
| -32600 | Invalid Request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32603 | Internal error |
| -32000 | Insufficient funds |
| -32001 | Nonce too low |
| -32002 | Gas limit exceeded |

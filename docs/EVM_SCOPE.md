# LuxTensor EVM Compatibility

## Overview

LuxTensor uses **revm** (Rust EVM) for full EVM bytecode execution, enabling Solidity smart contract deployment and execution.

## ✅ Fully Supported Features

### EVM Execution

- **Contract Deployment** via `EvmExecutor::deploy()`
- **Contract Calls** via `EvmExecutor::call()`
- **Static Calls** for read-only operations
- **Full EVM bytecode execution** with proper gas accounting
- **State management** with Database and DatabaseCommit traits

### Precompiled Contracts

All standard Ethereum precompiles are enabled:

- `0x01` - ECRECOVER
- `0x02` - SHA256
- `0x03` - RIPEMD160
- `0x04` - IDENTITY
- `0x05` - MODEXP
- `0x06-0x08` - BN128 operations
- `0x09` - BLAKE2F

### EVM Specification

- **SpecId: CANCUN** - Latest EVM features enabled
- **EIP-1559** dynamic fees
- **EIP-170** max code size (24KB)
- Proper gas costs following Ethereum specifications

### RPC Methods (eth_*)

| Method | Status | Notes |
|--------|--------|-------|
| `eth_chainId` | ✅ | Returns 777 (LuxTensor) |
| `eth_blockNumber` | ✅ | Current block height |
| `eth_getBalance` | ✅ | Account balance query |
| `eth_getTransactionCount` | ✅ | Nonce query |
| `eth_gasPrice` | ✅ | Fixed 1 gwei |
| `eth_estimateGas` | ✅ | Estimation |
| `eth_sendTransaction` | ✅ | Submit transactions |
| `eth_getTransactionReceipt` | ✅ | Transaction results |
| `eth_getCode` | ✅ | Contract bytecode |
| `eth_call` | ⚠️ | Returns empty (needs full state) |
| `eth_accounts` | ✅ | Pre-funded test accounts |
| `net_version` | ✅ | Chain ID |

### Successfully Deployed Contracts

- **MDTToken** at `0xb2519110ec53731e1e7353ec109151b200000001`
- **MDTVesting** at `0x0c642480eb531543431553eb8024640c00000002`

## Development Tools

### Hardhat Integration

```javascript
// hardhat.config.js
module.exports = {
    solidity: "0.8.20",
    networks: {
        luxtensor_local: {
            url: "http://127.0.0.1:8545",
            chainId: 1337,
        }
    }
}
```

### Deployment Script

```bash
cd luxtensor/contracts
npm install
npx hardhat run scripts/deploy.js --network luxtensor_local
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ContractExecutor                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │   EvmExecutor   │  │  ContractState  │  │   Deployed  │  │
│  │   (revm)        │  │   (storage)     │  │  Contracts  │  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  │
├─────────────────────────────────────────────────────────────┤
│                    revm_integration.rs                       │
│  • Precompiles configuration                                 │
│  • Gas cost constants                                        │
│  • ABI encoding/decoding                                     │
│  • ERC-20/ERC-721 selectors                                  │
└─────────────────────────────────────────────────────────────┘
```

## Roadmap

### Phase 2 (Q2 2026)

- [ ] Full `eth_call` with state access
- [ ] `eth_getLogs` and event subscriptions
- [ ] `eth_subscribe` WebSocket support
- [ ] EIP-2929 access lists

### Phase 3 (Q3 2026)

- [ ] Cross-chain bridge support
- [ ] ERC-4337 account abstraction
- [ ] Layer 2 integration

---

*LuxTensor Chain ID: 777 (testnet: 1337)*

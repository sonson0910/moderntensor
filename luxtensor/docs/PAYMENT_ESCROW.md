# PaymentEscrow System

The PaymentEscrow contract manages MDT token payments for AI inference requests in LuxTensor's pay-per-compute economy.

## Overview

```
┌─────────────┐     deposit      ┌───────────────┐
│   Requester │ ──────────────► │ PaymentEscrow │
└─────────────┘                  └───────┬───────┘
                                         │
          ┌──────────────────────────────┼──────────────────────────────┐
          │                              │                              │
          ▼                              ▼                              ▼
   ┌─────────────┐               ┌─────────────┐               ┌─────────────┐
   │   Timeout   │               │   Fulfill   │               │   Refund    │
   │   (60 min)  │               │   (Miner)   │               │ (Requester) │
   └─────────────┘               └─────────────┘               └─────────────┘
```

## Contract Address

Deployed via `scripts/deploy-ai-oracle.js`

## Key Functions

### `deposit(bytes32 requestId, uint256 amount)`

Lock MDT tokens for an AI request.

```solidity
// Requester deposits MDT for a request
paymentEscrow.deposit(requestId, 1e18); // 1 MDT
```

### `releaseToMiner(bytes32 requestId, address miner)`

Release funds to miner after successful fulfillment. Called by AIOracle only.

```solidity
// Protocol fee: 1% to treasury
// Miner receives: 99% of deposited amount
```

### `refund(bytes32 requestId)`

Requester can claim refund after timeout (default: 60 minutes).

```solidity
// After timeout expires, requester can reclaim
paymentEscrow.refund(requestId);
```

## State Machine

```
PENDING ─► RELEASED (miner paid)
    │
    └────► REFUNDED (timeout expired)
```

## Protocol Fee

- **Fee Rate**: 1% (100 basis points)
- **Recipient**: Protocol Treasury
- **Purpose**: Network sustainability

## Security Features

- **ReentrancyGuard**: Prevents reentrancy attacks
- **Ownable**: Admin functions protected
- **SafeERC20**: Safe token transfers
- **Timeout**: 60 minute escrow period

## Integration

```solidity
// 1. User requests AI inference
bytes32 requestId = aiOracle.requestAI(modelHash, inputData, timeout);

// 2. Internal: AIOracle deposits to escrow
// (handled automatically)

// 3. Miner fulfills request with proof
aiOracle.fulfillRequest(requestId, result, proof);

// 4. Internal: Escrow releases payment to miner
// (miner receives 99%, protocol receives 1%)
```

## Source Code

- Contract: `contracts/src/templates/PaymentEscrow.sol`
- Deploy Script: `contracts/scripts/deploy-ai-oracle.js`

---

*Last updated: 2026-02-02*

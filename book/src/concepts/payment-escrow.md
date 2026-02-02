# PaymentEscrow System

The **PaymentEscrow** is the financial heart of ModernTensor's pay-per-compute economy. It ensures miners get paid for valid work and users get refunded if work isn't delivered.

## The Problem

In a decentralized system, how do you ensure:

1. **Miners** get paid after doing expensive computations?
2. **Users** don't lose money if a miner goes offline?

## The Solution: Pay-per-Compute Escrow

The `PaymentEscrow.sol` contract acts as a trustless intermediary.

### Workflow

1. **Deposit**: User initiates a request. The cost (in MDT) is transferred to the Escrow contract.
2. **Lock**: Funds are locked and tied to a `requestId`.
3. **Fulfill**: Miner submits a ZK proof. The AI Oracle verifies it.
4. **Release**: If verified, the Escrow releases 99% to the miner. 1% goes to the protocol treasury.
5. **Refund**: If no miner fulfills within 60 minutes, the user can claim a full refund.

## Smart Contract Details

- **Protocol Fee**: 1% (Sustainable funding for network development).
- **Timeout**: 60 minutes hardcoded safety valve.
- **Security**: ReentrancyGuard, SafeERC20.

```solidity
// Example: Checking escrow status
function getRequestStatus(bytes32 requestId) view returns (Status) {
    return escrow.requests(requestId).status;
}
```

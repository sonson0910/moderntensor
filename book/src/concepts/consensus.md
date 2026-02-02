# Consensus

ModernTensor utilizes a robust **Proof of Stake (PoS)** consensus mechanism to secure the network and reach agreement on the global state.

## Validator Set

- **Selection**: Validators are selected based on total stake (Self-stake + Delegated stake).
- **Epochs**: The validator set is refreshed every Epoch (approx. 24 hours).
- **Slashing**: Malicious behavior (double signing, downtime) results in stake slashing.

## AI Consensus vs. Ledger Consensus

ModernTensor effectively runs two consensus layers:

1. **Ledger Consensus (Layer 1)**: Standard PoS. Agreeing on token balances, smart contract states, and ordering of blocks.
2. **AI Consensus (Layer 2)**: Probabilistic & Verifiable through zkML. Agreeing that "Computation X produced Result Y".

## Finality

Blocks are finalized using a deterministic gadget (similar to IBFT 2.0), ensuring fast finality (<2 seconds) required for real-time AI applications.

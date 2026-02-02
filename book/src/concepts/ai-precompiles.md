# Native AI Precompiles

ModernTensor implements native AI opcodes as EVM precompiled contracts. This allows smart contracts to "speak" AI directly.

## The 4 Opcodes

| Address | Name | Purpose |
|---------|------|---------|
| `0x10`  | AI_REQUEST | Submit AI inference requests |
| `0x11`  | VERIFY_PROOF | Verify zkML proofs |
| `0x12`  | GET_RESULT | Retrieve inference results |
| `0x13`  | COMPUTE_PAYMENT | Calculate payment for request |

## How it Works

1. **Request**: User calls `AI_REQUEST` with a model hash and input.
2. **Execution**: Miners pick up the request from the P2P network.
3. **Verification**: Using `VERIFY_PROOF`, the miner submits a ZK proof that they ran the model correctly.
4. **Result**: The result is stored and retrievable via `GET_RESULT`.

## Code Example

```solidity
function runModel(bytes32 modelHash, bytes32 inputHash) public payable {
    // 1. Calculate cost
    uint256 cost = IPrecompile(0x13).computePayment(modelHash, inputHash);

    // 2. Fund escrow
    escrow.deposit{value: cost}(requestId);

    // 3. Request inference
    IPrecompile(0x10).aiRequest(modelHash, inputHash);
}
```

*For full technical specification, see [Reference](../developers/rpc-reference.md)*

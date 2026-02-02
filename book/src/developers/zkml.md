# Zero-Knowledge Machine Learning (zkML)

ModernTensor native supports **zkML**, allowing miners to prove they ran a specific model on specific data without revealing the model weights or the private data.

We use **ezkl** under the hood for circuit generation and proving.

## Workflow

1. **Model**: Data Scientist trains a PyTorch/TensorFlow model.
2. **ONNX**: Convert model to ONNX format.
3. **Circuit**: Compile ONNX to a ZK Circuit (Verifier Contract).
4. **Proof**: Miner generates a proof `pi` for result `y = f(x)`.
5. **Verify**: Blockchain verifies `pi` via Opcode `0x11` (VERIFY_PROOF).

## Using the SDK for zkML

The SDK abstracts the complexity of `ezkl`.

### 1. Generating a Proof (Miner Side)

```python
from moderntensor.sdk.zkml import Prover

# Load the compiled circuit settings
prover = Prover(
    model_path="model.onnx",
    settings_path="settings.json",
    pk_path="pk.key"
)

# Run inference and generate proof
input_data = [0.1, 0.5, ...]
result, proof = prover.prove(input_data)

print(f"Inference Result: {result}")
print(f"ZK Proof: {proof.hex()[:64]}...")
```

### 2. Verifying a Proof (On-Chain)

You typically do not need to do this manually; the **Validators** or the **Consensus Engine** handles it. However, you can check validity via SDK:

```python
from moderntensor.sdk.zkml import Verifier

verifier = Verifier(vk_path="vk.key")

is_valid = verifier.verify(proof, public_inputs=[result])
print(f"Proof Valid: {is_valid}")
```

## Smart Contract Integration

To verify a proof inside a Solidity contract, use the precompile at `0x11`:

```solidity
// Interface for the ZK Verifier Precompile
interface IZKVerifier {
    function verify(
        bytes calldata proof,
        bytes calldata public_inputs
    ) external view returns (bool);
}

contract MyAIApp {
    function submitResult(bytes calldata proof, bytes calldata result) external {
        // Call Precompile 0x11
        IZKVerifier verifier = IZKVerifier(address(0x11));

        bool success = verifier.verify(proof, result);
        require(success, "Invalid ZK Proof");

        // Logic if proof is valid...
    }
}
```

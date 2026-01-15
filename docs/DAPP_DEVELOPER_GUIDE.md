# dApp Developer Guide for LuxTensor

Build decentralized applications on ModernTensor's AI-native blockchain.

## Quick Start

### 1. Connect to LuxTensor

```javascript
// ethers.js
import { ethers } from "ethers";

const provider = new ethers.JsonRpcProvider("http://localhost:8545");
const wallet = new ethers.Wallet(PRIVATE_KEY, provider);

// Check connection
const blockNumber = await provider.getBlockNumber();
console.log(`Connected! Block: ${blockNumber}`);
```

```python
# web3.py
from web3 import Web3

w3 = Web3(Web3.HTTPProvider("http://localhost:8545"))
print(f"Connected: {w3.is_connected()}")
print(f"Block: {w3.eth.block_number}")
```

### 2. Deploy a Contract

```javascript
// Deploy ERC-20 Token
const TokenFactory = new ethers.ContractFactory(abi, bytecode, wallet);
const token = await TokenFactory.deploy("MyToken", "MTK", 1000000);
await token.waitForDeployment();
console.log(`Deployed at: ${await token.getAddress()}`);
```

### 3. Interact with AI APIs

```javascript
// Submit AI task
const response = await provider.send("mt_submitAITask", [{
    model_hash: "0x1234567890abcdef",
    input_data: "Explain quantum computing",
    requester: wallet.address,
    reward: 100
}]);
console.log(`Task ID: ${response}`);

// Get result
const result = await provider.send("mt_getAIResult", [response]);
console.log(`Status: ${result.status}, Output: ${result.output}`);
```

---

## Network Configuration

### Testnet

| Parameter | Value |
|-----------|-------|
| RPC URL | `http://localhost:8545` |
| Chain ID | `1337` |
| Currency | `MDT` |
| Block Time | ~6 seconds |

### Mainnet (Coming Q2 2026)

| Parameter | Value |
|-----------|-------|
| RPC URL | `https://rpc.luxtensor.io` |
| Chain ID | `777` |
| Currency | `MDT` |
| Explorer | `https://explorer.luxtensor.io` |

---

## Hardhat Setup

### Installation

```bash
mkdir my-luxtensor-dapp
cd my-luxtensor-dapp
npm init -y
npm install --save-dev hardhat @nomicfoundation/hardhat-toolbox
npx hardhat init
```

### hardhat.config.js

```javascript
require("@nomicfoundation/hardhat-toolbox");

module.exports = {
    solidity: "0.8.20",
    networks: {
        luxtensor_local: {
            url: "http://127.0.0.1:8545",
            chainId: 1337,
            accounts: [
                // Test accounts (DO NOT USE IN PRODUCTION)
                "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            ]
        },
        luxtensor_testnet: {
            url: "https://testnet.luxtensor.io",
            chainId: 1337,
            accounts: [process.env.PRIVATE_KEY]
        }
    }
};
```

### Deploy

```bash
npx hardhat run scripts/deploy.js --network luxtensor_local
```

---

## Foundry Setup

### Installation

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

### foundry.toml

```toml
[profile.default]
src = "src"
out = "out"
libs = ["lib"]
solc = "0.8.20"

[rpc_endpoints]
luxtensor_local = "http://127.0.0.1:8545"
luxtensor_testnet = "https://testnet.luxtensor.io"
```

### Deploy

```bash
forge create --rpc-url luxtensor_local src/MyToken.sol:MyToken
```

---

## AI-Specific Features

### AI Oracle Contract

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IAIOracle {
    function requestAI(
        bytes32 modelHash,
        bytes calldata input
    ) external payable returns (bytes32 requestId);

    function getResult(bytes32 requestId)
        external view returns (bytes memory output, bool completed);
}

contract AIConsumer {
    IAIOracle public oracle;

    constructor(address _oracle) {
        oracle = IAIOracle(_oracle);
    }

    function askAI(bytes32 modelHash, bytes calldata question)
        external payable returns (bytes32)
    {
        return oracle.requestAI{value: msg.value}(modelHash, question);
    }
}
```

### zkML Proof Verification

```solidity
// Verify miner actually ran the model
interface IZkMLVerifier {
    function verify(
        bytes calldata proof,
        bytes calldata publicInputs,
        bytes calldata publicOutputs
    ) external view returns (bool);
}

contract VerifiedAI {
    IZkMLVerifier public verifier;

    function verifyAndUse(
        bytes calldata proof,
        bytes calldata inputs,
        bytes calldata outputs
    ) external view returns (bool) {
        return verifier.verify(proof, inputs, outputs);
    }
}
```

---

## RPC Methods Reference

### Standard Ethereum (eth_*)

| Method | Description |
|--------|-------------|
| `eth_blockNumber` | Current block height |
| `eth_getBalance` | Account balance |
| `eth_sendRawTransaction` | Submit signed tx |
| `eth_getTransactionReceipt` | Tx status |
| `eth_call` | Read-only contract call |
| `eth_estimateGas` | Gas estimation |

### ModernTensor AI (mt_*)

| Method | Description |
|--------|-------------|
| `mt_submitAITask` | Submit AI computation task |
| `mt_getAIResult` | Get task result |
| `mt_getActiveModels` | List available AI models |
| `mt_estimateTaskCost` | Estimate MDT cost |

### Staking (staking_*)

| Method | Description |
|--------|-------------|
| `staking_getValidators` | List validators |
| `staking_addStake` | Stake MDT |
| `staking_claimRewards` | Claim rewards |

---

## Example dApps

### 1. AI-Powered Prediction Market

```solidity
contract AIPrediction {
    struct Market {
        string question;
        bytes32 aiModelHash;
        uint256 deadline;
        uint256 yesPool;
        uint256 noPool;
    }

    mapping(uint256 => Market) public markets;

    function createMarket(string calldata question, bytes32 model) external {
        // Create market that resolves via AI
    }

    function resolveWithAI(uint256 marketId) external {
        // Call AI oracle to resolve
    }
}
```

### 2. AI-Generated NFT

```solidity
contract AINft is ERC721 {
    function mint(string calldata prompt) external payable {
        // Submit AI task for image generation
        // Mint NFT with result as metadata
    }
}
```

### 3. Decentralized AI Inference

```solidity
contract InferenceMarket {
    function requestInference(
        bytes32 modelHash,
        bytes calldata input,
        uint256 reward
    ) external payable {
        // Post inference request
        // Miners compete to provide best result
    }
}
```

---

## Best Practices

### Security

1. **Validate AI outputs** - Don't blindly trust AI results
2. **Use zkML proofs** - Verify computation was done correctly
3. **Set timeouts** - AI tasks may take time
4. **Handle failures** - Have fallback logic

### Gas Optimization

1. **Batch AI requests** - Reduce overhead
2. **Use events** - Cheaper than storage
3. **Off-chain computation** - Move complex logic off-chain

### Testing

```javascript
describe("AI Integration", function() {
    it("should submit AI task", async function() {
        const tx = await contract.submitTask("prompt");
        const receipt = await tx.wait();
        expect(receipt.status).to.equal(1);
    });
});
```

---

## Resources

- **Faucet**: Get testnet MDT at `/faucet`
- **Explorer**: View transactions at `/explorer`
- **Discord**: Community support
- **GitHub**: [github.com/moderntensor](https://github.com/moderntensor)

---

## Support

Having issues? Check:

1. **RPC is running** - `curl http://localhost:8545`
2. **Correct Chain ID** - 1337 (testnet) or 777 (mainnet)
3. **Enough MDT** - Get from faucet

---

*ModernTensor Foundation - Building the AI-Native Blockchain*

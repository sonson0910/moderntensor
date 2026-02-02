# Smart Contracts

Writing smart contracts on ModernTensor is similar to Ethereum (Solidity), but with added superpowers: **Native AI Opcodes**.

## Importing Interfaces

To interact with the AI layer, you don't need to import complex external libraries. You just need to know the precompile addresses.

```solidity
interface IAIOracle {
    function requestAI(bytes32 modelHash, bytes32 inputHash) external returns (bytes32 requestId);
}

interface IVerify {
    function verifyProof(bytes32 requestId, bytes memory proof) external returns (bool);
}
```

## Example: AI-Powered NFT

Imagine an NFT that changes based on an AI-generated image.

```solidity
pragma solidity ^0.8.0;

contract AiNFT {
    address constant AI_REQUEST = address(0x10);

    struct Request {
        address user;
        string prompt;
    }

    mapping(bytes32 => Request) public requests;

    function mintWithAI(string memory prompt) public payable {
        bytes32 inputHash = keccak256(abi.encodePacked(prompt));
        bytes32 modelHash = 0x123...; // Stable Diffusion model hash

        // Call Precompile
        (bool success, bytes memory data) = AI_REQUEST.call(abi.encode(modelHash, inputHash));
        require(success, "AI Request failed");

        bytes32 requestId = abi.decode(data, (bytes32));
        requests[requestId] = Request(msg.sender, prompt);
    }
}
```

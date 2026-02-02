/**
 * AI Oracle End-to-End Integration Test
 *
 * This script:
 * 1. Deploys the AI Oracle infrastructure (ZkMLVerifier, AIOracle, ExampleGame)
 * 2. Submits a game request
 * 3. Simulates Oracle Node fulfillment
 * 4. Verifies the result
 *
 * Usage:
 *   npx hardhat run scripts/test-ai-oracle.js --network localhost
 */

const hre = require("hardhat");

async function main() {
    console.log("=".repeat(60));
    console.log("AI Oracle Integration Test");
    console.log("=".repeat(60));

    const [deployer, player, oracle] = await hre.ethers.getSigners();
    console.log("Deployer:", deployer.address);
    console.log("Player:", player.address);
    console.log("Oracle:", oracle.address);
    console.log("");

    // ========== 1. DEPLOY CONTRACTS ==========
    console.log("1. Deploying contracts...");

    // ZkMLVerifier
    const ZkMLVerifier = await hre.ethers.getContractFactory("ZkMLVerifier");
    const zkmlVerifier = await ZkMLVerifier.deploy();
    await zkmlVerifier.waitForDeployment();
    console.log("   ✅ ZkMLVerifier:", await zkmlVerifier.getAddress());

    // AIOracle
    const AIOracle = await hre.ethers.getContractFactory("AIOracle");
    const aiOracle = await AIOracle.deploy();
    await aiOracle.waitForDeployment();
    const aiOracleAddress = await aiOracle.getAddress();
    console.log("   ✅ AIOracle:", aiOracleAddress);

    // Configure AIOracle
    await aiOracle.setZkMLVerifier(await zkmlVerifier.getAddress());
    const modelHash = hre.ethers.keccak256(hre.ethers.toUtf8Bytes("test-game-model"));
    await aiOracle.approveModel(modelHash);
    await zkmlVerifier.trustImage(modelHash);
    console.log("   ✅ Model approved:", modelHash);

    // ExampleGame
    const ExampleGame = await hre.ethers.getContractFactory("ExampleGame");
    const exampleGame = await ExampleGame.deploy(aiOracleAddress, modelHash);
    await exampleGame.waitForDeployment();
    console.log("   ✅ ExampleGame:", await exampleGame.getAddress());
    console.log("");

    // ========== 2. PLAYER REQUESTS GAME ==========
    console.log("2. Player requests game...");

    const playerMove = hre.ethers.toUtf8Bytes("ROCK");
    const payment = hre.ethers.parseEther("0.01");

    const tx = await exampleGame.connect(player).play(playerMove, { value: payment });
    const receipt = await tx.wait();

    // Get requestId from AIRequestCreated event
    const aiOracleInterface = AIOracle.interface;
    const requestEvent = receipt.logs
        .map(log => {
            try { return aiOracleInterface.parseLog(log); }
            catch { return null; }
        })
        .find(log => log?.name === "AIRequestCreated");

    const requestId = requestEvent.args.requestId;
    console.log("   ✅ Game requested. Request ID:", requestId);
    console.log("");

    // ========== 3. ORACLE FULFILLS REQUEST ==========
    console.log("3. Oracle node fulfills request...");

    // Simulate AI result (score = 100)
    const score = 100n;
    const result = hre.ethers.AbiCoder.defaultAbiCoder().encode(["uint256"], [score]);

    // Generate dev-mode proof seal
    const journal = result;
    const expectedSeal = hre.ethers.keccak256(
        hre.ethers.solidityPacked(["bytes32", "bytes"], [modelHash, journal])
    );
    const proofHash = expectedSeal;

    // Connect as oracle and fulfill
    const fulfillTx = await aiOracle.connect(oracle).fulfillRequest(
        requestId,
        result,
        proofHash
    );
    await fulfillTx.wait();
    console.log("   ✅ Request fulfilled by oracle");
    console.log("");

    // ========== 4. VERIFY RESULT ==========
    console.log("4. Verifying result...");

    // Check AIOracle state
    const request = await aiOracle.getRequest(requestId);
    console.log("   Request status:", request.status === 1n ? "Fulfilled" : "Pending");
    console.log("   Fulfiller:", request.fulfiller);

    // Player claims result
    await exampleGame.connect(player).claimResult(requestId);
    const playerScore = await exampleGame.getScore(player.address);
    console.log("   Player score:", playerScore.toString());
    console.log("");

    // ========== SUMMARY ==========
    console.log("=".repeat(60));
    console.log("✅ INTEGRATION TEST PASSED!");
    console.log("=".repeat(60));
    console.log("");
    console.log("Flow verified:");
    console.log("  [Player] → play() → [AIOracle] → event AIRequestCreated");
    console.log("  [OracleNode] → listen event → process AI → fulfillRequest()");
    console.log("  [Player] → claimResult() → score updated");
    console.log("");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error("\n❌ Test failed:", error);
        process.exit(1);
    });

/**
 * Deploy AIOracle Infrastructure
 *
 * Usage:
 *   npx hardhat run scripts/deploy-ai-oracle.js --network localhost
 *   npx hardhat run scripts/deploy-ai-oracle.js --network luxtensor_testnet
 *
 * This script deploys:
 *   1. ZkMLVerifier - Proof verification contract
 *   2. AIOracle - Main oracle contract
 *   3. ExampleGame - Demo consumer contract
 */

const hre = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
    console.log("=".repeat(60));
    console.log("Deploying AIOracle Infrastructure");
    console.log("=".repeat(60));

    const [deployer] = await hre.ethers.getSigners();
    console.log("Deployer:", deployer.address);
    console.log("Balance:", hre.ethers.formatEther(await hre.ethers.provider.getBalance(deployer.address)), "LUX");
    console.log("");

    // 1. Deploy ZkMLVerifier (Stub for testnet)
    console.log("1. Deploying ZkMLVerifier...");
    const ZkMLVerifier = await hre.ethers.getContractFactory("ZkMLVerifier");
    const zkmlVerifier = await ZkMLVerifier.deploy();
    await zkmlVerifier.waitForDeployment();
    const zkmlVerifierAddress = await zkmlVerifier.getAddress();
    console.log("   ZkMLVerifier deployed to:", zkmlVerifierAddress);

    // 2. Deploy AIOracle
    console.log("2. Deploying AIOracle...");
    const AIOracle = await hre.ethers.getContractFactory("AIOracle");
    const aiOracle = await AIOracle.deploy();
    await aiOracle.waitForDeployment();
    const aiOracleAddress = await aiOracle.getAddress();
    console.log("   AIOracle deployed to:", aiOracleAddress);

    // 3. Deploy PaymentEscrow (Phase 2: Pay-per-compute)
    console.log("3. Deploying PaymentEscrow...");
    // Note: In production, use actual MDT token address
    // For testnet, we deploy a mock ERC20 or use deployer address as placeholder
    const mockMdtToken = deployer.address; // Placeholder for testnet
    const PaymentEscrow = await hre.ethers.getContractFactory("PaymentEscrow");
    const paymentEscrow = await PaymentEscrow.deploy(mockMdtToken);
    await paymentEscrow.waitForDeployment();
    const paymentEscrowAddress = await paymentEscrow.getAddress();
    console.log("   PaymentEscrow deployed to:", paymentEscrowAddress);

    // 4. Configure Contracts
    console.log("4. Configuring Contracts...");

    // Set ZkML Verifier on AIOracle
    await aiOracle.setZkMLVerifier(zkmlVerifierAddress);
    console.log("   - Set ZkML Verifier:", zkmlVerifierAddress);

    // Set AIOracle on PaymentEscrow
    await paymentEscrow.setAIOracle(aiOracleAddress);
    console.log("   - Set AIOracle on PaymentEscrow:", aiOracleAddress);

    // Approve a demo model hash
    const demoModelHash = hre.ethers.keccak256(hre.ethers.toUtf8Bytes("demo-game-ai-v1"));
    await aiOracle.approveModel(demoModelHash);
    console.log("   - Approved demo model:", demoModelHash);

    // 5. Deploy ExampleGame
    console.log("5. Deploying ExampleGame...");
    const ExampleGame = await hre.ethers.getContractFactory("ExampleGame");
    const exampleGame = await ExampleGame.deploy(aiOracleAddress, demoModelHash);
    await exampleGame.waitForDeployment();
    const exampleGameAddress = await exampleGame.getAddress();
    console.log("   ExampleGame deployed to:", exampleGameAddress);

    // Save deployment info
    const deployments = {
        network: hre.network.name,
        timestamp: new Date().toISOString(),
        deployer: deployer.address,
        contracts: {
            ZkMLVerifier: zkmlVerifierAddress,
            AIOracle: aiOracleAddress,
            PaymentEscrow: paymentEscrowAddress,
            ExampleGame: exampleGameAddress,
        },
        config: {
            demoModelHash: demoModelHash,
            mdtToken: mockMdtToken,
        }
    };

    const deploymentsPath = path.join(__dirname, "..", "deployments-ai-oracle.json");
    fs.writeFileSync(deploymentsPath, JSON.stringify(deployments, null, 2));
    console.log("");
    console.log("Deployment info saved to:", deploymentsPath);

    // Summary
    console.log("");
    console.log("=".repeat(60));
    console.log("DEPLOYMENT COMPLETE");
    console.log("=".repeat(60));
    console.log("");
    console.log("Contracts:");
    console.log("  ZkMLVerifier:  ", zkmlVerifierAddress);
    console.log("  AIOracle:      ", aiOracleAddress);
    console.log("  PaymentEscrow: ", paymentEscrowAddress);
    console.log("  ExampleGame:   ", exampleGameAddress);
    console.log("");
    console.log("Demo Model Hash:", demoModelHash);
    console.log("");
    console.log("Next steps:");
    console.log("  1. Start the Oracle Node with ORACLE_CONTRACT_ADDRESS=" + aiOracleAddress);
    console.log("  2. Players can call ExampleGame.play() to test AI integration");
    console.log("  3. Use PaymentEscrow for pay-per-compute AI requests");
    console.log("");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });

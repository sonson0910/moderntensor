/**
 * Deploy MDT Token Contracts
 *
 * Deploys:
 * 1. MDTToken - Main ERC20 token
 * 2. MDTVesting - Vesting contract for locked tokens
 */

const hre = require("hardhat");

async function main() {
    const [deployer] = await hre.ethers.getSigners();

    console.log("=".repeat(60));
    console.log("🚀 ModernTensor Token Deployment");
    console.log("=".repeat(60));
    console.log("Deployer:", deployer.address);
    console.log("Balance:", hre.ethers.formatEther(await deployer.provider.getBalance(deployer.address)), "LUX");
    console.log("");

    // Deploy MDTToken
    console.log("📝 Deploying MDTToken...");
    const MDTToken = await hre.ethers.getContractFactory("MDTToken");
    const token = await MDTToken.deploy();
    await token.waitForDeployment();
    const tokenAddress = await token.getAddress();
    console.log("✅ MDTToken deployed:", tokenAddress);

    // Deploy MDTVesting
    console.log("📝 Deploying MDTVesting...");
    const MDTVesting = await hre.ethers.getContractFactory("MDTVesting");
    const vesting = await MDTVesting.deploy(tokenAddress);
    await vesting.waitForDeployment();
    const vestingAddress = await vesting.getAddress();
    console.log("✅ MDTVesting deployed:", vestingAddress);

    // Get allocations
    const allocations = await token.getAllocations();
    console.log("\n📊 Token Allocations:");
    console.log("  Emission Rewards:", hre.ethers.formatEther(allocations[0]), "MDT (45%)");
    console.log("  Ecosystem Grants:", hre.ethers.formatEther(allocations[1]), "MDT (12%)");
    console.log("  Team & Core Dev:", hre.ethers.formatEther(allocations[2]), "MDT (10%)");
    console.log("  Private Sale:", hre.ethers.formatEther(allocations[3]), "MDT (8%)");
    console.log("  IDO:", hre.ethers.formatEther(allocations[4]), "MDT (5%)");
    console.log("  DAO Treasury:", hre.ethers.formatEther(allocations[5]), "MDT (10%)");
    console.log("  Initial Liquidity:", hre.ethers.formatEther(allocations[6]), "MDT (5%)");
    console.log("  Foundation Reserve:", hre.ethers.formatEther(allocations[7]), "MDT (5%)");

    console.log("\n" + "=".repeat(60));
    console.log("📋 Deployment Summary");
    console.log("=".repeat(60));
    console.log("MDTToken:", tokenAddress);
    console.log("MDTVesting:", vestingAddress);
    console.log("\n⚠️  NEXT STEPS:");
    console.log("1. Run execute-tge.js to mint tokens");
    console.log("2. Verify contracts");
    console.log("3. finishMinting() to lock forever");

    // Save deployment info
    const fs = require("fs");
    const deploymentInfo = {
        network: hre.network.name,
        deployer: deployer.address,
        timestamp: new Date().toISOString(),
        contracts: {
            MDTToken: tokenAddress,
            MDTVesting: vestingAddress
        }
    };

    fs.writeFileSync(
        "deployments.json",
        JSON.stringify(deploymentInfo, null, 2)
    );
    console.log("\n✅ Deployment info saved to deployments.json");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });

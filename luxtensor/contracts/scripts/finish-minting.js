/**
 * Finish Minting - PERMANENTLY LOCK TOKEN SUPPLY
 *
 * ⚠️  WARNING: THIS IS IRREVERSIBLE!
 * After running this script:
 * - No more tokens can EVER be minted
 * - Contract ownership is renounced
 * - Total supply is locked at 21,000,000 MDT forever
 */

const hre = require("hardhat");
const fs = require("fs");
const readline = require("readline");

async function main() {
    const [deployer] = await hre.ethers.getSigners();

    console.log("=".repeat(60));
    console.log("🔒 PERMANENT MINTING LOCK");
    console.log("=".repeat(60));
    console.log("");
    console.log("⚠️  WARNING: THIS ACTION IS IRREVERSIBLE!");
    console.log("");

    // Load deployment info
    if (!fs.existsSync("deployments.json")) {
        console.error("❌ deployments.json not found. Run deploy.js first!");
        process.exit(1);
    }

    const deployment = JSON.parse(fs.readFileSync("deployments.json"));
    const token = await hre.ethers.getContractAt("MDTToken", deployment.contracts.MDTToken);

    // Check current state
    const mintingFinished = await token.mintingFinished();
    if (mintingFinished) {
        console.log("✅ Minting is already finished. Contract is locked.");
        process.exit(0);
    }

    const allMinted = await token.allCategoriesMinted();
    if (!allMinted) {
        console.error("❌ Not all categories are minted! Run execute-tge.js first.");
        process.exit(1);
    }

    const totalSupply = await token.totalSupply();
    const maxSupply = await token.MAX_SUPPLY();

    console.log("📊 Current State:");
    console.log("  Total Supply:", hre.ethers.formatEther(totalSupply), "MDT");
    console.log("  Max Supply:", hre.ethers.formatEther(maxSupply), "MDT");
    console.log("  All Categories Minted:", allMinted);
    console.log("  Owner:", await token.owner());
    console.log("");

    if (totalSupply !== maxSupply) {
        console.error("❌ Supply mismatch! Cannot lock.");
        process.exit(1);
    }

    // Confirm with user
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    const answer = await new Promise((resolve) => {
        rl.question("Type 'LOCK FOREVER' to confirm: ", resolve);
    });
    rl.close();

    if (answer !== "LOCK FOREVER") {
        console.log("❌ Aborted. Minting NOT locked.");
        process.exit(1);
    }

    console.log("\n🔐 Executing finishMinting()...");
    const tx = await token.finishMinting();
    await tx.wait();

    console.log("\n" + "=".repeat(60));
    console.log("✅ MINTING PERMANENTLY LOCKED");
    console.log("=".repeat(60));
    console.log("Transaction:", tx.hash);
    console.log("Total Supply:", hre.ethers.formatEther(totalSupply), "MDT (locked forever)");
    console.log("Owner:", await token.owner(), "(should be 0x0)");
    console.log("");
    console.log("🎉 Token Generation Event complete!");
    console.log("   No more MDT can ever be minted.");
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });

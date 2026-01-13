/**
 * Execute TGE - Token Generation Event
 *
 * Mints all token allocations and optionally locks minting forever
 */

const hre = require("hardhat");
const fs = require("fs");

// Allocation addresses (CONFIGURE THESE BEFORE RUNNING!)
const ADDRESSES = {
    // Emission rewards pool - controlled by consensus layer
    emissionPool: process.env.EMISSION_POOL || "0x0000000000000000000000000000000000000001",

    // Ecosystem grants - DAO controlled
    ecosystemGrants: process.env.ECOSYSTEM_GRANTS || "0x0000000000000000000000000000000000000002",

    // Team & Core Dev - MUST use VestingContract
    teamVesting: null, // Will be set to VestingContract address

    // Private Sale - MUST use VestingContract
    privateSaleVesting: null, // Will be set to VestingContract address

    // IDO - MUST use VestingContract
    idoVesting: null, // Will be set to VestingContract address

    // DAO Treasury - Multi-sig
    daoTreasury: process.env.DAO_TREASURY || "0x0000000000000000000000000000000000000003",

    // Initial Liquidity - DEX router or pool
    liquidityPool: process.env.LIQUIDITY_POOL || "0x0000000000000000000000000000000000000004",

    // Foundation Reserve - Multi-sig
    foundationReserve: process.env.FOUNDATION_RESERVE || "0x0000000000000000000000000000000000000005"
};

async function main() {
    const [deployer] = await hre.ethers.getSigners();

    console.log("=".repeat(60));
    console.log("🎉 ModernTensor TGE Execution");
    console.log("=".repeat(60));

    // Load deployment info
    if (!fs.existsSync("deployments.json")) {
        console.error("❌ deployments.json not found. Run deploy.js first!");
        process.exit(1);
    }

    const deployment = JSON.parse(fs.readFileSync("deployments.json"));
    console.log("📦 Loading contracts from deployment...");

    const token = await hre.ethers.getContractAt("MDTToken", deployment.contracts.MDTToken);
    const vesting = await hre.ethers.getContractAt("MDTVesting", deployment.contracts.MDTVesting);

    const vestingAddress = deployment.contracts.MDTVesting;

    // Update addresses to use vesting contract
    ADDRESSES.teamVesting = vestingAddress;
    ADDRESSES.privateSaleVesting = vestingAddress;
    ADDRESSES.idoVesting = vestingAddress;

    console.log("MDTToken:", deployment.contracts.MDTToken);
    console.log("MDTVesting:", vestingAddress);

    // Check current state
    const mintingFinished = await token.mintingFinished();
    if (mintingFinished) {
        console.error("❌ Minting already finished!");
        process.exit(1);
    }

    const currentSupply = await token.totalSupply();
    console.log("\n📊 Current Supply:", hre.ethers.formatEther(currentSupply), "MDT");

    // Set TGE timestamp on vesting contract
    const tgeTimestamp = await token.tgeTimestamp();
    if (tgeTimestamp == 0n) {
        console.log("\n⏰ Setting TGE timestamp on vesting contract...");
        const now = Math.floor(Date.now() / 1000);
        await vesting.setTGE(now);
        console.log("✅ TGE timestamp set:", new Date(now * 1000).toISOString());
    }

    console.log("\n🚀 Executing TGE mints...\n");

    // Category enum values (must match contract)
    const Category = {
        EmissionRewards: 0,
        EcosystemGrants: 1,
        TeamCoreDev: 2,
        PrivateSale: 3,
        IDO: 4,
        DaoTreasury: 5,
        InitialLiquidity: 6,
        FoundationReserve: 7
    };

    // Execute TGE for each category
    const mints = [
        { category: Category.EmissionRewards, to: ADDRESSES.emissionPool, name: "Emission Rewards" },
        { category: Category.EcosystemGrants, to: ADDRESSES.ecosystemGrants, name: "Ecosystem Grants" },
        { category: Category.TeamCoreDev, to: ADDRESSES.teamVesting, name: "Team & Core Dev" },
        { category: Category.PrivateSale, to: ADDRESSES.privateSaleVesting, name: "Private Sale" },
        { category: Category.IDO, to: ADDRESSES.idoVesting, name: "IDO" },
        { category: Category.DaoTreasury, to: ADDRESSES.daoTreasury, name: "DAO Treasury" },
        { category: Category.InitialLiquidity, to: ADDRESSES.liquidityPool, name: "Initial Liquidity" },
        { category: Category.FoundationReserve, to: ADDRESSES.foundationReserve, name: "Foundation Reserve" }
    ];

    for (const mint of mints) {
        const remaining = await token.remainingAllocation(mint.category);
        if (remaining > 0n) {
            console.log(`📝 Minting ${mint.name} to ${mint.to}...`);
            const tx = await token.executeTGE(mint.category, mint.to);
            await tx.wait();
            console.log(`✅ ${mint.name}: ${hre.ethers.formatEther(remaining)} MDT`);
        } else {
            console.log(`⏭️  ${mint.name}: Already minted`);
        }
    }

    // Check final supply
    const finalSupply = await token.totalSupply();
    console.log("\n📊 Final Supply:", hre.ethers.formatEther(finalSupply), "MDT");

    // Check if we can finish minting
    const allMinted = await token.allCategoriesMinted();
    console.log("All categories minted:", allMinted);

    if (allMinted) {
        console.log("\n⚠️  ALL TOKENS MINTED!");
        console.log("To permanently lock minting, run:");
        console.log("  npx hardhat run scripts/finish-minting.js --network <network>");
    }

    console.log("\n" + "=".repeat(60));
    console.log("✅ TGE EXECUTION COMPLETE");
    console.log("=".repeat(60));
}

main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });

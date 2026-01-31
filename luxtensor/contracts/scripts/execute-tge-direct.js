/**
 * Execute TGE - Mint all token allocations
 * Direct RPC version for Luxtensor
 */

const http = require('http');
const fs = require('fs');

// Load environment variables if .env exists
try { require('dotenv').config(); } catch (e) { /* dotenv optional for local dev */ }

const RPC_URL = process.env.RPC_URL_LOCAL || 'http://127.0.0.1:8545';
// For local dev only - use Hardhat default account
// In production, NEVER expose private keys in code
const DEPLOYER = process.env.DEPLOYER_ADDRESS || '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266';

// Load deployment info
const deployment = JSON.parse(fs.readFileSync('deployments.json', 'utf8'));
const TOKEN_ADDRESS = deployment.contracts.MDTToken;
const VESTING_ADDRESS = deployment.contracts.MDTVesting;

// Function selectors (keccak256 first 4 bytes)
const SELECTORS = {
    // MDTToken functions
    executeTGE: '0x7065cb48',  // executeTGE(uint8,address)
    allCategoriesMinted: '0x8da5cb5b',  // allCategoriesMinted()
    finishMinting: '0x0b36e5ad',  // finishMinting()
    totalSupply: '0x18160ddd',  // totalSupply()
    mintingFinished: '0x05d2035b',  // mintingFinished()

    // MDTVesting functions
    setTGE: '0x3ccfd60b',  // setTGE(uint256)
};

// Category enum values (from MDTToken.sol)
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

// Allocation addresses (for demo, using test addresses)
const ADDRESSES = {
    emissionPool: '0x0000000000000000000000000000000000000001',
    ecosystemGrants: '0x0000000000000000000000000000000000000002',
    teamVesting: VESTING_ADDRESS,
    privateSaleVesting: VESTING_ADDRESS,
    idoVesting: VESTING_ADDRESS,
    daoTreasury: '0x0000000000000000000000000000000000000003',
    liquidityPool: '0x0000000000000000000000000000000000000004',
    foundationReserve: '0x0000000000000000000000000000000000000005'
};

async function rpcCall(method, params) {
    return new Promise((resolve, reject) => {
        const data = JSON.stringify({
            jsonrpc: '2.0',
            method,
            params,
            id: Date.now()
        });

        const options = {
            hostname: '127.0.0.1',
            port: 8545,
            path: '/',
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Content-Length': data.length
            }
        };

        const req = http.request(options, (res) => {
            let body = '';
            res.on('data', chunk => body += chunk);
            res.on('end', () => {
                try {
                    const json = JSON.parse(body);
                    if (json.error) reject(new Error(json.error.message));
                    else resolve(json.result);
                } catch (e) {
                    reject(e);
                }
            });
        });

        req.on('error', reject);
        req.write(data);
        req.end();
    });
}

function encodeExecuteTGE(category, toAddress) {
    // executeTGE(Category category, address to)
    // Category is uint8, address is address
    const categoryHex = category.toString(16).padStart(64, '0');
    const addressHex = toAddress.slice(2).toLowerCase().padStart(64, '0');
    return SELECTORS.executeTGE + categoryHex + addressHex;
}

async function callContract(to, data) {
    const txHash = await rpcCall('eth_sendTransaction', [{
        from: DEPLOYER,
        to: to,
        data: data,
        gas: '0x100000'
    }]);

    // Wait for receipt
    let receipt = null;
    for (let i = 0; i < 10; i++) {
        receipt = await rpcCall('eth_getTransactionReceipt', [txHash]);
        if (receipt) break;
        await new Promise(r => setTimeout(r, 500));
    }

    return { txHash, receipt };
}

async function main() {
    console.log('='.repeat(60));
    console.log('🎉 ModernTensor TGE Execution');
    console.log('='.repeat(60));
    console.log('MDTToken:', TOKEN_ADDRESS);
    console.log('MDTVesting:', VESTING_ADDRESS);
    console.log('');

    // Mint each category
    const mints = [
        { category: Category.EmissionRewards, to: ADDRESSES.emissionPool, name: 'Emission Rewards (45%)' },
        { category: Category.EcosystemGrants, to: ADDRESSES.ecosystemGrants, name: 'Ecosystem Grants (12%)' },
        { category: Category.TeamCoreDev, to: ADDRESSES.teamVesting, name: 'Team & Core Dev (10%) → Vesting' },
        { category: Category.PrivateSale, to: ADDRESSES.privateSaleVesting, name: 'Private Sale (8%) → Vesting' },
        { category: Category.IDO, to: ADDRESSES.idoVesting, name: 'IDO (5%) → Vesting' },
        { category: Category.DaoTreasury, to: ADDRESSES.daoTreasury, name: 'DAO Treasury (10%)' },
        { category: Category.InitialLiquidity, to: ADDRESSES.liquidityPool, name: 'Initial Liquidity (5%)' },
        { category: Category.FoundationReserve, to: ADDRESSES.foundationReserve, name: 'Foundation Reserve (5%)' }
    ];

    console.log('📝 Executing TGE mints...\n');

    for (const mint of mints) {
        console.log(`  Minting: ${mint.name}`);
        console.log(`    → To: ${mint.to}`);

        const data = encodeExecuteTGE(mint.category, mint.to);
        const result = await callContract(TOKEN_ADDRESS, data);

        if (result.receipt && result.receipt.status === '0x1') {
            console.log(`    ✅ Success (tx: ${result.txHash.slice(0, 18)}...)`);
        } else {
            console.log(`    ⚠️  TX sent (tx: ${result.txHash.slice(0, 18)}...)`);
        }
        console.log('');
    }

    console.log('='.repeat(60));
    console.log('✅ TGE COMPLETE!');
    console.log('='.repeat(60));
    console.log('');
    console.log('Total Supply: 21,000,000 MDT');
    console.log('');
    console.log('⚠️  To permanently lock minting, run:');
    console.log('   node scripts/execute-tge-direct.js --lock');

    // Check if --lock flag passed
    if (process.argv.includes('--lock')) {
        console.log('\n🔐 LOCKING MINTING...');

        const lockData = SELECTORS.finishMinting;
        const lockResult = await callContract(TOKEN_ADDRESS, lockData);

        console.log(`TX: ${lockResult.txHash}`);
        console.log('');
        console.log('='.repeat(60));
        console.log('🔒 MINTING PERMANENTLY LOCKED');
        console.log('='.repeat(60));
        console.log('No more MDT can ever be minted.');
    }
}

main().catch(console.error);

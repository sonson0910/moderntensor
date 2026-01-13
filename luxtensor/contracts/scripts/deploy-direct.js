/**
 * Direct Deploy via RPC
 * Bypasses Hardhat signing - uses Luxtensor's pre-funded unlocked accounts
 */

const http = require('http');
const fs = require('fs');

const RPC_URL = 'http://127.0.0.1:8545';
const DEPLOYER = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266';

// Load compiled contract bytecode
const MDTTokenArtifact = require('../artifacts/src/MDTToken.sol/MDTToken.json');
const MDTVestingArtifact = require('../artifacts/src/MDTVesting.sol/MDTVesting.json');

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

async function deployContract(bytecode, constructorArgs = '') {
    const data = bytecode + constructorArgs;

    const txHash = await rpcCall('eth_sendTransaction', [{
        from: DEPLOYER,
        data: data,
        gas: '0x1000000' // 16M gas
    }]);

    console.log('  TX Hash:', txHash);

    // Wait for receipt
    let receipt = null;
    for (let i = 0; i < 10; i++) {
        receipt = await rpcCall('eth_getTransactionReceipt', [txHash]);
        if (receipt) break;
        await new Promise(r => setTimeout(r, 1000));
    }

    if (!receipt) {
        throw new Error('Transaction not mined');
    }

    return receipt.contractAddress;
}

async function main() {
    console.log('='.repeat(60));
    console.log('🚀 ModernTensor Direct Deploy (Luxtensor)');
    console.log('='.repeat(60));

    // Check balance
    const balance = await rpcCall('eth_getBalance', [DEPLOYER, 'latest']);
    const balanceLux = parseInt(balance, 16) / 1e18;
    console.log('Deployer:', DEPLOYER);
    console.log('Balance:', balanceLux.toFixed(2), 'LUX');

    // Deploy MDTToken
    console.log('\n📝 Deploying MDTToken...');
    const tokenAddress = await deployContract(MDTTokenArtifact.bytecode);
    console.log('✅ MDTToken:', tokenAddress);

    // Encode constructor args (address _token)
    const tokenAddressArg = tokenAddress.slice(2).padStart(64, '0');

    // Deploy MDTVesting
    console.log('\n📝 Deploying MDTVesting...');
    const vestingAddress = await deployContract(
        MDTVestingArtifact.bytecode,
        tokenAddressArg
    );
    console.log('✅ MDTVesting:', vestingAddress);

    // Save deployment info
    const deploymentInfo = {
        network: 'luxtensor_local',
        chainId: 1337,
        deployer: DEPLOYER,
        timestamp: new Date().toISOString(),
        contracts: {
            MDTToken: tokenAddress,
            MDTVesting: vestingAddress
        }
    };

    fs.writeFileSync('deployments.json', JSON.stringify(deploymentInfo, null, 2));

    console.log('\n' + '='.repeat(60));
    console.log('📋 Deployment Complete!');
    console.log('='.repeat(60));
    console.log('MDTToken:', tokenAddress);
    console.log('MDTVesting:', vestingAddress);
    console.log('\nSaved to deployments.json');
}

main().catch(console.error);

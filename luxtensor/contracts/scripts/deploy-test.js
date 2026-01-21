/**
 * Test Deploy via RPC
 * Simple MDTToken deployment with longer timeout
 */

const http = require('http');
const fs = require('fs');

const RPC_URL = 'http://127.0.0.1:8545';
const DEPLOYER = '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266';

// Load compiled contract bytecode
const MDTTokenArtifact = require('../artifacts/src/MDTToken.sol/MDTToken.json');

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

async function main() {
    console.log('='.repeat(60));
    console.log('🚀 ModernTensor Test Deploy (Luxtensor)');
    console.log('='.repeat(60));

    // Check balance
    const balance = await rpcCall('eth_getBalance', [DEPLOYER, 'latest']);
    const balanceLux = parseInt(balance, 16) / 1e18;
    console.log('Deployer:', DEPLOYER);
    console.log('Balance:', balanceLux.toFixed(2), 'LUX');

    // Check current block
    const currentBlock = await rpcCall('eth_blockNumber', []);
    console.log('Current Block:', parseInt(currentBlock, 16));

    // Send deployment transaction
    console.log('\n📝 Deploying MDTToken...');
    const bytecode = MDTTokenArtifact.bytecode;
    console.log('Bytecode length:', bytecode.length);

    const txHash = await rpcCall('eth_sendTransaction', [{
        from: DEPLOYER,
        data: bytecode,
        gas: '0x1000000' // 16M gas
    }]);
    console.log('TX Hash:', txHash);

    // Wait for receipt with longer timeout
    console.log('Waiting for transaction receipt...');
    let receipt = null;
    for (let i = 0; i < 30; i++) {
        await new Promise(r => setTimeout(r, 2000)); // 2 seconds per attempt
        receipt = await rpcCall('eth_getTransactionReceipt', [txHash]);
        if (receipt) {
            console.log('✅ Transaction mined in attempt', i + 1);
            break;
        }
        console.log(`  Attempt ${i + 1}/30...`);
    }

    if (!receipt) {
        console.log('⚠️ Transaction still pending after timeout');
        console.log('TX Hash for manual check:', txHash);
        return;
    }

    console.log('\n✅ MDTToken deployed at:', receipt.contractAddress);
    console.log('Block Number:', parseInt(receipt.blockNumber, 16));
    console.log('Gas Used:', parseInt(receipt.gasUsed, 16));

    // Test: Call balanceOf on the deployed contract
    console.log('\n📊 Testing token contract...');
    const balanceOfData = '0x70a08231000000000000000000000000' + DEPLOYER.slice(2).padStart(64, '0');
    const tokenBalance = await rpcCall('eth_call', [{
        to: receipt.contractAddress,
        data: balanceOfData
    }, 'latest']);
    console.log('Token Balance (raw):', tokenBalance);
    console.log('Token Balance:', (BigInt(tokenBalance) / BigInt(1e18)).toString(), 'MDT');

    // Save deployment
    const deployInfo = {
        network: 'luxtensor_local',
        timestamp: new Date().toISOString(),
        MDTToken: receipt.contractAddress,
        txHash: txHash
    };
    fs.writeFileSync('deployment-test.json', JSON.stringify(deployInfo, null, 2));
    console.log('\nSaved to deployment-test.json');
}

main().catch(console.error);

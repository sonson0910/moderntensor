// LuxTensor - Test Script for eth_sendRawTransaction with Signed Transactions
// Run with: node test_signed_tx.js

const crypto = require('crypto');

// Hardhat Account #0 (for testing only - NEVER use in production!)
const PRIVATE_KEY = 'ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80';
const FROM_ADDRESS = 'f39Fd6e51aad88F6F4ce6aB8827279cffFb92266';
const TO_ADDRESS = '70997970C51812dc3A010C7d01b50e0d17dc79C8';

const RPC_URL = 'http://localhost:8545';

// Helper to convert hex string to bytes
function hexToBytes(hex) {
    hex = hex.replace('0x', '');
    const bytes = [];
    for (let i = 0; i < hex.length; i += 2) {
        bytes.push(parseInt(hex.substr(i, 2), 16));
    }
    return Buffer.from(bytes);
}

// Helper to pad number to fixed bytes
function padBytes(num, length) {
    const hex = num.toString(16).padStart(length * 2, '0');
    return hexToBytes(hex);
}

// Create raw transaction bytes
// Format: from(20) + nonce(8) + to(20) + value(16) + gas(8) + data_len(4) + data + v(1) + r(32) + s(32)
function createRawTransaction(from, nonce, to, value, gas, data, v, r, s) {
    const parts = [
        hexToBytes(from),           // 20 bytes
        padBytes(nonce, 8),         // 8 bytes
        hexToBytes(to),             // 20 bytes
        padBytes(value, 16),        // 16 bytes (u128)
        padBytes(gas, 8),           // 8 bytes
        padBytes(data.length / 2, 4), // 4 bytes (data length)
        hexToBytes(data || ''),     // variable
        Buffer.from([v]),           // 1 byte
        hexToBytes(r),              // 32 bytes
        hexToBytes(s),              // 32 bytes
    ];
    return Buffer.concat(parts);
}

// Simple signing (for demo - in production use proper secp256k1)
function signMessage(message, privateKey) {
    // This is a placeholder - in production, use proper ECDSA with secp256k1
    const hash = crypto.createHash('sha256').update(message).digest();
    const sig = crypto.createHash('sha256').update(Buffer.concat([hash, hexToBytes(privateKey)])).digest();
    return {
        r: sig.toString('hex').padStart(64, '0'),
        s: sig.toString('hex').padStart(64, '0'),
        v: 0
    };
}

// Create signing message (same format as Transaction::signing_message in Rust)
function createSigningMessage(nonce, from, to, value, gasPrice, gasLimit, data) {
    const parts = [
        padBytes(nonce, 8),
        hexToBytes(from),
        to ? hexToBytes(to) : Buffer.alloc(20, 0),
        padBytes(value, 16),
        padBytes(gasPrice, 8),
        padBytes(gasLimit, 8),
        hexToBytes(data || '')
    ];
    return Buffer.concat(parts);
}

async function sendRpcRequest(method, params) {
    const response = await fetch(RPC_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            jsonrpc: '2.0',
            method: method,
            params: params,
            id: 1
        })
    });
    return response.json();
}

async function main() {
    console.log('🔗 LuxTensor - Testing eth_sendRawTransaction');
    console.log('================================================\n');

    // Step 1: Get current nonce
    console.log('1. Getting current nonce...');
    const nonceResult = await sendRpcRequest('eth_getTransactionCount', ['0x' + FROM_ADDRESS, 'latest']);
    const nonce = parseInt(nonceResult.result, 16) || 0;
    console.log(`   Nonce: ${nonce}\n`);

    // Step 2: Create transaction
    console.log('2. Creating transaction...');
    const txParams = {
        nonce: nonce,
        from: FROM_ADDRESS,
        to: TO_ADDRESS,
        value: BigInt('1000000000000000000'),  // 1 ETH
        gas: 21000,
        gasPrice: 1,
        data: ''
    };
    console.log(`   From:  0x${txParams.from}`);
    console.log(`   To:    0x${txParams.to}`);
    console.log(`   Value: ${txParams.value} wei (1 ETH)\n`);

    // Step 3: Sign transaction
    console.log('3. Signing transaction...');
    const signingMessage = createSigningMessage(
        txParams.nonce,
        txParams.from,
        txParams.to,
        Number(txParams.value),
        txParams.gasPrice,
        txParams.gas,
        txParams.data
    );
    const signature = signMessage(signingMessage, PRIVATE_KEY);
    console.log(`   Signature R: 0x${signature.r.substring(0, 16)}...`);
    console.log(`   Signature S: 0x${signature.s.substring(0, 16)}...`);
    console.log(`   V: ${signature.v}\n`);

    // Step 4: Create raw transaction
    console.log('4. Creating raw transaction bytes...');
    const rawTx = createRawTransaction(
        txParams.from,
        txParams.nonce,
        txParams.to,
        Number(txParams.value),
        txParams.gas,
        txParams.data,
        signature.v,
        signature.r,
        signature.s
    );
    console.log(`   Raw TX length: ${rawTx.length} bytes`);
    console.log(`   Raw TX hex: 0x${rawTx.toString('hex').substring(0, 40)}...\n`);

    // Step 5: Send raw transaction
    console.log('5. Sending eth_sendRawTransaction...');
    const sendResult = await sendRpcRequest('eth_sendRawTransaction', ['0x' + rawTx.toString('hex')]);

    if (sendResult.error) {
        console.log(`   ❌ Error: ${sendResult.error.message}\n`);
    } else {
        console.log(`   ✅ TX Hash: ${sendResult.result}\n`);
    }

    // Step 6: Check balances
    console.log('6. Checking balances after 5 seconds...');
    await new Promise(r => setTimeout(r, 5000));

    const fromBalance = await sendRpcRequest('eth_getBalance', ['0x' + FROM_ADDRESS, 'latest']);
    const toBalance = await sendRpcRequest('eth_getBalance', ['0x' + TO_ADDRESS, 'latest']);

    console.log(`   From balance: ${fromBalance.result}`);
    console.log(`   To balance:   ${toBalance.result}\n`);

    console.log('================================================');
    console.log('✅ Test complete!');
}

main().catch(console.error);

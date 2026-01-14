/**
 * LuxTensor - Production Transaction Test with ethers.js
 *
 * This demonstrates the REAL production flow:
 * 1. Client creates transaction
 * 2. Client signs with their private key (ethers.js)
 * 3. Client sends signed raw transaction to node
 * 4. Node verifies signature and includes in block
 *
 * Run: npm install ethers && node test_production_tx.js
 */

// Use CommonJS for better compatibility
const { ethers } = require('ethers');

// Configuration
const RPC_URL = 'http://localhost:8545';

// Test accounts (Hardhat default - for testing only!)
const ACCOUNTS = [
    {
        address: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
        privateKey: '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'
    },
    {
        address: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
        privateKey: '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d'
    },
    {
        address: '0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC',
        privateKey: '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a'
    }
];

async function main() {
    console.log('🔗 LuxTensor - Production Transaction Test');
    console.log('==========================================\n');

    // Create provider
    const provider = new ethers.JsonRpcProvider(RPC_URL);

    // Create wallet from private key (CLIENT-SIDE signing)
    const wallet = new ethers.Wallet(ACCOUNTS[0].privateKey, provider);

    console.log('📋 Configuration:');
    console.log(`   RPC URL: ${RPC_URL}`);
    console.log(`   Sender:  ${wallet.address}`);
    console.log(`   Receiver: ${ACCOUNTS[1].address}\n`);

    // Step 1: Get chain info
    console.log('1️⃣ Getting chain info...');
    try {
        const network = await provider.getNetwork();
        console.log(`   Chain ID: ${network.chainId}\n`);
    } catch (e) {
        console.log(`   Note: ${e.message}\n`);
    }

    // Step 2: Get balances before
    console.log('2️⃣ Getting balances BEFORE transaction...');
    const balanceBefore = await provider.getBalance(wallet.address);
    const receiverBalanceBefore = await provider.getBalance(ACCOUNTS[1].address);
    console.log(`   Sender balance:   ${ethers.formatEther(balanceBefore)} ETH`);
    console.log(`   Receiver balance: ${ethers.formatEther(receiverBalanceBefore)} ETH\n`);

    // Step 3: Create and sign transaction (CLIENT-SIDE)
    console.log('3️⃣ Creating and signing transaction (CLIENT-SIDE with ethers.js)...');
    const tx = {
        to: ACCOUNTS[1].address,
        value: ethers.parseEther('1.0'),  // Send 1 ETH
        gasLimit: 21000,
        gasPrice: ethers.parseUnits('1', 'gwei'),
        nonce: await provider.getTransactionCount(wallet.address)
    };

    console.log(`   To:    ${tx.to}`);
    console.log(`   Value: ${ethers.formatEther(tx.value)} ETH`);
    console.log(`   Nonce: ${tx.nonce}`);

    // Sign the transaction (this is done CLIENT-SIDE, private key never leaves client)
    const signedTx = await wallet.signTransaction(tx);
    console.log(`   ✅ Transaction signed!`);
    console.log(`   Signed TX: ${signedTx.substring(0, 50)}...\n`);

    // Step 4: Send signed raw transaction to node
    console.log('4️⃣ Sending signed transaction via eth_sendRawTransaction...');
    try {
        const txResponse = await provider.broadcastTransaction(signedTx);
        console.log(`   ✅ Transaction sent!`);
        console.log(`   TX Hash: ${txResponse.hash}\n`);

        // Step 5: Wait for transaction to be mined
        console.log('5️⃣ Waiting for transaction to be mined (max 30s)...');
        const receipt = await txResponse.wait(1, 30000);
        if (receipt) {
            console.log(`   ✅ Transaction mined!`);
            console.log(`   Block: ${receipt.blockNumber}`);
            console.log(`   Gas used: ${receipt.gasUsed}\n`);
        }
    } catch (e) {
        console.log(`   ⚠️ Transaction status: ${e.message}\n`);
    }

    // Step 6: Get balances after
    console.log('6️⃣ Getting balances AFTER transaction (waiting 5s for block)...');
    await new Promise(r => setTimeout(r, 5000));
    const balanceAfter = await provider.getBalance(wallet.address);
    const receiverBalanceAfter = await provider.getBalance(ACCOUNTS[1].address);
    console.log(`   Sender balance:   ${ethers.formatEther(balanceAfter)} ETH`);
    console.log(`   Receiver balance: ${ethers.formatEther(receiverBalanceAfter)} ETH\n`);

    // Summary
    console.log('==========================================');
    const senderDiff = balanceBefore - balanceAfter;
    const receiverDiff = receiverBalanceAfter - receiverBalanceBefore;
    console.log('📊 Summary:');
    console.log(`   Sender spent:    ${ethers.formatEther(senderDiff)} ETH`);
    console.log(`   Receiver gained: ${ethers.formatEther(receiverDiff)} ETH`);

    if (receiverDiff > 0n) {
        console.log('\n✅ SUCCESS: Transaction was verified and included in block!');
    } else {
        console.log('\n⚠️ Transaction may still be pending or failed verification');
    }
    console.log('==========================================');
}

main().catch(error => {
    console.error('Error:', error.message);
    process.exit(1);
});

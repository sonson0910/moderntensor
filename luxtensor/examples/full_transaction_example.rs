// Full transaction example demonstrating end-to-end transaction processing

use luxtensor_core::block::{Block, BlockHeader};
use luxtensor_core::transaction::Transaction;
use luxtensor_core::types::Address;
use luxtensor_crypto::KeyPair;
use luxtensor_storage::{BlockchainDB, StateDB};
use std::sync::Arc;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 LuxTensor - Full Transaction Example\n");

    // 1. Initialize database and state
    println!("1️⃣  Initializing database and state...");
    let temp_dir = TempDir::new()?;
    let db = Arc::new(BlockchainDB::open(temp_dir.path().to_str().unwrap())?);
    let state_db = StateDB::new(db.inner_db());
    println!("   ✅ Database initialized\n");

    // 2. Generate keypairs
    println!("2️⃣  Generating keypairs...");
    let sender_keypair = KeyPair::generate();
    let receiver_keypair = KeyPair::generate();
    let sender_addr = Address::from(sender_keypair.address());
    let receiver_addr = Address::from(receiver_keypair.address());
    println!("   Sender:   {}", sender_addr);
    println!("   Receiver: {}\n", receiver_addr);

    // 3. Setup initial balances
    println!("3️⃣  Setting up initial balances...");
    state_db.set_balance(&sender_addr, 1_000_000)?;
    state_db.set_balance(&receiver_addr, 0)?;
    let initial_state_root = state_db.commit()?;
    println!("   ✅ Balances set\n");

    // 4. Create genesis block
    println!("4️⃣  Creating genesis block...");
    let genesis = Block {
        header: BlockHeader::new(
            1, 0, 1000, [0u8; 32], initial_state_root,
            [0u8; 32], [0u8; 32], [1u8; 32], [0u8; 64],
            0, 1_000_000, vec![],
        ),
        transactions: vec![],
    };
    db.store_block(&genesis)?;
    println!("   ✅ Genesis created\n");

    // 5. Create and execute transaction
    println!("5️⃣  Creating transaction...");
    let transfer_amount = 100_000u128;
    let tx = Transaction {
        chain_id: 8898, // Devnet
        nonce: 0,
        from: sender_addr,
        to: Some(receiver_addr),
        value: transfer_amount,
        gas_price: 1,
        gas_limit: 21_000,
        data: vec![],
        v: 0,
        r: [0u8; 32],
        s: [0u8; 32],
    };
    println!("   Transfer: {} tokens\n", transfer_amount);

    // 6. Execute transaction
    println!("6️⃣  Executing transaction...");
    state_db.transfer(&sender_addr, &receiver_addr, transfer_amount)?;
    state_db.increment_nonce(&sender_addr)?;
    let new_state_root = state_db.commit()?;
    println!("   ✅ Transaction executed\n");

    // 7. Create block with transaction
    println!("7️⃣  Creating block...");
    let block = Block {
        header: BlockHeader::new(
            1, 1, 1001, genesis.hash(), new_state_root,
            [0u8; 32], [0u8; 32], [1u8; 32], [0u8; 64],
            21_000, 1_000_000, vec![],
        ),
        transactions: vec![tx],
    };
    db.store_block(&block)?;
    println!("   ✅ Block stored\n");

    println!("🎉 Example completed successfully!");
    Ok(())
}

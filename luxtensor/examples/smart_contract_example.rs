// Smart contract deployment and execution example

use luxtensor_contracts::{ContractCode, ContractExecutor, ExecutionContext};
use luxtensor_core::types::Address;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 LuxTensor - Smart Contract Example\n");

    // 1. Create executor
    println!("1️⃣  Creating contract executor...");
    let executor = ContractExecutor::new();
    println!("   ✅ Executor initialized\n");

    // 2. Deploy contract
    println!("2️⃣  Deploying contract...");
    let code = ContractCode(vec![0x60, 0x60, 0x60, 0x40, 0x52, 0x60, 0x04, 0x35]);
    let deployer = Address::from([1u8; 20]);
    
    let (contract_address, deploy_result) = executor.deploy_contract(
        code.clone(),
        deployer,
        1_000_000,   // initial balance
        1_000_000,   // gas limit
        1,           // block number
    )?;
    
    println!("   ✅ Contract deployed!");
    println!("   Address:  {:?}", hex::encode(&contract_address.0));
    println!("   Gas used: {}\n", deploy_result.gas_used);

    // 3. Set storage
    println!("3️⃣  Setting contract storage...");
    let key = [1u8; 32];
    let value = [42u8; 32];
    executor.set_storage(&contract_address, key, value)?;
    println!("   ✅ Storage set\n");

    // 4. Retrieve storage
    println!("4️⃣  Retrieving storage...");
    let retrieved = executor.get_storage(&contract_address, &key)?;
    assert_eq!(value, retrieved);
    println!("   ✅ Storage verified\n");

    // 5. Call contract
    println!("5️⃣  Calling contract...");
    let context = ExecutionContext {
        caller: Address::from([2u8; 20]),
        contract_address,
        value: 0,
        gas_limit: 100_000,
        gas_price: 1,
        block_number: 2,
        timestamp: 1000,
    };
    
    let call_result = executor.call_contract(context, vec![0x01, 0x02])?;
    println!("   ✅ Contract called");
    println!("   Gas used: {}", call_result.gas_used);
    println!("   Success:  {}\n", call_result.success);

    // 6. Get statistics
    println!("6️⃣  Getting statistics...");
    let stats = executor.get_stats();
    println!("   Contracts: {}", stats.total_contracts);
    println!("   Code size: {} bytes\n", stats.total_code_size);

    println!("🎉 Example completed successfully!");
    Ok(())
}

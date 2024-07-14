use alloy::{providers::ProviderBuilder, sol, sol_types::{SolCall, SolEvent}};
use evm_sim::fork_db::{create_eth_provider, ForkDB};
use revm::{
    db::{CacheDB, EmptyDB},
    inspector_handle_register,
    inspectors::CustomPrintTracer,
    primitives::{address, ExecutionResult, Output, TxKind, U256},
    Evm,
};

// WETH interface and events
sol! {
    function balanceOf(address dst) public returns(uint);
    function withdraw(uint wad) public;
    event Deposit(address indexed dst, uint wad);
    event Withdrawal(address indexed src, uint wad);
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the Ethereum provider and forked db 
    let rpc_url = "https://rpc.ankr.com/eth".parse()?;
    let provider = ProviderBuilder::new().on_http(rpc_url);
    let eth_provider = create_eth_provider(provider);
    let db = CacheDB::new(EmptyDB::default());
    let fork_db = ForkDB::new(db, eth_provider);

    let mut fork_db = fork_db;

    // Amount to deposit and withdraw
    let amount = U256::from(77777777); 

    // Run the simulations
    simulate_weth_deposit_with_trace(&mut fork_db, amount)?;
    get_weth_balance(&mut fork_db)?;
    simulate_weth_withdrawal(&mut fork_db, amount)?;
    get_weth_balance(&mut fork_db)?;

    Ok(())
}

// Simulate a WETH deposit with tracing
fn simulate_weth_deposit_with_trace(fork_db: &mut ForkDB, amount: U256) -> Result<(), Box<dyn std::error::Error>> {
    println!("Simulating WETH deposit...");

    let tracer = CustomPrintTracer::default();
    
    let mut evm = Evm::builder()
        .with_db(fork_db)
        .with_external_context(tracer)
        .append_handler_register(inspector_handle_register)
        .modify_tx_env(|tx| {
            tx.caller = address!("a6715EAFe5D215B82cb9e90A9d6c8970A7C90033"); 
            tx.transact_to = TxKind::Call(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")); 
            tx.value = amount; 
        })
        .build();

    let result = evm.transact_commit().unwrap();

    let logs = match result {
        ExecutionResult::Success { logs, .. } => logs,
        result => {
            println!("Execution failed: {result:?}");
            return Err(Box::from("Transaction execution failed"));
        }
    };

    let decoded_deposit_log = Deposit::decode_log(&logs[0], false)?;
    println!("Decoded WETH Deposit log: To: {} Amount: {}", decoded_deposit_log.address, decoded_deposit_log.wad);

    // Validate 
    assert_eq!(decoded_deposit_log.address, address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), "Simulation Failed (To)");
    assert_eq!(decoded_deposit_log.wad, amount, "Simulation Failed (Amount)");

    println!("WETH deposit simulation successful.");
    Ok(())
}

// print the WETH balance of the specified address
fn get_weth_balance(fork_db: &mut ForkDB) -> Result<(), Box<dyn std::error::Error>> {
    println!("Fetching WETH balance...");

    let mut evm = Evm::builder()
        .with_db(fork_db)
        .modify_tx_env(|tx| {
            tx.caller = address!("a6715EAFe5D215B82cb9e90A9d6c8970A7C90033"); 
            tx.transact_to = TxKind::Call(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")); 
            tx.data = balanceOfCall::new((address!("a6715EAFe5D215B82cb9e90A9d6c8970A7C90033"),)).abi_encode().into();
            tx.value = U256::from(0); 
        })
        .build();

    let result_and_state = evm.transact().unwrap();
    let result = result_and_state.result;

    let data = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => {
            println!("Execution failed: {result:?}");
            return Err(Box::from("Transaction execution failed"));
        }
    };

    let decoded_data = balanceOfCall::abi_decode_returns(&data, false)?;
    println!("Fetched WETH Balance: {}", decoded_data._0);

    Ok(())
}

// Simulate a WETH withdrawal
fn simulate_weth_withdrawal(fork_db: &mut ForkDB, amount: U256) -> Result<(), Box<dyn std::error::Error>> {
    println!("Simulating WETH withdrawal...");

    let mut evm = Evm::builder()
        .with_db(fork_db)
        .modify_tx_env(|tx| {
            tx.caller = address!("a6715EAFe5D215B82cb9e90A9d6c8970A7C90033"); 
            tx.transact_to = TxKind::Call(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")); 
            tx.data = withdrawCall::new((amount,)).abi_encode().into(); 
            tx.value = U256::from(0);
        })
        .build();

    let result = evm.transact_commit().unwrap();

    let logs = match result {
        ExecutionResult::Success { logs, .. } => logs,
        result => {
            println!("Execution failed: {result:?}");
            return Err(Box::from("Transaction execution failed"));
        }
    };

    let decoded_withdrawal_log = Withdrawal::decode_log(&logs[0], false)?;
    println!("Decoded WETH Withdrawal log: From: {} Amount: {}", decoded_withdrawal_log.address, decoded_withdrawal_log.wad);

    assert_eq!(decoded_withdrawal_log.address, address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), "Simulation Failed (From)");
    assert_eq!(decoded_withdrawal_log.wad, amount, "Simulation Failed (Amount)");

    println!("WETH withdrawal simulation successful.");
    Ok(())
}

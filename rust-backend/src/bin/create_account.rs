use rust_backend::account::create::{create_faucet_account, create_wallet_account};
use std::env;
use std::path::PathBuf;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        println!("Usage:");
        println!("  cargo run --bin create_account -- wallet");
        println!("  cargo run --bin create_account -- faucet [symbol] [decimals] [max_supply]");
        return Ok(());
    }
    
    let command = &args[1];
    let rpc_url = env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    let keystore_path = PathBuf::from("./keystore");
    // Use separate store file for script to avoid conflicts with running server
    let store_path = PathBuf::from("./account_store.sqlite3");
    
    match command.as_str() {
        "wallet" => {
            println!("Creating wallet account...");
            let account_id = create_wallet_account(&keystore_path, &store_path, &rpc_url).await?;
            println!("✅ Wallet account created!");
            println!("Account ID: {}", account_id);
        }
        "faucet" => {
            let symbol = args.get(2).map(|s| s.as_str()).unwrap_or("MID");
            let decimals: u8 = args
                .get(3)
                .and_then(|s| s.parse().ok())
                .unwrap_or(8);
            let max_supply: u64 = args
                .get(4)
                .and_then(|s| s.parse().ok())
                .unwrap_or(1_000_000);
            
            println!("Creating faucet account...");
            println!("Symbol: {}, Decimals: {}, Max Supply: {}", symbol, decimals, max_supply);
            
            let faucet_id = create_faucet_account(
                &keystore_path,
                &store_path,
                &rpc_url,
                symbol,
                decimals,
                max_supply,
            )
            .await?;
            
            println!("✅ Faucet account created!");
            println!("Faucet Account ID: {}", faucet_id);
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Use 'wallet' or 'faucet'");
        }
    }
    
    Ok(())
}


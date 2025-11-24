use miden_client::{
    address::NetworkId,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::TransactionRequestBuilder,
    note::NoteType,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_objects::{
    account::AccountId,
    asset::FungibleAsset,
};
use rand::rngs::StdRng;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    // Get faucet ID from environment or args
    let env_faucet = env::var("WTAZ_FAUCET_ID").ok();
    let faucet_id_str = if let Some(ref env_val) = env_faucet {
        if args.len() > 1 && !args[1].is_empty() {
            args[1].as_str()
        } else {
            env_val.as_str()
        }
    } else {
        args.get(1)
            .map(|s| s.as_str())
            .unwrap_or("mtst1ap4fmar45fmq7gp364k5jrvj2uymhtvq")
    };
    
    // Get recipient account ID from args
    let recipient_account_id = args.get(2)
        .map(|s| s.as_str())
        .unwrap_or("mtst1azl5yvzz0gv9aypmjjwrnwnfqc405r84_qruqqypuyph");
    
    // Get amount from args (default 5 WTAZ with 8 decimals)
    let default_amount = "5".to_string();
    let amount_str = args.get(3).map(|s| s.as_str()).unwrap_or(&default_amount);
    let amount: u64 = amount_str.parse()
        .map_err(|_| "Invalid amount. Use a number like 5 for 5 tokens")?;
    let mint_amount = amount * 100_000_000u64; // Convert to 8 decimals
    
    let rpc_url = env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    let keystore_path = PathBuf::from("./keystore");
    let store_path = PathBuf::from("./faucet_store.sqlite3");
    
    println!("🚀 Minting private note...");
    println!("Faucet ID: {}", faucet_id_str);
    println!("Recipient Account ID: {}", recipient_account_id);
    println!("Amount: {} WTAZ ({} with 8 decimals)", amount, mint_amount);
    
    // Initialize client
    let endpoint = Endpoint::try_from(rpc_url.as_str())
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path.clone())
            .map_err(|e| format!("Failed to create keystore: {}", e))?,
    );
    
    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path.clone())
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;
    
    // Sync state
    let sync_summary = client.sync_state().await
        .map_err(|e| format!("Failed to sync state: {}", e))?;
    println!("📡 Synced to block: {}", sync_summary.block_num);
    
    // Parse faucet account ID
    let faucet_id = if faucet_id_str.starts_with("mtst") || faucet_id_str.starts_with("mm") {
        AccountId::from_bech32(faucet_id_str)
            .map_err(|e| format!("Invalid faucet_id bech32: {}", e))?
            .1
    } else {
        let hex_str = if faucet_id_str.starts_with("0x") {
            &faucet_id_str[2..]
        } else {
            faucet_id_str
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Invalid faucet_id hex: {}", e))?
    };
    
    // Parse recipient account ID
    // Note: Rust SDK's bech32 parser doesn't handle underscores
    let recipient_id = if recipient_account_id.starts_with("mtst") || recipient_account_id.starts_with("mm") {
        match AccountId::from_bech32(recipient_account_id) {
            Ok((_, acc_id)) => acc_id,
            Err(e) => {
                // Bech32 parsing failed (likely due to underscores)
                eprintln!("\n⚠️  Bech32 parsing failed: {}", e);
                eprintln!("💡 The Rust SDK doesn't support underscores in bech32 format.");
                eprintln!("   Please provide the hex format of your account ID instead.");
                eprintln!("   You can find it in your browser's localStorage: miden_account_id_hex");
                eprintln!("   Or convert it using the Miden SDK in JavaScript:");
                eprintln!("   const acc = AccountId.fromBech32('{}');", recipient_account_id);
                eprintln!("   console.log(acc.toHex());");
                eprintln!("\n   Then run this script with the hex format:");
                eprintln!("   cargo run --release --bin mint_private_note {} <hex_account_id>", faucet_id_str);
                return Err(format!("Bech32 format with underscores not supported. Please provide hex format (0x...). Error: {}", e).into());
            }
        }
    } else {
        let hex_str = if recipient_account_id.starts_with("0x") {
            &recipient_account_id[2..]
        } else {
            recipient_account_id
        };
        
        // Handle hex length - pad to 30 chars if needed
        let final_hex_part = if hex_str.len() < 30 {
            format!("{:0>30}", hex_str) // Left-pad with zeros to 30 chars
        } else if hex_str.len() > 30 {
            hex_str[hex_str.len() - 30..].to_string() // Take the last 30 characters
        } else {
            hex_str.to_string()
        };
        
        let hex_with_prefix = format!("0x{}", final_hex_part);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Invalid recipient account ID (hex): {}", e))?
    };
    
    println!("✅ Using faucet account: {}", faucet_id.to_bech32(NetworkId::Testnet));
    
    // Check if faucet account exists in client
    println!("🔍 Checking if faucet account is in client...");
    match client.get_account(faucet_id).await {
        Ok(Some(_)) => {
            println!("✅ Faucet account found in client");
        }
        Ok(None) => {
            println!("⚠️  Faucet account not found in client's account list");
            println!("💡 The faucet account needs to be added to the client.");
            println!("   If you created the faucet with setup_faucet.rs, it should be in the store.");
            println!("   Make sure you're using the same store path: {:?}", store_path);
            return Err("Faucet account not found in client. Please ensure the faucet was created using setup_faucet.rs with the same store path.".into());
        }
        Err(e) => {
            println!("⚠️  Error checking faucet account: {}", e);
            println!("   Continuing anyway, but transaction may fail if account is missing...");
        }
    }
    
    // Create asset
    let fungible_asset = FungibleAsset::new(faucet_id, mint_amount)
        .map_err(|e| format!("Failed to create asset: {}", e))?;
    
    // Mint a PUBLIC note (for testing - recipient can easily consume these)
    println!("\n💰 Minting {} WTAZ tokens as a PUBLIC note...", amount);
    let transaction_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            fungible_asset,
            recipient_id,
            NoteType::Public,  // Public note - recipient can consume easily
            client.rng(),
        )
        .map_err(|e| format!("Failed to build mint transaction: {}", e))?;
    
    // Submit transaction
    println!("📤 Submitting transaction to network...");
    let tx_id = client
        .submit_new_transaction(faucet_id, transaction_request)
        .await
        .map_err(|e| {
            let error_msg = format!("Failed to submit transaction: {}", e);
            println!("❌ Error details: {}", error_msg);
            println!("💡 Make sure:");
            println!("   1. The faucet account exists and is deployed on-chain");
            println!("   2. The faucet key is in the keystore");
            println!("   3. The faucet was created using setup_faucet.rs");
            error_msg
        })?;
    
    println!("✅ Minted {} WTAZ tokens as PUBLIC note. Transaction ID: {:?}", amount, tx_id);
    
    println!("⏳ Waiting 5 seconds for transaction confirmation...");
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    client.sync_state().await
        .map_err(|e| format!("Failed to sync state: {}", e))?;
    
    println!("\n✅ Mint complete!");
    println!("Faucet Account ID: {}", faucet_id.to_bech32(NetworkId::Testnet));
    println!("Recipient Account ID: {}", recipient_id.to_bech32(NetworkId::Testnet));
    println!("Amount: {} WTAZ", amount);
    println!("Transaction ID: {:?}", tx_id);
    println!("\n💡 Note: This is a PUBLIC note. The recipient should be able to see and consume it after syncing.");
    
    Ok(())
}


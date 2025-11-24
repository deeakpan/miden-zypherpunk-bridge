use miden_client::{
    account::component::BasicFungibleFaucet,
    address::NetworkId,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::TransactionRequestBuilder,
    note::NoteType,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_lib::account::auth::AuthRpoFalcon512;
use miden_objects::{
    account::{AccountBuilder, AccountId, AccountStorageMode, AccountType},
    asset::FungibleAsset,
    Felt,
};
use miden_objects::asset::TokenSymbol;
use rand::{rngs::StdRng, RngCore, rng};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    
    // Get recipient account ID from args or use default
    // Note: Rust SDK bech32 parser doesn't handle underscores, so use hex format
    // To get your hex account ID: Open browser console and run: localStorage.getItem("miden_account_id_hex")
    let recipient_account_id = args.get(1)
        .map(|s| s.as_str())
        .unwrap_or("c0dc14527e071e9041a922564c3502"); // Default hex format (30 chars)
    
    let rpc_url = env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    let keystore_path = PathBuf::from("./keystore");
    let store_path = PathBuf::from("./faucet_store.sqlite3");
    
    println!("🚀 Setting up faucet and minting tokens...");
    println!("Recipient Account ID: {}", recipient_account_id);
    
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
    
    // Parse recipient account ID
    // Note: Rust SDK's bech32 parser doesn't handle underscores, so we try bech32 first
    // and fall back to hex if it fails
    let recipient_id = if recipient_account_id.starts_with("mtst") || recipient_account_id.starts_with("mm") {
        match AccountId::from_bech32(recipient_account_id) {
            Ok((_, acc_id)) => acc_id,
            Err(_) => {
                // Bech32 parsing failed (likely due to underscores), try to get hex from localStorage
                // or use a default hex format. For now, we'll use the hex format.
                // The user should provide hex format if bech32 fails
                eprintln!("⚠️  Bech32 parsing failed (underscores not supported by Rust SDK)");
                eprintln!("💡 Please provide the hex format of your account ID instead.");
                eprintln!("   You can find it in your browser's localStorage: miden_account_id_hex");
                eprintln!("   Or use the hex format: 0x...");
                return Err("Bech32 format with underscores not supported. Please use hex format (0x...)".into());
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
    
    // Always create a new faucet (or you can check for existing one by trying to get it)
    // For simplicity, we'll always create a new one
    let faucet_id = {
        println!("🔨 Creating new faucet...");
        
        // Generate faucet seed
        let mut rng = rng();
        let mut init_seed = [0u8; 32];
        rng.fill_bytes(&mut init_seed);
        
        // Faucet parameters
        let symbol = TokenSymbol::new("WTAZ").map_err(|e| format!("Invalid symbol: {}", e))?;
        let decimals = 8;
        let max_supply = Felt::new(1_000_000_000); // 1 billion tokens
        
        // Generate key pair
        let key_pair = AuthSecretKey::new_rpo_falcon512();
        
        // Build the faucet account
        let faucet_account = AccountBuilder::new(init_seed)
            .account_type(AccountType::FungibleFaucet)
            .storage_mode(AccountStorageMode::Public)
            .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
            .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply)
                .map_err(|e| format!("Failed to create faucet component: {}", e))?)
            .build()
            .map_err(|e| format!("Failed to build faucet: {}", e))?;
        
        // Add the faucet to the client
        client
            .add_account(&faucet_account, false)
            .await
            .map_err(|e| format!("Failed to add faucet: {}", e))?;
        
        // Add the key pair to the keystore
        keystore.add_key(&key_pair)
            .map_err(|e| format!("Failed to add key to keystore: {}", e))?;
        
        let faucet_id = faucet_account.id();
        println!("✅ Faucet created!");
        println!("Faucet Account ID (bech32): {}", faucet_id.to_bech32(NetworkId::Testnet));
        println!("Faucet Account ID (hex): {}", faucet_id.to_hex());
        
        // Resync to show newly deployed faucet
        client.sync_state().await
            .map_err(|e| format!("Failed to sync state: {}", e))?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        faucet_id
    };
    
    // Mint tokens to recipient account using the SDK's built-in method
    println!("\n💰 Minting 5 WTAZ tokens to recipient...");
    let mint_amount = 5_00000000u64; // 5 tokens with 8 decimals
    
    // Create asset
    let fungible_asset = FungibleAsset::new(faucet_id, mint_amount)
        .map_err(|e| format!("Failed to create asset: {}", e))?;
    
    // Use the SDK's build_mint_fungible_asset method
    let transaction_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            fungible_asset,
            recipient_id,
            NoteType::Public,
            client.rng(),
        )
        .map_err(|e| format!("Failed to build mint transaction: {}", e))?;
    
    // Submit transaction using the client's helper method
    let tx_id = client
        .submit_new_transaction(faucet_id, transaction_request)
        .await
        .map_err(|e| format!("Failed to submit transaction: {}", e))?;
    
    println!("✅ Minted 5 WTAZ tokens. Transaction ID: {:?}", tx_id);
    
    println!("⏳ Waiting 5 seconds for transaction confirmation...");
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    client.sync_state().await
        .map_err(|e| format!("Failed to sync state: {}", e))?;
    
    println!("\n✅ Setup complete!");
    println!("Faucet Account ID (bech32): {}", faucet_id.to_bech32(NetworkId::Testnet));
    println!("Faucet Account ID (hex): {}", faucet_id.to_hex());
    println!("Minted 5 WTAZ tokens to: {}", recipient_account_id);
    println!("\n💡 Set this environment variable:");
    println!("   WTAZ_FAUCET_ID={}", faucet_id.to_hex());
    
    Ok(())
}


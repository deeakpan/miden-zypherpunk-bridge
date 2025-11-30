use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use miden_client::{
    address::NetworkId,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::TransactionRequestBuilder,
    note::NoteType,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_objects::{account::AccountId, asset::FungibleAsset};
use rand::rngs::StdRng;
use rust_backend::db::faucets::FaucetStore;

const RECIPIENT: &str = "mtst1arvm76ccx49gpyrtdrqu0wy6cyu5m862";
const AMOUNT: u64 = 2_000_000_000; // 20 tokens with 8 decimals (20 * 10^8)

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    
    println!("{}", "=".repeat(60));
    println!("Mint Public Note - 20 Tokens");
    println!("{}", "=".repeat(60));
    println!("Recipient: {}", RECIPIENT);
    println!("Amount: 20 tokens ({} base units)", AMOUNT);
    println!();
    
    // Parse recipient account ID
    let recipient_id = if RECIPIENT.starts_with("mtst") || RECIPIENT.starts_with("mm") {
        AccountId::from_bech32(RECIPIENT)
            .map_err(|e| format!("Invalid recipient bech32: {}", e))?
            .1
    } else {
        let hex_str = if RECIPIENT.starts_with("0x") {
            &RECIPIENT[2..]
        } else {
            RECIPIENT
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Invalid recipient hex: {}", e))?
    };
    
    println!("[1] ✅ Parsed recipient account ID");
    println!();
    
    // Get project paths
    let project_root = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    
    // Check if we're in rust-backend directory
    let project_root = if project_root.ends_with("rust-backend") {
        project_root.parent()
            .ok_or("Failed to get parent directory")?
            .to_path_buf()
    } else {
        project_root
    };
    
    let keystore_path = project_root.join("rust-backend").join("keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let faucet_store_path = project_root.join("faucets.db");
    let rpc_url = env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    println!("[2] Getting faucet from faucets.db...");
    let faucet_store = FaucetStore::new(faucet_store_path)
        .map_err(|e| format!("Failed to open faucet store: {}", e))?;
    
    let faucet_id = faucet_store.get_faucet_id("zcash_testnet")
        .map_err(|e| format!("Failed to query faucet store: {}", e))?
        .ok_or("No faucet found in faucets.db. Please create a faucet first.")?;
    
    println!("[2] ✅ Faucet ID: {}", faucet_id.to_bech32(NetworkId::Testnet));
    println!();
    
    // Initialize client
    println!("[3] Initializing Miden client...");
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
    println!("[3] Syncing client state...");
    client.sync_state().await
        .map_err(|e| format!("Failed to sync state: {}", e))?;
    println!("[3] ✅ Client synced");
    println!();
    
    // Create asset
    println!("[4] Creating fungible asset...");
    let fungible_asset = FungibleAsset::new(faucet_id, AMOUNT)
        .map_err(|e| format!("Failed to create asset: {}", e))?;
    println!("[4] ✅ Asset created");
    println!();
    
    // Mint a PUBLIC note
    println!("[5] Minting 20 tokens as PUBLIC note...");
    let transaction_request = TransactionRequestBuilder::new()
        .build_mint_fungible_asset(
            fungible_asset,
            recipient_id,
            NoteType::Public,  // Public note - recipient can consume easily
            client.rng(),
        )
        .map_err(|e| format!("Failed to build mint transaction: {}", e))?;
    
    // Submit transaction
    println!("[5] Submitting transaction to network...");
    let tx_id = client
        .submit_new_transaction(faucet_id, transaction_request)
        .await
        .map_err(|e| format!("Failed to submit transaction: {}", e))?;
    
    println!("[5] ✅ Transaction submitted!");
    println!("    Transaction ID: {:?}", tx_id);
    println!();
    
    println!("{}", "=".repeat(60));
    println!("✅ MINT COMPLETE!");
    println!("{}", "=".repeat(60));
    println!();
    println!("Recipient: {}", recipient_id.to_bech32(NetworkId::Testnet));
    println!("Amount: 20 tokens");
    println!("Transaction ID: {:?}", tx_id);
    println!();
    println!("💡 This is a PUBLIC note. The recipient can see and consume it after syncing.");
    
    Ok(())
}


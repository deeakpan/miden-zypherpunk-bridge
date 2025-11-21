#[macro_use]
extern crate rocket;

use miden_client::{
    account::component::{BasicFungibleFaucet, BasicWallet},
    address::NetworkId,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient, NodeRpcClient},
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_lib::account::auth::AuthRpoFalcon512;
use miden_objects::{
    account::{AccountBuilder, AccountStorageMode, AccountType},
    asset::TokenSymbol,
    Felt,
};
use rand::{rngs::StdRng, RngCore, rng};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct BlockInfo {
    block_num: u32,
    chain_tip: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct AccountResponse {
    account_id: String,
    success: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct FaucetResponse {
    faucet_account_id: String,
    symbol: String,
    decimals: u8,
    max_supply: String,
    success: bool,
}

struct State {
    rpc: Arc<dyn NodeRpcClient + Send + Sync + 'static>,
    keystore: Arc<FilesystemKeyStore<StdRng>>,
}

async fn init_client(keystore: Arc<FilesystemKeyStore<StdRng>>) -> Result<miden_client::Client<FilesystemKeyStore<StdRng>>, String> {
    // Initialize client
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    let endpoint = Endpoint::try_from(rpc_url.as_str())
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    
    // Use absolute path to avoid working directory issues
    let store_path = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?
        .join("store.sqlite3");
    
    let store_path_display = store_path.clone();
    
    ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {} (store path: {:?})", e, store_path_display))
}

#[get("/block")]
async fn get_block(state: &rocket::State<State>) -> Result<Json<BlockInfo>, String> {
    // Get latest block header
    let (block_header, _) = state
        .rpc
        .get_block_header_by_number(None, false)
        .await
        .map_err(|e| format!("RPC error: {}", e))?;

    // Get chain tip by syncing notes
    let sync_response = state
        .rpc
        .sync_notes(0u32.into(), None, &BTreeSet::new())
        .await
        .map_err(|e| format!("RPC error: {}", e))?;

    Ok(Json(BlockInfo {
        block_num: block_header.block_num().as_u32(),
        chain_tip: sync_response.chain_tip.as_u32(),
    }))
}

#[get("/health")]
fn health() -> &'static str {
    "OK"
}

#[post("/account/create")]
async fn create_account(state: &rocket::State<State>) -> Result<Json<AccountResponse>, String> {
    let keystore = state.keystore.clone();
    
    // Run client operations in blocking context to avoid Send requirement
    let result = tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| "Not in async context".to_string())?;
        
        handle.block_on(async {
            let mut client = init_client(keystore.clone()).await?;
            
            // Generate account seed
            let mut init_seed = [0_u8; 32];
            rng().fill_bytes(&mut init_seed);
            
            // Generate key pair
            let key_pair = AuthSecretKey::new_rpo_falcon512();
            
            // Build the account
            let account = AccountBuilder::new(init_seed)
                .account_type(AccountType::RegularAccountUpdatableCode)
                .storage_mode(AccountStorageMode::Public)
                .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
                .with_component(BasicWallet)
                .build()
                .map_err(|e| format!("Failed to build account: {}", e))?;
            
            // Add the account to the client
            client
                .add_account(&account, false)
                .await
                .map_err(|e| format!("Failed to add account: {}", e))?;
            
            // Add the key pair to the keystore
            keystore
                .add_key(&key_pair)
                .map_err(|e| format!("Failed to add key to keystore: {}", e))?;
            
            let account_id_bech32 = account.id().to_bech32(NetworkId::Testnet);
            
            Ok::<_, String>(AccountResponse {
                account_id: account_id_bech32,
                success: true,
            })
        })
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))??;
    
    Ok(Json(result))
}

#[post("/faucet/create")]
async fn create_faucet(state: &rocket::State<State>) -> Result<Json<FaucetResponse>, String> {
    let keystore = state.keystore.clone();
    
    // Run client operations in blocking context to avoid Send requirement
    let result = tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::try_current()
            .map_err(|_| "Not in async context".to_string())?;
        
        handle.block_on(async {
            let mut client = init_client(keystore.clone()).await?;
            
            // Generate faucet seed
            let mut init_seed = [0u8; 32];
            rng().fill_bytes(&mut init_seed);
            
            // Faucet parameters
            let symbol = TokenSymbol::new("MID").map_err(|e| format!("Invalid symbol: {}", e))?;
            let decimals = 8;
            let max_supply = Felt::new(1_000_000);
            
            // Generate key pair
            let key_pair = AuthSecretKey::new_rpo_falcon512();
            
            // Build the faucet account
            let faucet_account = AccountBuilder::new(init_seed)
                .account_type(AccountType::FungibleFaucet)
                .storage_mode(AccountStorageMode::Public)
                .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
                .with_component(BasicFungibleFaucet::new(symbol, decimals, max_supply).map_err(|e| format!("Failed to create faucet component: {}", e))?)
                .build()
                .map_err(|e| format!("Failed to build faucet: {}", e))?;
            
            // Add the faucet to the client
            client
                .add_account(&faucet_account, false)
                .await
                .map_err(|e| format!("Failed to add faucet: {}", e))?;
            
            // Add the key pair to the keystore
            keystore
                .add_key(&key_pair)
                .map_err(|e| format!("Failed to add key to keystore: {}", e))?;
            
            let faucet_account_id_bech32 = faucet_account.id().to_bech32(NetworkId::Testnet);
            
            // Resync to show newly deployed faucet
            client
                .sync_state()
                .await
                .map_err(|e| format!("Failed to sync state: {}", e))?;
            
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            Ok::<_, String>(FaucetResponse {
                faucet_account_id: faucet_account_id_bech32,
                symbol: "MID".to_string(),
                decimals,
                max_supply: max_supply.to_string(),
                success: true,
            })
        })
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))??;
    
    Ok(Json(result))
}

#[launch]
fn rocket() -> _ {
    // Connect to testnet (same as bridge) - can override with RPC_URL env var
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    println!("Connecting to RPC endpoint: {}", rpc_url);
    
    let endpoint = Endpoint::try_from(rpc_url.as_str())
        .expect("Failed to parse RPC endpoint");
    
    let rpc = Arc::new(GrpcClient::new(&endpoint, 10_000));
    
    // Initialize keystore
    let keystore_path = PathBuf::from("./keystore");
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path)
            .expect("Failed to create keystore"),
    );
    
    rocket::build()
        .manage(State {
            rpc,
            keystore,
        })
        .mount("/", routes![get_block, health, create_account, create_faucet])
}

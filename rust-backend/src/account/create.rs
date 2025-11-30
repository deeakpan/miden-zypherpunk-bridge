use miden_client::{
    account::component::{BasicFungibleFaucet, BasicWallet},
    address::NetworkId,
    auth::AuthSecretKey,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_lib::account::auth::AuthRpoFalcon512;
use crate::account::token_wrapper::TokenWrapperAccount;
use miden_objects::{
    account::{AccountBuilder, AccountStorageMode, AccountType},
    asset::TokenSymbol,
    Felt,
};
use rand::{rngs::StdRng, RngCore};
use rand::rng;
use std::path::PathBuf;

pub async fn create_wallet_account(
    keystore_path: &PathBuf,
    store_path: &PathBuf,
    rpc_url: &str,
) -> Result<String, String> {
    // Initialize client
    let endpoint = Endpoint::try_from(rpc_url)
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = std::sync::Arc::new(GrpcClient::new(&endpoint, 10_000));
    let keystore = std::sync::Arc::new(
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
    
    // Generate account seed
    let mut rng = rng();
    let mut init_seed = [0_u8; 32];
    rng.fill_bytes(&mut init_seed);
    
    // Generate key pair
    let key_pair = AuthSecretKey::new_rpo_falcon512();
    
    // Build the account
    let account = AccountBuilder::new(init_seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Private)
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
    
    Ok(account_id_bech32)
}

pub async fn create_faucet_account(
    keystore_path: &PathBuf,
    store_path: &PathBuf,
    rpc_url: &str,
    symbol: &str,
    decimals: u8,
    max_supply: u64,
) -> Result<String, String> {
    // Initialize client
    let endpoint = Endpoint::try_from(rpc_url)
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = std::sync::Arc::new(GrpcClient::new(&endpoint, 10_000));
    let keystore = std::sync::Arc::new(
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
    
    // Generate faucet seed
    let mut rng = rng();
    let mut init_seed = [0u8; 32];
    rng.fill_bytes(&mut init_seed);
    
    println!("[Create Faucet] Generated new random seed for faucet account");
    
    // Faucet parameters
    let token_symbol = TokenSymbol::new(symbol)
        .map_err(|e| format!("Invalid symbol: {}", e))?;
    let max_supply_felt = Felt::new(max_supply);
    
    // Generate key pair
    let key_pair = AuthSecretKey::new_rpo_falcon512();
    
    // Build the faucet account with TokenWrapperAccount component
    // For Zcash bridge, origin_network=0 and origin_address=[0; 3] (not used)
    let origin_network = 0u64;
    let origin_address = [Felt::new(0); 3];
    
    let faucet_account = AccountBuilder::new(init_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
        .with_component(TokenWrapperAccount::new(origin_network, origin_address))
        .with_component(
            BasicFungibleFaucet::new(token_symbol, decimals, max_supply_felt)
                .map_err(|e| format!("Failed to create faucet component: {}", e))?,
        )
        .build()
        .map_err(|e| format!("Failed to build faucet: {}", e))?;
    
    let new_faucet_id = faucet_account.id();
    let new_faucet_id_bech32 = new_faucet_id.to_bech32(NetworkId::Testnet);
    println!("[Create Faucet] New faucet account ID: {}", new_faucet_id_bech32);
    
    // Add the faucet to the client
    client
        .add_account(&faucet_account, false)
        .await
        .map_err(|e| format!("Failed to add faucet: {}", e))?;
    
    println!("[Create Faucet] ✅ Successfully added new faucet account to client");
    
    // Add the key pair to the keystore
    keystore
        .add_key(&key_pair)
        .map_err(|e| format!("Failed to add key to keystore: {}", e))?;
    
    // Sync state - account will deploy automatically on first transaction (minting or withdrawal)
    client
        .sync_state()
        .await
        .map_err(|e| format!("Failed to sync state: {}", e))?;
    
    println!("[Create Faucet] ✅ Faucet account created with TokenWrapperAccount component: {}", new_faucet_id_bech32);
    println!("[Create Faucet] Note: Account will deploy automatically on first use (minting or withdrawal)");
    Ok(new_faucet_id_bech32)
}


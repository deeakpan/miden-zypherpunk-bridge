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
use rocket::http::Status;
use rocket::response::status;
use rocket_cors::{AllowedOrigins, CorsOptions};
use rust_backend::bridge::deposit::{ClaimDepositRequest, ClaimDepositResponse};
use rust_backend::db::deposits::DepositTracker;
use rust_backend::miden::recipient::build_deposit_recipient;
use rust_backend::zcash::bridge_wallet::BridgeWallet;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::time::Duration;
use miden_objects::{account::AccountId, Word};

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
    bridge_wallet: BridgeWallet,
    deposit_tracker: Arc<Mutex<DepositTracker>>,
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
    let keystore_clone = state.keystore.clone();
    let keystore_for_key = state.keystore.clone();
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut client = init_client(keystore_clone).await?;
            
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
            keystore_for_key.add_key(&key_pair)
                .map_err(|e| format!("Failed to add key to keystore: {}", e))?;
            
            let account_id_bech32 = account.id().to_bech32(NetworkId::Testnet);
            
            Ok(AccountResponse {
                account_id: account_id_bech32,
                success: true,
            })
        })
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))?
    .map_err(|e: String| format!("Client operation error: {}", e))?;

    Ok(Json(result))
}

#[post("/faucet/create")]
async fn create_faucet(state: &rocket::State<State>) -> Result<Json<FaucetResponse>, String> {
    let keystore_clone = state.keystore.clone();
    let keystore_for_key = state.keystore.clone();
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut client = init_client(keystore_clone).await?;
            
            // Generate faucet seed
            let mut rng = rng();
            let mut init_seed = [0u8; 32];
            rng.fill_bytes(&mut init_seed);
            
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
            keystore_for_key.add_key(&key_pair)
                .map_err(|e| format!("Failed to add key to keystore: {}", e))?;
            
            let faucet_account_id_bech32 = faucet_account.id().to_bech32(NetworkId::Testnet);
            
            // Resync to show newly deployed faucet
            client
                .sync_state()
                .await
                .map_err(|e| format!("Failed to sync state: {}", e))?;
            
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            Ok(FaucetResponse {
                faucet_account_id: faucet_account_id_bech32,
                symbol: "MID".to_string(),
                decimals,
                max_supply: max_supply.to_string(),
                success: true,
            })
        })
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))?
    .map_err(|e: String| format!("Client operation error: {}", e))?;

    Ok(Json(result))
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct MintRequest {
    faucet_id: String,
    recipient_id: String,
    amount: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct MintResponse {
    success: bool,
    note_id: Option<String>,
    transaction_id: Option<String>,
    message: String,
}

#[post("/faucet/mint", format = "json", data = "<request>")]
async fn mint_from_faucet(
    state: &rocket::State<State>,
    request: Json<MintRequest>,
) -> Result<Json<MintResponse>, String> {
    // Parse faucet ID
    let faucet_id = if request.faucet_id.starts_with("mtst") || request.faucet_id.starts_with("mm") {
        AccountId::from_bech32(&request.faucet_id)
            .map_err(|e| format!("Invalid faucet_id bech32: {}", e))?
            .1
    } else {
        let hex_str = if request.faucet_id.starts_with("0x") {
            &request.faucet_id[2..]
        } else {
            &request.faucet_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Invalid faucet_id hex: {}", e))?
    };

    // Parse recipient ID
    let recipient_id = if request.recipient_id.starts_with("mtst") || request.recipient_id.starts_with("mm") {
        AccountId::from_bech32(&request.recipient_id)
            .map_err(|e| format!("Invalid recipient_id bech32: {}", e))?
            .1
    } else {
        let hex_str = if request.recipient_id.starts_with("0x") {
            &request.recipient_id[2..]
        } else {
            &request.recipient_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Invalid recipient_id hex: {}", e))?
    };

    // Parse amount
    let amount = request.amount.parse::<u64>()
        .map_err(|e| format!("Invalid amount: {}", e))?;

    // Mint note using the bridge deposit mint function
    let project_root = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let keystore_path = project_root.join("keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());

    // Generate a random secret for the note
    let mut rng = rng();
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let secret = Word::from(secret_bytes);

    let (note_id, tx_id) = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            rust_backend::bridge::deposit::mint_deposit_note(
                recipient_id,
                secret,
                faucet_id,
                amount,
                keystore_path,
                store_path,
                &rpc_url,
            )
            .await
        })
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))?
    .map_err(|e: String| format!("Mint note error: {}", e))?;

    Ok(Json(MintResponse {
        success: true,
        note_id: Some(note_id),
        transaction_id: Some(tx_id),
        message: format!("Successfully minted {} tokens to recipient", amount),
    }))
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct HashRequest {
    account_id: String,
    secret: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct HashResponse {
    recipient_hash: String,
    success: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ErrorResponse {
    success: bool,
    error: String,
}

#[options("/deposit/hash")]
fn options_hash() -> rocket::http::Status {
    rocket::http::Status::Ok
}

#[post("/deposit/hash", format = "json", data = "<request>")]
async fn generate_hash_endpoint(
    request: Json<HashRequest>,
) -> Result<Json<HashResponse>, status::Custom<Json<ErrorResponse>>> {
    // Trim whitespace from account_id
    let account_id_str = request.account_id.trim();
    
    if account_id_str.is_empty() {
        return Err(status::Custom(
            Status::BadRequest,
            Json(ErrorResponse {
                success: false,
                error: "account_id cannot be empty. Please provide a valid Miden account ID in bech32 (mtst1...) or hex format.".to_string(),
            }),
        ));
    }
    
    // Parse account_id and secret - handle both hex and bech32 formats
    // Try bech32 first if it starts with mtst/mm, otherwise try hex
    let account_id = if account_id_str.starts_with("mtst") || account_id_str.starts_with("mm") {
        // Try bech32 format first (e.g., mtst1...)
        match AccountId::from_bech32(account_id_str) {
            Ok((_, acc_id)) => acc_id,
            Err(bech32_err) => {
                // If bech32 fails, try hex as fallback (maybe user pasted hex that starts with mtst)
                let hex_str = if account_id_str.starts_with("0x") {
                    &account_id_str[2..]
                } else {
                    account_id_str
                };
                AccountId::from_hex(hex_str).map_err(|hex_err| {
                    status::Custom(
                        Status::BadRequest,
                        Json(ErrorResponse {
                            success: false,
                            error: format!(
                                "Invalid account_id format. Tried bech32 (mtst1...): {}. Tried hex: {}. Please provide a valid Miden account ID in bech32 (mtst1...) or hex format.",
                                bech32_err, hex_err
                            ),
                        }),
                    )
                })?
            }
        }
    } else {
        // Parse hex format - check if it starts with 0x
        let hex_str = if account_id_str.starts_with("0x") {
            &account_id_str[2..]
        } else {
            account_id_str
        };
        
        eprintln!("DEBUG /deposit/hash: Received hex_str: '{}', length: {}", hex_str, hex_str.len());
        
        // AccountId::from_hex expects 32 characters total (including 0x prefix)
        // So if hex is 30 chars, add 0x to make 32 total. If 32 chars, add 0x to make 34 (but that's wrong)
        // Actually, let's just add 0x prefix - it should handle the length
        let hex_with_prefix = if !hex_str.starts_with("0x") {
            format!("0x{}", hex_str)
        } else {
            hex_str.to_string()
        };
        
        eprintln!("DEBUG /deposit/hash: Final hex with prefix: '{}', length: {}", hex_with_prefix, hex_with_prefix.len());
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| {
                eprintln!("DEBUG /deposit/hash: Failed to parse hex '{}': {:?}", hex_with_prefix, e);
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!(
                            "Invalid hex account_id: {}. Please provide a valid Miden account ID in bech32 (mtst1...) or hex format (with or without 0x prefix).",
                            e
                        ),
                    }),
                )
            })?
    };
    
    // Parse secret - Word::try_from expects hex with 0x prefix
    let secret_hex = if request.secret.starts_with("0x") {
        request.secret.clone()
    } else {
        format!("0x{}", request.secret)
    };
    
    let secret = Word::try_from(secret_hex.as_str())
        .map_err(|e| {
            status::Custom(
                Status::BadRequest,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Invalid secret: {}", e),
                }),
            )
        })?;
    
    // Build recipient and get hash
    let recipient = build_deposit_recipient(account_id, secret)
        .map_err(|e| {
            status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to build recipient: {}", e),
                }),
            )
        })?;
    let recipient_hash = recipient.digest().to_hex();
    
    Ok(Json(HashResponse {
        recipient_hash,
        success: true,
    }))
}

#[options("/deposit/claim")]
fn options_claim() -> rocket::http::Status {
    rocket::http::Status::Ok
}

#[post("/deposit/claim", format = "json", data = "<request>")]
async fn claim_deposit_endpoint(
    state: &rocket::State<State>,
    request: Json<ClaimDepositRequest>,
) -> Result<Json<ClaimDepositResponse>, String> {
    // Parse account_id and secret - handle both hex and bech32 formats
    let account_id = if request.account_id.starts_with("mtst") || request.account_id.starts_with("mm") {
        // Parse bech32 format (e.g., mtst1...) - returns (NetworkId, AccountId)
        let (_, acc_id) = AccountId::from_bech32(&request.account_id)
            .map_err(|e| format!("Invalid bech32 account_id: {}", e))?;
        acc_id
    } else {
        // Parse hex format - check if it starts with 0x
        let hex_str = if request.account_id.starts_with("0x") {
            &request.account_id[2..]
        } else {
            &request.account_id
        };
        
        // AccountId::from_hex expects hex with 0x prefix
        let hex_with_prefix = if !hex_str.starts_with("0x") {
            format!("0x{}", hex_str)
        } else {
            hex_str.to_string()
        };
        
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Invalid hex account_id: {}", e))?
    };
    
    // Parse secret - Word::try_from expects hex with 0x prefix
    let secret_hex = if request.secret.starts_with("0x") {
        request.secret.clone()
    } else {
        format!("0x{}", request.secret)
    };
    
    let secret = Word::try_from(secret_hex.as_str())
        .map_err(|e| format!("Invalid secret: {}", e))?;
    
    // Rebuild recipient hash to scan for deposits
    let recipient = build_deposit_recipient(account_id, secret)
        .map_err(|e| format!("Failed to build recipient: {}", e))?;
    let recipient_hash = recipient.digest().to_hex();
    
    // Check if this recipient hash has already been claimed (double-spend protection)
    {
        let tracker = state.deposit_tracker.lock()
            .map_err(|e| format!("Failed to lock deposit tracker: {}", e))?;
        
        if tracker.is_claimed(&recipient_hash)
            .map_err(|e| format!("Failed to check claim status: {}", e))? {
            return Ok(Json(ClaimDepositResponse {
                success: false,
                note_id: None,
                transaction_id: None,
                message: "This deposit has already been claimed. Each recipient hash can only be used once.".to_string(),
            }));
        }
    } // Lock released here
    
    // Scan bridge Zcash testnet wallet for deposits with this memo
    let bridge_address = std::env::var("BRIDGE_ZCASH_ADDRESS")
        .unwrap_or_else(|_| "utest1s7vrs7ycxvpu379zvtxt0fnc0efseur2f8g2s8puqls7nk45l6p7wvglu3rph9us9qzsjww44ly3wxlsul0jcpqx8qwvwqz4sq48rjj0cn59956sjsrz5ufuswd5ujy89n3vh264wx3843pxscnrf0ulku4990h65h5ll9r0j3q82mjgm2sx7lfnrkfkuqw9l2m7yfmgc4jvzq6n8j2".to_string());
    
    let deposit_info = rust_backend::bridge::deposit::scan_zcash_deposits(
        &state.bridge_wallet,
        &recipient_hash,
        &bridge_address,
    )
    .await
    .map_err(|e| format!("Failed to scan deposits: {}", e))?;
    
    let (txid, amount) = deposit_info.ok_or_else(|| {
        "No deposit found with matching recipient hash. Make sure you've sent TAZ to the bridge address with the correct memo.".to_string()
    })?;
    
    // Get faucet_id from env or use a default
    let faucet_id_hex = std::env::var("WTAZ_FAUCET_ID")
        .unwrap_or_else(|_| "0x00000000000000000000000000000000".to_string());
    let faucet_id = AccountId::from_hex(&faucet_id_hex)
        .map_err(|e| format!("Invalid faucet_id: {}", e))?;
    
    // Claim the deposit by minting note to user's account
    // Wrap in spawn_blocking to handle Send/Sync issues with Miden client
    let project_root = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let keystore_path = project_root.join("keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    let (note_id, tx_id) = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            rust_backend::bridge::deposit::mint_deposit_note(
                account_id,
                secret,
                faucet_id,
                amount,
                keystore_path,
                store_path,
                &rpc_url,
            )
            .await
        })
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))?
    .map_err(|e: String| format!("Mint deposit note error: {}", e))?;
    
    // Record the claim to prevent double-spending
    // NOTE: We only store recipient_hash, NOT account_id, for privacy
    let tracker = state.deposit_tracker.lock()
        .map_err(|e| format!("Failed to lock deposit tracker: {}", e))?;
    
    tracker.record_claim(
        &recipient_hash,
        &txid.clone(),
        amount,
    )
    .map_err(|e| format!("Failed to record claim: {}", e))?;
    
    Ok(Json(ClaimDepositResponse {
        success: true,
        note_id: Some(note_id),
        transaction_id: Some(tx_id),
        message: format!("Deposit claimed successfully. Note minted to account."),
    }))
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
    
    // Initialize bridge wallet
    let project_root = std::env::current_dir()
        .expect("Failed to get current directory");
    let bridge_wallet = BridgeWallet::new(project_root.clone());
    
    // Initialize deposit tracker database
    let db_path = project_root.join("deposits.db");
    let deposit_tracker = DepositTracker::new(db_path)
        .expect("Failed to initialize deposit tracker database");
    
    rocket::build()
        .manage(State {
            rpc,
            keystore,
            bridge_wallet,
            deposit_tracker: Arc::new(Mutex::new(deposit_tracker)),
        })
        .mount("/", routes![get_block, health, create_account, create_faucet, mint_from_faucet, options_hash, generate_hash_endpoint, options_claim, claim_deposit_endpoint])
        .attach(
            CorsOptions::default()
                .allowed_origins(AllowedOrigins::all())
                .allowed_methods(
                    vec![rocket::http::Method::Get, rocket::http::Method::Post, rocket::http::Method::Options]
                        .into_iter()
                        .map(From::from)
                        .collect(),
                )
                .allowed_headers(rocket_cors::AllowedHeaders::some(&[
                    "Authorization",
                    "Accept",
                    "Content-Type",
                ]))
                .allow_credentials(true)
                .to_cors()
                .expect("Failed to create CORS fairing")
        )
}

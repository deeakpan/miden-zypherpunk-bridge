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
use rust_backend::miden::notes::reconstruct_deposit_note;
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
    account_id: String, // bech32
    account_id_hex: String, // hex format
    success: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ReconstructNoteRequest {
    account_id: String,
    secret: String,
    faucet_id: String,
    amount: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ReconstructNoteResponse {
    note_id: String,
    recipient_hash: String,
    faucet_id: String,
    amount: u64,
    success: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct PrepareConsumeRequest {
    account_id: String,
    secret: String,
    faucet_id: String,
    amount: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct PrepareConsumeResponse {
    note_id: String,
    note_commitment: String,
    recipient_hash: String,
    faucet_id: String,
    amount: u64,
    success: bool,
    message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ConsumeNoteRequest {
    account_id: String, // Can be bech32 or hex
    secret: String,
    faucet_id: String,
    amount: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct ConsumeNoteResponse {
    transaction_id: String,
    note_id: String,
    success: bool,
    message: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct BalanceRequest {
    account_id: String, // Can be bech32 or hex
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct BalanceResponse {
    balance: String, // Balance in tokens (e.g., "0.3")
    balance_raw: u64, // Raw balance in smallest units
    faucet_id: String,
    success: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct PoolBalanceRequest {
    faucet_id: Option<String>, // Optional, if not provided uses default
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct PoolBalanceResponse {
    balance: String,
    balance_raw: u64,
    faucet_id: String,
    success: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct WithdrawalRequest {
    account_id: String, // User's Miden account (bech32 or hex)
    zcash_address: String, // Zcash testnet address
    amount: u64, // Amount in base units (8 decimals)
    faucet_id: Option<String>, // Optional, defaults to wTAZ faucet
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct WithdrawalResponse {
    note_id: String,
    transaction_id: String,
    success: bool,
    message: String,
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
    bridge_wallet: Arc<BridgeWallet>,
    deposit_tracker: Arc<Mutex<DepositTracker>>,
}

async fn init_client(keystore: Arc<FilesystemKeyStore<StdRng>>) -> Result<miden_client::Client<FilesystemKeyStore<StdRng>>, String> {
    // Initialize client
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    let endpoint = Endpoint::try_from(rpc_url.as_str())
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    
    // Use bridge_store.sqlite3 in project root (same as main bridge client)
    let current_dir = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    
    // If we're in rust-backend, go up one level to project root
    let project_root = if current_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "rust-backend")
        .unwrap_or(false) {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir
    };
    
    let store_path = project_root.join("bridge_store.sqlite3");
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

#[options("/account/create")]
fn options_create_account() -> status::Custom<&'static str> {
    status::Custom(rocket::http::Status::Ok, "")
}

#[post("/account/create")]
async fn create_account(state: &rocket::State<State>) -> Result<Json<AccountResponse>, status::Custom<Json<serde_json::Value>>> {
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
            use miden_objects::utils::Serializable;
            let account_bytes = account.id().to_bytes();
            let account_id_hex: String = format!("0x{}", account_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
            
            Ok(AccountResponse {
                account_id: account_id_bech32,
                account_id_hex,
                success: true,
            })
        })
    })
    .await;
    
    let inner_result: Result<AccountResponse, String> = match result {
        Ok(inner) => inner,
        Err(e) => {
            let error_json = serde_json::json!({
                "success": false,
                "error": format!("Spawn blocking error: {}", e)
            });
            return Err(status::Custom(rocket::http::Status::InternalServerError, Json(error_json)));
        }
    };
    
    match inner_result {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            let error_json = serde_json::json!({
                "success": false,
                "error": format!("Failed to create account: {}", e)
            });
            Err(status::Custom(rocket::http::Status::InternalServerError, Json(error_json)))
        }
    }
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
    _state: &rocket::State<State>,
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
    // Generate random bytes synchronously before any await to avoid Send issues
    // Use a block scope to ensure rng is dropped before await
    let secret_bytes: [u8; 32] = {
        let mut rng = rng();
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        bytes
    };
    // Convert [u8; 32] to Word (which is [Felt; 4])
    // Split into 4 chunks of 8 bytes each, convert to u64, then to Felt
    let secret = Word::new([
        Felt::new(u64::from_le_bytes(secret_bytes[0..8].try_into().unwrap())),
        Felt::new(u64::from_le_bytes(secret_bytes[8..16].try_into().unwrap())),
        Felt::new(u64::from_le_bytes(secret_bytes[16..24].try_into().unwrap())),
        Felt::new(u64::from_le_bytes(secret_bytes[24..32].try_into().unwrap())),
    ]);

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

// Simple GET endpoint for fast hash generation (query params instead of JSON body)
// Rocket requires query params to be optional, so we check them manually
#[get("/deposit/hash?<account_id>&<secret>")]
fn get_hash_endpoint(
    account_id: Option<String>,
    secret: Option<String>,
) -> Result<Json<HashResponse>, status::Custom<Json<ErrorResponse>>> {
    let account_id = account_id.ok_or_else(|| {
        status::Custom(
            Status::BadRequest,
            Json(ErrorResponse {
                success: false,
                error: "Missing account_id parameter".to_string(),
            }),
        )
    })?;
    
    let secret = secret.ok_or_else(|| {
        status::Custom(
            Status::BadRequest,
            Json(ErrorResponse {
                success: false,
                error: "Missing secret parameter".to_string(),
            }),
        )
    })?;
    
    generate_hash_internal(&account_id, &secret)
}

#[post("/deposit/hash", format = "json", data = "<request>")]
async fn generate_hash_endpoint(
    request: Json<HashRequest>,
) -> Result<Json<HashResponse>, status::Custom<Json<ErrorResponse>>> {
    generate_hash_internal(&request.account_id, &request.secret)
}

// Internal function to generate hash (shared by GET and POST endpoints)
fn generate_hash_internal(
    account_id_str: &str,
    secret_str: &str,
) -> Result<Json<HashResponse>, status::Custom<Json<ErrorResponse>>> {
    // Trim whitespace from account_id and secret
    let account_id_str = account_id_str.trim();
    let secret_str = secret_str.trim();
    
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
    let secret_hex = if secret_str.starts_with("0x") {
        secret_str.to_string()
    } else {
        format!("0x{}", secret_str)
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
    
    // Get or create faucet automatically (auto-deploy on first deposit)
    let project_root = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;
    let keystore_path = project_root.join("keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let faucet_store_path = project_root.join("faucets.db");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    // Get or create faucet (auto-deploy if needed)
    let faucet_id = tokio::task::spawn_blocking({
        let keystore_path = keystore_path.clone();
        let store_path = store_path.clone();
        let faucet_store_path = faucet_store_path.clone();
        let rpc_url = rpc_url.clone();
        move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                rust_backend::bridge::deposit::get_or_create_zcash_faucet(
                    keystore_path,
                    store_path,
                    &rpc_url,
                    faucet_store_path,
                )
                .await
            })
        }
    })
    .await
    .map_err(|e| format!("Spawn blocking error: {}", e))?
    .map_err(|e: String| format!("Get or create faucet error: {}", e))?;
    
    // Claim the deposit by minting note to user's account
    // Wrap in spawn_blocking to handle Send/Sync issues with Miden client
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

#[post("/note/reconstruct", format = "json", data = "<request>")]
async fn reconstruct_note_endpoint(
    _state: &rocket::State<State>,
    request: Json<ReconstructNoteRequest>,
) -> Result<Json<ReconstructNoteResponse>, String> {
    // Parse account_id
    let account_id = if request.account_id.starts_with("mtst") || request.account_id.starts_with("mm") {
        let (_, acc_id) = AccountId::from_bech32(&request.account_id)
            .map_err(|e| format!("Invalid bech32 account_id: {}", e))?;
        acc_id
    } else {
        let hex_str = if request.account_id.starts_with("0x") {
            &request.account_id[2..]
        } else {
            &request.account_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Failed to parse account_id: {}", e))?
    };
    
    // Parse secret
    let secret_hex = if request.secret.starts_with("0x") {
        request.secret.clone()
    } else {
        format!("0x{}", request.secret)
    };
    let secret = Word::try_from(secret_hex.as_str())
        .map_err(|e| format!("Failed to parse secret: {}", e))?;
    
    // Parse faucet_id
    let faucet_id = if request.faucet_id.starts_with("mtst") || request.faucet_id.starts_with("mm") {
        let (_, fid) = AccountId::from_bech32(&request.faucet_id)
            .map_err(|e| format!("Invalid bech32 faucet_id: {}", e))?;
        fid
    } else {
        let hex_str = if request.faucet_id.starts_with("0x") {
            &request.faucet_id[2..]
        } else {
            &request.faucet_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Failed to parse faucet_id: {}", e))?
    };
    
    // Reconstruct the note
    let note = reconstruct_deposit_note(account_id, secret, faucet_id, request.amount)
        .map_err(|e| format!("Failed to reconstruct note: {:?}", e))?;
    
    // Get note ID and recipient hash
    let note_id = note.id().to_hex();
    let recipient = build_deposit_recipient(account_id, secret)
        .map_err(|e| format!("Failed to build recipient: {:?}", e))?;
    let recipient_hash = recipient.digest().to_hex();
    
    Ok(Json(ReconstructNoteResponse {
        note_id,
        recipient_hash,
        faucet_id: request.faucet_id.clone(),
        amount: request.amount,
        success: true,
    }))
}

#[post("/note/consume", format = "json", data = "<request>")]
async fn consume_note_endpoint(
    _state: &rocket::State<State>,
    request: Json<ConsumeNoteRequest>,
) -> Result<Json<ConsumeNoteResponse>, status::Custom<Json<ErrorResponse>>> {
    // Parse account_id (accepts both bech32 and hex)
    let account_id = if request.account_id.starts_with("mtst") || request.account_id.starts_with("mm") {
        let (_, acc_id) = AccountId::from_bech32(&request.account_id)
            .map_err(|e| {
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("Invalid bech32 account_id: {}", e),
                    }),
                )
            })?;
        acc_id
    } else {
        let hex_str = if request.account_id.starts_with("0x") {
            &request.account_id[2..]
        } else {
            &request.account_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| {
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("Failed to parse account_id: {}", e),
                    }),
                )
            })?
    };
    
    // Parse secret
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
                    error: format!("Failed to parse secret: {}", e),
                }),
            )
        })?;
    
    // Parse faucet_id
    let faucet_id = if request.faucet_id.starts_with("mtst") || request.faucet_id.starts_with("mm") {
        let (_, fid) = AccountId::from_bech32(&request.faucet_id)
            .map_err(|e| {
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("Invalid bech32 faucet_id: {}", e),
                    }),
                )
            })?;
        fid
    } else {
        let hex_str = if request.faucet_id.starts_with("0x") {
            &request.faucet_id[2..]
        } else {
            &request.faucet_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| {
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("Failed to parse faucet_id: {}", e),
                    }),
                )
            })?
    };
    
    // Setup paths (same logic as init_client)
    let current_dir = std::env::current_dir()
        .map_err(|e| {
            status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to get current directory: {}", e),
                }),
            )
        })?;
    
    // If we're in rust-backend, go up one level to project root
    let project_root = if current_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "rust-backend")
        .unwrap_or(false) {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir
    };
    
    let keystore_path = project_root.join("rust-backend").join("keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    // Execute consumption transaction
    let (tx_id, note_id) = tokio::task::spawn_blocking({
        let keystore_path = keystore_path.clone();
        let store_path = store_path.clone();
        let rpc_url = rpc_url.clone();
        move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(async {
                consume_deposit_note(
                    account_id,
                    secret,
                    faucet_id,
                    request.amount,
                    keystore_path,
                    store_path,
                    &rpc_url,
                )
                .await
            })
        }
    })
    .await
    .map_err(|e| {
        status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: format!("Spawn blocking error: {}", e),
            }),
        )
    })?
    .map_err(|e: String| {
        status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: format!("Consume note error: {}", e),
            }),
        )
    })?;
    
    Ok(Json(ConsumeNoteResponse {
        transaction_id: tx_id,
        note_id,
        success: true,
        message: "Note consumed successfully!".to_string(),
    }))
}

// Helper function to consume a deposit note (extracted from consume_note.rs pattern)
async fn consume_deposit_note(
    account_id: AccountId,
    secret: Word,
    faucet_id: AccountId,
    amount: u64,
    keystore_path: PathBuf,
    store_path: PathBuf,
    rpc_url: &str,
) -> Result<(String, String), String> {
    use miden_client::transaction::TransactionRequestBuilder;
    use miden_objects::note::NoteTag;
    
    // Initialize Miden client
    let endpoint = Endpoint::try_from(rpc_url)
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    
    if !keystore_path.exists() {
        return Err(format!("Keystore directory does not exist: {:?}", keystore_path));
    }
    
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path.clone())
            .map_err(|e| format!("Failed to create keystore at {:?}: {}", keystore_path, e))?,
    );
    
    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;
    
    // Add bridge note tag
    use rust_backend::miden::notes::BRIDGE_USECASE;
    client.add_note_tag(NoteTag::for_local_use_case(BRIDGE_USECASE, 0).expect("Bridge use case tag should be valid"))
        .await
        .map_err(|e| format!("Failed to add note tag: {}", e))?;
    
    // Sync state
    client.sync_state().await
        .map_err(|e| format!("Failed to sync client state: {}", e))?;
    
    // Check if account exists
    let wallet_account = client.get_account(account_id).await
        .map_err(|e| format!("Failed to get account: {}", e))?;
    
    if wallet_account.is_none() {
        return Err(format!(
            "Account {} not found in client store. The account must be created and added to the client first.",
            account_id.to_bech32(miden_objects::address::NetworkId::Testnet)
        ));
    }
    
    // Reconstruct the note
    println!("[Consume Note] Reconstructing note...");
    let note = reconstruct_deposit_note(account_id, secret, faucet_id, amount)
        .map_err(|e| format!("Failed to reconstruct note: {:?}", e))?;
    
    // Get note ID and commitment before moving the note
    let note_id = note.id();
    let note_id_hex = note_id.to_hex();
    let note_commitment = note.commitment();
    println!("[Consume Note] Note reconstructed:");
    println!("  Note ID: {}", note_id_hex);
    println!("  Note Commitment: 0x{}", note_commitment.to_hex());
    
    // Build consume transaction using unauthenticated_input_notes
    println!("[Consume Note] Building transaction...");
    let secret_word: miden_objects::Word = secret;
    let tx_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(note, Some(secret_word.into()))])
        .build()
        .map_err(|e| {
            let error_msg = format!("{:?}", e);
            eprintln!("[Consume Note] Transaction build error: {}", error_msg);
            format!("Failed to build transaction: {}", error_msg)
        })?;
    println!("[Consume Note] Transaction built successfully");
    
    // Execute transaction (same pattern as mint_deposit_note)
    println!("[Consume Note] Executing transaction...");
    println!("  Account: {}", account_id.to_bech32(miden_objects::address::NetworkId::Testnet));
    println!("  Note ID: {}", note_id_hex);
    println!("  Faucet ID: {}", faucet_id.to_bech32(miden_objects::address::NetworkId::Testnet));
    println!("  Amount: {}", amount);
    
    let tx_result = client
        .execute_transaction(account_id, tx_request)
        .await
        .map_err(|e| {
            let error_msg = format!("{:?}", e);
            eprintln!("[Consume Note] Transaction execution failed: {}", error_msg);
            format!("Failed to execute transaction: {}", error_msg)
        })?;
    
    // Prove transaction
    println!("[Consume Note] Proving transaction...");
    let proven_tx = client
        .prove_transaction(&tx_result)
        .await
        .map_err(|e| {
            let error_msg = format!("{:?}", e);
            eprintln!("[Consume Note] Transaction proof failed: {}", error_msg);
            format!("Failed to prove transaction: {}", error_msg)
        })?;
    
    // Submit proven transaction
    println!("[Consume Note] Submitting proven transaction...");
    let submission_height = client
        .submit_proven_transaction(proven_tx, &tx_result)
        .await
        .map_err(|e| {
            // Format the error with full details
            let error_debug = format!("{:?}", e);
            let error_display = format!("{}", e);
            eprintln!("[Consume Note] Transaction submission failed!");
            eprintln!("  Error (Display): {}", error_display);
            eprintln!("  Error (Debug): {}", error_debug);
            format!("Failed to submit transaction: {}", error_debug)
        })?;
    
    // Apply transaction
    client
        .apply_transaction(&tx_result, submission_height)
        .await
        .map_err(|e| {
            let error_msg = format!("{:?}", e);
            eprintln!("[Consume Note] Transaction apply failed: {}", error_msg);
            format!("Failed to apply transaction: {}", error_msg)
        })?;
    
    let tx_id = tx_result.executed_transaction().id().to_hex();
    
    println!("[Consume Note] Transaction submitted successfully!");
    println!("  TX ID: 0x{}", tx_id);
    
    Ok((tx_id, note_id_hex))
}

#[options("/account/balance")]
fn options_account_balance() -> rocket::http::Status {
    rocket::http::Status::Ok
}

#[post("/account/balance", format = "json", data = "<request>")]
async fn get_account_balance(
    _state: &rocket::State<State>,
    request: Json<BalanceRequest>,
) -> Result<Json<BalanceResponse>, status::Custom<Json<ErrorResponse>>> {
    // Parse account_id (accepts both bech32 and hex)
    let account_id = if request.account_id.starts_with("mtst") || request.account_id.starts_with("mm") {
        let (_, acc_id) = AccountId::from_bech32(&request.account_id)
            .map_err(|e| {
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("Invalid bech32 account_id: {}", e),
                    }),
                )
            })?;
        acc_id
    } else {
        let hex_str = if request.account_id.starts_with("0x") {
            &request.account_id[2..]
        } else {
            &request.account_id
        };
        let hex_with_prefix = format!("0x{}", hex_str);
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| {
                status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        success: false,
                        error: format!("Failed to parse account_id: {}", e),
                    }),
                )
            })?
    };
    
    // Setup paths
    let current_dir = std::env::current_dir()
        .map_err(|e| {
            status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Failed to get current directory: {}", e),
                }),
            )
        })?;
    
    let project_root = if current_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "rust-backend")
        .unwrap_or(false) {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir
    };
    
    let keystore_path = project_root.join("rust-backend").join("keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
    
    // Get faucet ID from env or use default
    let faucet_id_hex = std::env::var("FAUCET_ID")
        .unwrap_or_else(|_| {
            // Try to get from faucets.db
            use rust_backend::db::faucets::FaucetStore;
            let faucet_store = FaucetStore::new(project_root.join("faucets.db"))
                .ok();
            if let Some(store) = faucet_store {
                if let Ok(Some(faucet_id)) = store.get_faucet_id("zcash") {
                    use miden_objects::utils::Serializable;
                    let faucet_bytes = faucet_id.to_bytes();
                    format!("0x{}", faucet_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        });
    
    println!("[Balance Endpoint] Using faucet ID: {}", faucet_id_hex);
    
    if faucet_id_hex.is_empty() {
        return Err(status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: "Faucet ID not configured. Set FAUCET_ID env var or ensure faucets.db exists.".to_string(),
            }),
        ));
    }
    
    let faucet_id = AccountId::from_hex(&faucet_id_hex)
        .map_err(|e| {
            status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    success: false,
                    error: format!("Invalid faucet ID: {}", e),
                }),
            )
        })?;
    
    // Get balance
    let balance_result = tokio::task::spawn_blocking({
        let keystore_path = keystore_path.clone();
        let store_path = store_path.clone();
        let rpc_url = rpc_url.clone();
        move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            rt.block_on(async {
                get_account_balance_helper(
                    account_id,
                    faucet_id,
                    keystore_path,
                    store_path,
                    &rpc_url,
                )
                .await
            })
        }
    })
    .await
    .map_err(|e| {
        status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: format!("Spawn blocking error: {}", e),
            }),
        )
    })?
    .map_err(|e: String| {
        status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to get balance: {}", e),
            }),
        )
    })?;
    
    Ok(Json(BalanceResponse {
        balance: balance_result.0,
        balance_raw: balance_result.1,
        faucet_id: faucet_id_hex,
        success: true,
    }))
}

// Helper function to get account balance
async fn get_account_balance_helper(
    account_id: AccountId,
    faucet_id: AccountId,
    keystore_path: PathBuf,
    store_path: PathBuf,
    rpc_url: &str,
) -> Result<(String, u64), String> {
    // Initialize full client (needed for private accounts - they're stored locally, not queryable via RPC)
    let endpoint = Endpoint::try_from(rpc_url)
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    
    if !keystore_path.exists() {
        return Err(format!("Keystore directory does not exist: {:?}", keystore_path));
    }
    
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path.clone())
            .map_err(|e| format!("Failed to create keystore at {:?}: {}", keystore_path, e))?,
    );
    
    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore)
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;
    
    // Sync state to get latest account data
    client.sync_state().await
        .map_err(|e| format!("Failed to sync client state: {}", e))?;
    
    // Get account from client store (works for both public and private accounts)
    // Private accounts are stored locally, not queryable via RPC
    let account_record = client.get_account(account_id).await
        .map_err(|e| format!("Failed to get account from client: {}", e))?;
    
    let account_record = account_record
        .ok_or_else(|| {
            format!(
                "Account {} not found in client store. The account must be created and added to the client first.",
                account_id.to_bech32(miden_objects::address::NetworkId::Testnet)
            )
        })?;
    
    // Get the account object from AccountRecord
    // AccountRecord has an account() method that returns &Account
    let account = account_record.account();
    let vault = account.vault();
    
    // Get balance for the faucet
    println!("[Balance Helper] Getting balance for faucet: {}", faucet_id.to_bech32(miden_objects::address::NetworkId::Testnet));
    let balance = vault.get_balance(faucet_id)
        .map_err(|e| format!("Failed to get balance from vault: {:?}", e))?;
    
    println!("[Balance Helper] Raw balance: {}", balance);
    
    // Convert to tokens (8 decimals for wTAZ)
    // get_balance returns u64 directly
    let balance_raw = balance;
    let balance_tokens = balance_raw as f64 / 1e8;
    let balance_str = if balance_tokens % 1.0 == 0.0 {
        format!("{}", balance_tokens as u64)
    } else {
        format!("{}", balance_tokens).trim_end_matches('0').trim_end_matches('.').to_string()
    };
    
    Ok((balance_str, balance_raw))
}

#[options("/pool/balance")]
fn options_pool_balance() -> rocket::http::Status {
    rocket::http::Status::Ok
}

#[post("/pool/balance", format = "json", data = "<request>")]
async fn get_pool_balance(
    state: &rocket::State<State>,
    request: Json<PoolBalanceRequest>,
) -> Result<Json<PoolBalanceResponse>, status::Custom<Json<ErrorResponse>>> {
    // Request is optional (can be empty JSON), we ignore it and use default faucet
    let _ = request;
    // Get Zcash bridge wallet balance (pool balance in TAZ)
    let balance_result = tokio::task::spawn_blocking({
        let bridge_wallet = state.bridge_wallet.clone();
        move || {
            bridge_wallet.get_balance()
        }
    })
    .await
    .map_err(|e| {
        status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: format!("Spawn blocking error: {}", e),
            }),
        )
    })?
    .map_err(|e: String| {
        status::Custom(
            Status::InternalServerError,
            Json(ErrorResponse {
                success: false,
                error: format!("Failed to get pool balance: {}", e),
            }),
        )
    })?;
    
    // Parse TAZ balance string to number
    let balance_str = balance_result.spendable.trim();
    let balance_num: f64 = balance_str.parse().unwrap_or(0.0);
    
    // Convert to base units (8 decimals for wTAZ, but TAZ uses 8 decimals too)
    let balance_raw = (balance_num * 1e8) as u64;
    
    Ok(Json(PoolBalanceResponse {
        balance: balance_str.to_string(),
        balance_raw,
        faucet_id: "zcash".to_string(), // Not applicable for Zcash balance
        success: true,
    }))
}

#[options("/withdrawal/create")]
fn options_withdrawal_create() -> rocket::http::Status {
    rocket::http::Status::Ok
}

#[post("/withdrawal/create", format = "json", data = "<request>")]
async fn create_withdrawal(
    _state: &rocket::State<State>,
    request: Json<WithdrawalRequest>,
) -> Result<Json<WithdrawalResponse>, status::Custom<Json<ErrorResponse>>> {
    // Request is required but not yet implemented
    let _ = request;
    // TODO: Implement withdrawal note creation and consumption
    // For now, return error indicating it's not yet implemented
    Err(status::Custom(
        Status::NotImplemented,
        Json(ErrorResponse {
            success: false,
            error: "Withdrawal functionality not yet implemented. Need to compile CROSSCHAIN script first.".to_string(),
        }),
    ))
}

#[launch]
fn rocket() -> _ {
    // Load .env file from project root (works whether running from root or rust-backend)
    let current_dir = std::env::current_dir()
        .expect("Failed to get current directory");
    
    // Try to load .env from root directory (go up one level if we're in rust-backend)
    let env_path = if current_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "rust-backend")
        .unwrap_or(false) {
        current_dir.parent().unwrap().join(".env")
    } else {
        current_dir.join(".env")
    };
    
    if env_path.exists() {
        dotenv::from_path(&env_path).ok();
        println!("Loaded .env from: {:?}", env_path);
    } else {
        // Also try .env in current directory
        dotenv::dotenv().ok();
    }
    
    // Set project_root - if we're in rust-backend, go up one level, otherwise use current dir
    let project_root = if current_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "rust-backend")
        .unwrap_or(false) {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir.clone()
    };
    
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
    
    // Initialize bridge wallet (project_root already set above)
    let bridge_wallet = Arc::new(BridgeWallet::new(project_root.clone()));
    
    // Initialize deposit tracker database
    let db_path = project_root.join("deposits.db");
    let deposit_tracker = DepositTracker::new(db_path)
        .expect("Failed to initialize deposit tracker database");
    
    // Deploy wTAZ faucet on startup if it doesn't exist
    println!("[Server] Checking for wTAZ faucet...");
    let keystore_path = PathBuf::from("./keystore");
    let store_path = project_root.join("bridge_store.sqlite3");
    let faucet_store_path = project_root.join("faucets.db");
    let rpc_url_clone = rpc_url.clone();
    
    // Deploy faucet synchronously using a new runtime
    let faucet_result = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            rust_backend::bridge::deposit::get_or_create_zcash_faucet(
                keystore_path,
                store_path,
                &rpc_url_clone,
                faucet_store_path,
            )
            .await
        })
    })
    .join();
    
    match faucet_result {
        Ok(Ok(faucet_id)) => {
            let faucet_bech32 = faucet_id.to_bech32(NetworkId::Testnet);
            use miden_objects::utils::Serializable;
            let faucet_bytes = faucet_id.to_bytes();
            let faucet_hex: String = faucet_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            println!("[Server] ✅ wTAZ Faucet ready:");
            println!("[Server]    Bech32: {}", faucet_bech32);
            println!("[Server]    Hex:    0x{}", faucet_hex);
            println!("[Server]    Use this faucet ID for .mno files and UI balance display");
        }
        Ok(Err(e)) => {
            eprintln!("[Server] ⚠️  Failed to deploy faucet: {}", e);
            eprintln!("[Server]    Faucet will be created on first deposit");
        }
        Err(e) => {
            eprintln!("[Server] ⚠️  Failed to spawn faucet deployment task: {:?}", e);
        }
    }
    
    // Allow port to be configured via ROCKET_PORT env var, default to 8001
    let port = std::env::var("ROCKET_PORT")
        .unwrap_or_else(|_| "8001".to_string())
        .parse::<u16>()
        .unwrap_or(8001);
    
    println!("[Server] Rocket server starting on http://127.0.0.1:{}", port);
    rocket::build()
        .configure(rocket::Config::figment().merge(("port", port)))
        .manage(State {
            rpc,
            keystore,
            bridge_wallet,
            deposit_tracker: Arc::new(Mutex::new(deposit_tracker)),
        })
        .mount("/", routes![get_block, health, options_create_account, create_account, create_faucet, mint_from_faucet, options_hash, get_hash_endpoint, generate_hash_endpoint, options_claim, claim_deposit_endpoint, reconstruct_note_endpoint, consume_note_endpoint, options_account_balance, get_account_balance, options_pool_balance, get_pool_balance, options_withdrawal_create, create_withdrawal])
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

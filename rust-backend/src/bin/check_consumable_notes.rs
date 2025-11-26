use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_lib::note::utils::build_p2id_recipient;
use miden_objects::{
    account::AccountId,
    address::NetworkId,
    note::NoteTag,
    Word,
};
use rand::rngs::StdRng;
use rust_backend::miden::notes::BRIDGE_USECASE;
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Increase stack size to 8MB to avoid stack overflow on Windows
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024) // 8MB stack
        .spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(run_check())
        })
        .map_err(|e| format!("Failed to spawn thread: {}", e))?;
    
    result.join()
        .map_err(|e| format!("Thread panicked: {:?}", e))?
}

async fn run_check() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Check Consumable Notes ===\n");

    // Get wallet account ID and optional secret from command line args
    let args: Vec<String> = std::env::args().collect();
    let wallet_hex = args.get(1)
        .cloned()
        .unwrap_or_else(|| "0x15b60587076ae990231575179eb3ce".to_string());
    let secret_hex = args.get(2).cloned(); // Optional secret for P2ID notes
    
    println!("Checking consumable notes for wallet: {}", wallet_hex);
    if let Some(ref secret) = secret_hex {
        println!("With secret (for P2ID notes): {}\n", secret);
    } else {
        println!("(No secret provided - will only find public notes)\n");
    }

    // Parse account ID
    let wallet_id = if wallet_hex.starts_with("mtst") {
        // Bech32 format
        AccountId::from_bech32(&wallet_hex)
            .map_err(|e| format!("Failed to parse bech32 account ID: {}", e))?
            .1
    } else {
        // Hex format - ensure it has 0x prefix (AccountId::from_hex expects it)
        let hex_with_prefix = if wallet_hex.starts_with("0x") {
            wallet_hex.clone()
        } else {
            format!("0x{}", wallet_hex)
        };
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Failed to parse hex account ID: {}", e))?
    };

    println!("Wallet Account ID:");
    println!("   Hex: {}", wallet_id.to_hex());
    println!("   Bech32: {}\n", wallet_id.to_bech32(NetworkId::Testnet));

    // Setup paths
    let test_dir = PathBuf::from("./test_wallet");
    std::fs::create_dir_all(&test_dir).ok();
    let keystore_path = test_dir.join("keystore");
    let store_path = test_dir.join("test_store.sqlite3");
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());

    println!("[1] Initializing Miden client...");
    let endpoint = Endpoint::try_from(rpc_url.as_str())
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path.clone())
            .map_err(|e| format!("Failed to create keystore: {}", e))?,
    );

    let mut client = ClientBuilder::new()
        .rpc(rpc_client.clone())
        .sqlite_store(store_path.clone())
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;

    // Add note tag so client tracks notes with BRIDGE_USECASE tag
    let bridge_tag = NoteTag::for_local_use_case(BRIDGE_USECASE, 0)
        .map_err(|e| format!("Failed to create bridge tag: {:?}", e))?;
    client.add_note_tag(bridge_tag).await
        .map_err(|e| format!("Failed to add note tag: {}", e))?;

    println!("✅ Client initialized\n");

    // Create or load wallet account
    println!("[2] Setting up wallet account...");
    
    // Note: We don't actually need to create the account, just use the wallet_id
    // for get_consumable_notes. The account just needs to exist on-chain.
    
    println!("✅ Wallet account setup (using provided account ID)\n");

    println!("[3] Syncing state...");
    client.sync_state().await.map_err(|e| format!("Failed to sync: {}", e))?;
    println!("✅ State synced\n");

    println!("[4] Getting consumable notes...");
    let consumable_notes = client
        .get_consumable_notes(Some(wallet_id))
        .await
        .map_err(|e| format!("Failed to get consumable notes: {}", e))?;

    println!("✅ Found {} consumable note(s) (public notes only)\n", consumable_notes.len());

    // If secret is provided, reconstruct the P2ID note
    if let Some(ref secret_hex) = secret_hex {
        println!("[5] Reconstructing P2ID note with secret...");
        
        // Parse secret
        let secret_hex_clean = if secret_hex.starts_with("0x") {
            secret_hex.clone()
        } else {
            format!("0x{}", secret_hex)
        };
        let secret = Word::try_from(secret_hex_clean.as_str())
            .map_err(|e| format!("Failed to parse secret: {}", e))?;
        
        // Build recipient
        let recipient = build_p2id_recipient(wallet_id, secret)
            .map_err(|e| format!("Failed to build recipient: {:?}", e))?;
        let recipient_hash = recipient.digest();
        
        println!("   ✅ Recipient hash: {}", recipient_hash.to_hex());
        println!("\n   ⚠️  IMPORTANT: P2ID notes are PRIVATE and cannot be found via");
        println!("   get_consumable_notes(). The client needs the note registered in its");
        println!("   store to find it. However, you can use the reconstructed note directly");
        println!("   in a transaction to consume it.");
        println!("\n   To consume this note, you need:");
        println!("   1. The reconstructed note (with account_id + secret)");
        println!("   2. The correct assets and metadata");
        println!("   3. Use it directly in a consume transaction\n");
        
        // Note: We can't actually check if the note exists on-chain without
        // knowing the assets/faucet_id/amount. The user needs to provide those
        // to fully reconstruct the note and check if it's consumable.
    }

    if consumable_notes.is_empty() {
        println!("⚠️  No public consumable notes found for this wallet.");
        if secret_hex.is_none() {
            println!("   Note: For P2ID (private) notes, you need to provide the secret:");
            println!("   cargo run --bin check_consumable_notes -- <wallet_id> <secret>");
        }
        println!("   Make sure:");
        println!("   - The wallet account ID is correct");
        println!("   - Notes have been minted to this wallet");
        println!("   - Enough time has passed for notes to be on-chain");
        println!("   - The note tag matches (BRIDGE_USECASE: {})", BRIDGE_USECASE);
    } else {
        println!("📋 Consumable Notes (Public):");
        for (i, (note, _)) in consumable_notes.iter().enumerate() {
        println!("\n   Note #{}:", i + 1);
        println!("   - Note ID: {}", note.id().to_hex());
        
        if let Some(metadata) = note.metadata() {
            println!("   - Sender: {}", metadata.sender().to_hex());
            println!("   - Tag: {:?}", metadata.tag());
            println!("   - Type: {:?}", metadata.note_type());
        }
        
        // Show assets
        let assets = note.assets();
        println!("   - Assets: {} asset(s)", assets.num_assets());
        }
    }

    println!("\n=== Check Complete ===");
    Ok(())
}


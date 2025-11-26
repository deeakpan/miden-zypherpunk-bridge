use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::TransactionRequestBuilder,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_objects::{
    account::AccountId,
    note::NoteTag,
    Word,
};
use rand::rngs::StdRng;
use rust_backend::miden::notes::{reconstruct_deposit_note, BRIDGE_USECASE};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(run_consume())
        })
        .unwrap()
        .join()
        .unwrap();

    result
}

async fn run_consume() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Consume P2ID Note ===\n");

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        println!("Usage: cargo run --bin consume_note -- <wallet_id> <secret> <faucet_id> <amount>");
        println!("Example: cargo run --bin consume_note -- 0x72eb70ee509f749031b3b79e92c337 0x28aaea4ab31c6e5ead3021334aec89a01deb619a19c575d8ef2b6c04b136e9ff mtst1aq970sz4muwgugppxfqytu8knve4zt0t 1000");
        return Ok(());
    }

    let wallet_hex = &args[1];
    let secret_hex = &args[2];
    let faucet_hex = &args[3];
    let amount: u64 = args[4].parse()
        .map_err(|e| format!("Failed to parse amount: {}", e))?;

    println!("Wallet ID: {}", wallet_hex);
    println!("Secret: {}", secret_hex);
    println!("Faucet ID: {}", faucet_hex);
    println!("Amount: {}\n", amount);

    // Parse wallet ID
    let wallet_id = if wallet_hex.starts_with("mtst") {
        AccountId::from_bech32(wallet_hex)
            .map_err(|e| format!("Failed to parse wallet bech32: {}", e))?
            .1
    } else {
        let hex_with_prefix = if wallet_hex.starts_with("0x") {
            wallet_hex.clone()
        } else {
            format!("0x{}", wallet_hex)
        };
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Failed to parse wallet hex: {}", e))?
    };

    // Parse faucet ID
    let faucet_id = if faucet_hex.starts_with("mtst") {
        AccountId::from_bech32(faucet_hex)
            .map_err(|e| format!("Failed to parse faucet bech32: {}", e))?
            .1
    } else {
        let hex_with_prefix = if faucet_hex.starts_with("0x") {
            faucet_hex.clone()
        } else {
            format!("0x{}", faucet_hex)
        };
        AccountId::from_hex(&hex_with_prefix)
            .map_err(|e| format!("Failed to parse faucet hex: {}", e))?
    };

    // Parse secret
    let secret_hex_clean = if secret_hex.starts_with("0x") {
        secret_hex.clone()
    } else {
        format!("0x{}", secret_hex)
    };
    let secret = Word::try_from(secret_hex_clean.as_str())
        .map_err(|e| format!("Failed to parse secret: {}", e))?;

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

    client.add_note_tag(NoteTag::for_local_use_case(BRIDGE_USECASE, 0).expect("Bridge use case tag should be valid")).await?;

    println!("✅ Client initialized\n");

    // Reconstruct the note
    println!("[2] Reconstructing P2ID note...");
    let note = reconstruct_deposit_note(wallet_id, secret, faucet_id, amount)
        .map_err(|e| format!("Failed to reconstruct note: {:?}", e))?;
    let note_id = note.id();
    let note_id_hex = note_id.to_hex();
    println!("✅ Note reconstructed:");
    println!("   Note ID: {}\n", note_id_hex);


    // Get wallet account (needed for consuming)
    println!("[3] Setting up wallet account...");
    let wallet_account = client.get_account(wallet_id).await
        .map_err(|e| format!("Failed to get account: {}", e))?;
    
    if let Some(_acc) = wallet_account {
        println!("   Found existing wallet account in store");
    } else {
        println!("   Wallet account not in store - you need to add it first");
        println!("   The wallet account must exist on-chain and be added to the client");
        return Err("Wallet account not found in client store. Add it first.".into());
    };

    println!("✅ Wallet account ready\n");

    // Build consume transaction using unauthenticated_input_notes (like custom notes)
    println!("[4] Building consume transaction...");
    // For P2ID notes, use unauthenticated_input_notes with the note and secret
    // This is the pattern shown in the custom note tutorial
    println!("   Using unauthenticated_input_notes with note and secret...");
    let secret_word: miden_objects::Word = secret;
    let tx_request = TransactionRequestBuilder::new()
        .unauthenticated_input_notes([(note, Some(secret_word.into()))])
        .build()
        .map_err(|e| format!("Failed to build transaction: {:?}", e))?;

    println!("✅ Transaction built\n");

    // Submit transaction (using submit_new_transaction like in the docs)
    println!("[5] Submitting transaction...");
    let tx_id = client
        .submit_new_transaction(wallet_id, tx_request)
        .await
        .map_err(|e| format!("Failed to submit transaction: {}", e))?;
    println!("✅ Transaction submitted!");
    println!("   TX ID: {:?}\n", tx_id);

    println!("=== Consume Complete ===");
    Ok(())
}


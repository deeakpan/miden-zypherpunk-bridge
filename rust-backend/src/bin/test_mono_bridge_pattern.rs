use miden_client::{
    account::component::{BasicFungibleFaucet, BasicWallet},
    auth::AuthSecretKey,
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
};
use rust_backend::bridge::deposit::mint_deposit_note;
use rust_backend::miden::notes::BRIDGE_USECASE;
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_lib::account::auth::AuthRpoFalcon512;
use miden_lib::note::utils::build_p2id_recipient;
use miden_objects::{
    account::{AccountBuilder, AccountStorageMode, AccountType},
    address::NetworkId,
    asset::{Asset, FungibleAsset},
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteMetadata, NoteTag, NoteType,
    },
    FieldElement, Felt, Word,
};
use rand::rngs::StdRng;
use rand::RngCore;
use rand::rng;
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Increase stack size to 8MB to avoid stack overflow on Windows
    let result = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024) // 8MB stack
        .spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(run_test())
        })
        .map_err(|e| format!("Failed to spawn thread: {}", e))?;
    
    result.join()
        .map_err(|e| format!("Thread panicked: {:?}", e))?
}

async fn run_test() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Testing Mono Bridge Pattern ===\n");

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

    // Step 1: Create private wallet account
    println!("[2] Creating private wallet account...");
    let mut seed = [0u8; 32];
    rng().fill_bytes(&mut seed);
    let key_pair = AuthSecretKey::new_rpo_falcon512();

    let wallet_account = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Private)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .map_err(|e| format!("Failed to build wallet: {}", e))?;

    // Don't add wallet to client yet - we'll add it later for consuming
    // This matches mint_deposit_note_from_hash which doesn't have the recipient account
    keystore
        .add_key(&key_pair)
        .map_err(|e| format!("Failed to add key: {}", e))?;

    let wallet_id = wallet_account.id();
    let wallet_hex = wallet_id.to_hex();
    let wallet_bech32 = wallet_id.to_bech32(NetworkId::Testnet);

    println!("✅ Wallet created:");
    println!("   Hex: {}", wallet_hex);
    println!("   Bech32: {}\n", wallet_bech32);

    // Step 2: Generate recipient hash (like mono bridge CLI does)
    println!("[3] Generating recipient hash (account_id + secret)...");
    let secret: Word = {
        use miden_objects::crypto::rand::{FeltRng, RpoRandomCoin};
        let mut rng = RpoRandomCoin::new(Word::new([
            Felt::new(rand::random::<u64>()),
            Felt::new(rand::random::<u64>()),
            Felt::new(rand::random::<u64>()),
            Felt::new(rand::random::<u64>()),
        ]));
        rng.draw_word()
    };

    let recipient = build_p2id_recipient(wallet_id, secret)
        .map_err(|e| format!("Failed to build recipient: {:?}", e))?;
    let recipient_hash = recipient.digest();
    let recipient_hash_hex = recipient_hash.to_hex();

    println!("✅ Recipient hash generated:");
    println!("   Account ID: {}", wallet_hex);
    println!("   Secret: {}", secret.to_hex());
    println!("   Recipient Hash: {}\n", recipient_hash_hex);

    // Step 3: Create faucet
    println!("[4] Creating faucet...");
    let mut faucet_seed = [0u8; 32];
    rng().fill_bytes(&mut faucet_seed);
    let faucet_key = AuthSecretKey::new_rpo_falcon512();

    let faucet_account = AccountBuilder::new(faucet_seed)
        .account_type(AccountType::FungibleFaucet)
        .storage_mode(AccountStorageMode::Public)
        .with_auth_component(AuthRpoFalcon512::new(faucet_key.public_key().to_commitment()))
        .with_component(BasicFungibleFaucet::new(
            miden_objects::asset::TokenSymbol::new("TEST").unwrap(),
            8,
            Felt::new(1_000_000),
        ).unwrap())
        .build()
        .map_err(|e| format!("Failed to build faucet: {}", e))?;

    client
        .add_account(&faucet_account, false)
        .await
        .map_err(|e| format!("Failed to add faucet: {}", e))?;

    keystore
        .add_key(&faucet_key)
        .map_err(|e| format!("Failed to add faucet key: {}", e))?;

    let faucet_id = faucet_account.id();
    println!("✅ Faucet created: {}\n", faucet_id.to_bech32(NetworkId::Testnet));

    // Sync to deploy faucet
    client.sync_state().await.map_err(|e| format!("Failed to sync: {}", e))?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Step 4: Mint note using normal mint_deposit_note (with full recipient)
    // This uses OutputNote::Full instead of PartialNote
    println!("[5] Minting note with full recipient (account_id + secret)...");
    println!("   Using mint_deposit_note function (normal, not hash-only)...");
    
    let amount = 1000u64;
    let (note_id, tx_id) = mint_deposit_note(
        wallet_id,  // account_id
        secret,      // secret
        faucet_id,
        amount,
        keystore_path.clone(),
        store_path.clone(),
        &rpc_url,
    )
    .await
    .map_err(|e| format!("Failed to mint note: {}", e))?;
    
    println!("✅ Transaction executed and submitted!");
    println!("   TX ID: {}", tx_id);
    println!("   Note ID: {}\n", note_id);

    // Step 5: Reconstruct the note client-side (like mono bridge reconstruct command)
    println!("[6] Reconstructing note client-side (account_id + secret)...");
    let reconstructed_recipient = build_p2id_recipient(wallet_id, secret)
        .map_err(|e| format!("Failed to rebuild recipient: {:?}", e))?;

    let reconstructed_assets = NoteAssets::new(vec![Asset::from(
        FungibleAsset::new(faucet_id, amount)
            .map_err(|e| format!("Failed to create asset: {}", e))?,
    )])
    .map_err(|e| format!("Failed to create assets: {}", e))?;

    let reconstructed_metadata = NoteMetadata::new(
        faucet_id,
        NoteType::Private,
        NoteTag::for_local_use_case(BRIDGE_USECASE, 0)
            .map_err(|e| format!("Invalid tag: {:?}", e))?,
        NoteExecutionHint::always(),
        Felt::ZERO,
    )
    .map_err(|e| format!("Failed to create metadata: {}", e))?;

    let reconstructed_note = Note::new(
        reconstructed_assets,
        reconstructed_metadata,
        reconstructed_recipient,
    );

    let reconstructed_note_id = reconstructed_note.id().to_hex();
    println!("✅ Note reconstructed:");
    println!("   Note ID: {}", reconstructed_note_id);
    println!("   Matches minted note: {}\n", note_id == reconstructed_note_id);

    // Step 6: Verify note can be found (consumption can be done manually)
    println!("[7] Verifying note can be found...");
    println!("   Adding wallet account to client (needed for get_consumable_notes)...");
    client
        .add_account(&wallet_account, false)
        .await
        .map_err(|e| format!("Failed to add wallet: {}", e))?;
    
    // IMPORTANT: For P2ID notes, the client needs the secret to compute the recipient hash
    // and match it against notes on-chain. The client can't find P2ID notes without the secret.
    // We've reconstructed the note with the secret above. The client should be able to find it
    // when syncing if the note tag matches and the account is added, but it may need the note
    // to be registered with the client first (which requires the secret).
    // For P2ID notes, you typically need to reconstruct the note client-side with the secret
    // and then the client can find it during sync.
    
    println!("   Waiting 10 seconds for note to be available on-chain...");
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    println!("   Syncing state...");
    client.sync_state().await.map_err(|e| format!("Failed to sync: {}", e))?;

    // Get consumable notes
    let consumable_notes = client
        .get_consumable_notes(Some(wallet_id))
        .await
        .map_err(|e| format!("Failed to get consumable notes: {}", e))?;

    println!("   Found {} consumable notes", consumable_notes.len());

    if consumable_notes.iter().any(|(n, _)| n.id().to_hex() == note_id) {
        println!("✅ Note found in consumable notes!");
        println!("   You can consume it manually using the reconstructed note data.\n");
    } else {
        println!("⚠️  Note not found in consumable notes yet.");
        println!("   It may need more time to sync, or you may need to reconstruct it client-side.\n");
    }

    println!("📝 Summary:");
    println!("   - Wallet created: {} (hex: {})", wallet_bech32, wallet_hex);
    println!("   - Secret: {}", secret.to_hex());
    println!("   - Recipient hash: {}", recipient_hash_hex);
    println!("   - Note ID: {}", note_id);
    println!("   - Reconstructed note ID: {}", reconstructed_note_id);
    println!("   - Match: {}\n", note_id == reconstructed_note_id);

    println!("=== Test Complete ===");
    Ok(())
}


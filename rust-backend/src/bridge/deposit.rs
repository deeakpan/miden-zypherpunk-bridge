use crate::account::create::create_faucet_account;
use crate::db::faucets::FaucetStore;
use crate::miden::recipient::build_deposit_recipient;
use crate::zcash::bridge_wallet::BridgeWallet;
use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    transaction::{OutputNote, TransactionRequestBuilder},
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_objects::{
    account::AccountId,
    asset::FungibleAsset,
    note::{Note, NoteAssets, NoteExecutionHint, NoteMetadata, NoteTag, NoteType},
    FieldElement, Felt, Word,
};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimDepositRequest {
    pub account_id: String,
    pub secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimDepositResponse {
    pub success: bool,
    pub note_id: Option<String>,
    pub transaction_id: Option<String>,
    pub message: String,
}

/// Scan bridge Zcash testnet wallet for deposits with a specific memo (recipient hash)
pub async fn scan_zcash_deposits(
    bridge_wallet: &BridgeWallet,
    recipient_hash: &str,
    bridge_address: &str,
) -> Result<Option<(String, u64)>, String> {
    // First, enhance transactions to get memo data
    bridge_wallet.enhance_transactions()
        .map_err(|e| format!("Failed to enhance transactions: {}", e))?;
    
    // List transactions from bridge wallet
    let tx_output = bridge_wallet.list_transactions(None)
        .map_err(|e| format!("Failed to list transactions: {}", e))?;
    
    // Parse transactions
    let transactions = bridge_wallet.parse_transactions(&tx_output)
        .map_err(|e| format!("Failed to parse transactions: {}", e))?;
    
    // Find transaction with matching memo and to bridge address
    for tx in transactions {
        // Check if memo matches recipient_hash
        if let Some(memo) = &tx.memo {
            if memo.trim() == recipient_hash.trim() {
                // Check if it's to the bridge address
                if let Some(to_addr) = &tx.to_address {
                    if to_addr == bridge_address {
                        return Ok(Some((tx.txid, tx.amount)));
                    }
                }
                // Also check if amount > 0 (valid deposit)
                if tx.amount > 0 {
                    return Ok(Some((tx.txid, tx.amount)));
                }
            }
        }
    }
    
    Ok(None)
}

/// Get or create faucet for Zcash testnet
/// Returns the faucet_id, creating it if it doesn't exist
pub async fn get_or_create_zcash_faucet(
    keystore_path: PathBuf,
    store_path: PathBuf,
    rpc_url: &str,
    faucet_store_path: PathBuf,
) -> Result<AccountId, String> {
    // Open faucet store
    let faucet_store = FaucetStore::new(faucet_store_path)
        .map_err(|e| format!("Failed to open faucet store: {}", e))?;
    
    // Check if faucet exists
    const ZCASH_ORIGIN_NETWORK: &str = "zcash_testnet";
    if let Some(faucet_id) = faucet_store.get_faucet_id(ZCASH_ORIGIN_NETWORK)
        .map_err(|e| format!("Failed to query faucet store: {}", e))? {
        return Ok(faucet_id);
    }
    
    // Faucet doesn't exist, create it
    println!("[Bridge] Creating Zcash testnet faucet (wTAZ)...");
    let faucet_id_bech32 = create_faucet_account(
        &keystore_path,
        &store_path,
        rpc_url,
        "TAZ",  // Symbol
        8,      // Decimals (same as Zcash)
        1_000_000_000_000_000_000u64, // Max supply (1 billion TAZ)
    )
    .await
    .map_err(|e| format!("Failed to create faucet: {}", e))?;
    
    // Parse faucet_id from bech32
    let faucet_id = AccountId::from_bech32(&faucet_id_bech32)
        .map_err(|e| format!("Failed to parse faucet_id: {}", e))?
        .1;
    
    // Get hex representation for logging
    use miden_objects::utils::Serializable;
    let faucet_bytes = faucet_id.to_bytes();
    let faucet_hex: String = faucet_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let faucet_hex_padded = if faucet_hex.len() < 30 {
        format!("{:0>30}", faucet_hex)
    } else {
        faucet_hex
    };
    
    // Store faucet_id in database
    faucet_store.store_faucet_id(ZCASH_ORIGIN_NETWORK, &faucet_id)
        .map_err(|e| format!("Failed to store faucet_id: {}", e))?;
    
    println!("[Bridge] ✅ Created and stored Zcash testnet faucet:");
    println!("[Bridge]    Bech32: {}", faucet_id_bech32);
    println!("[Bridge]    Hex:    0x{}", faucet_hex_padded);
    println!("[Bridge]    Database: faucets.db");
    Ok(faucet_id)
}

/// Mint a deposit note from recipient hash (automatic minting by relayer)
/// 
/// This is called automatically by the relayer when it detects a deposit.
/// The user just needs to sync and consume the note.
pub async fn mint_deposit_note_from_hash(
    recipient_hash: Word,
    faucet_id: AccountId,
    amount: u64,
    keystore_path: PathBuf,
    store_path: PathBuf,
    rpc_url: &str,
) -> Result<(String, String), String> {
    // Initialize Miden client
    let endpoint = Endpoint::try_from(rpc_url)
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path)
            .map_err(|e| format!("Failed to create keystore: {}", e))?,
    );
    
    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;
    
    // Create asset (wTAZ tokens)
    let asset = FungibleAsset::new(faucet_id, amount)
        .map_err(|e| format!("Failed to create asset: {}", e))?;
    
    let assets = NoteAssets::new(vec![asset.into()])
        .map_err(|e| format!("Failed to create note assets: {}", e))?;
    
    // Create note metadata
    use crate::miden::notes::BRIDGE_USECASE;
    let metadata = NoteMetadata::new(
        faucet_id, // Sender is the faucet
        NoteType::Private,
        NoteTag::for_local_use_case(BRIDGE_USECASE, 0)
            .map_err(|e| format!("Invalid tag: {:?}", e))?,
        NoteExecutionHint::always(),
        Felt::ZERO,
    )
    .map_err(|e| format!("Failed to create metadata: {}", e))?;
    
    // Create transaction to mint note with recipient hash
    // Following mono bridge pattern exactly: use recipient_hash.into() to match their code
    // Note: The mono bridge uses recipient.into() where recipient is Word
    let tx_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Partial(
            miden_objects::note::PartialNote::new(metadata, recipient_hash.into(), assets),
        )])
        .build()
        .map_err(|e| format!("Failed to build transaction: {}", e))?;
    
    // Execute transaction
    let tx_result = client
        .execute_transaction(faucet_id, tx_request)
        .await
        .map_err(|e| format!("Failed to execute transaction: {}", e))?;
    
    // Prove transaction
    let proven_tx = client
        .prove_transaction(&tx_result)
        .await
        .map_err(|e| format!("Failed to prove transaction: {}", e))?;
    
    // Submit transaction
    let submission_height = client
        .submit_proven_transaction(proven_tx, &tx_result)
        .await
        .map_err(|e| format!("Failed to submit transaction: {}", e))?;
    
    // Apply transaction
    client
        .apply_transaction(&tx_result, submission_height)
        .await
        .map_err(|e| format!("Failed to apply transaction: {}", e))?;
    
    // Get note ID and transaction ID
    let note_id = tx_result
        .created_notes()
        .get_note(0)
        .id()
        .to_hex();
    let tx_id = tx_result.executed_transaction().id().to_hex();
    
    Ok((note_id, tx_id))
}

/// Mint a deposit note (privacy-preserving: account_id not stored)
/// 
/// The bridge mints a P2ID note with the recipient. The user will scan
/// and consume the note client-side using WebClient.getConsumableNotes().
/// 
/// NOTE: account_id is required to build the P2ID recipient, but it's
/// NOT stored in the database for privacy.
pub async fn mint_deposit_note(
    account_id: AccountId,
    secret: Word,
    faucet_id: AccountId,
    amount: u64,
    keystore_path: PathBuf,
    store_path: PathBuf,
    rpc_url: &str,
) -> Result<(String, String), String> {
    // Initialize Miden client
    let endpoint = Endpoint::try_from(rpc_url)
        .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
    
    let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
    let keystore = Arc::new(
        FilesystemKeyStore::<StdRng>::new(keystore_path)
            .map_err(|e| format!("Failed to create keystore: {}", e))?,
    );
    
    let mut client = ClientBuilder::new()
        .rpc(rpc_client)
        .sqlite_store(store_path)
        .authenticator(keystore.clone())
        .in_debug_mode(true.into())
        .build()
        .await
        .map_err(|e| format!("Failed to build client: {}", e))?;
    
    // Create asset (wTAZ tokens)
    let asset = FungibleAsset::new(faucet_id, amount)
        .map_err(|e| format!("Failed to create asset: {}", e))?;
    
    let assets = NoteAssets::new(vec![asset.into()])
        .map_err(|e| format!("Failed to create note assets: {}", e))?;
    
    // Create note metadata
    // Use BRIDGE_USECASE tag (20050519) for our bridge
    use crate::miden::notes::BRIDGE_USECASE;
    let metadata = NoteMetadata::new(
        faucet_id, // Sender is the faucet
        NoteType::Private,
        NoteTag::for_local_use_case(BRIDGE_USECASE, 0)
            .map_err(|e| format!("Invalid tag: {:?}", e))?,
        NoteExecutionHint::always(),
        Felt::ZERO,
    )
    .map_err(|e| format!("Failed to create metadata: {}", e))?;
    
    // Build recipient - P2ID note (requires account_id + secret)
    // Log account_id for debugging
    use miden_objects::utils::Serializable;
    let account_bytes = account_id.to_bytes();
    let account_hex: String = account_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    println!("[Bridge] Minting note for account_id:");
    println!("[Bridge]   Hex: 0x{}", account_hex);
    println!("[Bridge]   Bech32: {}", account_id.to_bech32(miden_objects::address::NetworkId::Testnet));
    
    let recipient = build_deposit_recipient(account_id, secret)
        .map_err(|e| format!("Failed to build recipient: {}", e))?;
    
    println!("[Bridge]   Recipient digest: {}", recipient.digest().to_hex());
    
    // Create a complete note with full recipient (as per Miden docs)
    let note = Note::new(assets, metadata, recipient);
    
    // Create transaction to mint note using OutputNote::Full (complete note)
    let tx_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Full(note)])
        .build()
        .map_err(|e| format!("Failed to build transaction: {}", e))?;
    
    // Execute transaction
    let tx_result = client
        .execute_transaction(faucet_id, tx_request)
        .await
        .map_err(|e| format!("Failed to execute transaction: {}", e))?;
    
    // Prove transaction
    let proven_tx = client
        .prove_transaction(&tx_result)
        .await
        .map_err(|e| format!("Failed to prove transaction: {}", e))?;
    
    // Submit transaction
    let submission_height = client
        .submit_proven_transaction(proven_tx, &tx_result)
        .await
        .map_err(|e| format!("Failed to submit transaction: {}", e))?;
    
    // Apply transaction
    client
        .apply_transaction(&tx_result, submission_height)
        .await
        .map_err(|e| format!("Failed to apply transaction: {}", e))?;
    
    // Get note ID and transaction ID
    let note_id = tx_result
        .created_notes()
        .get_note(0)
        .id()
        .to_hex();
    let tx_id = tx_result.executed_transaction().id().to_hex();
    
    Ok((note_id, tx_id))
}


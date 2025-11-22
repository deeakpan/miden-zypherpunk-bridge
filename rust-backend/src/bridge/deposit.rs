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
    note::{NoteAssets, NoteExecutionHint, NoteMetadata, NoteTag, NoteType},
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
    let metadata = NoteMetadata::new(
        faucet_id, // Sender is the faucet
        NoteType::Private,
        NoteTag::for_local_use_case(1, 0)
            .map_err(|e| format!("Invalid tag: {:?}", e))?,
        NoteExecutionHint::always(),
        Felt::ZERO,
    )
    .map_err(|e| format!("Failed to create metadata: {}", e))?;
    
    // Build recipient - P2ID note (requires account_id + secret)
    let recipient = build_deposit_recipient(account_id, secret)
        .map_err(|e| format!("Failed to build recipient: {}", e))?;
    
    // Create transaction to mint note
    let tx_request = TransactionRequestBuilder::new()
        .own_output_notes(vec![OutputNote::Partial(
            miden_objects::note::PartialNote::new(metadata, recipient.digest(), assets),
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


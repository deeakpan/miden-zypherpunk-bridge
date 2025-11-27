use crate::zcash::bridge_wallet::BridgeWallet;
use miden_client::{
    builder::ClientBuilder,
    keystore::FilesystemKeyStore,
    rpc::{Endpoint, GrpcClient},
    store::NoteFilter,
};
use miden_client_sqlite_store::ClientBuilderSqliteExt;
use miden_objects::{
    note::NoteTag,
};
use rand::rngs::StdRng;
use crate::miden::notes::{BRIDGE_USECASE, decode_zcash_address};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, interval};

/// Miden exit relayer that polls for burn notes and sends Zcash transactions
pub struct MidenExitRelayer {
    bridge_wallet: Arc<BridgeWallet>,
    project_root: PathBuf,
    scan_interval: Duration,
    processed_note_ids: Arc<Mutex<HashSet<String>>>,
    last_scanned_block: Arc<Mutex<u32>>,
}

impl MidenExitRelayer {
    pub fn new(
        bridge_wallet: Arc<BridgeWallet>,
        project_root: PathBuf,
        scan_interval_secs: u64,
    ) -> Self {
        Self {
            bridge_wallet,
            project_root,
            scan_interval: Duration::from_secs(scan_interval_secs),
            processed_note_ids: Arc::new(Mutex::new(HashSet::new())),
            last_scanned_block: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn start(&self) {
        println!("[Miden Exit Relayer] Starting...");
        let mut interval = interval(self.scan_interval);

        loop {
            interval.tick().await;
            if let Err(e) = self.scan_and_process_exits().await {
                eprintln!("[Miden Exit Relayer] Error: {}", e);
            }
        }
    }

    async fn scan_and_process_exits(&self) -> Result<(), String> {
        println!("[Miden Exit Relayer] Scanning for exit events...");

        // Initialize Miden client
        let endpoint = Endpoint::try_from("https://rpc.testnet.miden.io")
            .map_err(|e| format!("Failed to parse RPC endpoint: {}", e))?;
        
        let rpc_client = Arc::new(GrpcClient::new(&endpoint, 10_000));
        let keystore_path = self.project_root.join("rust-backend").join("keystore");
        let store_path = self.project_root.join("bridge_store.sqlite3");

        if !keystore_path.exists() {
            return Err("Keystore directory does not exist".to_string());
        }

        let keystore = Arc::new(
            FilesystemKeyStore::<StdRng>::new(keystore_path.clone())
                .map_err(|e| format!("Failed to create keystore: {}", e))?,
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
        let bridge_tag = NoteTag::for_local_use_case(BRIDGE_USECASE, 0)
            .map_err(|e| format!("Failed to create bridge tag: {:?}", e))?;
        client.add_note_tag(bridge_tag).await
            .map_err(|e| format!("Failed to add note tag: {}", e))?;

        // Sync state
        client.sync_state().await
            .map_err(|e| format!("Failed to sync client state: {}", e))?;

        // Get last scanned block
        let last_block = {
            let mut last = self.last_scanned_block.lock().unwrap();
            let current = client.get_sync_height().await
                .map_err(|e| format!("Failed to get sync height: {}", e))?
                .as_u32();
            
            let start_block = if *last == 0 {
                // Start from current block - 100 (scan last 100 blocks on first run)
                current.saturating_sub(100)
            } else {
                *last + 1
            };
            
            *last = current;
            start_block
        };

        // Get committed input notes (these are notes that were consumed)
        let notes = client.get_input_notes(NoteFilter::Committed).await
            .map_err(|e| format!("Failed to get input notes: {}", e))?;

        println!("[Miden Exit Relayer] Found {} committed notes", notes.len());

        // Get list of already processed note IDs (clone to avoid holding lock during processing)
        let processed_ids: HashSet<String> = {
            let processed = self.processed_note_ids.lock().unwrap();
            processed.clone()
        };
        
        // Filter for bridge exit notes (notes with bridge tag that were consumed)
        for note_record in notes.iter() {
            let note_id = note_record.id().to_hex();
            
            // Skip if already processed
            if processed_ids.contains(&note_id) {
                continue;
            }

            // Check if note has bridge tag
            let metadata = note_record.metadata()
                .ok_or_else(|| "Note missing metadata".to_string())?;
            
            if metadata.tag() != bridge_tag {
                continue;
            }

            // Check if note was consumed after our last scan
            let inclusion_proof = note_record.inclusion_proof()
                .ok_or_else(|| "Note missing inclusion proof".to_string())?;
            
            let block_num = inclusion_proof.location().block_num().as_u32();
            if block_num < last_block {
                continue;
            }

            // Extract exit event data from note inputs
            // Note: This assumes the note was created with crosschain script
            // The inputs should contain: output_serial_number (4 felts), dest_chain, dest_addr (3 felts), etc.
            let details = note_record.details();
            let inputs = details.inputs().values();
            
            if inputs.len() < 8 {
                println!("[Miden Exit Relayer] Note {} has insufficient inputs, skipping", note_id);
                continue;
            }

            // Extract destination chain (input[4])
            let dest_chain = inputs[4].as_int();
            
            // Zcash testnet chain ID (matches withdrawal note creation in main.rs)
            const ZCASH_TESTNET_CHAIN_ID: u64 = 2;
            
            if dest_chain != ZCASH_TESTNET_CHAIN_ID {
                println!("[Miden Exit Relayer] Note {} is for chain {}, not Zcash (expected {}), skipping", note_id, dest_chain, ZCASH_TESTNET_CHAIN_ID);
                continue;
            }

            // Extract Zcash address (inputs[5..8] - 3 felts)
            // Note: inputs are: [output_serial_num[3], output_serial_num[2], output_serial_num[1], output_serial_num[0], dest_chain, zcash_addr[2], zcash_addr[1], zcash_addr[0], ...]
            let zcash_address_felts = [
                inputs[7], // dest_addr[0] (zcash_address[0] from note creation)
                inputs[6], // dest_addr[1] (zcash_address[1] from note creation)
                inputs[5], // dest_addr[2] (zcash_address[2] from note creation)
            ];

            // Decode Zcash address from felts
            // Note: We need to store the original address mapping or use deterministic encoding
            // For now, we'll use a hash-based approach (same as encode_zcash_address)
            let zcash_address = decode_zcash_address(zcash_address_felts)
                .map_err(|e| format!("Failed to decode Zcash address: {}", e))?;

            // Extract amount from note assets (not inputs!)
            // The amount is in the fungible asset that was burned
            let assets = note_record.assets();
            let mut amount = 0u64;
            
            // Iterate through assets to find fungible asset
            for asset in assets.iter() {
                if let miden_objects::asset::Asset::Fungible(fungible_asset) = asset {
                    amount = fungible_asset.amount();
                    break;
                }
            }
            
            if amount == 0 {
                println!("[Miden Exit Relayer] Note {} has no fungible assets or zero amount, skipping", note_id);
                continue;
            }

            println!("[Miden Exit Relayer] Processing exit:");
            println!("  Note ID: {}", note_id);
            println!("  Zcash Address: {}", zcash_address);
            println!("  Amount: {} (base units)", amount);

            // Send Zcash transaction
            let amount_taz = amount as f64 / 1e8;
            let amount_str = format!("{:.8}", amount_taz);
            match self.bridge_wallet.send(&zcash_address, &amount_str, None, None) {
                Ok(txid) => {
                    println!("[Miden Exit Relayer] ✅ Sent {} TAZ to {}: {}", amount_taz, zcash_address, txid);
                    
                    // Mark as processed
                    let mut processed = self.processed_note_ids.lock().unwrap();
                    processed.insert(note_id);
                }
                Err(e) => {
                    eprintln!("[Miden Exit Relayer] ❌ Failed to send Zcash: {}", e);
                    // Don't mark as processed so we can retry
                }
            }
        }

        Ok(())
    }
}


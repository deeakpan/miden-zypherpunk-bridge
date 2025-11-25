use crate::bridge::deposit::{get_or_create_zcash_faucet, mint_deposit_note_from_hash};
use crate::zcash::bridge_wallet::BridgeWallet;
use miden_objects::Word;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, interval};

/// Zcash deposit relayer that periodically scans for deposits and automatically mints notes
pub struct ZcashRelayer {
    bridge_wallet: Arc<BridgeWallet>,
    memo_file: PathBuf,
    scan_interval: Duration,
    processed_txids: Arc<Mutex<HashSet<String>>>,
    project_root: PathBuf,
}

impl ZcashRelayer {
    pub fn new(
        bridge_wallet: Arc<BridgeWallet>,
        project_root: PathBuf,
        scan_interval_secs: u64,
    ) -> Self {
        let memo_file = project_root.join("test_memo.txt");
        
        // Load already processed txids from file
        let processed_txids = Self::load_processed_txids(&memo_file);
        
        Self {
            bridge_wallet,
            memo_file,
            scan_interval: Duration::from_secs(scan_interval_secs),
            processed_txids: Arc::new(Mutex::new(processed_txids)),
            project_root,
        }
    }

    /// Load already processed txids from the memo file
    fn load_processed_txids(memo_file: &PathBuf) -> HashSet<String> {
        let mut txids = HashSet::new();
        
        if let Ok(file) = File::open(memo_file) {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Extract txid from format: "TXID: <txid> | ..."
                    if let Some(txid_start) = line.find("TXID: ") {
                        let txid_part = &line[txid_start + 6..];
                        if let Some(txid_end) = txid_part.find(" |") {
                            let txid = txid_part[..txid_end].trim().to_string();
                            txids.insert(txid);
                        }
                    }
                }
            }
        }
        
        txids
    }

    /// Store memo to file
    fn store_memo(&self, txid: &str, memo: &str, amount: u64) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.memo_file)
            .map_err(|e| format!("Failed to open memo file: {}", e))?;
        
        let memo_entry = format!("TXID: {} | Amount: {} zatoshis | Memo: {}\n", txid, amount, memo);
        file.write_all(memo_entry.as_bytes())
            .map_err(|e| format!("Failed to write memo: {}", e))?;
        
        Ok(())
    }

    /// Mint note automatically for a deposit
    async fn mint_note_for_deposit(&self, recipient_hash: Word, amount: u64) -> Result<(String, String), String> {
        let keystore_path = self.project_root.join("keystore");
        let store_path = self.project_root.join("bridge_store.sqlite3");
        let faucet_store_path = self.project_root.join("faucets.db");
        let rpc_url = std::env::var("RPC_URL")
            .unwrap_or_else(|_| "https://rpc.testnet.miden.io".to_string());
        
        // Get or create faucet
        let faucet_id = get_or_create_zcash_faucet(
            keystore_path.clone(),
            store_path.clone(),
            &rpc_url,
            faucet_store_path,
        )
        .await?;
        
        // Mint note with recipient hash (no account_id needed - just the hash)
        mint_deposit_note_from_hash(
            recipient_hash,
            faucet_id,
            amount,
            keystore_path,
            store_path,
            &rpc_url,
        )
        .await
    }

    /// Scan for deposits and extract memos
    async fn scan_and_extract_memos(&self) {
        println!("[Zcash Relayer] Starting Zcash deposit scan...");
        
        match self.bridge_wallet.extract_all_memos() {
            Ok(memos) => {
                let total_count = memos.len();
                println!("[Zcash Relayer] Found {} transactions with memos", total_count);
                
                if total_count == 0 {
                    println!("[Zcash Relayer] No transactions with memos found");
                    return;
                }
                
                // Step 1: Identify new transactions while holding the lock (synchronously)
                let mut work_items = Vec::new();
                let mut skipped_count = 0;
                
                {
                    let processed = self.processed_txids.lock().unwrap();
                    
                    for (txid, memo, amount) in memos {
                        // Skip if already processed
                        if processed.contains(&txid) {
                            skipped_count += 1;
                            continue;
                        }
                        
                        // Extract recipient_hash from memo (remove "Memo::Text(" and ")")
                        let recipient_hash = memo
                            .trim()
                            .strip_prefix("Memo::Text(\"")
                            .and_then(|s| s.strip_suffix("\")"))
                            .or_else(|| {
                                // Also handle plain hex without Memo::Text wrapper
                                if memo.trim().starts_with("0x") {
                                    Some(memo.trim())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| memo.trim());
                        
                        // Validate recipient hash format: must be 0x + 64 hex chars = 66 chars total
                        // Example: "0x33de110b5f9b695a98f1539a5f83325602fa559b816990d814224a53eea2f7c5"
                        if recipient_hash.len() != 66 || !recipient_hash.starts_with("0x") {
                            println!("[Zcash Relayer] Skipping tx {} - invalid recipient hash format (expected 0x + 64 hex chars, got {} chars): {}", txid, recipient_hash.len(), recipient_hash);
                            continue;
                        }
                        
                        // Validate it's all hex after 0x
                        let hex_part = &recipient_hash[2..];
                        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) || hex_part.len() != 64 {
                            println!("[Zcash Relayer] Skipping tx {} - recipient hash is not valid hex (64 chars): {}", txid, recipient_hash);
                            continue;
                        }
                        
                        // Parse recipient_hash as Word
                        let recipient_word = match Word::try_from(recipient_hash) {
                            Ok(word) => word,
                            Err(e) => {
                                eprintln!("[Zcash Relayer] Invalid recipient hash in tx {}: {} - {}", txid, recipient_hash, e);
                                continue;
                            }
                        };
                        
                        // Store work item for async processing
                        work_items.push((txid, recipient_hash.to_string(), recipient_word, amount));
                    }
                } // Lock is dropped here
                
                // Step 2: Process work items asynchronously (without holding the lock)
                let mut new_count = 0;
                for (txid, recipient_hash, recipient_word, amount) in work_items {
                    println!("[Zcash Relayer] Found new deposit in tx {}: {} (amount: {} zatoshis)", txid, recipient_hash, amount);
                    
                    // Automatically mint note with recipient hash
                    println!("[Zcash Relayer] Minting note for deposit tx {}...", txid);
                    match self.mint_note_for_deposit(recipient_word, amount).await {
                        Ok((note_id, tx_id)) => {
                            // Re-acquire lock to mark as processed
                            {
                                let mut processed = self.processed_txids.lock().unwrap();
                                processed.insert(txid.clone());
                            }
                            new_count += 1;
                            println!("[Zcash Relayer] ✅ Minted note {} (tx: {}) for deposit tx {}", note_id, tx_id, txid);
                            
                            // Also store in memo file for reference
                            let _ = self.store_memo(&txid, &recipient_hash, amount);
                        }
                        Err(e) => {
                            eprintln!("[Zcash Relayer] ❌ Failed to mint note for tx {}: {}", txid, e);
                        }
                    }
                }
                
                if new_count == 0 {
                    println!("[Zcash Relayer] No new memos found ({} total, {} already processed)", total_count, skipped_count);
                } else {
                    println!("[Zcash Relayer] Processed {} new memos ({} total, {} skipped)", new_count, total_count, skipped_count);
                }
            }
            Err(e) => {
                eprintln!("[Zcash Relayer] Failed to extract memos: {}", e);
            }
        }
    }

    /// Start the relayer as a background task
    pub async fn start(self) {
        println!("[Zcash Relayer] Starting Zcash relayer with scan interval: {:?} seconds", self.scan_interval.as_secs());
        
        // Run initial scan
        self.scan_and_extract_memos().await;
        
        // Set up periodic scanning
        let mut interval = interval(self.scan_interval);
        
        loop {
            interval.tick().await;
            self.scan_and_extract_memos().await;
        }
    }
}


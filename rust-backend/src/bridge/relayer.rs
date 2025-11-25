use crate::bridge::deposit::{get_or_create_zcash_faucet, mint_deposit_note};
use crate::zcash::bridge_wallet::BridgeWallet;
use miden_objects::{account::AccountId, Word};
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
    async fn mint_note_for_deposit(&self, account_id: miden_objects::account::AccountId, secret: Word, amount: u64) -> Result<(String, String), String> {
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
        
        // Mint note with account_id + secret (builds full recipient)
        crate::bridge::deposit::mint_deposit_note(
            account_id,
            secret,
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
                        
                        // Extract memo content (remove "Memo::Text(" and ")")
                        let memo_content = memo
                            .trim()
                            .strip_prefix("Memo::Text(\"")
                            .and_then(|s| s.strip_suffix("\")"))
                            .or_else(|| {
                                // Also handle plain text without Memo::Text wrapper
                                Some(memo.trim())
                            })
                            .unwrap_or_else(|| memo.trim());
                        
                        // Check if memo contains account_id|secret format
                        if let Some(pipe_pos) = memo_content.find('|') {
                            // Parse account_id|secret format
                            let account_id_str = &memo_content[..pipe_pos];
                            let secret_str = &memo_content[pipe_pos + 1..];
                            
                            // Validate account_id (should be 30 hex chars = 15 bytes, with or without 0x)
                            // AccountId::from_hex expects 0x + 30 hex chars = 32 total chars
                            let account_id_hex = if account_id_str.starts_with("0x") {
                                &account_id_str[2..]
                            } else {
                                account_id_str
                            };
                            
                            if !account_id_hex.chars().all(|c| c.is_ascii_hexdigit()) {
                                println!("[Zcash Relayer] Skipping tx {} - account_id contains non-hex characters: {}", txid, account_id_str);
                                continue;
                            }
                            
                            if account_id_hex.len() > 30 {
                                println!("[Zcash Relayer] Skipping tx {} - account_id too long (max 30 hex chars, got {}): {}", txid, account_id_hex.len(), account_id_str);
                                continue;
                            }
                            
                            // Pad with leading zeros to 30 chars if needed (AccountId expects 30 hex chars)
                            let account_id_padded = if account_id_hex.len() < 30 {
                                format!("{:0>30}", account_id_hex)
                            } else {
                                account_id_hex.to_string()
                            };
                            
                            // AccountId::from_hex expects 0x prefix + 30 hex chars
                            let account_id_for_parse = format!("0x{}", account_id_padded);
                            
                            // Validate secret (should be 64 hex chars, with or without 0x)
                            let secret_hex = if secret_str.starts_with("0x") {
                                &secret_str[2..]
                            } else {
                                secret_str
                            };
                            
                            if secret_hex.len() != 64 || !secret_hex.chars().all(|c| c.is_ascii_hexdigit()) {
                                println!("[Zcash Relayer] Skipping tx {} - invalid secret format (expected 64 hex chars, got {} chars): {}", txid, secret_hex.len(), secret_str);
                                continue;
                            }
                            
                            // Parse account_id and secret
                            let account_id = match miden_objects::account::AccountId::from_hex(&account_id_for_parse) {
                                Ok(id) => id,
                                Err(e) => {
                                    eprintln!("[Zcash Relayer] Invalid account_id in tx {}: {} (padded: {}) - {}", txid, account_id_str, account_id_for_parse, e);
                                    continue;
                                }
                            };
                            
                            let secret_with_prefix = if secret_str.starts_with("0x") {
                                secret_str.to_string()
                            } else {
                                format!("0x{}", secret_str)
                            };
                            let secret_word = match Word::try_from(secret_with_prefix.as_str()) {
                                Ok(word) => word,
                                Err(e) => {
                                    eprintln!("[Zcash Relayer] Invalid secret in tx {}: {} - {}", txid, secret_str, e);
                                    continue;
                                }
                            };
                            
                            // Store work item with account_id and secret
                            work_items.push((txid, account_id, secret_word, amount));
                        } else {
                            // Fallback: try to parse as old hash format (for backward compatibility)
                            let recipient_hash = memo_content;
                            
                            // Validate recipient hash format: must be 0x + 64 hex chars = 66 chars total
                            if recipient_hash.len() != 66 || !recipient_hash.starts_with("0x") {
                                println!("[Zcash Relayer] Skipping tx {} - invalid memo format (expected account_id|secret or 0x + 64 hex chars, got {} chars): {}", txid, recipient_hash.len(), recipient_hash);
                                continue;
                            }
                            
                            // Validate it's all hex after 0x
                            let hex_part = &recipient_hash[2..];
                            if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) || hex_part.len() != 64 {
                                println!("[Zcash Relayer] Skipping tx {} - recipient hash is not valid hex (64 chars): {}", txid, recipient_hash);
                                continue;
                            }
                            
                            // Parse recipient_hash as Word (old format - will fail to mint but won't crash)
                            let recipient_word = match Word::try_from(recipient_hash) {
                                Ok(word) => word,
                                Err(e) => {
                                    eprintln!("[Zcash Relayer] Invalid recipient hash in tx {}: {} - {}", txid, recipient_hash, e);
                                    continue;
                                }
                            };
                            
                            // Store work item with hash (old format - will need to be updated)
                            println!("[Zcash Relayer] Warning: tx {} uses old hash format. Please use account_id|secret format.", txid);
                            // For now, skip old format transactions
                            continue;
                        }
                    }
                } // Lock is dropped here
                
                // Step 2: Process work items asynchronously (without holding the lock)
                let mut new_count = 0;
                for (txid, account_id, secret, amount) in work_items {
                    println!("[Zcash Relayer] Found new deposit in tx {}: account_id={}, amount={} zatoshis", txid, account_id, amount);

                    // Automatically mint note with account_id + secret
                    println!("[Zcash Relayer] Minting note for deposit tx {}...", txid);
                    match self.mint_note_for_deposit(account_id, secret, amount).await {
                        Ok((note_id, tx_id)) => {
                            // Re-acquire lock to mark as processed
                            {
                                let mut processed = self.processed_txids.lock().unwrap();
                                processed.insert(txid.clone());
                            }
                            new_count += 1;
                            println!("[Zcash Relayer] ✅ Minted note {} (tx: {}) for deposit tx {}", note_id, tx_id, txid);

                            // Also store in memo file for reference
                            let _ = self.store_memo(&txid, &format!("{}|{}", account_id, secret), amount);
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


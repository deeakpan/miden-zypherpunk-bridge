use crate::zcash::bridge_wallet::BridgeWallet;
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, interval};

/// Zcash deposit relayer that periodically scans for deposits and extracts memos
pub struct ZcashRelayer {
    bridge_wallet: Arc<BridgeWallet>,
    memo_file: PathBuf,
    scan_interval: Duration,
    processed_txids: Arc<Mutex<HashSet<String>>>,
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
                
                let mut processed = self.processed_txids.lock().unwrap();
                let mut new_count = 0;
                let mut skipped_count = 0;
                
                for (txid, memo, amount) in memos {
                    // Skip if already processed
                    if processed.contains(&txid) {
                        skipped_count += 1;
                        continue;
                    }
                    
                    println!("[Zcash Relayer] Found new memo in tx {}: {} (amount: {} zatoshis)", txid, memo, amount);
                    
                    match self.store_memo(&txid, &memo, amount) {
                        Ok(_) => {
                            processed.insert(txid.clone());
                            new_count += 1;
                            println!("[Zcash Relayer] Stored memo from tx {} to test_memo.txt", txid);
                        }
                        Err(e) => {
                            eprintln!("[Zcash Relayer] Failed to store memo from tx {}: {}", txid, e);
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


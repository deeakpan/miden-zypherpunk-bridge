use std::path::PathBuf;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ZcashBalance {
    pub total: String,
    pub spendable: String,
    pub pending: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ZcashAddress {
    pub address: String,
    pub account_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionInfo {
    pub txid: String,
    pub amount: u64,
    pub memo: Option<String>,
    pub to_address: Option<String>,
}

pub struct BridgeWallet {
    wallet_dir: PathBuf,
    identity_file: PathBuf,
    zcash_devtool_dir: PathBuf,
}

impl BridgeWallet {
    pub fn new(project_root: PathBuf) -> Self {
        let wallet_dir = project_root.join("wallet").join("bridge_wallet");
        let identity_file = wallet_dir.join("key.txt");
        let zcash_devtool_dir = project_root.join("wallet").join("zcash-devtool");
        
        Self {
            wallet_dir,
            identity_file,
            zcash_devtool_dir,
        }
    }

    /// Execute a zcash-devtool command
    fn exec_command(&self, args: Vec<&str>) -> Result<String, String> {
        let mut cmd = Command::new("cargo");
        cmd.args(&["run", "--release", "--all-features", "--"]);
        cmd.args(&args);
        cmd.current_dir(&self.zcash_devtool_dir);
        
        let output = cmd.output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Command failed: {}", stderr));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get bridge wallet balance
    pub fn get_balance(&self) -> Result<ZcashBalance, String> {
        let wallet_path = self.wallet_dir.to_str()
            .ok_or("Invalid wallet path")?;
        
        let output = self.exec_command(vec![
            "wallet",
            "-w", wallet_path,
            "balance",
        ])?;
        
        self.parse_balance(&output)
    }

    /// Sync bridge wallet
    pub fn sync(&self) -> Result<String, String> {
        let wallet_path = self.wallet_dir.to_str()
            .ok_or("Invalid wallet path")?;
        
        self.exec_command(vec![
            "wallet",
            "-w", wallet_path,
            "sync",
            "-s", "zecrocks",
        ])
    }

    /// List addresses in bridge wallet
    pub fn list_addresses(&self, account_id: Option<&str>) -> Result<Vec<ZcashAddress>, String> {
        let wallet_path = self.wallet_dir.to_str()
            .ok_or("Invalid wallet path")?;
        
        let mut args = vec!["wallet", "-w", wallet_path, "list-addresses"];
        if let Some(acc_id) = account_id {
            args.push("--account-id");
            args.push(acc_id);
        }
        
        let output = self.exec_command(args)?;
        self.parse_addresses(&output)
    }

    /// Enhance transactions to get memo data
    pub fn enhance_transactions(&self) -> Result<String, String> {
        let wallet_path = self.wallet_dir.to_str()
            .ok_or("Invalid wallet path")?;
        
        self.exec_command(vec![
            "wallet",
            "-w", wallet_path,
            "enhance",
            "-s", "zecrocks",
        ])
    }

    /// Get memos from incoming transactions (deposits) after enhancing
    /// This will sync, enhance, and then extract memos from received transactions only
    pub fn extract_all_memos(&self) -> Result<Vec<(String, String, u64)>, String> {
        // Step 1: Sync wallet to get latest transactions from chain
        println!("[Bridge Wallet] Syncing wallet...");
        self.sync()?;
        println!("[Bridge Wallet] Sync complete");
        
        // Step 2: Enhance transactions to download memo data
        println!("[Bridge Wallet] Enhancing transactions to get memo data...");
        self.enhance_transactions()?;
        println!("[Bridge Wallet] Enhancement complete");
        
        // Step 3: List all transactions
        println!("[Bridge Wallet] Listing transactions...");
        let tx_output = self.list_transactions(None)?;
        
        // Step 4: Parse transactions to extract memos
        let transactions = self.parse_transactions(&tx_output)?;
        println!("[Bridge Wallet] Parsed {} transactions", transactions.len());
        
        // Get bridge wallet addresses to filter for incoming transactions only
        let bridge_addresses = match self.list_addresses(None) {
            Ok(addrs) => {
                let addr_set: std::collections::HashSet<String> = addrs
                    .into_iter()
                    .map(|addr| addr.address)
                    .collect();
                println!("[Bridge Wallet] Successfully loaded {} bridge addresses: {:?}", addr_set.len(), addr_set);
                addr_set
            }
            Err(e) => {
                eprintln!("[Bridge Wallet] ⚠️ Failed to list addresses: {}. Will process all transactions with valid memos.", e);
                // If we can't get addresses, process all transactions with valid memos
                // This is acceptable for a private chain where only the bridge wallet can see transactions
                std::collections::HashSet::new()
            }
        };
        
        // Extract memos from transactions
        // If bridge_addresses is empty (couldn't load), process all transactions with valid memos
        // Otherwise, only process transactions sent TO bridge addresses
        let check_address = !bridge_addresses.is_empty();
        println!("[Bridge Wallet] Address filtering: {}", if check_address { "enabled" } else { "disabled (processing all valid memos)" });
        
        let mut memos = Vec::new();
        for tx in &transactions {
            // Only process transactions that:
            // 1. Have a memo
            // 2. Have positive amount (money coming in)
            // 3. (If addresses loaded) Are sent TO one of the bridge wallet addresses
            if let Some(memo) = &tx.memo {
                let memo_trimmed = memo.trim();
                
                if !memo_trimmed.is_empty() 
                    && memo_trimmed != "Empty" 
                    && !memo_trimmed.starts_with("Memo::Empty")
                    && tx.amount > 0 {
                    
                    // If we have bridge addresses, check to_address matches
                    // If we don't have addresses (private chain), process all valid memos
                    let should_process = if check_address {
                        if let Some(to_addr) = &tx.to_address {
                            bridge_addresses.contains(to_addr)
                        } else {
                            false
                        }
                    } else {
                        // No address filtering - process all transactions with valid memos
                        true
                    };
                    
                    if should_process {
                        println!("[Bridge Wallet] ✅ Processing tx {} with memo: {}", tx.txid, memo_trimmed);
                        memos.push((tx.txid.clone(), memo.clone(), tx.amount));
                    }
                }
            }
        }
        
        Ok(memos)
    }

    /// List transactions
    pub fn list_transactions(&self, account_id: Option<&str>) -> Result<String, String> {
        let wallet_path = self.wallet_dir.to_str()
            .ok_or("Invalid wallet path")?;
        
        let mut args = vec!["wallet", "-w", wallet_path, "list-tx"];
        if let Some(acc_id) = account_id {
            args.push("--account-id");
            args.push(acc_id);
        }
        
        self.exec_command(args)
    }

    /// Parse transaction output to extract memo and amount
    /// 
    /// The list-tx output format is:
    /// <txid_hex>
    ///      Mined: <height> (<timestamp>)
    ///     Amount: <amount> TAZ
    ///   Fee paid: <fee>
    ///   Sent X notes, received Y notes, Z memos
    ///   Output 0 (ORCHARD)
    ///     Value: <amount> TAZ
    ///     To: <address>
    ///     Memo: <memo>
    pub fn parse_transactions(&self, output: &str) -> Result<Vec<TransactionInfo>, String> {
        let mut transactions = Vec::new();
        let lines: Vec<&str> = output.lines().collect();
        
        let mut current_tx: Option<TransactionInfo> = None;
        let mut in_output = false;
        
        for line in lines {
            let line = line.trim();
            
            // Skip empty lines and headers
            if line.is_empty() || line == "Transactions:" {
                continue;
            }
            
            // Transaction ID is a hex string (64 chars) on its own line
            if line.len() == 64 && line.chars().all(|c| c.is_ascii_hexdigit()) {
                if let Some(tx) = current_tx.take() {
                    transactions.push(tx);
                }
                current_tx = Some(TransactionInfo {
                    txid: line.to_string(),
                    amount: 0,
                    memo: None,
                    to_address: None,
                });
                in_output = false;
                continue;
            }
            
            if let Some(tx) = &mut current_tx {
                // Parse amount from "Amount: X.XXXXXXXX TAZ" (testnet)
                if line.starts_with("Amount:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(amount_taz) = parts[1].parse::<f64>() {
                            // Convert TAZ to zatoshis (8 decimals)
                            tx.amount = (amount_taz * 100_000_000.0) as u64;
                        }
                    }
                }
                
                // Check if we're in an output section
                if line.starts_with("Output") {
                    in_output = true;
                }
                
                // Parse "To: <address>"
                if in_output && line.starts_with("To:") {
                    let addr = line.strip_prefix("To:").unwrap_or("").trim().to_string();
                    if !addr.is_empty() {
                        tx.to_address = Some(addr);
                    }
                }
                
                // Parse "Memo: <memo>"
                if in_output && line.starts_with("Memo:") {
                    let memo_part = line.strip_prefix("Memo:").unwrap_or("").trim();
                    // Handle different memo formats
                    if memo_part.starts_with("Text(") {
                        // Extract text from Text("...")
                        if let Some(start) = memo_part.find('"') {
                            if let Some(end) = memo_part.rfind('"') {
                                if end > start {
                                    let memo_text = &memo_part[start+1..end];
                                    if !memo_text.is_empty() {
                                        tx.memo = Some(memo_text.to_string());
                                    }
                                }
                            }
                        }
                    } else if !memo_part.is_empty() && memo_part != "Empty" {
                        tx.memo = Some(memo_part.to_string());
                    }
                }
                
                // Reset output flag when we hit a new transaction section
                if line.starts_with("Mined:") || line.starts_with("Unmined") || line.starts_with("Expired") {
                    in_output = false;
                }
            }
        }
        
        if let Some(tx) = current_tx {
            transactions.push(tx);
        }
        
        Ok(transactions)
    }

    /// Send TAZ from bridge wallet (Zcash testnet)
    pub fn send(
        &self,
        address: &str,
        amount: &str,
        memo: Option<&str>,
        account_id: Option<&str>,
    ) -> Result<String, String> {
        let wallet_path = self.wallet_dir.to_str()
            .ok_or("Invalid wallet path")?;
        let identity_path = self.identity_file.to_str()
            .ok_or("Invalid identity path")?;
        
        let mut args = vec![
            "wallet",
            "-w", wallet_path,
            "send",
            "--identity", identity_path,
            "--address", address,
            "--value", amount,
            "--target-note-count", "1",
            "-s", "zecrocks",
        ];
        
        if let Some(acc_id) = account_id {
            args.push("--account-id");
            args.push(acc_id);
        }
        
        if let Some(m) = memo {
            args.push("--memo");
            args.push(m);
        }
        
        self.exec_command(args)
    }

    /// Parse balance from CLI output
    fn parse_balance(&self, output: &str) -> Result<ZcashBalance, String> {
        let lines: Vec<&str> = output.lines().collect();
        let mut total = "0".to_string();
        let mut spendable = "0".to_string();
        let pending = "0".to_string();
        
        for line in lines {
            let line = line.trim();
            if line.starts_with("Balance:") {
                // Parse "Balance:   0.19990000 TAZ"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    total = parts[1].to_string();
                }
            }
            if line.contains("Sapling Spendable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&x| x == "Spendable:") {
                    if pos + 1 < parts.len() {
                        spendable = parts[pos + 1].to_string();
                    }
                }
            }
            if line.contains("Orchard Spendable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&x| x == "Spendable:") {
                    if pos + 1 < parts.len() {
                        let orchard: f64 = parts[pos + 1].parse().unwrap_or(0.0);
                        let sapling: f64 = spendable.parse().unwrap_or(0.0);
                        spendable = format!("{:.8}", orchard + sapling);
                    }
                }
            }
        }
        
        Ok(ZcashBalance {
            total,
            spendable,
            pending,
        })
    }

    /// Parse addresses from CLI output
    fn parse_addresses(&self, output: &str) -> Result<Vec<ZcashAddress>, String> {
        // Simple parsing - adjust based on actual CLI output format
        let mut addresses = Vec::new();
        
        for line in output.lines() {
            let line = line.trim();
            // Look for address patterns (utest1... or ztest...)
            if line.starts_with("utest1") || line.starts_with("ztest") {
                addresses.push(ZcashAddress {
                    address: line.to_string(),
                    account_id: None,
                });
            }
        }
        
        Ok(addresses)
    }
}


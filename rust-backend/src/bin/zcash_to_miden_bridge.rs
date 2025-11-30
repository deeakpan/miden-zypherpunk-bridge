use std::env;
use serde_json::json;

const AMOUNT: f64 = 0.3; // TAZ amount
const BACKEND_URL: &str = "http://127.0.0.1:8001";
const FRONTEND_URL: &str = "http://localhost:3000";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    
    println!("{}", "=".repeat(60));
    println!("Zcash → Miden Bridge Script");
    println!("{}", "=".repeat(60));
    println!("Amount: {} TAZ", AMOUNT);
    println!();

    // Get or create account (same as UI does)
    let (account_id, account_id_hex) = if let Ok(env_account) = env::var("MIDEN_ACCOUNT_ID") {
        println!("[0] Using account from MIDEN_ACCOUNT_ID env var");
        // If using env var, we need to get hex from backend or convert it
        // For now, try to get hex from env var too, or use bech32 as fallback
        let hex = env::var("MIDEN_ACCOUNT_ID_HEX").unwrap_or_else(|_| {
            // If bech32, we'll need to convert - but for now just use as-is
            // The relayer will handle bech32 if we fix it
            env_account.clone()
        });
        (env_account, hex)
    } else {
        println!("[0] No account found, creating new account via /account/create...");
        let create_url = format!("{}/account/create", BACKEND_URL);
        let create_response = reqwest::Client::new()
            .post(&create_url)
            .send()
            .await?;
        
        if !create_response.status().is_success() {
            let error_text = create_response.text().await?;
            return Err(format!("Failed to create account: {}", error_text).into());
        }
        
        let account_data: serde_json::Value = create_response.json().await?;
        let new_account_id = account_data["account_id"]
            .as_str()
            .ok_or("Invalid response from /account/create")?
            .to_string();
        let new_account_id_hex = account_data["account_id_hex"]
            .as_str()
            .ok_or("Invalid response from /account/create: missing account_id_hex")?
            .to_string();
        
        println!("[0] ✅ Created new account: {}", new_account_id);
        println!("[0]    Hex: {}", new_account_id_hex);
        (new_account_id, new_account_id_hex)
    };
    
    println!("[0] Using account: {} (hex: {}...)", account_id, &account_id_hex[..20.min(account_id_hex.len())]);
    
    // Generate secret (32 bytes = 64 hex chars)
    let secret_bytes: [u8; 32] = rand::random();
    let secret_hex = format!("0x{}", hex::encode(secret_bytes));
    println!("[0] Generated secret: {}...{}", &secret_hex[2..18], &secret_hex[secret_hex.len()-8..]);
    println!();
    
    // Call hash endpoint
    println!("[1] Generating recipient hash...");
    let hash_url = format!("{}/deposit/hash?account_id={}&secret={}", 
        BACKEND_URL, 
        urlencoding::encode(&account_id), 
        urlencoding::encode(&secret_hex)
    );
    
    let hash_response = reqwest::get(&hash_url).await?;
    let hash_data: serde_json::Value = hash_response.json().await?;
    
    if !hash_data["success"].as_bool().unwrap_or(false) {
        return Err(format!("Failed to generate hash: {}", hash_data["error"].as_str().unwrap_or("Unknown error")).into());
    }
    
    let recipient_hash = hash_data["recipient_hash"].as_str().unwrap();
    println!("[1] ✅ Hash generated: {}...", &recipient_hash[..30]);
    println!();
    
    // Get deposit address (bridge wallet address - where to send TO)
    let deposit_address = env::var("BRIDGE_ZCASH_ADDRESS")
        .unwrap_or_else(|_| "utest1s7vrs7ycxvpu379zvtxt0fnc0efseur2f8g2s8puqls7nk45l6p7wvglu3rph9us9qzsjww44ly3wxlsul0jcpqx8qwvwqz4sq48rjj0cn59956sjsrz5ufuswd5ujy89n3vh264wx3843pxscnrf0ulku4990h65h5ll9r0j3q82mjgm2sx7lfnrkfkuqw9l2m7yfmgc4jvzq6n8j2".to_string());
    
    // Format memo as account_id|secret (like frontend does)
    // Frontend uses hex account_id (without 0x prefix) for the memo
    // Remove 0x prefix from hex if present
    let account_id_hex_for_memo = if account_id_hex.starts_with("0x") {
        &account_id_hex[2..]
    } else {
        &account_id_hex
    };
    let memo = format!("{}|{}", account_id_hex_for_memo, secret_hex);
    
    // Sync personal wallet first
    println!("[2] Syncing personal wallet...");
    let frontend_url = env::var("NEXT_PUBLIC_URL")
        .unwrap_or_else(|_| FRONTEND_URL.to_string());
    let sync_url = format!("{}/api/wallet/sync", frontend_url);
    let sync_response = reqwest::Client::new()
        .post(&sync_url)
        .send()
        .await?;
    
    if sync_response.status().is_success() {
        if let Ok(sync_data) = sync_response.json::<serde_json::Value>().await {
            if sync_data["success"].as_bool().unwrap_or(false) {
                println!("[2] ✅ Wallet synced");
            } else {
                println!("[2] ⚠️  Sync warning: {}", sync_data["error"].as_str().unwrap_or("Unknown"));
            }
        }
    } else {
        println!("[2] ⚠️  Sync failed, continuing anyway...");
    }
    println!();
    
    // Check balance
    println!("[3] Checking personal wallet balance...");
    let balance_url = format!("{}/api/wallet/balance", frontend_url);
    let balance_response = reqwest::get(&balance_url).await?;
    
    if balance_response.status().is_success() {
        if let Ok(balance_data) = balance_response.json::<serde_json::Value>().await {
            if let Some(balance_obj) = balance_data["balance"].as_object() {
                if let Some(spendable_str) = balance_obj["spendable"].as_str() {
                    let spendable: f64 = spendable_str.parse().unwrap_or(0.0);
                    let required = AMOUNT + 0.0001; // Add small buffer for fees
                    println!("    Current balance: {:.8} TAZ", spendable);
                    println!("    Required: {:.8} TAZ (including fees)", required);
                    if spendable < required {
                        return Err(format!(
                            "Insufficient balance: have {:.8} TAZ, need {:.8} TAZ (including fees). Please fund your personal wallet.",
                            spendable, required
                        ).into());
                    }
                    println!("    ✅ Balance sufficient");
                }
            }
        }
    }
    println!();
    
    // Send transaction from personal wallet using Next.js API (like frontend does)
    println!("[4] Sending {} TAZ from personal wallet to bridge wallet...", AMOUNT);
    println!("    To: {}", deposit_address);
    println!("    Memo format: account_id|secret");
    println!("    Account ID (hex): {}...", &account_id_hex_for_memo[..16.min(account_id_hex_for_memo.len())]);
    println!("    Secret: {}...{}", &secret_hex[2..18], &secret_hex[secret_hex.len()-8..]);
    
    let send_url = format!("{}/api/wallet/send", frontend_url);
    let amount_str = format!("{:.8}", AMOUNT);
    let send_body = json!({
        "address": deposit_address,
        "amount": amount_str,
        "memo": memo
    });
    
    let send_response = reqwest::Client::new()
        .post(&send_url)
        .json(&send_body)
        .send()
        .await?;
    
    if !send_response.status().is_success() {
        let error_text = send_response.text().await?;
        return Err(format!("Failed to send transaction: {}", error_text).into());
    }
    
    let send_data: serde_json::Value = send_response.json().await?;
    if send_data["success"].as_bool().unwrap_or(false) {
        println!("[4] ✅ Transaction sent!");
        if let Some(tx_id) = send_data["txid"].as_str() {
            println!("    Transaction ID: {}", tx_id);
        }
    } else {
        let error = send_data["error"].as_str().unwrap_or("Unknown error");
        return Err(format!("Failed to send transaction: {}", error).into());
    }
    
    println!();
    println!("[5] Waiting for transaction to be detected...");
    println!("   (Polling every 5 seconds for up to 2 minutes)");
    println!();
    
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 24; // 2 minutes (5 second intervals)
    
    loop {
        attempts += 1;
        
        // Call claim endpoint
        let claim_url = format!("{}/deposit/claim", BACKEND_URL);
        let claim_body = json!({
            "account_id": account_id,
            "secret": secret_hex
        });
        
        let claim_response = reqwest::Client::new()
            .post(&claim_url)
            .json(&claim_body)
            .send()
            .await?;
        
        let status = claim_response.status();
        let response_text = claim_response.text().await?;
        
        // Try to parse as JSON first
        if let Ok(claim_data) = serde_json::from_str::<serde_json::Value>(&response_text) {
            // Successfully parsed as JSON
            if claim_data["success"].as_bool().unwrap_or(false) {
                println!("[5] ✅ Deposit claimed!");
                let note_id = claim_data["note_id"].as_str();
                let tx_id = claim_data["transaction_id"].as_str();
                
                if let Some(nid) = note_id {
                    println!("    Note ID: {}", nid);
                }
                if let Some(tid) = tx_id {
                    println!("    Transaction ID: {}", tid);
                }
                println!("    Message: {}", claim_data["message"].as_str().unwrap_or(""));
                println!();
                
                // Wait 2 minutes for note to be available, then consume it
                println!("[6] Waiting 2 minutes for note to be available on-chain...");
                tokio::time::sleep(tokio::time::Duration::from_secs(120)).await;
                println!("[6] ✅ Wait complete");
                println!();
                
                // Consume the note - need account_id, secret, faucet_id, and amount
                println!("[7] Consuming note...");
                let consume_url = format!("{}/note/consume", BACKEND_URL);
                
                // Get faucet_id - backend will auto-detect from faucets.db, but we can pass empty string
                // Amount in base units (zatoshis)
                let amount_base = (AMOUNT * 1e8) as u64;
                
                let consume_body = json!({
                    "account_id": account_id,
                    "secret": secret_hex,
                    "faucet_id": "", // Backend will auto-detect from faucets.db
                    "amount": amount_base
                });
                
                let consume_response = reqwest::Client::new()
                    .post(&consume_url)
                    .json(&consume_body)
                    .send()
                    .await?;
                
                let consume_status = consume_response.status();
                let consume_text = consume_response.text().await?;
                
                if consume_status.is_success() {
                    if let Ok(consume_data) = serde_json::from_str::<serde_json::Value>(&consume_text) {
                        if consume_data["success"].as_bool().unwrap_or(false) {
                            println!("[7] ✅ Note consumed!");
                            if let Some(ctid) = consume_data["transaction_id"].as_str() {
                                println!("    Transaction ID: {}", ctid);
                            }
                            if let Some(cnid) = consume_data["note_id"].as_str() {
                                println!("    Note ID: {}", cnid);
                            }
                        } else {
                            eprintln!("[7] ⚠️  Consume returned success=false: {}", consume_text);
                        }
                    } else {
                        println!("[7] ✅ Note consumed! (Response: {})", consume_text);
                    }
                } else {
                    eprintln!("[7] ⚠️  Consume failed ({}): {}", consume_status, consume_text);
                }
                
                println!();
                println!("{}", "=".repeat(60));
                println!("✅ BRIDGE SUCCESSFUL!");
                println!("{}", "=".repeat(60));
                return Ok(());
            }
            
            // JSON response but not successful
            let error_msg = claim_data["error"].as_str()
                .or_else(|| claim_data["message"].as_str())
                .unwrap_or("Unknown error");
            
            if error_msg.contains("No deposit found") || error_msg.contains("No deposit") {
                if attempts >= MAX_ATTEMPTS {
                    return Err("Timeout: No deposit found after 5 minutes".into());
                }
                print!("\r   Attempt {}/{}... (no deposit found yet)", attempts, MAX_ATTEMPTS);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            } else {
                return Err(format!("Claim failed: {}", error_msg).into());
            }
        } else {
            // Not JSON - treat as plain text error
            let error_msg = response_text.trim();
            
            if error_msg.contains("No deposit found") || error_msg.contains("No deposit") {
                if attempts >= MAX_ATTEMPTS {
                    return Err(format!("Timeout: No deposit found after 5 minutes. Last message: {}", error_msg).into());
                }
                print!("\r   Attempt {}/{}... (no deposit found yet)", attempts, MAX_ATTEMPTS);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            } else {
                // Non-success status or other error
                if !status.is_success() {
                    eprintln!("\n[2] Claim endpoint returned error status: {}", status);
                    eprintln!("    Response: {}", error_msg);
                    if attempts >= MAX_ATTEMPTS {
                        return Err(format!("Claim failed after {} attempts. Last error: {} - {}", attempts, status, error_msg).into());
                    }
                    print!("\r   Attempt {}/{}... (error: {})", attempts, MAX_ATTEMPTS, status);
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                } else {
                    return Err(format!("Unexpected response: {}", error_msg).into());
                }
            }
        }
    }
}


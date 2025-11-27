use rust_backend::bridge::relayer::ZcashRelayer;
use rust_backend::zcash::bridge_wallet::BridgeWallet;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    println!("=== Zcash → Miden Relayer ===");
    println!("Scans Zcash wallet for deposits and mints Miden notes");
    println!();

    // Get project root
    let current_dir = std::env::current_dir()
        .expect("Failed to get current directory");
    
    let project_root = if current_dir.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "rust-backend")
        .unwrap_or(false) {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir
    };

    println!("Project root: {:?}", project_root);

    // Get scan interval from env (default 5 seconds)
    let scan_interval = std::env::var("ZCASH_RELAYER_INTERVAL_SECS")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<u64>()
        .unwrap_or(5);

    println!("Scan interval: {} seconds", scan_interval);
    println!();

    // Initialize bridge wallet
    let bridge_wallet = Arc::new(BridgeWallet::new(project_root.clone()));

    // Create and start relayer
    let relayer = ZcashRelayer::new(
        bridge_wallet,
        project_root,
        scan_interval,
    );

    println!("✅ Zcash relayer started!");
    println!("Press Ctrl+C to stop");
    println!();

    relayer.start().await;
}


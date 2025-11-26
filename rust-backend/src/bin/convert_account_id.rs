use miden_objects::account::AccountId;
use miden_objects::address::NetworkId;

fn main() {
    let hex_account_id = "c17e52b187d0e6901faf7e325cb5ae";
    let bech32_with_underscore = "mtst1arqhu543slgwdyql4alryh944c8z6m9d_qruqqypuyph";
    
    println!("=== Test 1: Hex → Bech32 ===");
    println!("Hex: {}", hex_account_id);
    println!("Expected bech32 (with underscore): {}", bech32_with_underscore);
    println!();
    
    // Add 0x prefix for parsing
    let hex_with_prefix = format!("0x{}", hex_account_id);
    
    match AccountId::from_hex(&hex_with_prefix) {
        Ok(account_id) => {
            let bech32 = account_id.to_bech32(NetworkId::Testnet);
            println!("✅ Converted bech32: {}", bech32);
            println!("   Length: {} chars", bech32.len());
            println!();
            
            if bech32 == bech32_with_underscore {
                println!("✅ MATCHES (including underscore)");
            } else {
                println!("❌ DOES NOT MATCH");
                println!("   Our bech32: {}", bech32);
                println!("   Expected:   {}", bech32_with_underscore);
            }
        }
        Err(e) => {
            println!("❌ Failed to parse hex: {:?}", e);
        }
    }
    
    println!("\n=== Test 2: Bech32 (with underscore) → Hex ===");
    println!("Trying to parse: {}", bech32_with_underscore);
    match AccountId::from_bech32(bech32_with_underscore) {
        Ok((_network, account_id)) => {
            let hex = account_id.to_hex();
            println!("✅ Parsed successfully!");
            println!("   Hex: {}", hex);
            println!("   Original hex: {}", hex_with_prefix);
            if hex == hex_with_prefix {
                println!("✅ MATCHES original hex!");
            } else {
                println!("❌ DOES NOT MATCH original hex!");
            }
        }
        Err(e) => {
            println!("❌ Failed to parse bech32 with underscore: {:?}", e);
            println!("\nTrying without underscore part...");
            // Try just the first part (before underscore)
            let bech32_clean = bech32_with_underscore.split('_').next().unwrap();
            println!("Trying: {}", bech32_clean);
            match AccountId::from_bech32(bech32_clean) {
                Ok((_network, account_id)) => {
                    let hex = account_id.to_hex();
                    println!("✅ Parsed successfully!");
                    println!("   Hex: {}", hex);
                    println!("   Original hex: {}", hex_with_prefix);
                    if hex == hex_with_prefix {
                        println!("✅ MATCHES original hex!");
                    } else {
                        println!("❌ DOES NOT MATCH original hex!");
                    }
                }
                Err(e2) => {
                    println!("❌ Also failed: {:?}", e2);
                }
            }
        }
    }
}


use miden_objects::account::AccountId;
use miden_objects::address::NetworkId;

fn main() {
    let bech32_address = "mtst1azmkj4h4ce7vfyp9u04mzac2pvvdnjjt_qruqqypuyph";
    
    println!("Testing account ID format conversion:");
    println!("=====================================\n");
    
    // Try parsing from bech32
    println!("Attempting to parse bech32: {}", bech32_address);
    match AccountId::from_bech32(bech32_address) {
        Ok((network, account_id)) => {
            println!("✅ Parsed bech32 successfully!");
            println!("   Network: {:?}", network);
            
            // Convert to hex
            let hex = account_id.to_hex();
            println!("   Hex: {}", hex);
            
            // Convert back to bech32
            let bech32_again = account_id.to_bech32(NetworkId::Testnet);
            println!("   Bech32 (converted back): {}", bech32_again);
            
            // Verify they're the same
            if bech32_again == bech32_address {
                println!("\n✅ SUCCESS: Bech32 → Hex → Bech32 = Same address!");
            } else {
                println!("\n⚠️  WARNING: Bech32 conversion mismatch!");
                println!("   Original: {}", bech32_address);
                println!("   Converted: {}", bech32_again);
            }
            
            // Test hex → bech32
            println!("\n--- Testing Hex → Bech32 ---");
            match AccountId::from_hex(&hex) {
                Ok(account_from_hex) => {
                    let bech32_from_hex = account_from_hex.to_bech32(NetworkId::Testnet);
                    println!("✅ Hex: {}", hex);
                    println!("   Bech32: {}", bech32_from_hex);
                    
                    if account_from_hex == account_id {
                        println!("\n✅ SUCCESS: Hex and Bech32 represent the SAME account!");
                    } else {
                        println!("\n❌ ERROR: Hex and Bech32 are different accounts!");
                    }
                }
                Err(e) => {
                    println!("❌ Failed to parse hex: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to parse bech32: {:?}", e);
            println!("\nTrying alternative: Maybe it's hex format?");
            
            // Try as hex instead
            let hex_str = if bech32_address.starts_with("0x") {
                bech32_address.to_string()
            } else {
                format!("0x{}", bech32_address)
            };
            
            match AccountId::from_hex(&hex_str) {
                Ok(account_id) => {
                    println!("✅ Parsed as hex: {}", hex_str);
                    let hex = account_id.to_hex();
                    let bech32 = account_id.to_bech32(NetworkId::Testnet);
                    println!("   Hex: {}", hex);
                    println!("   Bech32: {}", bech32);
                }
                Err(e2) => {
                    println!("❌ Also failed as hex: {:?}", e2);
                    println!("\nThe address format might be invalid or corrupted.");
                }
            }
        }
    }
    
    // Also test creating a new account to show the format
    println!("\n\n--- Creating a test account to show format ---");
    use miden_objects::account::AccountBuilder;
    use miden_objects::account::{AccountType, AccountStorageMode};
    use miden_lib::account::auth::AuthRpoFalcon512;
    use miden_client::auth::AuthSecretKey;
    use miden_client::account::component::BasicWallet;
    use rand::RngCore;
    use rand::rng;
    
    let mut seed = [0u8; 32];
    rng().fill_bytes(&mut seed);
    
    let key_pair = AuthSecretKey::new_rpo_falcon512();
    
    let test_account = AccountBuilder::new(seed)
        .account_type(AccountType::RegularAccountUpdatableCode)
        .storage_mode(AccountStorageMode::Private)
        .with_auth_component(AuthRpoFalcon512::new(key_pair.public_key().to_commitment()))
        .with_component(BasicWallet)
        .build()
        .unwrap();
    
    let test_hex = test_account.id().to_hex();
    let test_bech32 = test_account.id().to_bech32(NetworkId::Testnet);
    
    println!("Test account created:");
    println!("   Hex: {}", test_hex);
    println!("   Bech32: {}", test_bech32);
    
    // Verify round-trip
    let parsed_from_hex = AccountId::from_hex(&test_hex).unwrap();
    let parsed_from_bech32 = AccountId::from_bech32(&test_bech32).unwrap().1;
    
    if parsed_from_hex == parsed_from_bech32 && parsed_from_hex == test_account.id() {
        println!("\n✅ SUCCESS: All formats represent the same account!");
    }
}


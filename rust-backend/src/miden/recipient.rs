use miden_lib::note::utils::build_p2id_recipient;
use miden_objects::{account::AccountId, note::NoteRecipient, Word};

/// Build a P2ID (Pay-to-ID) recipient for deposit notes
/// 
/// This creates a recipient that can only be consumed by the specified account
/// when they provide the matching secret.
pub fn build_deposit_recipient(
    account_id: AccountId,
    secret: Word,
) -> Result<NoteRecipient, String> {
    build_p2id_recipient(account_id, secret)
        .map_err(|e| format!("Failed to build recipient: {:?}", e))
}

/// Generate a random secret for a deposit
pub fn generate_secret() -> Word {
    use miden_objects::crypto::rand::{FeltRng, RpoRandomCoin};
    use miden_objects::Felt;
    
    // Use RpoRandomCoin for deterministic randomness
    let mut rng = RpoRandomCoin::new(Word::new([
        Felt::new(rand::random::<u64>()),
        Felt::new(rand::random::<u64>()),
        Felt::new(rand::random::<u64>()),
        Felt::new(rand::random::<u64>()),
    ]));
    
    rng.draw_word()
}


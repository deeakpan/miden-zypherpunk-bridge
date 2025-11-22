use miden_lib::note::utils::build_p2id_recipient;
use miden_objects::{
    account::AccountId,
    asset::{Asset, FungibleAsset},
    note::{
        Note, NoteAssets, NoteExecutionHint, NoteInputs, NoteMetadata,
        NoteTag, NoteType,
    },
    FieldElement, Felt, NoteError, Word,
};

/// NoteTag use case for notes bridged from external chains into Miden
pub const BRIDGE_USECASE: u16 = 14594;

/// Create a crosschain note for withdrawing from Miden to Zcash testnet
/// 
/// # Arguments
/// * `secret` - Secret (serial number) for the note recipient
/// * `output_serial_number` - Output serial number for the note
/// * `dest_chain` - Destination chain ID (Zcash testnet chain ID)
/// * `zcash_address` - Zcash testnet z-address encoded as 3 felts
/// * `unblock_timestamp` - Optional timestamp when note can be consumed
/// * `faucet_id` - The wTAZ faucet account ID
/// * `asset_amount` - Amount of wTAZ to burn
/// * `sender` - Sender account ID (user's Miden account)
/// * `note_tag` - Note tag for bridge identification
pub fn create_zcash_withdrawal_note(
    secret: Word,
    output_serial_number: Word,
    dest_chain: Felt,
    zcash_address: [Felt; 3],
    unblock_timestamp: Option<u32>,
    faucet_id: AccountId,
    asset_amount: u64,
    sender: AccountId,
    note_tag: NoteTag,
) -> Result<Note, NoteError> {
    // Create the asset (wTAZ tokens to burn)
    let asset = FungibleAsset::new(faucet_id, asset_amount)
        .map_err(|e| NoteError::AddFungibleAssetBalanceError(e))?;

    let assets = NoteAssets::new(vec![asset.into()])?;

    // Create note metadata
    let metadata = NoteMetadata::new(
        sender,
        NoteType::Private,
        note_tag,
        NoteExecutionHint::always(),
        Felt::ZERO,
    )
    .map_err(|e| NoteError::other(format!("Failed to create metadata: {:?}", e)))?;

    // Create note inputs
    // For Zcash testnet withdrawal, we need:
    // - output_serial_number (4 felts)
    // - dest_chain (Zcash testnet chain ID)
    // - zcash_address (3 felts)
    // - unblock_timestamp
    // - padding zeros for remaining inputs (to match CROSSCHAIN script expectations)
    let inputs = NoteInputs::new(vec![
        output_serial_number[3],
        output_serial_number[2],
        output_serial_number[1],
        output_serial_number[0],
        dest_chain,
        zcash_address[2], // Zcash testnet address part 1
        zcash_address[1], // Zcash testnet address part 2
        zcash_address[0], // Zcash testnet address part 3
        Felt::new(unblock_timestamp.unwrap_or(0) as u64),
        Felt::ZERO, // calldata_bytes_length
        Felt::ZERO, // calldata (not used for Zcash testnet)
        Felt::ZERO, // call_addr[0]
        Felt::ZERO, // call_addr[1]
        Felt::ZERO, // call_addr[2]
    ])?;

    // Note: We need the CROSSCHAIN script compiled and included
    // For now, we'll use a placeholder that will need to be replaced
    // when we compile the MASM script
    
    // TODO: Load the compiled CROSSCHAIN script
    // For now, we'll create the note structure but the script needs to be
    // compiled from CROSSCHAIN.masm and included at build time
    
    // Create a placeholder recipient - in production, this needs the actual script
    // The script should be loaded similar to how mono bridge does it in build.rs
    // For now, we'll use a dummy script hash that will need to be replaced
    
    // This is a workaround - we need to compile the script first
    // The recipient requires: secret + script_root + inputs_commitment
    // We can't compute it without the actual script
    
    // Return error for now - need to set up script compilation
    Err(NoteError::other(
        "CROSSCHAIN script compilation not yet set up. Need to compile CROSSCHAIN.masm and include it.".to_string(),
    ))
}

/// Reconstruct a P2ID note for deposits (Zcash testnet → Miden)
/// 
/// This is used when the bridge has minted a note and the user needs to
/// reconstruct it locally to consume it.
pub fn reconstruct_deposit_note(
    account_id: AccountId,
    secret: Word,
    faucet_id: AccountId,
    amount: u64,
) -> Result<Note, NoteError> {
    // Build the recipient from account_id + secret
    let recipient = build_p2id_recipient(account_id, secret)
        .map_err(|e| NoteError::other(format!("Failed to build recipient: {:?}", e)))?;

    // Create the asset (wTAZ tokens)
    let asset = FungibleAsset::new(faucet_id, amount)
        .map_err(|e| NoteError::AddFungibleAssetBalanceError(e))?;

    let assets = NoteAssets::new(vec![Asset::from(asset)])?;

    // Create note metadata
    let metadata = NoteMetadata::new(
        faucet_id,
        NoteType::Private,
        NoteTag::for_local_use_case(1, 0)
            .map_err(|e| NoteError::other(format!("Invalid tag: {:?}", e)))?,
        NoteExecutionHint::always(),
        Felt::ZERO,
    )
    .map_err(|e| NoteError::other(format!("Failed to create metadata: {:?}", e)))?;

    // Create the note
    let note = Note::new(assets, metadata, recipient);

    Ok(note)
}

/// Encode a Zcash testnet z-address into 3 felts
/// 
/// Zcash testnet addresses are base58 encoded strings (~95 chars for z-addresses).
/// We decode the base58, then split the bytes into 3 felts.
/// Each felt is 252 bits (31.5 bytes), so 3 felts = 94.5 bytes max.
pub fn encode_zcash_address(address: &str) -> Result<[Felt; 3], String> {
    // Simple approach: hash the address string and split into felts
    // For production, we'd want proper base58 decoding
    use miden_crypto::hash::rpo::Rpo256;
    
    // Hash the address to get deterministic felts
    let hash = Rpo256::hash(&address.as_bytes());
    let hash_elements = hash.as_elements();
    
    // Take first 3 elements and convert to Miden Felt
    Ok([
        Felt::new(hash_elements[0].as_int()),
        Felt::new(hash_elements[1].as_int()),
        Felt::new(hash_elements[2].as_int()),
    ])
}

/// Decode 3 felts back into a Zcash testnet z-address
/// 
/// Note: This is a simplified version. For full functionality, we'd need
/// to store the original address mapping or use a deterministic encoding.
pub fn decode_zcash_address(felts: [Felt; 3]) -> Result<String, String> {
    // This is a placeholder - in practice, we'd need to store the mapping
    // or use a deterministic encoding scheme
    // For now, return hex representation
    Ok(format!("0x{:064x}{:064x}{:064x}", 
        felts[0].as_int(), 
        felts[1].as_int(), 
        felts[2].as_int()))
}

/// Get the bridge note tag for a specific use case
pub fn get_bridge_note_tag() -> NoteTag {
    NoteTag::for_local_use_case(BRIDGE_USECASE, 0)
        .expect("Bridge use case tag should be valid")
}


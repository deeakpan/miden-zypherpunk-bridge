use miden_objects::{
    account::{AccountComponent, AccountType, StorageSlot},
    utils::{sync::LazyLock, Deserializable},
    assembly::Library,
    Felt, Word,
};

static TOKEN_WRAPPER_ACCOUNT_CODE: LazyLock<Library> = LazyLock::new(|| {
    let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/assets/contracts/fungible_wrapper.masl"));
    Library::read_from_bytes(bytes).expect("Shipped Token wrapper library is well-formed")
});

pub fn token_wrapper_account_library() -> Library {
    TOKEN_WRAPPER_ACCOUNT_CODE.clone()
}

pub struct TokenWrapperAccount {
    origin_network: u64,
    origin_address: [Felt; 3],
}

impl TokenWrapperAccount {
    pub fn new(origin_network: u64, origin_address: [Felt; 3]) -> Self {
        Self {
            origin_network,
            origin_address,
        }
    }
}

impl From<TokenWrapperAccount> for AccountComponent {
    fn from(wrapper: TokenWrapperAccount) -> Self {
        AccountComponent::new(
            token_wrapper_account_library(),
            vec![StorageSlot::Value(Word::new([
                Felt::new(wrapper.origin_network),
                wrapper.origin_address[2],
                wrapper.origin_address[1],
                wrapper.origin_address[0],
            ]))],
        )
        .expect("Failed to create TokenWrapperAccount component")
        .with_supported_type(AccountType::FungibleFaucet)
    }
}


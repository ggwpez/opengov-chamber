use frame_support::{construct_runtime, derive_impl, traits::ConstU32};
use frame_system::EnsureSigned;
use sp_runtime::{AccountId32, BuildStorage, traits::IdentityLookup};

pub type Balance = u64;
pub type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime! {
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Timestamp: pallet_timestamp,
        Revive: pallet_revive,
    }
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId32;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type AccountStore = System;
    // pallet_revive declares freeze reasons; balances must allow at least that many.
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = ConstU32<10>;
}

#[derive_impl(pallet_timestamp::config_preludes::TestDefaultConfig)]
impl pallet_timestamp::Config for Test {}

#[derive_impl(pallet_revive::config_preludes::TestDefaultConfig)]
impl pallet_revive::Config for Test {
    type Time = Timestamp;
    type Balance = Balance;
    type Currency = Balances;
    type AddressMapper = pallet_revive::AccountId32Mapper<Self>;
    type DepositPerByte = ();
    type DepositPerItem = ();
    type DepositPerChildTrieItem = ();
    // Override the prelude defaults — the prelude uses `Self::AccountId` which
    // doesn't substitute correctly under `derive_impl` for #[no_default_bounds] items.
    type UploadOrigin = EnsureSigned<AccountId32>;
    type InstantiateOrigin = EnsureSigned<AccountId32>;
}

/// Build a fresh `TestExternalities`. Inside the closure, call
/// [`fund`] for any account you want to give a balance to — pallet_revive
/// matches that pattern rather than seeding balances at genesis.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}

/// Set an account's free balance. Use inside `execute_with`.
pub fn fund(who: &AccountId32, amount: Balance) {
    use frame_support::traits::fungible::Mutate;
    let _ = <Balances as Mutate<AccountId32>>::set_balance(who, amount);
}

/// Solidity 4-byte selector: first 4 bytes of `keccak256(signature)`.
pub fn selector(signature: &str) -> [u8; 4] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&sp_core::keccak_256(signature.as_bytes()));
    [out[0], out[1], out[2], out[3]]
}

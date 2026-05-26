//! Test harness that runs the contract against the **real** Asset Hub Polkadot
//! runtime instead of a mock. We re-export the runtime's `Runtime` as `Test`
//! (plus the aggregate `RuntimeOrigin`/`RuntimeEvent`/`System`/`Balances`) so the
//! integration tests can drive `pallet_revive` exactly as the live chain wires
//! it — including the `XcmPrecompile` that `finalize()` dispatches through.

pub use asset_hub_polkadot_runtime::{
    Balances, Runtime as Test, RuntimeEvent, RuntimeGenesisConfig, RuntimeOrigin, System,
};

use sp_runtime::{AccountId32, BuildStorage};

/// Native balance type of Asset Hub (DOT has 10 decimals).
pub type Balance = <Test as pallet_balances::Config>::Balance;

/// A large per-account endowment for tests. Comfortably covers the contract's
/// code/storage deposits under Asset Hub's real pricing, while staying far below
/// `u128::MAX` so that funding several accounts can't overflow `TotalIssuance`
/// (which is what broke the storage-deposit hold when we used `u128::MAX / 2`).
pub const ENDOWMENT: Balance = 1 << 90;

/// Build a fresh `TestExternalities` from the runtime's default genesis.
///
/// Inside the closure, call [`fund`] for any account you want to give a balance
/// to — `pallet_revive` matches that pattern rather than seeding balances at
/// genesis.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = RuntimeGenesisConfig::default().build_storage().unwrap();
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

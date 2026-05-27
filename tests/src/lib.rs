//! Test harness that runs the contract against the **real** Asset Hub Polkadot
//! runtime instead of a mock. We re-export the runtime's `Runtime` as `Test`
//! (plus the aggregate `RuntimeOrigin`/`RuntimeEvent`/`System`/`Balances`) so the
//! integration tests can drive `pallet_revive` exactly as the live chain wires
//! it — including the `XcmPrecompile` that `finalize()` dispatches through.

pub use asset_hub_polkadot_runtime::{
    Balances, ConvictionVoting, Referenda, Runtime as Test, RuntimeEvent, RuntimeGenesisConfig,
    RuntimeOrigin, Scheduler, System,
};

use cumulus_pallet_parachain_system::RelaychainDataProvider;
use frame_support::traits::OnInitialize;
use sp_runtime::{traits::BlockNumberProvider, AccountId32, BuildStorage};

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

/// Relay-chain block number — the clock governance actually runs on.
pub type RelayBlockNumber = u32;

/// Mock the relay-chain block number that the governance pallets read.
///
/// On Asset Hub, `pallet_referenda`, `pallet_scheduler`, and
/// `pallet_conviction_voting` are all configured with
/// `BlockNumberProvider = RelaychainDataProvider` — they track the *relay* block,
/// not the parachain's `System` block. `set_block_number` is available here only
/// because the runtime is built with `std`. Use inside `execute_with`.
pub fn set_relay_block(n: RelayBlockNumber) {
    RelaychainDataProvider::<Test>::set_block_number(n);
}

/// The current mocked relay-chain block number.
pub fn relay_block() -> RelayBlockNumber {
    RelaychainDataProvider::<Test>::current_block_number()
}

/// Drive governance time forward until `cond` holds.
///
/// `pallet_referenda` has no `on_initialize`; a referendum only advances when the
/// scheduler fires the `nudge_referendum` alarm it set, and the scheduler services
/// its agenda at the *relay* block number. So we bump the relay block one at a time
/// and run the scheduler each step. Stepping by one keeps the scheduler's
/// `IncompleteSince` cursor continuous, so no alarm is ever skipped. Panics if
/// `cond` is still false after `max` blocks.
pub fn roll_relay_until(mut cond: impl FnMut() -> bool, max: RelayBlockNumber) {
    for _ in 0..max {
        if cond() {
            return;
        }
        set_relay_block(relay_block() + 1);
        // This scheduler ignores the argument and reads "now" from the relay
        // `BlockNumberProvider`; the value passed is irrelevant.
        Scheduler::on_initialize(System::block_number());
    }
    assert!(cond(), "referendum condition not met within {max} relay blocks");
}

/// Solidity 4-byte selector: first 4 bytes of `keccak256(signature)`.
pub fn selector(signature: &str) -> [u8; 4] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&sp_core::keccak_256(signature.as_bytes()));
    [out[0], out[1], out[2], out[3]]
}

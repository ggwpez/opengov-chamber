//! Prints the estimated **weight** and **transaction fee in DOT** for deploying
//! the contract on the *real* Asset Hub Polkadot runtime.
//!
//! Two different numbers are charged when you deploy a `pallet_revive` contract,
//! and people routinely conflate them:
//!
//!   * **Transaction fee** — `base_fee + length_fee + weight_fee`, priced by
//!     `pallet_transaction_payment`. This is consumed (burned/treasury), not
//!     refundable. It's what `compute_fee` returns.
//!   * **Storage deposit** — a *refundable hold* taken to cover the on-chain code
//!     blob + contract storage. It is NOT part of the tx fee. For a ~44 KB blob
//!     this dwarfs the fee, so we print it too to avoid the usual sticker shock.
//!
//! How the estimate is built (same flow a wallet/dapp uses):
//!   1. Dry-run `bare_instantiate` to measure the weight the deploy actually
//!      needs (`weight_required`) and the storage deposit it would lock.
//!   2. Construct the *real* `revive.instantiateWithCode` extrinsic with that
//!      weight as its limit — its declared `DispatchInfo` weight and its encoded
//!      length are exactly what the fee formula prices.
//!   3. Run it through `pallet_transaction_payment::compute_fee[_details]`.

use codec::Encode;
use contract_tests::{ENDOWMENT, RuntimeCall, RuntimeOrigin, Test, fund, new_test_ext};
use frame_support::dispatch::GetDispatchInfo;
use pallet_revive::{
    Code, TransactionLimits, Weight,
    test_utils::{ALICE, builder::BareInstantiateBuilder},
};

/// Release-mode PolkaVM blob — the same artifact the integration tests deploy and
/// the same bytes that would go on-chain. Built by `cd ../contract && cargo build`.
const BLOB: &[u8] = include_bytes!("../../target/contract.release.polkavm");

/// DOT has 10 decimals on Polkadot (and Asset Hub).
const PLANCK_PER_DOT: u128 = 10_000_000_000;

/// Format a planck amount as a fixed-point DOT string, e.g. `1.2340000000 DOT`.
fn dot(plancks: u128) -> String {
    let whole = plancks / PLANCK_PER_DOT;
    let frac = plancks % PLANCK_PER_DOT;
    format!("{whole}.{frac:010} DOT")
}

#[test]
fn deploy_fee_estimate() {
    new_test_ext().execute_with(|| {
        fund(&ALICE, ENDOWMENT);

        // 1. Dry-run: measure the real weight + storage deposit for this blob.
        //    `weight_required` is the value you'd pass as the on-chain weight limit.
        let dry = BareInstantiateBuilder::<Test>::bare_instantiate(
            RuntimeOrigin::signed(ALICE),
            Code::Upload(BLOB.to_vec()),
        )
        .transaction_limits(TransactionLimits::WeightAndDeposit {
            weight_limit: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
            deposit_limit: u128::MAX / 2,
        })
        .build();

        assert!(
            dry.result.is_ok(),
            "dry-run deploy failed: {:?}",
            dry.result
        );
        let weight_required = dry.weight_required;
        let storage_deposit = dry.storage_deposit.charge_or_zero();

        // 2. Build the actual extrinsic a user would sign, using the measured
        //    weight as the limit. (value = 0, no constructor data, no salt.)
        let call = RuntimeCall::Revive(pallet_revive::Call::instantiate_with_code {
            value: 0,
            weight_limit: weight_required,
            storage_deposit_limit: storage_deposit,
            code: BLOB.to_vec(),
            data: Vec::new(),
            salt: None,
        });

        let info = call.get_dispatch_info();
        // Encoded length of the call payload. The signed-extrinsic envelope adds a
        // fixed ~100 bytes of signature/extra on top; negligible next to a 44 KB
        // blob, so we price the call length directly.
        let len = call.encoded_size() as u32;

        // 3. Price it (tip = 0). The details split out base/length/weight fees.
        let details = pallet_transaction_payment::Pallet::<Test>::compute_fee_details(len, &info, 0);
        let fee = details.final_fee();
        let inc = details.inclusion_fee.expect("instantiate pays fees");

        let w = info.total_weight();
        eprintln!("\n========== Asset Hub Polkadot — contract deploy estimate ==========");
        eprintln!("blob size                : {} bytes", BLOB.len());
        eprintln!("encoded extrinsic length : {len} bytes");
        eprintln!("--- weight (instantiateWithCode limit) ---");
        eprintln!("  ref_time               : {}", w.ref_time());
        eprintln!("  proof_size             : {} bytes", w.proof_size());
        eprintln!("--- transaction fee (consumed, non-refundable) ---");
        eprintln!("  base fee               : {}", dot(inc.base_fee));
        eprintln!("  length fee             : {}", dot(inc.len_fee));
        eprintln!("  weight fee             : {}", dot(inc.adjusted_weight_fee));
        eprintln!("  TOTAL tx fee           : {}", dot(fee));
        eprintln!("--- storage deposit (refundable hold, NOT a fee) ---");
        eprintln!("  code + storage deposit : {}", dot(storage_deposit));
        eprintln!("--- grand total locked on deploy ---");
        eprintln!("  tx fee + deposit       : {}", dot(fee + storage_deposit));
        eprintln!("===================================================================\n");

        // Sanity: a real deploy of a non-empty blob must cost something.
        assert!(fee > 0, "tx fee should be non-zero");
        assert!(storage_deposit > 0, "storage deposit should be non-zero");
    });
}

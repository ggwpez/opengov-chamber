//! Encoding tests for the contract's `xcm::referendum` helpers.
//!
//! The contract hand-SCALE-encodes the `Referenda::submit` runtime call and its
//! XCM `Transact` wrapper using pinned pallet/variant indices (it has no access
//! to the runtime's types at build time). These tests rebuild the *same* call
//! with the **real** `asset-hub-polkadot-runtime` types and assert the bytes
//! match — so if the runtime ever renumbers a pallet, reorders `OriginCaller`,
//! or bumps an XCM version, CI catches the drift here instead of on-chain.

use asset_hub_polkadot_runtime::{
    OriginCaller, RuntimeCall, governance::pallet_custom_origins,
};
use codec::{Decode, Encode};
use contract::xcm::{IXcm, referendum};
use contract::sol_types::SolCall;
use contract_tests::Test;
use frame_support::traits::{Bounded, schedule::DispatchTime};
use sp_core::H256;
use xcm::{VersionedXcm, v5::prelude::*};

/// Sample preimage length / enactment delay, threaded through both the contract
/// helper and the real runtime call so the expected encoding is byte-identical.
/// Non-zero on purpose, so the test would catch either value being dropped.
const PREIMAGE_LEN: u32 = 42;
const ENACTMENT_DELAY: u32 = 100;

/// Build the *real* `RuntimeCall::Referenda(submit { .. })` the contract intends
/// to dispatch, using Asset Hub's own types.
fn expected_submit_call(call_hash: H256) -> RuntimeCall {
    RuntimeCall::Referenda(pallet_referenda::Call::submit {
        // `OriginCaller::Origins(WhitelistedCaller)` — the whitelisted-call track.
        proposal_origin: Box::new(OriginCaller::Origins(
            pallet_custom_origins::Origin::WhitelistedCaller,
        )),
        // `Bounded::Lookup { hash, len }` — a preimage lookup by hash.
        proposal: Bounded::Lookup {
            hash: call_hash,
            len: PREIMAGE_LEN,
        },
        enactment_moment: DispatchTime::After(ENACTMENT_DELAY),
    })
}

/// The contract hardcodes the referendum `SubmissionDeposit` and pallet-revive's
/// `NativeToEthRatio` (it can't read runtime constants at execution time). Pin
/// both to the real runtime values so a future change fails CI here rather than
/// on-chain — they're multiplied together for the `finalize()` deposit check.
#[test]
fn submission_deposit_matches_runtime() {
    use asset_hub_polkadot_runtime::Runtime;
    use frame_support::traits::Get;

    assert_eq!(
        referendum::SUBMISSION_DEPOSIT,
        <Runtime as pallet_referenda::Config>::SubmissionDeposit::get(),
        "contract's hardcoded SUBMISSION_DEPOSIT diverged from the runtime's",
    );
    assert_eq!(
        referendum::NATIVE_TO_ETH_RATIO,
        u128::from(<<Runtime as pallet_revive::Config>::NativeToEthRatio as Get<u32>>::get()),
        "contract's hardcoded NATIVE_TO_ETH_RATIO diverged from the runtime's",
    );
}

#[test]
fn encode_submit_call_matches_runtime() {
    let call_hash = [0xAA; 32];

    let from_contract = referendum::encode_submit_call(&call_hash, PREIMAGE_LEN, ENACTMENT_DELAY);
    let from_runtime = expected_submit_call(H256::from(call_hash)).encode();

    assert_eq!(
        from_contract, from_runtime,
        "contract's hand-encoded Referenda::submit call diverged from the real \
         asset-hub-polkadot-runtime encoding (check pallet/variant indices in xcm.rs)"
    );
}

#[test]
fn execute_calldata_wraps_submit_call_in_transact() {
    let call_hash = [0xAA; 32];

    // Decode the `IXcm.execute(message, weight)` calldata the contract dispatches.
    let calldata = referendum::build_execute_calldata(&call_hash, PREIMAGE_LEN, ENACTMENT_DELAY);
    let decoded = IXcm::executeCall::abi_decode_validate(&calldata)
        .expect("build_execute_calldata must produce valid IXcm.execute calldata");

    // The `message` is a SCALE-encoded `VersionedXcm`; decode it with real XCM types.
    let message: Vec<u8> = decoded.message.to_vec();
    let versioned = VersionedXcm::<()>::decode(&mut &message[..])
        .expect("message must decode as VersionedXcm");

    let xcm = match versioned {
        VersionedXcm::V5(xcm) => xcm,
        other => panic!("expected XCM v5, got {:?}", other),
    };

    // Exactly one instruction: a `Transact` carrying the submit call.
    assert_eq!(xcm.0.len(), 1, "expected a single-instruction XCM");
    match &xcm.0[0] {
        Instruction::Transact {
            origin_kind,
            call,
            ..
        } => {
            assert_eq!(*origin_kind, OriginKind::SovereignAccount);

            // The Transact's inner call must be the real Referenda::submit encoding.
            let inner = call.clone().into_encoded();
            let expected = expected_submit_call(H256::from(call_hash)).encode();
            assert_eq!(
                inner, expected,
                "Transact's inner call diverged from the real runtime encoding"
            );
        }
        other => panic!("expected a Transact instruction, got {:?}", other),
    }
}

/// Multiple by which the contract's hardcoded weights are allowed to exceed the
/// weight they must cover. Over-estimating is fine (and required — see below),
/// but a blowout this large almost certainly means a constant was fat-fingered
/// (e.g. a wrong unit), so we flag it. The placeholders target ~5x the real
/// cost, so 10x leaves headroom for tuning/weight drift without going slack.
const MAX_WEIGHT_HEADROOM: u64 = 10;

/// Assert `granted` covers `need` on both dimensions, by at least 1x and at most
/// [`MAX_WEIGHT_HEADROOM`]x. `need` is the real runtime figure; `granted` is what
/// the contract hardcodes.
fn assert_covers(label: &str, granted: Weight, need: Weight) {
    for (dim, g, n) in [
        ("ref_time", granted.ref_time(), need.ref_time()),
        ("proof_size", granted.proof_size(), need.proof_size()),
    ] {
        assert!(
            g >= n,
            "{label} {dim} ({g}) is below the runtime's required {n}; \
             the dispatch would run out of weight",
        );
        assert!(
            g <= n.saturating_mul(MAX_WEIGHT_HEADROOM),
            "{label} {dim} ({g}) over-estimates the runtime's {n} by more than \
             {MAX_WEIGHT_HEADROOM}x — likely a wrong constant in xcm.rs",
        );
    }
}

/// The contract grants two weights it can't read from the runtime at execution
/// time, so they're hardcoded in `xcm.rs`. Neither has to be *exact* (unlike the
/// pallet indices or the deposit) — a comfortable over-estimate is the intended
/// behaviour — but each MUST cover the real cost, or the dispatch fails with
/// `WeightLimitReached`/`Overweight`. We pull both out of the *actual* calldata
/// the contract emits (not the consts) so this tests what really gets dispatched.
#[test]
fn hardcoded_weights_cover_runtime_cost_with_bounded_headroom() {
    use pallet_referenda::WeightInfo;
    use asset_hub_polkadot_runtime::xcm_config::XcmConfig;
    use xcm_executor::{Config, traits::WeightBounds};

    let calldata = referendum::build_execute_calldata(&[0xAA; 32], PREIMAGE_LEN, ENACTMENT_DELAY);
    let decoded = IXcm::executeCall::abi_decode_validate(&calldata)
        .expect("build_execute_calldata must produce valid IXcm.execute calldata");

    // (1) The weight handed to the whole local XCM execution (`IXcm.execute`'s
    // arg) must cover what the runtime's own XCM weigher charges for this exact
    // message — the authority on what `execute()` needs. Decode as
    // `Xcm<RuntimeCall>` so the weigher can decode the Transact's inner call and
    // bill its real dispatch weight, then weigh it with the runtime's configured
    // `Weigher` (the same `xcm.rs` TODO suggests doing this via `weighMessage`).
    let message: Vec<u8> = decoded.message.to_vec();
    let mut xcm: Xcm<RuntimeCall> = VersionedXcm::<RuntimeCall>::decode(&mut &message[..])
        .expect("message must decode as VersionedXcm")
        .try_into()
        .expect("message must be a supported XCM version");
    let needed = <XcmConfig as Config>::Weigher::weight(&mut xcm, Weight::MAX)
        .expect("the runtime must be able to weigh the contract's XCM message");
    let granted = Weight::from_parts(decoded.weight.refTime, decoded.weight.proofSize);
    assert_covers("XCM execution weight", granted, needed);

    // (2) The inner Transact's `fallback_max_weight` — the weight assumed for the
    // dispatched call when it can't be decoded — must cover the real
    // `Referenda::submit` weight from the runtime's configured `WeightInfo`.
    let submit = <Test as pallet_referenda::Config>::WeightInfo::submit();
    let VersionedXcm::V5(xcm) = VersionedXcm::<()>::decode(&mut &message[..])
        .expect("message must decode as VersionedXcm")
    else {
        panic!("expected XCM v5");
    };
    match &xcm.0[0] {
        Instruction::Transact { fallback_max_weight, .. } => {
            let fallback =
                fallback_max_weight.expect("contract sets fallback_max_weight: Some(..)");
            assert_covers("Transact fallback_max_weight", fallback, submit);
        }
        other => panic!("expected a Transact instruction, got {other:?}"),
    }
}

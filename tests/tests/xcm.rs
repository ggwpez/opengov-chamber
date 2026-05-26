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
use frame_support::traits::{Bounded, schedule::DispatchTime};
use sp_core::H256;
use xcm::{VersionedXcm, v5::prelude::*};

/// The same placeholder values the contract bakes in (`xcm.rs`). Kept here so the
/// expected runtime call is byte-identical to what `encode_submit_call` builds.
const PREIMAGE_LEN: u32 = 0;
const ENACTMENT_DELAY: u32 = 0;

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

#[test]
fn encode_submit_call_matches_runtime() {
    let call_hash = [0xAA; 32];

    let from_contract = referendum::encode_submit_call(&call_hash);
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
    let calldata = referendum::build_execute_calldata(&call_hash);
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

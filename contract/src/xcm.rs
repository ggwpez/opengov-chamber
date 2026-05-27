//! Local XCM dispatch helpers for the proposal contract.

use alloy_core::sol;

// The XCM precompile's Solidity interface (see polkadot-sdk:
// `polkadot/xcm/pallet-xcm/precompiles/src/interface/IXcm.sol`). We only use
// `execute`, which runs a SCALE-encoded `VersionedXcm` locally under the
// caller's (this contract's) signed origin.
sol! {
    interface IXcm {
        struct Weight {
            uint64 refTime;
            uint64 proofSize;
        }
        function execute(bytes message, Weight weight) external;
    }
}

/// XCM precompile address on Asset Hub. `AddressMatcher::Fixed(10)` resolves
/// to `10 << 16 == 0xA0000` (confirmed in pallet-xcm precompile tests).
pub const XCM_PRECOMPILE_ADDR: [u8; 20] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0A, 0, 0,
];

/// Builds the `Referenda::submit` runtime call for a finalized proposal, wrapped
/// in a local XCM `Transact` so it can be dispatched through Asset Hub's XCM
/// precompile via `api::call`.
///
/// ⚠️ Several values here are runtime-specific and/or placeholders that MUST be
/// finalized before running against a live Asset Hub runtime — see the `TODO`s.
/// The SCALE layout (instruction/variant indices) is pinned to XCM v5 and the
/// pallet indices to the current Asset Hub `construct_runtime!`.
pub mod referendum {
    use super::IXcm;
    use alloc::vec::Vec;
    use alloy_core::sol_types::SolCall;
    use parity_scale_codec::{Compact, Encode};

    /// `pallet_referenda` index in Asset Hub `construct_runtime!`.
    const REFERENDA_PALLET_INDEX: u8 = 62;
    /// `Referenda::submit` is `#[pallet::call_index(0)]`.
    const REFERENDA_SUBMIT_CALL_INDEX: u8 = 0;

    /// `OriginCaller::Origins` variant — `construct_runtime!` numbers `OriginCaller`
    /// variants by pallet index, and `pallet_custom_origins` is index 63 on Asset Hub.
    const ORIGINS_VARIANT_INDEX: u8 = 63;
    /// `pallet_custom_origins::Origin::WhitelistedCaller` — the 14th (index 13)
    /// variant in that pallet's `Origin` enum.
    const WHITELISTED_CALLER_ORIGIN_INDEX: u8 = 13;

    /// `Referenda::submit` reserves `T::SubmissionDeposit` from the dispatch
    /// origin — which, via the XCM `Transact`, is the contract's own sovereign
    /// account. So `finalize()` requires the caller to send at least this much
    /// value, leaving the contract able to cover the deposit. Pinned to Asset
    /// Hub's `SubmissionDeposit` (10 DOT = `10 * DOLLARS`, DOT having 10 decimals)
    /// and verified against the real runtime in `tests/tests/xcm.rs`.
    ///
    /// This is in **native** plancks. Note pallet-revive denominates the value a
    /// contract observes via `value_transferred` in *EVM* units, so the check in
    /// `finalize()` scales this by [`NATIVE_TO_ETH_RATIO`].
    pub const SUBMISSION_DEPOSIT: u128 = 100_000_000_000;

    /// pallet-revive's `NativeToEthRatio` on Asset Hub (`10^(18-10)`): the factor
    /// between native plancks and the EVM-denominated balances that host functions
    /// like `value_transferred` report. Pinned + verified in `tests/tests/xcm.rs`.
    pub const NATIVE_TO_ETH_RATIO: u128 = 100_000_000;

    // --- Placeholders (see the module-level warning) ------------------------

    /// TODO(placeholder): fallback weight for the inner `submit` call. Set to
    /// roughly 5x the real `Referenda::submit` weight on Asset Hub (~204M
    /// ref_time / 42k proof_size); see `tests/tests/xcm.rs`.
    const FALLBACK_REF_TIME: u64 = 1_000_000_000;
    const FALLBACK_PROOF_SIZE: u64 = 210_000;
    /// TODO(placeholder): weight granted to the local XCM execution. Ideally this
    /// should be obtained from `IXcm.weighMessage(message)` rather than hardcoded.
    /// Set to roughly 5x the runtime weigher's figure for this message (~210M
    /// ref_time / 42k proof_size); see `tests/tests/xcm.rs`.
    pub const XCM_EXEC_REF_TIME: u64 = 1_100_000_000;
    pub const XCM_EXEC_PROOF_SIZE: u64 = 220_000;

    /// SCALE-encode `RuntimeCall::Referenda(submit { .. })` referencing `call_hash`
    /// as a preimage lookup.
    ///
    /// `preimage_len` is the encoded byte length of the preimage `call_hash`
    /// points at (`Bounded::Lookup { hash, len }`), and `enactment_delay` is the
    /// number of blocks to wait after the referendum passes
    /// (`DispatchTime::After(enactment_delay)`). Both come from the proposal.
    pub fn encode_submit_call(
        call_hash: &[u8; 32],
        preimage_len: u32,
        enactment_delay: u32,
    ) -> Vec<u8> {
        let mut call = Vec::new();
        call.push(REFERENDA_PALLET_INDEX);
        call.push(REFERENDA_SUBMIT_CALL_INDEX);

        // proposal_origin: Box<OriginCaller>.
        // `OriginCaller::Origins(pallet_custom_origins::Origin::WhitelistedCaller)`
        // — the governance track for dispatching whitelisted calls.
        call.push(ORIGINS_VARIANT_INDEX);
        call.push(WHITELISTED_CALLER_ORIGIN_INDEX);

        // proposal: Bounded::Lookup { hash, len } (variant index 2).
        call.push(0x02);
        call.extend_from_slice(call_hash);
        call.extend_from_slice(&preimage_len.to_le_bytes());

        // enactment_moment: DispatchTime::After(n) (variant index 1).
        call.push(0x01);
        call.extend_from_slice(&enactment_delay.to_le_bytes());

        call
    }

    /// Wrap an encoded runtime call in a single-instruction XCM v5 `Transact`,
    /// returning the SCALE-encoded `VersionedXcm` bytes expected by `IXcm.execute`.
    fn encode_xcm_transact(call: &[u8]) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.push(0x05); // VersionedXcm::V5

        // Xcm(Vec<Instruction>) with a single instruction.
        Compact(1u32).encode_to(&mut msg);

        // Transact { origin_kind, fallback_max_weight, call } (v5 instruction index 6).
        msg.push(0x06);
        msg.push(0x01); // OriginKind::SovereignAccount
        msg.push(0x01); // fallback_max_weight: Option::Some
        Compact(FALLBACK_REF_TIME).encode_to(&mut msg); // Weight.ref_time (compact)
        Compact(FALLBACK_PROOF_SIZE).encode_to(&mut msg); // Weight.proof_size (compact)

        // call: DoubleEncoded<Call> == length-prefixed Vec<u8>.
        Compact(call.len() as u32).encode_to(&mut msg);
        msg.extend_from_slice(call);

        msg
    }

    /// Build the full `IXcm.execute` calldata for finalizing `call_hash` into a
    /// referendum. See [`encode_submit_call`] for `preimage_len`/`enactment_delay`.
    pub fn build_execute_calldata(
        call_hash: &[u8; 32],
        preimage_len: u32,
        enactment_delay: u32,
    ) -> Vec<u8> {
        let call = encode_submit_call(call_hash, preimage_len, enactment_delay);
        let message = encode_xcm_transact(&call);

        IXcm::executeCall {
            message: message.into(),
            weight: IXcm::Weight {
                refTime: XCM_EXEC_REF_TIME,
                proofSize: XCM_EXEC_PROOF_SIZE,
            },
        }
        .abi_encode()
    }
}

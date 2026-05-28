//! Unit tests for the packed storage codec (`contract::codec`).
//!
//! These pin the on-storage byte layout: a drift here would silently corrupt
//! every existing proposal, so a golden vector is the load-bearing test.

use contract::{
    Address, B256, Contract, U256,
    codec::{
        self, CodecError, MAX_APPROVERS, MAX_ENCODED_LEN, MIN_ENCODED_LEN, MIN_IDENTITY_LEN,
        VERSION,
    },
};

/// `DispatchTime::After(n)` — the common enactment shape in these fixtures.
fn after(n: u32) -> Contract::DispatchTime {
    Contract::DispatchTime {
        kind: Contract::DispatchTimeKind::After,
        block: n,
    }
}

/// A non-zero proposal that exercises every field. Used as the golden vector
/// for both the Rust codec test below and the TS parity test in
/// `frontend/src/lib/proposalCodec.test.ts` — keep both in lockstep.
fn fixture() -> Contract::Proposal {
    Contract::Proposal {
        callHash: B256::repeat_byte(0xAA),
        callLen: 42,
        enactment: after(100),
        track: Contract::Track::WhitelistedCaller,
        creator: Address::repeat_byte(0x11),
        approvers: vec![Address::repeat_byte(0x22), Address::repeat_byte(0x33)],
        minApprovers: U256::from(2u64),
        approvedBy: vec![Address::repeat_byte(0x22)],
        status: Contract::ProposalStatus::Review,
    }
}

/// The bytes [`fixture`] encodes to. Hand-computed against the layout in
/// `codec.rs`: `version | callHash | callLen | enactment(kind+block) | track |
///              creator | approvers_len | approvers... | minApprovers |
///              approvedBy_len | approvedBy... | status`.
/// Layout = 67 + 20·(N+M) = 127 bytes.
fn fixture_bytes() -> Vec<u8> {
    let mut v = Vec::new();
    v.push(VERSION);
    v.extend_from_slice(&[0xAA; 32]); // callHash
    v.extend_from_slice(&42u32.to_le_bytes()); // callLen
    v.push(1); // enactment.kind = After
    v.extend_from_slice(&100u32.to_le_bytes()); // enactment.block
    v.push(1); // track = WhitelistedCaller
    v.extend_from_slice(&[0x11; 20]); // creator
    v.push(2); // approvers.len()
    v.extend_from_slice(&[0x22; 20]); // approvers[0]
    v.extend_from_slice(&[0x33; 20]); // approvers[1]
    v.push(2); // minApprovers
    v.push(1); // approvedBy.len()
    v.extend_from_slice(&[0x22; 20]); // approvedBy[0]
    v.push(0); // status = Review
    v
}

#[test]
fn round_trip_matches_input() {
    let prop = fixture();
    let bytes = codec::encode(&prop).unwrap();
    assert_eq!(bytes, fixture_bytes());
    assert_eq!(bytes.len(), 67 + 20 * (2 + 1));
    let decoded = codec::decode(&bytes).unwrap();
    assert_eq!(decoded, prop);
}

#[test]
fn empty_arrays_encode_to_min_len() {
    let prop = Contract::Proposal {
        callHash: B256::ZERO,
        callLen: 0,
        enactment: after(0),
        track: Contract::Track::Root,
        creator: Address::ZERO,
        approvers: vec![],
        minApprovers: U256::ZERO,
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
    };
    let bytes = codec::encode(&prop).unwrap();
    assert_eq!(bytes.len(), MIN_ENCODED_LEN);
    assert_eq!(codec::decode(&bytes).unwrap(), prop);
}

#[test]
fn each_status_round_trips() {
    for status in [
        Contract::ProposalStatus::Review,
        Contract::ProposalStatus::Submitted,
        Contract::ProposalStatus::Closed,
    ] {
        let mut prop = fixture();
        prop.status = status;
        let bytes = codec::encode(&prop).unwrap();
        assert_eq!(codec::decode(&bytes).unwrap(), prop);
    }
}

#[test]
fn each_enactment_kind_round_trips() {
    for kind in [
        Contract::DispatchTimeKind::At,
        Contract::DispatchTimeKind::After,
    ] {
        let mut prop = fixture();
        prop.enactment = Contract::DispatchTime { kind, block: 7 };
        let bytes = codec::encode(&prop).unwrap();
        assert_eq!(codec::decode(&bytes).unwrap(), prop);
    }
}

#[test]
fn each_track_round_trips() {
    for track in [Contract::Track::Root, Contract::Track::WhitelistedCaller] {
        let mut prop = fixture();
        prop.track = track;
        let bytes = codec::encode(&prop).unwrap();
        assert_eq!(codec::decode(&bytes).unwrap(), prop);
    }
}

#[test]
fn fits_max_approvers_at_storage_cap() {
    // The format is `67 + 20·(N+M)`; with M=0 that yields N ≤ 17 within the
    // 416-byte cap. Any more and we'd refuse to encode.
    let approvers: Vec<Address> = (0..17u8).map(Address::repeat_byte).collect();
    let prop = Contract::Proposal {
        callHash: B256::ZERO,
        callLen: 0,
        enactment: after(0),
        track: Contract::Track::Root,
        creator: Address::repeat_byte(0xFF),
        approvers: approvers.clone(),
        minApprovers: U256::from(approvers.len() as u64),
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
    };
    let bytes = codec::encode(&prop).unwrap();
    assert!(bytes.len() <= MAX_ENCODED_LEN);
    assert_eq!(bytes.len(), 67 + 20 * 17);
}

/// Worst case for the per-proposal lifecycle: every approver has also voted
/// (`approvers == approvedBy`, size-wise). At `MAX_APPROVERS = 8` the encoded
/// blob is `67 + 20·(8+8) = 387` bytes — under the 416 cap with 29 bytes of
/// headroom. One more approver and `propose` would create a proposal that
/// can't be fully approved without overflowing storage.
#[test]
fn max_approvers_with_full_approval_fits_storage_cap() {
    let approvers: Vec<Address> = (1..=MAX_APPROVERS as u8)
        .map(Address::repeat_byte)
        .collect();
    // Same list as `approvedBy` — every approver eventually approves.
    let approved_by = approvers.clone();

    let prop = Contract::Proposal {
        callHash: B256::repeat_byte(0xAA),
        callLen: u32::MAX,
        enactment: after(u32::MAX),
        track: Contract::Track::WhitelistedCaller,
        creator: Address::repeat_byte(0xFF),
        approvers,
        minApprovers: U256::from(MAX_APPROVERS as u64),
        approvedBy: approved_by,
        status: Contract::ProposalStatus::Submitted,
    };

    let bytes = codec::encode(&prop).unwrap();
    assert_eq!(bytes.len(), MIN_ENCODED_LEN + 20 * 2 * MAX_APPROVERS);
    assert!(
        bytes.len() <= MAX_ENCODED_LEN,
        "MAX_APPROVERS={MAX_APPROVERS} produced a {}-byte blob, exceeding the {MAX_ENCODED_LEN} \
         cap — the constant is wrong",
        bytes.len(),
    );
    // Round-trip too: the worst-case blob must be decodable.
    assert_eq!(codec::decode(&bytes).unwrap(), prop);
}

/// `MAX_APPROVERS + 1` with full approval would overflow; refuse to encode.
#[test]
fn one_more_than_max_approvers_overflows() {
    let n = MAX_APPROVERS + 1;
    let approvers: Vec<Address> = (1..=n as u8).map(Address::repeat_byte).collect();
    let approved_by = approvers.clone();

    let prop = Contract::Proposal {
        callHash: B256::ZERO,
        callLen: 0,
        enactment: after(0),
        track: Contract::Track::Root,
        creator: Address::repeat_byte(0xFF),
        approvers,
        minApprovers: U256::from(n as u64),
        approvedBy: approved_by,
        status: Contract::ProposalStatus::Submitted,
    };
    assert_eq!(codec::encode(&prop), Err(CodecError::TooLarge));
}

#[test]
fn rejects_oversized_proposal() {
    // 18 approvers + 0 votes = 427 bytes, over the 416 cap. Encode must refuse
    // rather than produce a blob `api::set_storage` will reject at runtime.
    let approvers: Vec<Address> = (0..18u8).map(Address::repeat_byte).collect();
    let prop = Contract::Proposal {
        callHash: B256::ZERO,
        callLen: 0,
        enactment: after(0),
        track: Contract::Track::Root,
        creator: Address::repeat_byte(0xFF),
        approvers,
        minApprovers: U256::from(1u64),
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
    };
    assert_eq!(codec::encode(&prop), Err(CodecError::TooLarge));
}

#[test]
fn rejects_min_approvers_overflowing_u8() {
    let mut prop = fixture();
    prop.minApprovers = U256::from(256u64);
    assert_eq!(codec::encode(&prop), Err(CodecError::LenOverflow));

    let mut prop = fixture();
    prop.minApprovers = U256::MAX;
    assert_eq!(codec::encode(&prop), Err(CodecError::LenOverflow));
}

#[test]
fn rejects_min_approvers_above_n() {
    // 2 approvers but minApprovers=3 — bogus, refuse to encode so the storage
    // doesn't end up holding a structurally undecidable proposal.
    let mut prop = fixture();
    prop.minApprovers = U256::from((prop.approvers.len() + 1) as u64);
    assert_eq!(codec::encode(&prop), Err(CodecError::MinApproversTooHigh));
}

#[test]
fn decode_rejects_truncated_input() {
    let bytes = codec::encode(&fixture()).unwrap();
    for cut in 0..bytes.len() {
        assert_eq!(
            codec::decode(&bytes[..cut]),
            Err(CodecError::TooShort),
            "decoding a {cut}-byte prefix should error",
        );
    }
}

#[test]
fn decode_rejects_trailing_bytes() {
    let mut bytes = codec::encode(&fixture()).unwrap();
    bytes.push(0x00);
    assert_eq!(codec::decode(&bytes), Err(CodecError::Trailing));
}

#[test]
fn decode_rejects_unknown_version() {
    let mut bytes = codec::encode(&fixture()).unwrap();
    bytes[0] = 0xFF;
    assert_eq!(codec::decode(&bytes), Err(CodecError::UnknownVersion(0xFF)));
}

#[test]
fn decode_rejects_bad_status() {
    let mut bytes = codec::encode(&fixture()).unwrap();
    // Last byte is `status`; 3..=254 are unused, 255 is alloy's __Invalid.
    let last = bytes.len() - 1;
    bytes[last] = 7;
    assert_eq!(codec::decode(&bytes), Err(CodecError::BadStatus(7)));
}

#[test]
fn decode_rejects_bad_enactment_kind() {
    let mut bytes = codec::encode(&fixture()).unwrap();
    // enactment.kind sits right after version + callHash + callLen.
    bytes[1 + 32 + 4] = 7;
    assert_eq!(codec::decode(&bytes), Err(CodecError::BadDispatchKind(7)));
}

#[test]
fn decode_rejects_bad_track() {
    let mut bytes = codec::encode(&fixture()).unwrap();
    // track sits after version + callHash + callLen + kind(1) + block(4).
    bytes[1 + 32 + 4 + 1 + 4] = 7;
    assert_eq!(codec::decode(&bytes), Err(CodecError::BadTrack(7)));
}

#[test]
fn decode_rejects_min_approvers_above_n() {
    // Synthesise a blob where min > N. The fixture has N=2, so a min byte
    // of 3 is invalid and decode must reject.
    let mut bytes = codec::encode(&fixture()).unwrap();
    // The min-byte sits right after `approvers` (which has 2 entries of 20 B):
    // 1 + 32 + 4 + (1 + 4) + 1 + 20 + 1 + 40 = 104. That's the min-byte offset.
    let min_offset = 1 + 32 + 4 + (1 + 4) + 1 + 20 + 1 + 20 * 2;
    bytes[min_offset] = 3;
    assert_eq!(codec::decode(&bytes), Err(CodecError::MinApproversTooHigh));
}

/// The identity encoder is *exactly* the leading section of the full encoder.
/// This is the property `proposal_key` relies on: changing only mutable fields
/// (`approvedBy`, `status`) must leave the identity bytes — and therefore the
/// hashed key — untouched.
#[test]
fn encode_identity_is_strict_prefix_of_encode() {
    for status in [
        Contract::ProposalStatus::Review,
        Contract::ProposalStatus::Submitted,
        Contract::ProposalStatus::Closed,
    ] {
        for approved_by in [vec![], vec![Address::repeat_byte(0x22)]] {
            let prop = Contract::Proposal {
                approvedBy: approved_by,
                status,
                ..fixture()
            };
            let full = codec::encode(&prop).unwrap();
            let identity = codec::encode_identity(&prop).unwrap();
            assert!(full.starts_with(&identity));
            assert_eq!(identity.len(), MIN_IDENTITY_LEN + 20 * prop.approvers.len());
        }
    }
}

/// Golden bytes for [`fixture`]. If this changes, every existing proposal on
/// chain becomes unreadable — bump [`VERSION`] and migrate intentionally.
#[test]
fn golden_vector() {
    let bytes = codec::encode(&fixture()).unwrap();
    let expected: [u8; 127] = [
        // version
        0x02, // callHash (32 × 0xAA)
        0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        0xAA, 0xAA, // callLen = 42 (LE u32)
        0x2A, 0x00, 0x00, 0x00, // enactment.kind = After (1)
        0x01, // enactment.block = 100 (LE u32)
        0x64, 0x00, 0x00, 0x00, // track = WhitelistedCaller (1)
        0x01, // creator (20 × 0x11)
        0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
        0x11, 0x11, 0x11, 0x11, 0x11, // approvers.len() = 2
        0x02, // approvers[0] (20 × 0x22)
        0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
        0x22, 0x22, 0x22, 0x22, 0x22, // approvers[1] (20 × 0x33)
        0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33, 0x33,
        0x33, 0x33, 0x33, 0x33, 0x33, // minApprovers = 2
        0x02, // approvedBy.len() = 1
        0x01, // approvedBy[0] (20 × 0x22)
        0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
        0x22, 0x22, 0x22, 0x22, 0x22, // status = Review
        0x00,
    ];
    assert_eq!(bytes.as_slice(), expected.as_slice());
}

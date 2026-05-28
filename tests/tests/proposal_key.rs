use contract::{Address, B256, Contract, ProposalError, U256, proposal_key};

fn base() -> Contract::Proposal {
    Contract::Proposal {
        callHash: B256::repeat_byte(0xAA),
        callLen: 42,
        enactment: Contract::DispatchTime {
            kind: Contract::DispatchTimeKind::After,
            block: 100,
        },
        track: Contract::Track::WhitelistedCaller,
        creator: Address::repeat_byte(0x11),
        approvers: vec![Address::repeat_byte(0x22), Address::repeat_byte(0x33)],
        minApprovers: U256::from(2u64),
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
    }
}

#[test]
fn proposal_key_is_deterministic() {
    assert_eq!(proposal_key(&base()), proposal_key(&base()));
}

#[test]
fn proposal_key_changes_when_any_field_changes() {
    let base_key = proposal_key(&base()).unwrap();

    let mut p = base();
    p.callHash = B256::repeat_byte(0xBB);
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with callHash"
    );

    let mut p = base();
    p.creator = Address::repeat_byte(0x99);
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with creator"
    );

    let mut p = base();
    p.approvers = vec![Address::repeat_byte(0x22), Address::repeat_byte(0x44)];
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change when an approver changes",
    );

    let mut p = base();
    p.approvers.push(Address::repeat_byte(0x55));
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change when approvers length changes",
    );

    let mut p = base();
    p.minApprovers = U256::from(1u64);
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with minApprovers"
    );

    let mut p = base();
    p.callLen += 1;
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with callLen"
    );

    let mut p = base();
    p.enactment.block += 1;
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with enactment.block"
    );

    let mut p = base();
    p.enactment.kind = Contract::DispatchTimeKind::At;
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with enactment.kind"
    );

    let mut p = base();
    p.track = Contract::Track::Root;
    assert_ne!(
        proposal_key(&p).unwrap(),
        base_key,
        "key must change with track"
    );
}

#[test]
fn proposal_key_ignores_approved_by() {
    let base_key = proposal_key(&base()).unwrap();

    // A single recorded approval must not move the key.
    let mut p = base();
    p.approvedBy = vec![Address::repeat_byte(0x44)];
    assert_eq!(
        proposal_key(&p).unwrap(),
        base_key,
        "approvedBy must not influence the key",
    );

    // Neither must many, in any order — `approvedBy` is excluded entirely, so
    // it isn't subject to the sorted/unique rules `approvers` is.
    let mut p = base();
    p.approvedBy = vec![
        Address::repeat_byte(0x99),
        Address::repeat_byte(0x11),
        Address::repeat_byte(0x99),
    ];
    assert_eq!(
        proposal_key(&p).unwrap(),
        base_key,
        "approvedBy contents must not influence the key",
    );
}

#[test]
fn proposal_key_ignores_status() {
    // The key is the proposal's identity, and `finalize`/`close` recompute it via
    // `set_proposal` *after* changing `status`. If `status` fed into the key, those
    // writes would land under a different key and orphan the stored proposal — so
    // the lifecycle status must not influence the key.
    let base_key = proposal_key(&base()).unwrap();

    for status in [
        Contract::ProposalStatus::Submitted,
        Contract::ProposalStatus::Closed,
    ] {
        let mut p = base();
        p.status = status;
        assert_eq!(
            proposal_key(&p).unwrap(),
            base_key,
            "status must not influence the key",
        );
    }
}

#[test]
fn proposal_key_errors_on_unsorted_approvers() {
    let mut p = base();
    p.approvers.reverse();
    assert_eq!(
        proposal_key(&p),
        Err(ProposalError::ApproversNotStrictlySorted)
    );
}

#[test]
fn proposal_key_errors_on_duplicate_approvers() {
    let mut p = base();
    let dup = Address::repeat_byte(0x22);
    p.approvers = vec![dup, dup];
    assert_eq!(
        proposal_key(&p),
        Err(ProposalError::ApproversNotStrictlySorted)
    );
}

#[test]
fn proposal_key_accepts_empty_approvers() {
    let mut p = base();
    p.approvers = vec![];
    p.minApprovers = U256::ZERO;
    let empty_key = proposal_key(&p).unwrap();
    assert_ne!(empty_key, proposal_key(&base()).unwrap());
}

#[test]
fn proposal_key_accepts_single_approver() {
    let mut p = base();
    p.approvers = vec![Address::repeat_byte(0x22)];
    p.minApprovers = U256::from(1u64);
    let single_key = proposal_key(&p).unwrap();
    assert_ne!(single_key, proposal_key(&base()).unwrap());
}

#[test]
fn proposal_key_handles_zero_values() {
    let p = Contract::Proposal {
        callHash: B256::ZERO,
        callLen: 0,
        enactment: Contract::DispatchTime {
            kind: Contract::DispatchTimeKind::At,
            block: 0,
        },
        track: Contract::Track::Root,
        creator: Address::ZERO,
        approvers: vec![],
        minApprovers: U256::ZERO,
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
    };
    proposal_key(&p).unwrap();
}

#[test]
fn proposal_key_handles_max_values() {
    let p = Contract::Proposal {
        callHash: B256::repeat_byte(0xFF),
        callLen: u32::MAX,
        enactment: Contract::DispatchTime {
            kind: Contract::DispatchTimeKind::After,
            block: u32::MAX,
        },
        track: Contract::Track::WhitelistedCaller,
        creator: Address::ZERO,
        approvers: vec![Address::repeat_byte(0xFF)],
        minApprovers: U256::from(1u64),
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
    };
    proposal_key(&p).unwrap();
}

#[test]
fn proposal_key_errors_on_min_approvers_too_high() {
    let mut p = base();
    p.minApprovers = U256::from(p.approvers.len() as u64 + 1);
    assert_eq!(proposal_key(&p), Err(ProposalError::MinApproversTooHigh));

    let mut p = base();
    p.minApprovers = U256::MAX;
    assert_eq!(proposal_key(&p), Err(ProposalError::MinApproversTooHigh));
}

#[test]
fn proposal_key_accepts_unanimous_min_approvers() {
    let mut p = base();
    p.minApprovers = U256::from(p.approvers.len() as u64);
    proposal_key(&p).unwrap();
}

#[test]
fn proposal_key_errors_on_creator_as_approver() {
    let mut p = base();
    p.creator = p.approvers[0];
    assert_eq!(proposal_key(&p), Err(ProposalError::CreatorIsApprover));
}

/// Golden hash: pins the on-chain key derivation. If this changes, every
/// already-stored proposal becomes unreachable — update only with intent.
#[test]
fn proposal_key_golden_hash() {
    let key = proposal_key(&base()).unwrap();
    // keccak256(b"Proposal:" || codec::encode_identity(base())). If this
    // changes, every already-stored proposal becomes unreachable — update only
    // with intent (e.g. bumping `codec::VERSION`).
    let expected: [u8; 32] = [
        0x22, 0x05, 0x87, 0x40, 0x84, 0xb7, 0xd4, 0x37, 0xa3, 0x69, 0x36, 0x26, 0x92, 0x57, 0x0f,
        0x45, 0x91, 0x88, 0x6f, 0x25, 0x5c, 0x23, 0xa8, 0x51, 0x0d, 0x85, 0x5e, 0x68, 0x38, 0x17,
        0xac, 0x23,
    ];
    assert_eq!(key, expected, "got: {:02x?}", key);
}

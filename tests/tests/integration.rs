use contract::{B256, Contract, sol_types::{SolCall, SolError, SolEvent, SolValue}};
use contract_tests::{Test, fund, new_test_ext, RuntimeEvent, RuntimeOrigin, System};
use pallet_revive::{
    Code, H160, TransactionLimits, Weight,
    test_utils::{
        ALICE, ALICE_ADDR,
        BOB, BOB_ADDR,
        CHARLIE,
        builder::{BareCallBuilder, BareInstantiateBuilder},
    },
};
use pallet_revive_uapi::ReturnFlags;
use contract::{Address, proposal_key};

/// Built by `cd ../contract && cargo build` (build.rs runs PvmBuilder).
/// Lands in the shared `target/` thanks to `.cargo/config.toml`.
const BLOB: &[u8] = include_bytes!("../../target/contract.release.polkavm");

/// Generous limits so we don't trip the deposit cap on a 170KB blob.
fn limits() -> TransactionLimits<Test> {
    TransactionLimits::WeightAndDeposit {
        weight_limit: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
        deposit_limit: u64::MAX / 2,
    }
}

#[test]
fn blob_size_is_sane() {
    let size = BLOB.len();
    if size > 100_000 {
        panic!("blob size is too large: {}", size);
    }
    eprintln!("blob size is sane: {}", size);
}

/// Fund ALICE, deploy the contract, and submit one proposal.
///
/// Returns the contract address and the proposal that was created.
fn setup_with_proposal() -> (H160, Contract::Proposal) {
    fund(&ALICE, u64::MAX / 2);
    let expected_proposal = Contract::Proposal {
        callHash: B256::repeat_byte(0xAA),
        creator: Address::from(ALICE_ADDR.0),
        approvers: vec![Address::from(BOB_ADDR.0)],
        minApprovers: contract::U256::from(1u64),
        approvedBy: vec![],
    };

    let contract = BareInstantiateBuilder::<Test>::bare_instantiate(
        RuntimeOrigin::signed(ALICE),
        Code::Upload(BLOB.to_vec()),
    )
    .transaction_limits(limits())
    .build_and_unwrap_contract();

    // propose
    let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), contract.addr)
        .data(Contract::proposeCall {
            callHash: B256::repeat_byte(0xAA),
            approvers: vec![Address::from(BOB_ADDR.0)],
            minApprovers: contract::U256::from(1u64),
        }.abi_encode())
        .transaction_limits(limits())
        .build_and_unwrap_result();

    (contract.addr, expected_proposal)
}

#[test]
fn get_propose_works() {
    new_test_ext().execute_with(|| {
        let (addr, expected_proposal) = setup_with_proposal();

        // fetch specific proposal
        let proposal_key = proposal_key(&expected_proposal).unwrap();
        let proposal = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::proposalCall {
                proposalHash: proposal_key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        let proposal = <Contract::Proposal>::abi_decode_validate(&proposal.data).unwrap();
        assert_eq!(proposal, expected_proposal);
    });
}

#[test]
fn get_all_proposal_works() {
    new_test_ext().execute_with(|| {
        let (addr, expected_proposal) = setup_with_proposal();

        // allProposals() → [expected_proposal]
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::allProposalsCall {}.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        let proposals = <Vec<Contract::Proposal>>::abi_decode_validate(&result.data).unwrap();
        assert_eq!(proposals, vec![expected_proposal]);
    });
}

#[test]
fn proposing_twice_errors() {
    new_test_ext().execute_with(|| {
        let (addr, _expected_proposal) = setup_with_proposal();

        // propose again should revert
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::proposeCall {
                callHash: B256::repeat_byte(0xAA),
                approvers: vec![Address::from(BOB_ADDR.0)],
                minApprovers: contract::U256::from(1u64),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());
    });
}

#[test]
fn propose_emits_event() {
    new_test_ext().execute_with(|| {
        let (addr, expected_proposal) = setup_with_proposal();

        // Find the `ContractEmitted` event our contract produced while proposing.
        let (topics, data) = System::events()
            .into_iter()
            .find_map(|record| match record.event {
                RuntimeEvent::Revive(pallet_revive::Event::ContractEmitted {
                    contract,
                    topics,
                    data,
                }) if contract == addr => Some((topics, data)),
                _ => None,
            })
            .expect("propose should emit a ContractEmitted event");

        // topic[0] is the event signature; topics[1..] are the indexed params.
        assert_eq!(topics.len(), 4);
        assert_eq!(topics[0].0, Contract::Proposed::SIGNATURE_HASH.0);
        assert_eq!(topics[1].0, expected_proposal.callHash.0);
        assert_eq!(topics[2].0, expected_proposal.creator.into_word().0);

        // The only non-indexed field, `minApprovers`, lands in the data section.
        assert_eq!(data, expected_proposal.minApprovers.to_be_bytes::<32>());
    });
}

#[test]
fn approve_records_approval() {
    new_test_ext().execute_with(|| {
        fund(&BOB, u64::MAX / 2);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // BOB is an authorized approver and approves the proposal.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        assert_eq!(result.flags, ReturnFlags::empty());

        // Approving records the caller in `approvedBy` and leaves `approvers`
        // (and therefore the key) untouched — so the proposal is still found
        // under its original key.
        let proposal = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::proposalCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        let proposal = <Contract::Proposal>::abi_decode_validate(&proposal.data).unwrap();
        assert_eq!(proposal.approvers, expected_proposal.approvers);
        assert_eq!(proposal.approvedBy, vec![Address::from(BOB_ADDR.0)]);
    });
}

#[test]
fn approve_emits_event() {
    new_test_ext().execute_with(|| {
        fund(&BOB, u64::MAX / 2);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // Take the *last* `ContractEmitted` event — propose emits one too, so
        // the approve event is the most recent.
        let (topics, _data) = System::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                RuntimeEvent::Revive(pallet_revive::Event::ContractEmitted {
                    contract,
                    topics,
                    data,
                }) if contract == addr => Some((topics, data)),
                _ => None,
            })
            .expect("approve should emit a ContractEmitted event");

        // topic[0] is the event signature; topic[1] is the indexed proposalHash.
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].0, Contract::Approved::SIGNATURE_HASH.0);
        assert_eq!(topics[1].0, key);
    });
}

#[test]
fn approve_nonexistent_proposal_reverts() {
    new_test_ext().execute_with(|| {
        fund(&CHARLIE, u64::MAX / 2);
        let (addr, _expected_proposal) = setup_with_proposal();

        // A hash that doesn't map to any stored proposal.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(CHARLIE), addr)
            .data(Contract::approveCall {
                proposalHash: B256::repeat_byte(0xFF),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());
    });
}

#[test]
fn approve_by_non_approver_reverts() {
    new_test_ext().execute_with(|| {
        fund(&CHARLIE, u64::MAX / 2);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // CHARLIE isn't in `approvers` (only BOB is), so approving reverts
        // with `NotAnApprover`.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(CHARLIE), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        // And nothing was recorded against the proposal.
        let proposal = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::proposalCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        let proposal = <Contract::Proposal>::abi_decode_validate(&proposal.data).unwrap();
        assert!(proposal.approvedBy.is_empty());
    });
}

#[test]
fn approving_twice_by_same_account_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, u64::MAX / 2);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // First approval from BOB (an authorized approver) succeeds.
        let first = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        assert_eq!(first.flags, ReturnFlags::empty());

        // BOB is now in `approvedBy`, so a second approval reverts with
        // `AlreadyApproved`.
        let second = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(second.flags, ReturnFlags::REVERT);
        assert!(second.data.is_empty());
    });
}

#[test]
fn finalize_after_threshold_succeeds() {
    new_test_ext().execute_with(|| {
        fund(&BOB, u64::MAX / 2);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // BOB approves, meeting `minApprovers: 1`.
        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // With the threshold met, finalize succeeds.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::empty());
    });
}

#[test]
fn finalize_emits_event() {
    new_test_ext().execute_with(|| {
        fund(&BOB, u64::MAX / 2);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // Take the *last* `ContractEmitted` event — propose and approve emit
        // their own, so the finalize event is the most recent.
        let (topics, _data) = System::events()
            .into_iter()
            .rev()
            .find_map(|record| match record.event {
                RuntimeEvent::Revive(pallet_revive::Event::ContractEmitted {
                    contract,
                    topics,
                    data,
                }) if contract == addr => Some((topics, data)),
                _ => None,
            })
            .expect("finalize should emit a ContractEmitted event");

        // topic[0] is the event signature; topic[1] is the indexed proposalHash,
        // topic[2] the indexed callHash.
        assert_eq!(topics.len(), 3);
        assert_eq!(topics[0].0, Contract::Finalized::SIGNATURE_HASH.0);
        assert_eq!(topics[1].0, key);
        assert_eq!(topics[2].0, expected_proposal.callHash.0);
    });
}

#[test]
fn finalize_below_threshold_reverts() {
    new_test_ext().execute_with(|| {
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // No approvals yet, so finalize reverts with the `NotApproved` error.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert_eq!(
            result.data,
            <Contract::NotApproved as SolError>::abi_encode(&Contract::NotApproved {}),
        );
    });
}

#[test]
fn finalize_nonexistent_proposal_reverts() {
    new_test_ext().execute_with(|| {
        let (addr, _expected_proposal) = setup_with_proposal();

        // A hash that doesn't map to any stored proposal.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: B256::repeat_byte(0xFF),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());
    });
}

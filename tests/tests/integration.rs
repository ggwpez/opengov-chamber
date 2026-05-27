use contract::{B256, Contract, sol_types::{SolCall, SolError, SolEvent, SolValue}};
use contract_tests::{Balances, ENDOWMENT, RuntimeEvent, RuntimeOrigin, System, Test, fund, new_test_ext};
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
use contract::{Address, U256, proposal_key, xcm::referendum::{SUBMISSION_DEPOSIT, NATIVE_TO_ETH_RATIO}};

/// Built by `cd ../contract && cargo build` (build.rs runs PvmBuilder).
/// Lands in the shared `target/` thanks to `.cargo/config.toml`.
const BLOB: &[u8] = include_bytes!("../../target/contract.release.polkavm");

/// Generous limits so we don't trip the deposit cap on a 170KB blob.
fn limits() -> TransactionLimits<Test> {
    TransactionLimits::WeightAndDeposit {
        weight_limit: Weight::from_parts(u64::MAX / 2, u64::MAX / 2),
        deposit_limit: u128::MAX / 2,
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
    fund(&ALICE, ENDOWMENT);
    let expected_proposal = Contract::Proposal {
        callHash: B256::repeat_byte(0xAA),
        callLen: 42,
        enactmentDelay: 100,
        creator: Address::from(ALICE_ADDR.0),
        approvers: vec![Address::from(BOB_ADDR.0)],
        minApprovers: contract::U256::from(1u64),
        approvedBy: vec![],
        status: Contract::ProposalStatus::Review,
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
            callLen: 42,
            enactmentDelay: 100,
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
                callLen: 42,
                enactmentDelay: 100,
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
        fund(&BOB, ENDOWMENT);
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
        fund(&BOB, ENDOWMENT);
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
        fund(&CHARLIE, ENDOWMENT);
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
        fund(&CHARLIE, ENDOWMENT);
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
        fund(&BOB, ENDOWMENT);
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
        fund(&BOB, ENDOWMENT);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // BOB approves, meeting `minApprovers: 1`.
        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // With the threshold met, finalize succeeds — sending the SubmissionDeposit
        // as value so the contract can cover the referendum deposit.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .native_value(SUBMISSION_DEPOSIT)
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::empty());
    });
}

/// `finalize()` should not merely *not error* — it should actually submit a
/// referendum into `pallet_referenda` referencing our preimage by `hash`/`len`.
///
/// Two non-obvious facts this test pins down (both found by reading raw runtime
/// storage, which we can do because the harness runs the *real* runtime):
///
/// 1. The XCM `Transact` dispatches `Referenda::submit` as the contract's own
///    sovereign account, which must hold the `SubmissionDeposit`. `finalize()`
///    collects it from the caller as call value (see `finalize_without_deposit_reverts`
///    for the no-value path); pallet-revive credits it to the contract before the
///    submit, so the deposit is covered. If it weren't, `submit` would fail with
///    `Balances::InsufficientBalance`, XCM `Transact` would *swallow* that error
///    (the XCM still reports `Complete`), and `finalize()` would return success
///    while creating nothing.
///
/// 2. The preimage itself is *not* present afterwards: the contract only
///    references it by hash (`Bounded::Lookup`) and never calls
///    `Preimage::note_preimage`. Referenda permits submitting against an un-noted
///    hash, so we assert the *reference* is correct rather than preimage presence.
#[test]
fn finalize_submits_referendum() {
    use frame_support::traits::{Bounded, fungible::Inspect};
    use sp_core::H256;

    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // Nothing submitted yet.
        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 0);

        let alice_before = Balances::balance(&ALICE);

        // Send the SubmissionDeposit as value; the contract then covers the deposit
        // when the XCM submit dispatches as its sovereign account (see fact #1).
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .native_value(SUBMISSION_DEPOSIT)
            .transaction_limits(limits())
            .build_and_unwrap_result();
        assert_eq!(result.flags, ReturnFlags::empty());

        // The deposit really left the caller (this is the contrast that makes
        // `finalize_without_deposit_reverts`'s "balance unchanged" meaningful): it
        // funded the deposit now held against the contract's sovereign account.
        // The caller is debited the SubmissionDeposit *plus* a small storage deposit
        // for recording their refundable tally, so we assert "at least".
        assert!(
            Balances::balance(&ALICE) <= alice_before - SUBMISSION_DEPOSIT,
            "successful finalize must debit the caller by at least the SubmissionDeposit",
        );

        // Exactly one referendum now exists, and its proposal is the preimage
        // lookup the contract built — matching `callHash` and the `callLen` we
        // threaded through (no longer the old `len: 0` placeholder).
        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 1);
        let info = pallet_referenda::ReferendumInfoFor::<Test>::get(0)
            .expect("referendum 0 should have been submitted");
        match info {
            pallet_referenda::ReferendumInfo::Ongoing(status) => {
                assert_eq!(
                    status.proposal,
                    Bounded::Lookup {
                        hash: H256::from(expected_proposal.callHash.0),
                        len: expected_proposal.callLen,
                    },
                    "submitted referendum must reference our preimage hash/len",
                );
            }
            other => panic!("expected an ongoing referendum, got {:?}", other),
        }

        // The preimage itself was never noted — only referenced (see fact #2).
        assert!(
            !pallet_preimage::PreimageFor::<Test>::contains_key((
                H256::from(expected_proposal.callHash.0),
                expected_proposal.callLen,
            )),
            "contract must not have noted the preimage, only referenced it",
        );
    });
}

/// Finalizing an approved proposal with less than the `SubmissionDeposit` reverts
/// with `InsufficientDeposit`, submits no referendum, and — crucially — the value
/// the caller attached is rolled back to them (pallet-revive moves value at frame
/// entry and unwinds it on `REVERT`). The companion `finalize_submits_referendum`
/// asserts the contrast: a *successful* finalize actually debits the deposit, so
/// this "balance unchanged" assertion isn't vacuously true.
#[test]
fn finalize_without_deposit_reverts() {
    use frame_support::traits::fungible::Inspect;
    use pallet_revive::AddressMapper;

    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        let contract_acct = <Test as pallet_revive::Config>::AddressMapper::to_account_id(&addr);
        let alice_before = Balances::balance(&ALICE);
        let contract_before = Balances::balance(&contract_acct);

        // One planck under the deposit, so finalize reverts at the deposit check.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .native_value(SUBMISSION_DEPOSIT - 1)
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert_eq!(
            result.data,
            <Contract::InsufficientDeposit as SolError>::abi_encode(
                &Contract::InsufficientDeposit {}
            ),
        );
        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 0);

        // The attached value was rolled back: caller made whole, nothing stuck
        // to the contract.
        assert_eq!(
            Balances::balance(&ALICE),
            alice_before,
            "reverted finalize must refund the caller's attached value",
        );
        assert_eq!(
            Balances::balance(&contract_acct),
            contract_before,
            "no value should remain with the contract after a revert",
        );
    });
}

#[test]
fn finalize_emits_event() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        System::reset_events();
        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .native_value(SUBMISSION_DEPOSIT)
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
fn finalize_by_non_owner_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected_proposal) = setup_with_proposal();
        let key = proposal_key(&expected_proposal).unwrap();

        // BOB approves, meeting `minApprovers: 1`, so the threshold check passes
        // and finalize reaches the owner check.
        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall {
                proposalHash: key.into(),
            }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // BOB is an approver but not the creator (ALICE is), so finalize reverts
        // with `NotOwner`.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::finalizeCall {
                proposalHash: key.into(),
            }.abi_encode())
            .native_value(SUBMISSION_DEPOSIT)
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        // Nothing was submitted.
        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 0);
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

/// Read a depositor's recorded tally via the `deposits(address)` view.
fn deposit_of(addr: H160, depositor: Address) -> U256 {
    let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
        .data(Contract::depositsCall { depositor }.abi_encode())
        .transaction_limits(limits())
        .build_and_unwrap_result();
    assert_eq!(result.flags, ReturnFlags::empty());
    <U256 as SolValue>::abi_decode_validate(&result.data).unwrap()
}

/// Approve (BOB) and finalize (ALICE) the proposal, attaching `SUBMISSION_DEPOSIT`
/// as value. Returns the contract address.
fn approved_and_finalized() -> H160 {
    fund(&BOB, ENDOWMENT);
    let (addr, expected_proposal) = setup_with_proposal();
    let key = proposal_key(&expected_proposal).unwrap();

    let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
        .data(Contract::approveCall { proposalHash: key.into() }.abi_encode())
        .transaction_limits(limits())
        .build_and_unwrap_result();

    let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
        .data(Contract::finalizeCall { proposalHash: key.into() }.abi_encode())
        .native_value(SUBMISSION_DEPOSIT)
        .transaction_limits(limits())
        .build_and_unwrap_result();
    assert_eq!(result.flags, ReturnFlags::empty());

    addr
}

// ----------------------------------------------------------- status lifecycle
//
// A proposal moves through `ProposalStatus`:
//
//   Review ──finalize──▶ Submitted   (terminal)
//      │
//      └────close──────▶ Closed      (terminal)
//
// Both target states are terminal: a Submitted proposal can't be closed, and a
// Closed proposal can't be finalized (or closed again). The tests below pin down
// each edge and each rejected transition.

/// Fetch a proposal by key via the `proposal(bytes32)` view.
fn fetch_proposal(addr: H160, key: [u8; 32]) -> Contract::Proposal {
    let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
        .data(Contract::proposalCall { proposalHash: key.into() }.abi_encode())
        .transaction_limits(limits())
        .build_and_unwrap_result();
    assert_eq!(result.flags, ReturnFlags::empty());
    <Contract::Proposal>::abi_decode_validate(&result.data).unwrap()
}

/// BOB (the sole approver) approves the proposal, meeting `minApprovers: 1`.
fn approve_by_bob(addr: H160, key: [u8; 32]) {
    let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
        .data(Contract::approveCall { proposalHash: key.into() }.abi_encode())
        .transaction_limits(limits())
        .build_and_unwrap_result();
    assert_eq!(result.flags, ReturnFlags::empty());
}

/// Call `finalize`, attaching the SubmissionDeposit as value.
fn finalize_as(
    signer: sp_runtime::AccountId32,
    addr: H160,
    key: [u8; 32],
) -> pallet_revive::ExecReturnValue {
    BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(signer), addr)
        .data(Contract::finalizeCall { proposalHash: key.into() }.abi_encode())
        .native_value(SUBMISSION_DEPOSIT)
        .transaction_limits(limits())
        .build_and_unwrap_result()
}

/// Call `close`.
fn close_as(
    signer: sp_runtime::AccountId32,
    addr: H160,
    key: [u8; 32],
) -> pallet_revive::ExecReturnValue {
    BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(signer), addr)
        .data(Contract::closeCall { proposalHash: key.into() }.abi_encode())
        .transaction_limits(limits())
        .build_and_unwrap_result()
}

/// A freshly proposed proposal starts in `Review`.
#[test]
fn propose_starts_in_review() {
    new_test_ext().execute_with(|| {
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Review);
    });
}

/// `Review -> Submitted`: a successful `finalize` advances the stored status to
/// `Submitted`, and the proposal remains queryable.
#[test]
fn finalize_marks_submitted() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        approve_by_bob(addr, key);
        assert_eq!(finalize_as(ALICE, addr, key).flags, ReturnFlags::empty());

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Submitted);
    });
}

/// `Review -> Closed`: `close` advances the stored status to `Closed`. The
/// proposal is *retained* (not deleted) so the closed status stays observable,
/// both via the single-proposal view and `allProposals`.
#[test]
fn close_marks_closed() {
    new_test_ext().execute_with(|| {
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        assert_eq!(close_as(ALICE, addr, key).flags, ReturnFlags::empty());

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Closed);

        // Still listed by `allProposals`, now flagged as closed.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::allProposalsCall {}.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        let all = <Vec<Contract::Proposal>>::abi_decode_validate(&result.data).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, Contract::ProposalStatus::Closed);
    });
}

/// `close` emits a `Closed` event for the proposal.
#[test]
fn close_emits_event() {
    new_test_ext().execute_with(|| {
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        System::reset_events();
        assert_eq!(close_as(ALICE, addr, key).flags, ReturnFlags::empty());

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
            .expect("close should emit a ContractEmitted event");

        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].0, Contract::Closed::SIGNATURE_HASH.0);
        assert_eq!(topics[1].0, key);
    });
}

/// Closing is only the creator's to do: a non-creator's `close` reverts and the
/// status is left in `Review`.
#[test]
fn close_by_non_owner_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        // BOB is an approver but not the creator (ALICE is).
        let result = close_as(BOB, addr, key);
        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Review);
    });
}

/// `close` on a hash that maps to no stored proposal reverts.
#[test]
fn close_nonexistent_proposal_reverts() {
    new_test_ext().execute_with(|| {
        let (addr, _expected) = setup_with_proposal();

        let result = close_as(ALICE, addr, B256::repeat_byte(0xFF).0);
        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());
    });
}

/// `Closed` is terminal: closing an already-closed proposal reverts and leaves
/// the status `Closed`.
#[test]
fn close_twice_reverts() {
    new_test_ext().execute_with(|| {
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        assert_eq!(close_as(ALICE, addr, key).flags, ReturnFlags::empty());

        let second = close_as(ALICE, addr, key);
        assert_eq!(second.flags, ReturnFlags::REVERT);
        assert!(second.data.is_empty());

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Closed);
    });
}

/// `Closed` is terminal w.r.t. finalize too: once closed, an otherwise-finalizable
/// proposal (threshold already met) can't be finalized — it reverts and submits
/// no referendum. This isolates the *status* guard from the approval-threshold
/// guard by approving before closing.
#[test]
fn finalize_after_close_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        approve_by_bob(addr, key);
        assert_eq!(close_as(ALICE, addr, key).flags, ReturnFlags::empty());

        // Threshold is met, so this revert is the status guard, not `NotApproved`.
        let result = finalize_as(ALICE, addr, key);
        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 0);
        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Closed);
    });
}

/// `Submitted` is terminal: a finalized proposal can't be closed. The close
/// reverts and the status stays `Submitted`.
#[test]
fn close_after_finalize_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        approve_by_bob(addr, key);
        assert_eq!(finalize_as(ALICE, addr, key).flags, ReturnFlags::empty());

        let result = close_as(ALICE, addr, key);
        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Submitted);
    });
}

/// Approvals are only valid in `Review`: once a proposal is closed, an approver
/// can no longer approve it. The revert is the status guard (BOB is a valid,
/// not-yet-recorded approver), and `approvedBy` stays empty.
#[test]
fn approve_after_close_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        assert_eq!(close_as(ALICE, addr, key).flags, ReturnFlags::empty());

        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall { proposalHash: key.into() }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        assert!(fetch_proposal(addr, key).approvedBy.is_empty());
    });
}

/// Likewise, an approver can't pile on more approvals after the proposal has been
/// finalized (Submitted). The pre-finalize approval from BOB remains, but a fresh
/// approval attempt reverts.
#[test]
fn approve_after_finalize_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        approve_by_bob(addr, key);
        assert_eq!(finalize_as(ALICE, addr, key).flags, ReturnFlags::empty());

        // The status guard runs before the approver/AlreadyApproved checks, so a
        // further approval by BOB (a valid approver) reverts on status alone.
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(BOB), addr)
            .data(Contract::approveCall { proposalHash: key.into() }.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        assert_eq!(result.flags, ReturnFlags::REVERT);
        assert!(result.data.is_empty());

        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Submitted);
    });
}

/// `Submitted` is terminal w.r.t. finalize too: finalizing twice reverts on the
/// second call and doesn't submit a second referendum.
#[test]
fn finalize_twice_reverts() {
    new_test_ext().execute_with(|| {
        fund(&BOB, ENDOWMENT);
        let (addr, expected) = setup_with_proposal();
        let key = proposal_key(&expected).unwrap();

        approve_by_bob(addr, key);
        assert_eq!(finalize_as(ALICE, addr, key).flags, ReturnFlags::empty());
        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 1);

        let second = finalize_as(ALICE, addr, key);
        assert_eq!(second.flags, ReturnFlags::REVERT);
        assert!(second.data.is_empty());

        // No second referendum, and the status is unchanged.
        assert_eq!(pallet_referenda::ReferendumCount::<Test>::get(), 1);
        assert_eq!(fetch_proposal(addr, key).status, Contract::ProposalStatus::Submitted);
    });
}

/// A successful `finalize` tallies the attached value against the caller. The
/// tally is EVM-denominated, so it's `SUBMISSION_DEPOSIT * NATIVE_TO_ETH_RATIO`.
#[test]
fn finalize_records_deposit() {
    new_test_ext().execute_with(|| {
        let addr = approved_and_finalized();

        let expected = U256::from(SUBMISSION_DEPOSIT) * U256::from(NATIVE_TO_ETH_RATIO);
        assert_eq!(deposit_of(addr, Address::from(ALICE_ADDR.0)), expected);
    });
}

/// When the contract *does* hold the funds, `refund()` pays the caller back their
/// whole tally and zeroes it. We top the contract up directly (a real `finalize`
/// immediately spends the deposit into the referendum — see
/// `refund_reverts_and_preserves_tally_when_contract_is_short`).
#[test]
fn refund_pays_caller_and_zeroes_tally() {
    use frame_support::traits::fungible::Inspect;
    use pallet_revive::AddressMapper;

    new_test_ext().execute_with(|| {
        let addr = approved_and_finalized();
        let contract_acct = <Test as pallet_revive::Config>::AddressMapper::to_account_id(&addr);

        // Give the contract enough free balance to honour the refund.
        fund(&contract_acct, ENDOWMENT);

        let alice_before = Balances::balance(&ALICE);
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::refundCall {}.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();
        assert_eq!(result.flags, ReturnFlags::empty());

        // The full SubmissionDeposit (native) came back to ALICE, plus the storage
        // deposit reclaimed by clearing her now-empty tally entry — so "at least".
        assert!(
            Balances::balance(&ALICE) >= alice_before + SUBMISSION_DEPOSIT,
            "refund must return at least the recorded deposit to the caller",
        );
        // ...and the tally is now zero (the entry was cleared, not just zeroed).
        assert_eq!(deposit_of(addr, Address::from(ALICE_ADDR.0)), U256::ZERO);
    });
}

/// The safety property: if the contract can't cover the refund, the call reverts,
/// no funds move, and the caller's tally is preserved (not zeroed). Here the
/// contract is short precisely because `finalize` already spent the deposit into
/// the referendum's reserve — so without an external top-up there's nothing to
/// pay the refund with.
#[test]
fn refund_reverts_and_preserves_tally_when_contract_is_short() {
    use frame_support::traits::fungible::Inspect;
    use pallet_revive::AddressMapper;

    new_test_ext().execute_with(|| {
        let addr = approved_and_finalized();
        let contract_acct = <Test as pallet_revive::Config>::AddressMapper::to_account_id(&addr);

        let tally_before = deposit_of(addr, Address::from(ALICE_ADDR.0));
        assert!(tally_before > U256::ZERO, "precondition: ALICE has a recorded deposit");

        let alice_before = Balances::balance(&ALICE);
        let contract_free_before = Balances::balance(&contract_acct);
        // The deposit was reserved by the referendum, so the contract's *free*
        // balance can't cover paying it back.
        assert!(
            contract_free_before < SUBMISSION_DEPOSIT,
            "precondition: contract lacks the free funds to refund (got {contract_free_before})",
        );

        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), addr)
            .data(Contract::refundCall {}.abi_encode())
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // The transfer failed, so the contract reverted the whole call.
        assert_eq!(result.flags, ReturnFlags::REVERT);

        // No funds moved...
        assert_eq!(Balances::balance(&ALICE), alice_before, "caller balance must be unchanged");
        assert_eq!(
            Balances::balance(&contract_acct),
            contract_free_before,
            "contract balance must be unchanged",
        );
        // ...and crucially the tally was NOT zeroed — the user keeps what they're owed.
        assert_eq!(
            deposit_of(addr, Address::from(ALICE_ADDR.0)),
            tally_before,
            "a failed refund must restore the tally, never lose the user's deposit",
        );
    });
}

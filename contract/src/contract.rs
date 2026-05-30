#![no_main]
#![no_std]

pub mod plumbing;

use alloy_core::{
    primitives::{Address, FixedBytes, U256, keccak256},
    sol_types::{EventTopic, SolCall, SolError, SolEvent, SolValue, sol_data},
};
use contract::{
    Contract, ProposalError, codec, expect_review, mark_closed, mark_submitted, proposal_key, xcm,
};
use pallet_revive_uapi::{CallFlags, HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

extern crate alloc;
use alloc::{vec, vec::Vec};

/// Upper bound on any single storage value, matching pallet-revive's
/// `STORAGE_BYTES` limit. Each proposal is stored as one value, so this bounds
/// the size of a single proposal (hence approvers-per-proposal) — *not* the
/// number of proposals, which the linked list below grows without bound.
const MAX_STORAGE_BYTES: usize = codec::MAX_ENCODED_LEN;

/// Head of the intrusive singly-linked list threading every proposal key (see
/// [`add_proposal_key`]): a storage slot holding the most-recently-added key,
/// or all-zeroes when there are no proposals.
const PROPOSALS_HEAD_KEY: &[u8] = b"proposals_head";

/// Domain tag for a key's "next" link slot, `keccak(tag || key)`. Distinct from
/// the `b"Proposal:"` identity domain so link slots never alias proposal slots.
const PROPOSAL_NEXT_DOMAIN: &[u8] = b"proposal_next:";

/// pallet-revive's builtin **System precompile**, at the literal address
/// `0x900`. (Builtin precompiles bake the matcher value straight into the
/// trailing address bytes — no `<< 16` shift, unlike the *public* `AddressMatcher`
/// that places the XCM precompile at `0x0A << 16`.) Its `terminate(address)`
/// routes to the runtime's `terminate_caller`, the only path that *actually*
/// removes an already-deployed contract.
const SYSTEM_PRECOMPILE_ADDR: [u8; 20] =
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09, 0];

/// Minimal ABI for the one System-precompile call we make. The contract's own
/// `Contract.sol` is unrelated, so we declare just `terminate(address)` here.
mod isystem {
    alloy_core::sol! {
        function terminate(address beneficiary) external;
    }
}

/// This is the constructor which is called once per contract.
///
/// Records the deployer in immutable data so `destroy` can both gate on them
/// (only they may tear the contract down) and pay the remaining balance back to
/// them. Immutable data is write-once, constructor-only — exactly this use.
#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {
    api::set_immutable_data(get_caller().as_slice());
}

/// This is the regular entry point when the contract is called.
#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
    let call_data_len = api::call_data_size();
    let mut call_data = vec![0u8; call_data_len as usize];
    api::call_data_copy(&mut call_data, 0);

    let selector: [u8; 4] = call_data[0..4].try_into().unwrap();

    match selector {
        Contract::proposeCall::SELECTOR => {
            let call: Contract::proposeCall =
                Contract::proposeCall::abi_decode_validate(&call_data)
                    .expect("Failed to decode propose call");
            let prop = Contract::Proposal {
                callHash: call.callHash,
                callLen: call.callLen,
                enactment: call.enactment,
                track: call.track,
                creator: get_caller(),
                approvers: call.approvers,
                minApprovers: call.minApprovers,
                approvedBy: Vec::new(),
                status: Contract::ProposalStatus::Review,
            };

            let key = match proposal_key(&prop) {
                Ok(k) => k,
                Err(_) => api::return_value(ReturnFlags::REVERT, &[]),
            };
            if get_proposal(&key).is_some() {
                api::return_value(ReturnFlags::REVERT, &[]);
            }
            set_proposal(&prop);
            add_proposal_key(key);

            events::proposed(&prop);

            api::return_value(ReturnFlags::empty(), &[]);
        }

        Contract::allProposalsCall::SELECTOR => {
            let proposals = get_all_proposals();
            api::return_value(ReturnFlags::empty(), &proposals.abi_encode());
        }

        Contract::proposalCall::SELECTOR => {
            let proposal_call = Contract::proposalCall::abi_decode_validate(&call_data)
                .expect("Failed to decode proposal call");

            let proposal = match get_proposal(&proposal_call.proposalHash) {
                Some(p) => p,
                None => api::return_value(ReturnFlags::REVERT, &[]),
            };

            api::return_value(ReturnFlags::empty(), &proposal.abi_encode());
        }

        Contract::approveCall::SELECTOR => {
            let approve_call = Contract::approveCall::abi_decode_validate(&call_data)
                .expect("Failed to decode approve call");

            match approve_proposal(&approve_call.proposalHash) {
                Ok(_) => (),
                Err(_) => api::return_value(ReturnFlags::REVERT, &[]),
            };

            events::approved(&approve_call.proposalHash);
            api::return_value(ReturnFlags::empty(), &[]);
        }

        Contract::finalizeCall::SELECTOR => {
            let finalize_call = Contract::finalizeCall::abi_decode_validate(&call_data)
                .expect("Failed to decode finalize call");

            let proposal = match finalize_proposal(&finalize_call.proposalHash) {
                Ok(p) => p,
                Err(ProposalError::NotApproved) => revert_not_approved(),
                Err(_) => api::return_value(ReturnFlags::REVERT, &[]),
            };

            let mut value_buf = [0u8; 32];
            api::value_transferred(&mut value_buf);
            let value = U256::from_le_bytes::<32>(value_buf);
            // `value` is EVM-denominated; the deposit is native, so scale it up.
            let required = U256::from(xcm::referendum::SUBMISSION_DEPOSIT)
                * U256::from(xcm::referendum::NATIVE_TO_ETH_RATIO);
            if value < required {
                revert_insufficient_deposit();
            }

            // Tally the funds sent against the depositor so they can later `refund`.
            increase_deposit(&get_caller(), value);

            // Persist the `Submitted` status. On a dispatch failure below we
            // `REVERT`, which unwinds this write along with the deposit tally.
            set_proposal(&proposal);

            // Dispatch `Referenda::submit` for `proposal.callHash` by executing a
            // local XCM `Transact` through Asset Hub's XCM precompile. The XCM runs
            // under this contract's sovereign account; the proposal's `enactment`
            // (DispatchTime) and `track` (referendum origin) are threaded into the
            // submitted referendum.
            let input = xcm::referendum::build_execute_calldata(
                &proposal.callHash.0,
                proposal.callLen,
                &proposal.enactment,
                &proposal.track,
            );
            let dispatched = api::call(
                CallFlags::empty(),
                &xcm::XCM_PRECOMPILE_ADDR,
                u64::MAX,       // ref_time limit: use all available
                u64::MAX,       // proof_size limit: use all available
                &[u8::MAX; 32], // no storage deposit limit
                &[0u8; 32],     // no value transferred
                &input,
                None,
            );
            if dispatched.is_err() {
                api::return_value(ReturnFlags::REVERT, &[]);
            }

            events::finalized(&finalize_call.proposalHash, &proposal.callHash);

            api::return_value(ReturnFlags::empty(), &[]);
        }

        Contract::depositsCall::SELECTOR => {
            let deposits_call = Contract::depositsCall::abi_decode_validate(&call_data)
                .expect("Failed to decode deposits call");

            let balance = get_deposit(&deposits_call.depositor);
            api::return_value(ReturnFlags::empty(), &balance.abi_encode());
        }

        Contract::refundCall::SELECTOR => {
            let caller = get_caller();
            let refunded = match refund(&caller) {
                Ok(amount) => amount,
                Err(_) => api::return_value(ReturnFlags::REVERT, &[]),
            };

            events::refunded(&caller, refunded);
            api::return_value(ReturnFlags::empty(), &[]);
        }

        Contract::destroyCall::SELECTOR => {
            // Only the original deployer may tear the contract down.
            let deployer = get_deployer();
            if get_caller() != deployer {
                revert_not_owner();
            }
            // Never destroy while deposits are still owed: termination sweeps the
            // entire balance to the deployer, which would strand every other
            // depositor's `refund`. The aggregate tally must be exactly zero.
            if get_total_owed() != U256::ZERO {
                revert_outstanding_deposits();
            }
            // Genuinely remove the contract — code, storage, account — and sweep
            // the balance to the deployer, by calling the System precompile's
            // `terminate(beneficiary)` (the runtime's `terminate_caller` path).
            //
            // We deliberately do NOT use the bare `terminate` host syscall: that
            // follows EIP-6780 and only deletes a contract destroyed in the same
            // transaction it was created in. For an already-deployed contract it
            // would merely sweep the free balance and leave the code + storage
            // on-chain — no actual undeploy.
            let input = isystem::terminateCall {
                beneficiary: deployer,
            }
            .abi_encode();
            let res = api::call(
                CallFlags::empty(),
                &SYSTEM_PRECOMPILE_ADDR,
                u64::MAX,       // ref_time limit: use all available
                u64::MAX,       // proof_size limit: use all available
                &[u8::MAX; 32], // no storage deposit limit
                &[0u8; 32],     // no value transferred
                &input,
                None,
            );
            if res.is_err() {
                api::return_value(ReturnFlags::REVERT, &[]);
            }
            api::return_value(ReturnFlags::empty(), &[]);
        }

        Contract::closeCall::SELECTOR => {
            let close_call = Contract::closeCall::abi_decode_validate(&call_data)
                .expect("Failed to decode close call");

            match close_proposal(&close_call.proposalHash) {
                Ok(()) => (),
                Err(_) => api::return_value(ReturnFlags::REVERT, &[]),
            };

            events::closed(&close_call.proposalHash);

            api::return_value(ReturnFlags::empty(), &[]);
        }

        _ => panic!("Unknown function selector"),
    }
}

fn get_proposal(key: &[u8; 32]) -> Option<Contract::Proposal> {
    let mut buf = [0u8; MAX_STORAGE_BYTES];
    let mut out: &mut [u8] = &mut buf;

    api::get_storage(StorageFlags::empty(), key, &mut out).ok()?;
    codec::decode(out).ok()
}

fn set_proposal(prop: &Contract::Proposal) {
    let key = proposal_key(prop).unwrap();

    // Unwrap rather than `REVERT`: the caller has already passed the higher-level
    // invariants (`proposal_key` succeeded, the proposal fits the array bounds),
    // so a codec failure here is a contract bug, not bad input.
    let out = codec::encode(prop).unwrap();
    api::set_storage(StorageFlags::empty(), &key, &out);
}

/// Storage slot holding the key added immediately before `key` — its "next"
/// pointer in the linked list. `keccak(PROPOSAL_NEXT_DOMAIN || key)`.
fn proposal_next_slot(key: &[u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(PROPOSAL_NEXT_DOMAIN.len() + 32);
    buf.extend_from_slice(PROPOSAL_NEXT_DOMAIN);
    buf.extend_from_slice(key);
    keccak256(&buf).0
}

/// Read a 32-byte link value from `slot`, or all-zeroes if the slot is empty.
/// Every link we store is exactly 32 bytes, so the zero-initialised buffer is
/// returned verbatim on a hit and doubles as the end-of-list sentinel on a miss.
fn read_link(slot: &[u8]) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let mut out: &mut [u8] = &mut buf;
    if api::get_storage(StorageFlags::empty(), slot, &mut out).is_err() {
        return [0u8; 32];
    }
    buf
}

/// The most-recently-added proposal key, or all-zeroes when the list is empty.
fn get_head() -> [u8; 32] {
    read_link(PROPOSALS_HEAD_KEY)
}

/// Prepend `key` to the proposal linked list. Each key occupies its own storage
/// value, so the list grows without bound — there is no single-value cap on the
/// number of proposals (the old `Vec`-in-one-value layout hit `STORAGE_BYTES`
/// at 11 keys). O(1): one link write + one head write.
///
/// `propose` rejects duplicate keys before calling this, so a key is never
/// linked twice (which would cycle the list).
fn add_proposal_key(key: [u8; 32]) {
    let prev_head = get_head();
    // `key.next = old head`, then `head = key`.
    api::set_storage(StorageFlags::empty(), &proposal_next_slot(&key), &prev_head);
    api::set_storage(StorageFlags::empty(), PROPOSALS_HEAD_KEY, &key);
}

/// Walk the list from the head, newest first, collecting every proposal key.
/// Stops at the all-zero sentinel (the first-ever proposal's `next`).
fn get_all_proposal_keys() -> Vec<[u8; 32]> {
    let mut keys = Vec::new();
    let mut cur = get_head();
    while cur != [0u8; 32] {
        keys.push(cur);
        cur = read_link(&proposal_next_slot(&cur));
    }
    keys
}

fn get_all_proposals() -> Vec<Contract::Proposal> {
    get_all_proposal_keys()
        .iter()
        .filter_map(get_proposal)
        .collect()
}

fn approve_proposal(key: &[u8; 32]) -> Result<(), ProposalError> {
    let mut proposal = get_proposal(key).ok_or(ProposalError::ProposalNotFound)?;

    // Approvals only count while the proposal is still in `Review`; a submitted
    // or closed proposal can't be approved.
    expect_review(&proposal)?;

    if !proposal.approvers.contains(&get_caller()) {
        return Err(ProposalError::NotAnApprover);
    }
    if proposal.approvedBy.contains(&get_caller()) {
        return Err(ProposalError::AlreadyApproved);
    }

    proposal.approvedBy.push(get_caller());
    set_proposal(&proposal);

    Ok(())
}

/// Load a proposal, verify it can be finalized, and advance it to `Submitted`.
///
/// Checks the approval threshold and creator, then performs the
/// `Review -> Submitted` transition (which rejects an already-submitted or
/// closed proposal). Returns the updated proposal so the caller can persist it
/// and act on its `callHash`.
fn finalize_proposal(key: &[u8; 32]) -> Result<Contract::Proposal, ProposalError> {
    let mut proposal = get_proposal(key).ok_or(ProposalError::ProposalNotFound)?;

    if U256::from(proposal.approvedBy.len() as u64) < proposal.minApprovers {
        return Err(ProposalError::NotApproved);
    }

    // only by the creator
    if proposal.creator != get_caller() {
        return Err(ProposalError::NotOwner);
    }

    mark_submitted(&mut proposal)?;

    Ok(proposal)
}

/// Storage key for a depositor's running tally: the `b"deposit:"` tag followed
/// by the 20-byte address, as a bare 32-byte key (8 + 20, the trailing 4 bytes
/// zero). No `keccak256` is needed — the key only has to be unique, and
/// pallet-revive already hashes every storage key into its trie (`blake2_256`
/// for this fixed-size access path).
fn deposit_key(addr: &Address) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(b"deposit:");
    key[8..28].copy_from_slice(addr.as_slice());
    key
}

fn get_deposit(addr: &Address) -> U256 {
    let key = deposit_key(addr);
    let mut buf = [0u8; 32];
    // Paired with `set_storage_or_clear`: the fixed-256-bit access path, which
    // fills `buf` with zeros for a missing (e.g. cleared/refunded) entry.
    api::get_storage_or_zero(StorageFlags::empty(), &key, &mut buf);
    U256::from_le_bytes::<32>(buf)
}

fn set_deposit(addr: &Address, amount: U256) {
    let key = deposit_key(addr);
    // `set_storage_or_clear` removes the entry when the value is all-zero, so a
    // fully-refunded depositor reclaims their storage deposit rather than leaving
    // a zeroed slot behind.
    api::set_storage_or_clear(StorageFlags::empty(), &key, &amount.to_le_bytes::<32>());
}

fn increase_deposit(addr: &Address, amount: U256) {
    set_deposit(addr, get_deposit(addr).saturating_add(amount));
    set_total_owed(get_total_owed().saturating_add(amount));
}

/// Storage key for the aggregate of every depositor's unrefunded tally. Tracked
/// so `destroy` can cheaply assert "nobody is owed anything" without iterating
/// the per-depositor `deposit:` slots (which storage can't enumerate).
///
/// A bare, zero-padded 32-byte key — its leading `b"total_owed"` tag can never
/// alias a `deposit:` key (those start with `b"deposit:"`). No hashing: it's a
/// constant, and pallet-revive hashes the key into its trie regardless.
const TOTAL_OWED_KEY: [u8; 32] = fixed_domain_key(b"total_owed");

/// Pack a short domain tag into a fixed 32-byte storage key, zero-padded. Used
/// for the fixed-size (`set_storage_or_clear`) access path, which requires a
/// `[u8; 32]` key. The tag must be ≤ 32 bytes.
const fn fixed_domain_key(tag: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    let mut i = 0;
    while i < tag.len() {
        key[i] = tag[i];
        i += 1;
    }
    key
}

fn get_total_owed() -> U256 {
    let mut buf = [0u8; 32];
    api::get_storage_or_zero(StorageFlags::empty(), &TOTAL_OWED_KEY, &mut buf);
    U256::from_le_bytes::<32>(buf)
}

fn set_total_owed(amount: U256) {
    // `set_storage_or_clear` reclaims the slot's storage deposit once the tally
    // hits zero (the fully-refunded steady state), mirroring `set_deposit`.
    api::set_storage_or_clear(
        StorageFlags::empty(),
        &TOTAL_OWED_KEY,
        &amount.to_le_bytes::<32>(),
    );
}

fn refund(addr: &Address) -> Result<U256, ()> {
    let balance = get_deposit(addr);
    set_deposit(addr, U256::ZERO);
    // On a failed transfer below the call reverts, unwinding this write too — so
    // the aggregate tracks the per-depositor tallies exactly, in lockstep.
    set_total_owed(get_total_owed().saturating_sub(balance));

    let res = api::call(
        CallFlags::empty(),
        &addr.0.0,
        u64::MAX,       // ref_time limit: use all available
        u64::MAX,       // proof_size limit: use all available
        &[u8::MAX; 32], // no storage deposit limit
        &balance.to_le_bytes::<32>(),
        &[],
        None,
    );
    if res.is_err() {
        return Err(());
    }

    Ok(balance)
}

/// Mark a proposal as `Closed`, leaving it stored so the status stays queryable.
///
/// Only the creator may close, and only before finalizing: `mark_closed`
/// performs the `Review -> Closed` transition and rejects a proposal that has
/// already been submitted or closed.
fn close_proposal(key: &[u8; 32]) -> Result<(), ProposalError> {
    let mut proposal = get_proposal(key).ok_or(ProposalError::ProposalNotFound)?;

    if proposal.creator != get_caller() {
        return Err(ProposalError::NotOwner);
    }

    mark_closed(&mut proposal)?;
    set_proposal(&proposal);

    Ok(())
}

mod events {
    use super::*;

    pub fn proposed(prop: &Contract::Proposal) {
        // `approvers` is an indexed dynamic array, so its topic is the keccak hash of
        // the encoded elements rather than the array itself.
        let approvers_topic =
            <sol_data::Array<sol_data::Address> as EventTopic>::encode_topic(&prop.approvers);
        let event = Contract::Proposed {
            callHash: prop.callHash,
            creator: prop.creator,
            approvers: approvers_topic.0,
            minApprovers: prop.minApprovers,
        };
        // 1 signature hash + 3 indexed params (`address[]` is hashed into its topic).
        let topics = event.encode_topics_array::<4>().map(|t| t.0.0);
        let data = event.encode_data();
        api::deposit_event(&topics, &data);
    }

    pub fn approved(key: &[u8; 32]) {
        let event = Contract::Approved {
            proposalHash: key.into(),
        };
        // 1 signature hash + 1 indexed param.
        let topics = event.encode_topics_array::<2>().map(|t| t.0.0);
        let data = event.encode_data();
        api::deposit_event(&topics, &data);
    }

    pub fn finalized(key: &[u8; 32], call_hash: &FixedBytes<32>) {
        let event = Contract::Finalized {
            proposalHash: key.into(),
            callHash: *call_hash,
        };
        // 1 signature hash + 2 indexed params.
        let topics = event.encode_topics_array::<3>().map(|t| t.0.0);
        let data = event.encode_data();
        api::deposit_event(&topics, &data);
    }

    pub fn refunded(to: &Address, amount: U256) {
        let event = Contract::Refunded { to: *to, amount };
        // 1 signature hash + 1 indexed param.
        let topics = event.encode_topics_array::<2>().map(|t| t.0.0);
        let data = event.encode_data();
        api::deposit_event(&topics, &data);
    }

    pub fn closed(key: &[u8; 32]) {
        let event = Contract::Closed {
            proposalHash: key.into(),
        };
        // 1 signature hash + 1 indexed param.
        let topics = event.encode_topics_array::<2>().map(|t| t.0.0);
        let data = event.encode_data();
        api::deposit_event(&topics, &data);
    }
}

/// Revert with the `NotApproved` Solidity error.
#[inline]
fn revert_not_approved() -> ! {
    let error = Contract::NotApproved {};
    let encoded_error = <Contract::NotApproved as SolError>::abi_encode(&error);
    api::return_value(ReturnFlags::REVERT, &encoded_error);
}

/// Revert with the `InsufficientDeposit` Solidity error.
#[inline]
fn revert_insufficient_deposit() -> ! {
    let error = Contract::InsufficientDeposit {};
    let encoded_error = <Contract::InsufficientDeposit as SolError>::abi_encode(&error);
    api::return_value(ReturnFlags::REVERT, &encoded_error);
}

/// Revert with the `NotOwner` Solidity error.
#[inline]
fn revert_not_owner() -> ! {
    let error = Contract::NotOwner {};
    let encoded_error = <Contract::NotOwner as SolError>::abi_encode(&error);
    api::return_value(ReturnFlags::REVERT, &encoded_error);
}

/// Revert with the `OutstandingDeposits` Solidity error.
#[inline]
fn revert_outstanding_deposits() -> ! {
    let error = Contract::OutstandingDeposits {};
    let encoded_error = <Contract::OutstandingDeposits as SolError>::abi_encode(&error);
    api::return_value(ReturnFlags::REVERT, &encoded_error);
}

/// The deployer recorded in immutable data by `deploy`. Only valid to read
/// outside the constructor (and only because `deploy` always sets it).
#[inline]
fn get_deployer() -> Address {
    let mut buf = [0u8; 20];
    let mut out: &mut [u8] = &mut buf;
    api::get_immutable_data(&mut out);
    buf.into()
}

/// Get the caller's address
#[inline]
fn get_caller() -> Address {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    caller.into()
}

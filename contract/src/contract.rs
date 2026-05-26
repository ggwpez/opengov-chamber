#![no_main]
#![no_std]

pub mod plumbing;

use alloy_core::{
    primitives::{Address, FixedBytes, U256},
    sol_types::{sol_data, EventTopic, SolCall, SolError, SolEvent, SolValue},
};
use contract::{Contract, ProposalError, proposal_key, xcm};
use pallet_revive_uapi::{CallFlags, HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

extern crate alloc;
use alloc::{vec, vec::Vec};

const MAX_PROPOSAL_BYTES: usize = 1024;
const ALL_PROPOSAL_KEYS_KEY: &[u8] = b"all_proposal_keys";

/// This is the constructor which is called once per contract.
#[polkavm_derive::polkavm_export]
pub extern "C" fn deploy() {}

/// This is the regular entry point when the contract is called.
#[polkavm_derive::polkavm_export]
pub extern "C" fn call() {
    let call_data_len = api::call_data_size();
    let mut call_data = vec![0u8; call_data_len as usize];
    api::call_data_copy(&mut call_data, 0);

    let selector: [u8; 4] = call_data[0..4].try_into().unwrap();

    match selector {
        Contract::proposeCall::SELECTOR => {
            let call: Contract::proposeCall = Contract::proposeCall::abi_decode_validate(&call_data)
                .expect("Failed to decode propose call");
            let prop = Contract::Proposal {
                callHash: call.callHash,
                callLen: call.callLen,
                enactmentDelay: call.enactmentDelay,
                creator: get_caller(),
                approvers: call.approvers,
                minApprovers: call.minApprovers,
                approvedBy: Vec::new(),
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

            // The XCM `Transact` dispatches `Referenda::submit` as this contract's
            // own sovereign account, which must hold the referendum
            // `SubmissionDeposit`. pallet-revive has already credited any value
            // sent with this call to the contract account, so require it to cover
            // the deposit. (On revert the transfer is rolled back, refunding the
            // caller.)
            let mut value_buf = [0u8; 32];
            api::value_transferred(&mut value_buf);
            let value = U256::from_le_bytes::<32>(value_buf);
            // `value` is EVM-denominated; the deposit is native, so scale it up.
            let required = U256::from(xcm::referendum::SUBMISSION_DEPOSIT)
                * U256::from(xcm::referendum::NATIVE_TO_ETH_RATIO);
            if value < required {
                revert_insufficient_deposit();
            }

            // Dispatch `Referenda::submit` for `proposal.callHash` by executing a
            // local XCM `Transact` through Asset Hub's XCM precompile. The XCM runs
            // under this contract's signed origin.
            let input = xcm::referendum::build_execute_calldata(
                &proposal.callHash.0,
                proposal.callLen,
                proposal.enactmentDelay,
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

        /*Contract::mintCall::SELECTOR => {
            let mint_call = Contract::mintCall::abi_decode_validate(&call_data)
                .expect("Failed to decode mint call");

            let new_recipient_balance =
                get_balance(&mint_call.to.into_array()).saturating_add(mint_call.amount);
            set_balance(&mint_call.to.into_array(), new_recipient_balance);

            let new_supply = get_total_supply().saturating_add(mint_call.amount);
            set_total_supply(new_supply);

            emit_transfer(Address::ZERO, mint_call.to, mint_call.amount);
        }

        Contract::totalSupplyCall::SELECTOR => {
            let total_supply = get_total_supply();
            api::return_value(ReturnFlags::empty(), &total_supply.to_be_bytes::<32>());
        }

        Contract::transferCall::SELECTOR => {
            let transfer_call = Contract::transferCall::abi_decode_validate(&call_data)
                .expect("Failed to decode transfer call");

            let caller = get_caller();
            let sender_balance = get_balance(&caller);

            if sender_balance < transfer_call.amount {
                revert_insufficient_balance();
            }

            let new_sender_balance = sender_balance - transfer_call.amount;

            let recipient_balance = get_balance(&transfer_call.to.into_array());
            let new_recipient_balance = recipient_balance + transfer_call.amount;

            set_balance(&caller, new_sender_balance);
            set_balance(&transfer_call.to.into_array(), new_recipient_balance);
            emit_transfer(
                Address::from(caller),
                transfer_call.to,
                transfer_call.amount,
            );
        }*/

        _ => panic!("Unknown function selector"),
    }
}

fn get_proposal(key: &[u8; 32]) -> Option<Contract::Proposal> {
    let mut buf = vec![0u8; MAX_PROPOSAL_BYTES]; // upper bound from max approvers
    let mut out = buf.as_mut_slice();

    api::get_storage(StorageFlags::empty(), key, &mut out).ok()?;
    Contract::Proposal::abi_decode_validate(&out).ok()
}

fn set_proposal(prop: &Contract::Proposal) {
    let key = proposal_key(prop).unwrap();

    let out = Contract::Proposal::abi_encode(prop);
    api::set_storage(StorageFlags::empty(), &key, &out);
}

fn set_all_proposal_keys(keys: &Vec<[u8; 32]>) {
    api::set_storage(StorageFlags::empty(), ALL_PROPOSAL_KEYS_KEY, &keys.abi_encode());
}

fn get_all_proposal_keys() -> Vec<[u8; 32]> {
    let mut buf = vec![0u8; MAX_PROPOSAL_BYTES]; // upper bound from max approvers
    let mut out = buf.as_mut_slice();

    if api::get_storage(StorageFlags::empty(), ALL_PROPOSAL_KEYS_KEY, &mut out).is_err() {
        return Vec::new();
    }
    <Vec<FixedBytes<32>>>::abi_decode_validate(&out)
        .map(|v| v.into_iter().map(|fb| fb.0).collect())
        .unwrap_or_default()
}

fn add_proposal_key(key: [u8; 32]) {
    let mut keys = get_all_proposal_keys();
    keys.push(key);
    set_all_proposal_keys(&keys);
}

fn get_all_proposals() -> Vec<Contract::Proposal> {
    get_all_proposal_keys()
        .iter()
        .filter_map(get_proposal)
        .collect()
}

fn approve_proposal(key: &[u8; 32]) -> Result<(), ProposalError> {
    let mut proposal = get_proposal(key).ok_or(ProposalError::ProposalNotFound)?;

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

/// Load a proposal and verify it has reached its approval threshold.
///
/// Returns the proposal on success so the caller can act on its `callHash`.
fn finalize_proposal(key: &[u8; 32]) -> Result<Contract::Proposal, ProposalError> {
    let proposal = get_proposal(key).ok_or(ProposalError::ProposalNotFound)?;

    if U256::from(proposal.approvedBy.len() as u64) < proposal.minApprovers {
        return Err(ProposalError::NotApproved);
    }

    Ok(proposal)
}

/// Get totalSupply from storage
/*fn get_total_supply() -> U256 {
    let key = total_supply_key();
    let mut supply_bytes = vec![0u8; 32];
    let mut supply_output = supply_bytes.as_mut_slice();

    match api::get_storage(StorageFlags::empty(), &key, &mut supply_output) {
        Ok(_) => U256::from_be_bytes::<32>(supply_output[0..32].try_into().unwrap()),
        Err(_) => U256::ZERO,
    }
}*/

/// Set totalSupply in storage
/*#[inline]
fn set_total_supply(amount: U256) {
    let key = total_supply_key();
    api::set_storage(StorageFlags::empty(), &key, &amount.to_be_bytes::<32>());
}*/

/// Get the balance for a given address from storage
/*#[inline]
fn get_balance(addr: &[u8; 20]) -> U256 {
    let key = balance_key(addr);
    let mut balance_bytes = vec![0u8; 32];
    let mut balance_output = balance_bytes.as_mut_slice();

    match api::get_storage(StorageFlags::empty(), &key, &mut balance_output) {
        Ok(_) => U256::from_be_bytes::<32>(balance_output[0..32].try_into().unwrap()),
        Err(_) => U256::ZERO,
    }
}*/

/// Set the balance for a given address in storage
/*#[inline]
{fn }set_balance(addr: &[u8; 20], amount: U256) {
    let key = balance_key(addr);
    api::set_storage(StorageFlags::empty(), &key, &amount.to_be_bytes::<32>());
}*/

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

// Emit a Transfer event
/*#[inline]
fn emit_transfer(from: Address, to: Address, value: U256) {
    let event = Contract::Transfer { from, to, value };
    let topics = [
        Contract::Transfer::SIGNATURE_HASH.0,
        event.from.into_word().0,
        event.to.into_word().0,
    ];
    let data = event.value.to_be_bytes::<32>();
    api::deposit_event(&topics, &data);
}*/

/*#[inline]
fn revert_insufficient_balance() -> ! {
    let error = Contract::InsufficientBalance {};
    let encoded_error = <Contract::InsufficientBalance as SolError>::abi_encode(&error);
    api::return_value(ReturnFlags::REVERT, &encoded_error);
}*/

/// Get the caller's address
#[inline]
fn get_caller() -> Address {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    caller.into()
}

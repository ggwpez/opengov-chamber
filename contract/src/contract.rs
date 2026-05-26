#![no_main]
#![no_std]

pub mod plumbing;

use alloy_core::{
    primitives::{Address, U256},
    sol_types::{SolCall, SolError, SolEvent, SolStruct, SolValue},
};
use contract::{Contract, proposal_key};
use pallet_revive_uapi::{HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

extern crate alloc;
use alloc::vec;

const MAX_PROPOSAL_BYTES: usize = 1024;

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
                creator: get_caller(),
                approvers: call.approvers,
                minApprovers: call.minApprovers
            };

            let key = match proposal_key(&prop) {
                Ok(k) => k,
                Err(_) => api::return_value(ReturnFlags::REVERT, &[]),
            };
            if get_proposal(&key).is_some() {
                panic!("Proposal already exists");
            }
            set_proposal(&prop);

            //let proposal = get_proposal(&propose_call.callHash);
            //api::return_value(ReturnFlags::empty(), &proposal.to_be_bytes::<32>());
            panic!("yass");
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

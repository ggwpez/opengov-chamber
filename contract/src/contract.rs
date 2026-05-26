#![no_main]
#![no_std]

pub mod plumbing;

use alloy_core::{
    primitives::{Address, U256},
    sol,
    sol_types::{SolCall, SolError, SolEvent},
};
use pallet_revive_uapi::{HostFn, HostFnImpl as api, ReturnFlags, StorageFlags};

extern crate alloc;
use alloc::vec;

sol!("Contract.sol");

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
        Contract::balanceOfCall::SELECTOR => {
            let balance_of_call = Contract::balanceOfCall::abi_decode_validate(&call_data)
                .expect("Failed to decode balanceOf call");

            let balance = get_balance(&balance_of_call.account.into_array());
            api::return_value(ReturnFlags::empty(), &balance.to_be_bytes::<32>());
        }

        Contract::mintCall::SELECTOR => {
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
        }

        _ => panic!("Unknown function selector"),
    }
}

/// Storage key for totalSupply (slot 0)
#[inline]
fn total_supply_key() -> [u8; 32] {
    [0u8; 32] // Slot 0
}

/// Storage key for balances[address]: "Balance:" prefix + zero pad + 20-byte address.
/// Safe vs total_supply_key (all-zero) because the prefix starts with non-zero 'B'.
fn balance_key(addr: &[u8; 20]) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[0..8].copy_from_slice(b"Balance:");
    key[12..32].copy_from_slice(addr);
    key
}

/// Get totalSupply from storage
fn get_total_supply() -> U256 {
    let key = total_supply_key();
    let mut supply_bytes = vec![0u8; 32];
    let mut supply_output = supply_bytes.as_mut_slice();

    match api::get_storage(StorageFlags::empty(), &key, &mut supply_output) {
        Ok(_) => U256::from_be_bytes::<32>(supply_output[0..32].try_into().unwrap()),
        Err(_) => U256::ZERO,
    }
}

/// Set totalSupply in storage
#[inline]
fn set_total_supply(amount: U256) {
    let key = total_supply_key();
    api::set_storage(StorageFlags::empty(), &key, &amount.to_be_bytes::<32>());
}

/// Get the balance for a given address from storage
#[inline]
fn get_balance(addr: &[u8; 20]) -> U256 {
    let key = balance_key(addr);
    let mut balance_bytes = vec![0u8; 32];
    let mut balance_output = balance_bytes.as_mut_slice();

    match api::get_storage(StorageFlags::empty(), &key, &mut balance_output) {
        Ok(_) => U256::from_be_bytes::<32>(balance_output[0..32].try_into().unwrap()),
        Err(_) => U256::ZERO,
    }
}

/// Set the balance for a given address in storage
#[inline]
fn set_balance(addr: &[u8; 20], amount: U256) {
    let key = balance_key(addr);
    api::set_storage(StorageFlags::empty(), &key, &amount.to_be_bytes::<32>());
}

/// Emit a Transfer event
#[inline]
fn emit_transfer(from: Address, to: Address, value: U256) {
    let event = Contract::Transfer { from, to, value };
    let topics = [
        Contract::Transfer::SIGNATURE_HASH.0,
        event.from.into_word().0,
        event.to.into_word().0,
    ];
    let data = event.value.to_be_bytes::<32>();
    api::deposit_event(&topics, &data);
}

/// Revert with an InsufficientBalance error
#[inline]
fn revert_insufficient_balance() -> ! {
    let error = Contract::InsufficientBalance {};
    let encoded_error = <Contract::InsufficientBalance as SolError>::abi_encode(&error);
    api::return_value(ReturnFlags::REVERT, &encoded_error);
}

/// Get the caller's address
#[inline]
fn get_caller() -> [u8; 20] {
    let mut caller = [0u8; 20];
    api::caller(&mut caller);
    caller
}

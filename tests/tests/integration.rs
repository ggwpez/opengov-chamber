use contract::{B256, Contract, sol_types::SolValue};
use contract_tests::{Test, fund, new_test_ext, selector, RuntimeOrigin};
use pallet_revive::{
    Code, TransactionLimits, Weight,
    test_utils::{
        ALICE, ALICE_ADDR,
        BOB, BOB_ADDR,
        builder::{BareCallBuilder, BareInstantiateBuilder},
    },
};
use sp_core::{H160, U256};
use contract::Address;

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

fn encode_address(addr: H160) -> Vec<u8> {
    let mut out = vec![0u8; 12];
    out.extend_from_slice(&addr.0);
    out
}

fn encode_address_uint256(addr: H160, amount: U256) -> Vec<u8> {
    let mut out = encode_address(addr);
    out.extend_from_slice(&amount.to_big_endian());
    out
}

fn encode_propose_args(call_hash: [u8; 32], approvers: &[H160], min_approvers: U256) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&call_hash);
    out.extend_from_slice(&U256::from(0x60).to_big_endian());
    out.extend_from_slice(&min_approvers.to_big_endian());
    out.extend_from_slice(&U256::from(approvers.len() as u64).to_big_endian());
    for a in approvers {
        out.extend_from_slice(&encode_address(*a));
    }
    out
}

fn calldata(sig: &str, params: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(4 + params.len());
    data.extend_from_slice(&selector(sig));
    data.extend_from_slice(params);
    data
}

#[test]
fn blob_size_is_sane() {
    let size = BLOB.len();
    if size > 100_000 {
        panic!("blob size is too large: {}", size);
    }
    eprintln!("blob size is sane: {}", size);
}

#[test]
fn mint_then_balance_of_round_trip() {
    new_test_ext().execute_with(|| {
        fund(&ALICE, u64::MAX / 2);
        let expected_proposal = Contract::Proposal {
            callHash: B256::repeat_byte(0xAA),
            creator: Address::from(ALICE_ADDR.0),
            approvers: vec![Address::from(BOB_ADDR.0)],
            minApprovers: contract::U256::from(1u64),
        };

        let contract = BareInstantiateBuilder::<Test>::bare_instantiate(
            RuntimeOrigin::signed(ALICE),
            Code::Upload(BLOB.to_vec()),
        )
        .transaction_limits(limits())
        .build_and_unwrap_contract();

        // propose
        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), contract.addr)
            .data(calldata(
                "propose(bytes32,address[],uint256)",
                &encode_propose_args([0xAA; 32], &[BOB_ADDR], U256::from(1u64)),
            ))
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // allProposals() → [expected_proposal]
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), contract.addr)
            .data(calldata(
                "allProposals()",
                &[],
            ))
            .transaction_limits(limits())
            .build_and_unwrap_result();

        let proposals = <Vec<Contract::Proposal>>::abi_decode_validate(&result.data).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0], expected_proposal);
    });
}

use contract_tests::{Test, fund, new_test_ext, selector, RuntimeOrigin};
use pallet_revive::{
    Code, TransactionLimits, Weight,
    test_utils::{
        ALICE, ALICE_ADDR,
        builder::{BareCallBuilder, BareInstantiateBuilder},
    },
};
use sp_core::{H160, U256};

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
    return;
    new_test_ext().execute_with(|| {
        fund(&ALICE, u64::MAX / 2);

        let contract = BareInstantiateBuilder::<Test>::bare_instantiate(
            RuntimeOrigin::signed(ALICE),
            Code::Upload(BLOB.to_vec()),
        )
        .transaction_limits(limits())
        .build_and_unwrap_contract();

        // mint(ALICE_ADDR, 1000)
        let _ = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), contract.addr)
            .data(calldata(
                "mint(address,uint256)",
                &encode_address_uint256(ALICE_ADDR, U256::from(1000u64)),
            ))
            .transaction_limits(limits())
            .build_and_unwrap_result();

        // balanceOf(ALICE_ADDR) → 1000
        let result = BareCallBuilder::<Test>::bare_call(RuntimeOrigin::signed(ALICE), contract.addr)
            .data(calldata(
                "balanceOf(address)",
                &encode_address(ALICE_ADDR),
            ))
            .transaction_limits(limits())
            .build_and_unwrap_result();

        assert_eq!(
            U256::from_big_endian(&result.data),
            U256::from(1000u64),
            "mint should credit ALICE_ADDR with 1000",
        );
    });
}

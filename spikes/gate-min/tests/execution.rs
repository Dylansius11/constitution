use gate_min::{ExecuteInput, GateMin, assemble_signed_execute, signing_preimage};
use near_sdk::{
    json_types::U128,
    test_utils::{VMContextBuilder, get_created_receipts},
    testing_env,
};
use serde_json::{Value, json};
use sha3::{Digest, Keccak256};

fn input() -> ExecuteInput {
    ExecuteInput {
        executor: [0x11; 20],
        certificate_id: [0x22; 32],
        new_impl: [0x33; 20],
        new_impl_codehash: [0x44; 32],
        expected_current_impl: [0x55; 20],
        deadline_s: 1_800_000_000,
        init_calldata: vec![0xde, 0xad, 0xbe, 0xef],
        nonce: 7,
        gas_limit: 300_000,
        max_fee_per_gas: U128(2_000_000_000),
        max_priority_fee_per_gas: U128(1_000_000),
    }
}

#[test]
fn execute_signing_request_binds_in_contract_preimage_and_callback_input() {
    testing_env!(
        VMContextBuilder::new()
            .prepaid_gas(near_sdk::Gas::from_tgas(100))
            .build()
    );
    let expected_hash = hex::encode(Keccak256::digest(signing_preimage(&input())));
    let expected_input = serde_json::to_value(input()).unwrap();
    drop(GateMin.sign_execute(input()));
    let receipts = get_created_receipts();
    assert_eq!(receipts.len(), 2);
    match &receipts[0].actions[0] {
        near_sdk::mock::MockAction::FunctionCallWeight { args, .. } => {
            let args: Value = serde_json::from_slice(args).unwrap();
            assert_eq!(args["request"]["payload_v2"]["Ecdsa"], expected_hash);
        }
        action => panic!("unexpected sign action {action:?}"),
    }
    match &receipts[1].actions[0] {
        near_sdk::mock::MockAction::FunctionCallWeight {
            method_name, args, ..
        } => {
            assert_eq!(method_name, b"on_execute_signed");
            assert_eq!(
                serde_json::from_slice::<Value>(args).unwrap(),
                json!({"input": expected_input})
            );
        }
        action => panic!("unexpected callback {action:?}"),
    }
}

#[test]
fn observed_signature_assembles_canonical_signed_type2_transaction() {
    let response = include_bytes!("../../../vectors/mpc-response.json");
    let raw = assemble_signed_execute(&input(), response).unwrap();
    assert_eq!(raw[0], 2);
    let fields = rlp::Rlp::new(&raw[1..]);
    assert_eq!(fields.item_count().unwrap(), 12);
    let preimage = signing_preimage(&input());
    let unsigned = rlp::Rlp::new(&preimage[1..]);
    for i in 0..9 {
        assert_eq!(
            fields.at(i).unwrap().as_raw(),
            unsigned.at(i).unwrap().as_raw()
        );
    }
    assert_eq!(fields.val_at::<u8>(9).unwrap(), 1);
    assert_eq!(
        hex::encode(fields.val_at::<Vec<u8>>(10).unwrap()),
        "03a3a381219d87a2d948ca31dd1223f5c8963e62239cf76e2c325fd9ba6bf32d"
    );
    assert_eq!(
        hex::encode(fields.val_at::<Vec<u8>>(11).unwrap()),
        "2617462cafebb333c8e8aafe74968f734517916c9bec7da562e9bfc48a66f810"
    );
}

#[test]
fn signature_parser_rejects_incomplete_and_noncanonical_responses() {
    let good: Value =
        serde_json::from_slice(include_bytes!("../../../vectors/mpc-response.json")).unwrap();
    for bad in [
        json!({}),
        json!({"scheme":"Ed25519"}),
        {
            let mut v = good.clone();
            v["recovery_id"] = json!(2);
            v
        },
        {
            let mut v = good.clone();
            v["s"]["scalar"] = json!("00".repeat(32));
            v
        },
        {
            let mut v = good.clone();
            v["s"]["scalar"] = json!("ff".repeat(32));
            v
        },
        {
            let mut v = good.clone();
            v["big_r"]["affine_point"] = json!("04".to_owned() + &"01".repeat(32));
            v
        },
    ] {
        assert!(assemble_signed_execute(&input(), &serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn execute_callback_records_failure_without_panicking() {
    testing_env!(
        VMContextBuilder::new().build(),
        near_sdk::test_vm_config(),
        near_parameters::RuntimeFeesConfig::test(),
        std::collections::HashMap::default(),
        vec![near_sdk::PromiseResult::Failed]
    );
    assert_eq!(GateMin.on_execute_signed(input())["status"], "failed");
}

#[test]
fn execute_callback_returns_exact_raw_transaction_and_hashes() {
    let response = include_bytes!("../../../vectors/mpc-response.json");
    testing_env!(
        VMContextBuilder::new().build(),
        near_sdk::test_vm_config(),
        near_parameters::RuntimeFeesConfig::test(),
        std::collections::HashMap::default(),
        vec![near_sdk::PromiseResult::Successful(response.to_vec())]
    );
    let result = GateMin.on_execute_signed(input());
    let raw = assemble_signed_execute(&input(), response).unwrap();
    assert_eq!(result["status"], "signed");
    assert_eq!(result["raw_tx"], format!("0x{}", hex::encode(&raw)));
    assert_eq!(
        result["tx_hash"],
        format!("0x{}", hex::encode(Keccak256::digest(&raw)))
    );
    assert_eq!(
        result["signing_hash"],
        format!(
            "0x{}",
            hex::encode(Keccak256::digest(signing_preimage(&input())))
        )
    );
}

#[test]
fn signature_scalars_with_leading_zeroes_use_minimal_rlp_integers() {
    let response = json!({"scheme":"Secp256k1",
        "big_r":{"affine_point": format!("02{:064x}", 1)}, "s":{"scalar":format!("{:064x}", 2)},
        "recovery_id":0});
    let raw = assemble_signed_execute(&input(), &serde_json::to_vec(&response).unwrap()).unwrap();
    let fields = rlp::Rlp::new(&raw[1..]);
    assert_eq!(fields.val_at::<Vec<u8>>(10).unwrap(), [1]);
    assert_eq!(fields.val_at::<Vec<u8>>(11).unwrap(), [2]);
}

use gate_min::{ExecuteInput, GateMin, MPC_ACCOUNT, MPC_PATH, execute_calldata, signing_preimage};
use k256::{
    ProjectivePoint, PublicKey, Scalar,
    elliptic_curve::{PrimeField, sec1::ToEncodedPoint},
};
use near_sdk::{
    json_types::U128,
    test_utils::{VMContextBuilder, get_created_receipts},
    testing_env,
};
use serde_json::Value;
use sha3::{Digest, Keccak256, Sha3_256};

fn fixed_input() -> ExecuteInput {
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

fn word(n: u128) -> Vec<u8> {
    let mut out = vec![0; 16];
    out.extend(n.to_be_bytes());
    out
}

// Independent standard ABI head/tail oracle, not alloy's encoder.
fn manual_calldata(input: &ExecuteInput) -> Vec<u8> {
    let mut out = vec![0x90, 0xc7, 0x0c, 0xd8];
    out.extend(input.certificate_id);
    out.extend([0; 12]);
    out.extend(input.new_impl);
    out.extend(input.new_impl_codehash);
    out.extend([0; 12]);
    out.extend(input.expected_current_impl);
    out.extend(word(input.deadline_s.into()));
    out.extend(word(192));
    out.extend(word(input.init_calldata.len() as u128));
    out.extend(&input.init_calldata);
    out.resize(4 + 224 + input.init_calldata.len().div_ceil(32) * 32, 0);
    out
}

// Independent RLP oracle: minimal big-endian integers and canonical long lengths.
fn minimal(n: u128) -> Vec<u8> {
    let bytes = n.to_be_bytes();
    bytes[bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len())..].to_vec()
}
fn rlp(payload: &[u8], list: bool) -> Vec<u8> {
    if !list && payload.len() == 1 && payload[0] < 128 {
        return payload.to_vec();
    }
    let base = if list { 0xc0 } else { 0x80 };
    let mut out = if payload.len() < 56 {
        vec![base + payload.len() as u8]
    } else {
        let length = minimal(payload.len() as u128);
        let mut prefix = vec![base + 55 + length.len() as u8];
        prefix.extend(length);
        prefix
    };
    out.extend(payload);
    out
}
fn manual_preimage(input: &ExecuteInput) -> Vec<u8> {
    let mut fields = Vec::new();
    for n in [
        84532,
        input.nonce as u128,
        input.max_priority_fee_per_gas.0,
        input.max_fee_per_gas.0,
        input.gas_limit as u128,
    ] {
        fields.extend(rlp(&minimal(n), false));
    }
    fields.extend(rlp(&input.executor, false));
    fields.extend(rlp(&[], false));
    fields.extend(rlp(&manual_calldata(input), false));
    fields.extend(rlp(&[], true));
    let mut out = vec![2];
    out.extend(rlp(&fields, true));
    out
}

#[test]
fn eip1559_preimage_matches_independent_abi_and_rlp() {
    let input = fixed_input();
    let preimage = signing_preimage(&input);
    assert_eq!(preimage, manual_preimage(&input));
    assert_eq!(
        Keccak256::digest(&preimage),
        Keccak256::digest(manual_preimage(&input))
    );
    // Independently computed with Python standard-library RLP/ABI and PyCryptodome Keccak.
    assert_eq!(
        hex::encode(Keccak256::digest(&preimage)),
        "f4cabf1bbeafae5b1b847737b03d31ac62d5805cb3204b04faefe5e22bb750c3"
    );
    println!(
        "preimage={}\nsigning_hash={:x}",
        hex::encode(&preimage),
        Keccak256::digest(&preimage)
    );
}

#[test]
fn abi_dynamic_bytes_boundaries_match_hand_encoder() {
    for len in [0, 1, 31, 32, 33, 1024] {
        let mut input = fixed_input();
        input.init_calldata = vec![0xa5; len];
        assert_eq!(
            execute_calldata(&input),
            manual_calldata(&input),
            "length {len}"
        );
    }
}

#[test]
#[should_panic(expected = "init calldata exceeds 1024 bytes")]
fn rejects_oversize_init_calldata() {
    let mut input = fixed_input();
    input.init_calldata = vec![0; 1025];
    signing_preimage(&input);
}

#[test]
#[should_panic(expected = "priority fee exceeds max fee")]
fn rejects_priority_fee_above_max_fee() {
    let mut input = fixed_input();
    input.max_fee_per_gas = U128(0);
    signing_preimage(&input);
}

// Host only: no k256, SHA3 or derivation code is linked into the contract wasm.
fn derive(root: &str, predecessor: &str, path: &str) -> (String, String, String) {
    let _: near_sdk::AccountId = predecessor.parse().expect("valid predecessor account id");
    let raw = bs58::decode(root.strip_prefix("secp256k1:").expect("secp256k1 root"))
        .into_vec()
        .expect("base58 root");
    assert_eq!(raw.len(), 64);
    let mut sec1 = vec![4];
    sec1.extend(raw);
    let root = PublicKey::from_sec1_bytes(&sec1).expect("root point on curve");
    let tweak = Sha3_256::digest(format!(
        "near-mpc-recovery v0.1.0 epsilon derivation:{predecessor},{path}"
    ));
    // Upstream rejects out-of-range scalars rather than reducing modulo the order.
    let scalar = Option::<Scalar>::from(Scalar::from_repr(tweak)).expect("tweak scalar in range");
    let point = ProjectivePoint::GENERATOR * scalar + ProjectivePoint::from(*root.as_affine());
    let key = PublicKey::from_affine(point.to_affine()).expect("derived point not infinity");
    let encoded = key.to_encoded_point(false);
    let raw = &encoded.as_bytes()[1..];
    let near_key = format!("secp256k1:{}", bs58::encode(raw).into_string());
    let address = format!("0x{}", hex::encode(&Keccak256::digest(raw)[12..]));
    (near_key, address, hex::encode(tweak))
}

#[test]
fn offline_derivation_matches_all_live_finalized_views() {
    let doc: Value =
        serde_json::from_str(include_str!("../../../vectors/mpc-derivation.json")).unwrap();
    let root = doc["root"]["decoded"].as_str().unwrap();
    for vector in doc["vectors"].as_array().unwrap() {
        let (key, address, tweak) = derive(
            root,
            vector["predecessor"].as_str().unwrap(),
            vector["path"].as_str().unwrap(),
        );
        let view = &vector["view"];
        let bytes: Vec<u8> =
            serde_json::from_value(view["response"]["result"]["result"].clone()).unwrap();
        let rpc_key: String = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(key, rpc_key);
        assert_eq!(key, view["decoded"]);
        assert_eq!(address, vector["evm_address"]);
        assert_eq!(tweak, vector["tweak_sha3_256"]);
        println!(
            "{} {} => {} at block {}",
            vector["predecessor"],
            vector["path"],
            address,
            view["response"]["result"]["block_height"]
        );
    }
}

#[test]
fn different_path_and_predecessor_change_derived_address() {
    let doc: Value =
        serde_json::from_str(include_str!("../../../vectors/mpc-derivation.json")).unwrap();
    let root = doc["root"]["decoded"].as_str().unwrap();
    let base = derive(root, "constitution-gate.testnet", MPC_PATH);
    assert_ne!(
        base.1,
        derive(
            root,
            "constitution-gate.testnet",
            "constitution/base-sepolia/v2"
        )
        .1
    );
    assert_ne!(
        base.1,
        derive(root, "constitution-spike.testnet", MPC_PATH).1
    );
}

#[test]
fn spike_sign_emits_exact_v2_request_deposit_and_gas() {
    testing_env!(
        VMContextBuilder::new()
            .prepaid_gas(near_sdk::Gas::from_tgas(100))
            .build()
    );
    drop(GateMin.spike_sign([0xab; 32]));
    let receipts = get_created_receipts();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].receiver_id.as_str(), MPC_ACCOUNT);
    match &receipts[0].actions[0] {
        near_sdk::mock::MockAction::FunctionCallWeight {
            method_name,
            args,
            attached_deposit,
            prepaid_gas,
            ..
        } => {
            assert_eq!(method_name, b"sign");
            assert_eq!(attached_deposit.as_yoctonear(), 1);
            assert_eq!(prepaid_gas.as_tgas(), 15);
            let request: Value = serde_json::from_slice(args).unwrap();
            assert_eq!(
                request,
                serde_json::json!({"request":{"payload_v2":{"Ecdsa":"ab".repeat(32)},"path":MPC_PATH,"domain_id":0}})
            );
        }
        action => panic!("unexpected action: {action:?}"),
    }
    match &receipts[1].actions[0] {
        near_sdk::mock::MockAction::FunctionCallWeight {
            method_name,
            attached_deposit,
            prepaid_gas,
            ..
        } => {
            assert_eq!(method_name, b"on_spike_signed");
            assert_eq!(attached_deposit.as_yoctonear(), 0);
            assert_eq!(prepaid_gas.as_tgas(), 20);
        }
        action => panic!("unexpected callback action: {action:?}"),
    }
}

#[test]
fn callback_preserves_success_bytes_and_records_failure_without_panic() {
    for (result, expected) in [
        (
            near_sdk::PromiseResult::Successful(br#"{"scheme":"Secp256k1"}"#.to_vec()),
            "{\"scheme\":\"Secp256k1\"}",
        ),
        (near_sdk::PromiseResult::Failed, ""),
        (near_sdk::PromiseResult::Successful(vec![0xff]), ""),
        (near_sdk::PromiseResult::Successful(vec![b'a'; 4097]), ""),
    ] {
        testing_env!(
            VMContextBuilder::new().build(),
            near_sdk::test_vm_config(),
            near_parameters::RuntimeFeesConfig::test(),
            std::collections::HashMap::default(),
            vec![result],
        );
        assert_eq!(GateMin.on_spike_signed(), expected);
    }
}

#[test]
fn observed_mpc_signature_recovers_to_live_derived_spike_key() {
    let response: Value =
        serde_json::from_str(include_str!("../../../vectors/mpc-response.json")).unwrap();
    assert_eq!(response["scheme"], "Secp256k1");
    let big_r = hex::decode(response["big_r"]["affine_point"].as_str().unwrap()).unwrap();
    let s = hex::decode(response["s"]["scalar"].as_str().unwrap()).unwrap();
    assert_eq!(big_r.len(), 33);
    assert!(big_r[0] == 2 || big_r[0] == 3);
    let signature = k256::ecdsa::Signature::from_scalars(
        <[u8; 32]>::try_from(&big_r[1..]).unwrap(),
        <[u8; 32]>::try_from(s.as_slice()).unwrap(),
    )
    .unwrap();
    let recovery_id = k256::ecdsa::RecoveryId::from_byte(
        u8::try_from(response["recovery_id"].as_u64().unwrap()).unwrap(),
    )
    .unwrap();
    let hash = Keccak256::digest(signing_preimage(&fixed_input()));
    let recovered =
        k256::ecdsa::VerifyingKey::recover_from_prehash(&hash, &signature, recovery_id).unwrap();
    let encoded = recovered.to_encoded_point(false);
    let raw = &encoded.as_bytes()[1..];
    let key = format!("secp256k1:{}", bs58::encode(raw).into_string());
    let address = format!("0x{}", hex::encode(&Keccak256::digest(raw)[12..]));
    let doc: Value =
        serde_json::from_str(include_str!("../../../vectors/mpc-derivation.json")).unwrap();
    let vector = &doc["vectors"][2];
    assert_eq!(key, vector["view"]["decoded"]);
    assert_eq!(address, vector["evm_address"]);
    let offline = derive(
        doc["root"]["decoded"].as_str().unwrap(),
        "constitution-spike.testnet",
        MPC_PATH,
    );
    assert_eq!(key, offline.0);
    assert_eq!(address, offline.1);
    println!("observed MPC signature recovers to {address}");
}

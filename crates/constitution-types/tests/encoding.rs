use constitution_types::*;
use serde_json::{from_value, json, to_value};
use std::collections::BTreeSet;

fn certificate() -> Certificate {
    Certificate {
        domain: DOMAIN,
        gate: "gate.testnet".parse().unwrap(),
        proposal_id: 7,
        attempt: 2,
        chain_id: 84532,
        proxy: [1; 20],
        executor: [2; 20],
        current_impl: [3; 20],
        current_impl_codehash: [4; 32],
        new_impl: [5; 20],
        new_impl_codehash: [6; 32],
        init_calldata_hash: [7; 32],
        source_commitment: [8; 32],
        rule_set_hash: [9; 32],
        verifier_pubkey: [10; 32],
        verifier_release_hash: [11; 32],
        evidence_block_number: 123,
        evidence_block_hash: [12; 32],
        evidence_root: [13; 32],
        ai: AiReview::Clean {
            evidence_hash: [14; 32],
            signature_kind: 1,
        },
        verdict: Verdict::Reject {
            rule_id: 2,
            reason_code: 22,
        },
        issued_at_s: 1_700_000_000,
        expires_at_s: 1_700_001_800,
    }
}

// [V] Independent Python oracle, exercised 2026-11-03 without importing Rust:
// import hashlib, struct
// b = b'constitution.v1\0' + struct.pack('<I', 12) + b'gate.testnet'
// b += struct.pack('<QIQ', 7, 2, 84532)
// for n, size in [(1,20),(2,20),(3,20),(4,32),(5,20),(6,32),
//                 (7,32),(8,32),(9,32),(10,32),(11,32)]:
//     b += bytes([n]) * size
// b += struct.pack('<Q', 123) + bytes([12])*32 + bytes([13])*32
// b += b'\x00' + bytes([14])*32 + b'\x01' + b'\x01'
// b += struct.pack('<BHQQ', 2, 22, 1700000000, 1700001800)
// print(len(b), hashlib.sha256(b).hexdigest())
// 482 8d17cdd6268c32fdb5d3a37967545bd42ad73403bb8a56587b49024425cfa11a
#[test]
fn certificate_hash_matches_independent_python_oracle() {
    let bytes = certificate().to_canonical_bytes().unwrap();
    assert_eq!(bytes.len(), 482);
    assert_eq!(
        to_value(Hex32(certificate_hash(&bytes))).unwrap(),
        json!("0x8d17cdd6268c32fdb5d3a37967545bd42ad73403bb8a56587b49024425cfa11a")
    );
    assert_eq!(&bytes[..16], b"constitution.v1\0");
}

#[test]
fn certificate_rejects_trailing_truncated_and_invalid_enum_bytes() {
    let bytes = certificate().to_canonical_bytes().unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(Certificate::from_canonical_bytes(&trailing).is_err());
    for end in 0..bytes.len() {
        assert!(Certificate::from_canonical_bytes(&bytes[..end]).is_err());
    }
    let mut invalid = bytes;
    invalid[428] = 255;
    assert!(Certificate::from_canonical_bytes(&invalid).is_err());
}

#[test]
fn hex_json_requires_lowercase_prefix_exact_size_and_valid_digits() {
    let address = format!("0x{}", "ab".repeat(20));
    assert_eq!(
        from_value::<Hex20>(json!(address)).unwrap(),
        Hex20([0xab; 20])
    );
    assert_eq!(to_value(Hex20([0xab; 20])).unwrap(), json!(address));
    let hash = format!("0x{}", "cd".repeat(32));
    assert_eq!(from_value::<Hex32>(json!(hash)).unwrap(), Hex32([0xcd; 32]));
    assert_eq!(to_value(Hex32([0xcd; 32])).unwrap(), json!(hash));
    for bad in [
        "ab".repeat(20),
        format!("0X{}", "ab".repeat(20)),
        format!("0x{}", "AB".repeat(20)),
        format!("0x{}", "gg".repeat(20)),
        format!("0x{}", "00".repeat(19)),
        format!("0x{}", "00".repeat(21)),
    ] {
        assert!(from_value::<Hex20>(json!(bad)).is_err(), "{bad}");
    }
    for bad in [
        "cd".repeat(32),
        format!("0x{}", "CD".repeat(32)),
        format!("0x{}", "zz".repeat(32)),
        format!("0x{}", "00".repeat(31)),
        format!("0x{}", "00".repeat(33)),
    ] {
        assert!(from_value::<Hex32>(json!(bad)).is_err(), "{bad}");
    }
    for bad in ["0", "0x0", "0xAB", "0xgg", "0X00", "00", "0xé"] {
        assert!(from_value::<HexBytes>(json!(bad)).is_err());
    }
    for bytes in [vec![], vec![0, 0xab, 0xff]] {
        let wrapped = HexBytes(bytes);
        assert_eq!(
            from_value::<HexBytes>(to_value(&wrapped).unwrap()).unwrap(),
            wrapped
        );
    }
    assert_eq!(to_value(HexBytes(vec![])).unwrap(), json!("0x"));
    assert!(from_value::<Hex20>(json!(vec![0; 20])).is_err());
}

#[test]
fn decimal_json_is_a_string_and_checked_for_overflow() {
    for n in [0, 1, u128::MAX] {
        assert_eq!(
            from_value::<U128Str>(json!(n.to_string())).unwrap(),
            U128Str(n)
        );
        assert_eq!(to_value(U128Str(n)).unwrap(), json!(n.to_string()));
    }
    for bad in [
        "",
        "-1",
        "+1",
        "1.0",
        " 1",
        "1 ",
        "340282366920938463463374607431768211456",
    ] {
        assert!(from_value::<U128Str>(json!(bad)).is_err(), "{bad}");
    }
    assert!(from_value::<U128Str>(json!(1)).is_err());
}

#[test]
fn wrappers_preserve_raw_borsh_encoding() {
    assert_eq!(borsh::to_vec(&Hex20([0xab; 20])).unwrap(), vec![0xab; 20]);
    assert_eq!(borsh::to_vec(&Hex32([0xcd; 32])).unwrap(), vec![0xcd; 32]);
    assert_eq!(
        borsh::to_vec(&HexBytes(vec![3, 4])).unwrap(),
        vec![2, 0, 0, 0, 3, 4]
    );
    assert_eq!(borsh::to_vec(&U128Str(u128::MAX)).unwrap(), vec![255; 16]);
    assert_eq!(borsh::to_vec(&Base64([1; 32])).unwrap(), vec![1; 32]);
    assert_eq!(borsh::to_vec(&Base64([2; 64])).unwrap(), vec![2; 64]);
    assert_eq!(
        borsh::to_vec(&Base64Bytes(vec![3, 4])).unwrap(),
        vec![2, 0, 0, 0, 3, 4]
    );
}

#[test]
fn base64_json_roundtrips_and_rejects_invalid_sizes() {
    let key = Base64([0; 32]);
    assert_eq!(
        to_value(key).unwrap(),
        json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
    );
    assert_eq!(
        from_value::<Base64<32>>(to_value(key).unwrap()).unwrap(),
        key
    );
    let signature = Base64([255; 64]);
    assert_eq!(
        from_value::<Base64<64>>(to_value(signature).unwrap()).unwrap(),
        signature
    );
    for bad in [
        "AA==",
        "!",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB=",
    ] {
        assert!(from_value::<Base64<32>>(json!(bad)).is_err());
    }
    let certificate = Base64Bytes(certificate().to_canonical_bytes().unwrap());
    let decoded = from_value::<Base64Bytes>(to_value(&certificate).unwrap()).unwrap();
    assert_eq!(decoded, certificate);
    assert!(Certificate::from_canonical_bytes(&decoded.0).is_ok());
}

#[test]
fn propose_json_matches_spec() {
    let input = json!({"new_impl":format!("0x{}", "11".repeat(20)),
        "new_impl_codehash":format!("0x{}", "22".repeat(32)),
        "expected_current_impl":format!("0x{}", "33".repeat(20)),
        "init_calldata":"0xabcdef", "source_commitment":format!("0x{}", "44".repeat(32))});
    let args: ProposeArgs = from_value(input.clone()).unwrap();
    assert_eq!(args.new_impl, Hex20([0x11; 20]));
    assert_eq!(args.init_calldata, HexBytes(vec![0xab, 0xcd, 0xef]));
    assert_eq!(to_value(args).unwrap(), input);
    assert_eq!(MAX_INIT_CALLDATA, 1024);
}

#[test]
fn shared_json_views_use_hex_base64_and_decimal_strings() {
    let value = to_value(certificate()).unwrap();
    assert_eq!(value["proxy"], json!(format!("0x{}", "01".repeat(20))));
    assert_eq!(
        value["verifier_pubkey"],
        to_value(Base64([10; 32])).unwrap()
    );
    let change = ChangeKind::AddVerifier {
        pubkey: [0; 32],
        release_hash: [1; 32],
        attestation_uri: "uri".into(),
    };
    let value = to_value(&change).unwrap();
    assert_eq!(
        value["AddVerifier"]["pubkey"],
        to_value(Base64([0; 32])).unwrap()
    );
    assert_eq!(from_value::<ChangeKind>(value).unwrap(), change);
}

#[test]
fn change_id_hash_matches_independent_python_oracle() {
    let gate = "gate.testnet".parse().unwrap();
    assert_eq!(
        to_value(Hex32(change_id_hash(&gate, 7))).unwrap(),
        json!("0x6b25c9bc80cd8062632f6d4c82c5403e27c6b3228c0f2f9f89bfc7618e08ee78")
    );
    assert_ne!(change_id_hash(&gate, 7), change_id_hash(&gate, 8));
}

#[test]
fn time_conversion_floors_and_checks_overflow() {
    assert_eq!(ns_to_s(999_999_999), 0);
    assert_eq!(ns_to_s(1_999_999_999), 1);
    assert_eq!(ns_to_s(u64::MAX), 18_446_744_073);
    assert_eq!(s_to_ns(0), Some(0));
    assert_eq!(s_to_ns(18_446_744_073), Some(18_446_744_073_000_000_000));
    assert_eq!(s_to_ns(18_446_744_074), None);
    assert_eq!(s_to_ns(u64::MAX), None);
    assert_eq!(TIMELOCK_NS, 604_800_000_000_000);
}

#[test]
fn rule_and_reason_ids_match_the_spec() {
    for id in 0..=6 {
        assert_eq!(RuleId::try_from(id).unwrap() as u8, id);
    }
    assert!(RuleId::try_from(7).is_err());
    let expected = [1, 2, 3, 4, 10, 11, 20, 21, 22, 23, 30, 31, 40, 41, 50, 51];
    for id in 0..=u16::MAX {
        assert_eq!(ReasonCode::try_from(id).is_ok(), expected.contains(&id));
    }
}

#[test]
fn gate_error_codes_are_unique_and_match_every_spec_prefix() {
    let expected = [
        "E10_ZERO_DEPOSIT",
        "E11_INSUFFICIENT_CREDIT",
        "E20_SIGNING_PAUSED",
        "E21_ACTIVE_PROPOSAL_EXISTS",
        "E22_INIT_CALLDATA_TOO_LONG",
        "E23_ZERO_ADDRESS",
        "E24_DUPLICATE_ACTIVE",
        "E30_NOT_FOUND",
        "E31_WRONG_STATE",
        "E32_NOT_EXPIRED",
        "E40_BAD_ENCODING",
        "E41_WRONG_DOMAIN",
        "E42_UNKNOWN_VERIFIER",
        "E43_BAD_SIGNATURE",
        "E44_PROPOSAL_EXPIRED",
        "E45_BINDING_MISMATCH",
        "E46_CERT_TIME",
        "E47_AI_REVIEW_REQUIRED",
        "E48_FEE_CAP",
        "E49_TOO_MANY_BUMPS",
        "E50_NOT_GOVERNOR",
        "E51_INVALID_CHANGE",
        "E52_NOT_FOUND",
        "E53_TIMELOCK_NOT_ELAPSED",
        "E54_NOT_PENDING",
        "E55_SIGNATURES_OUTSTANDING",
        "E56_NOT_GUARDIAN",
        "E57_LAST_VERIFIER",
    ];
    let actual: BTreeSet<_> = GateError::ALL.iter().map(GateError::code).collect();
    assert_eq!(actual.len(), GateError::ALL.len());
    assert_eq!(actual, expected.into_iter().collect());
}

#[test]
fn abi_schemas_use_wire_strings() {
    let schema = serde_json::to_value(schemars::schema_for!(ProposeArgs)).unwrap();
    for field in [
        "new_impl",
        "new_impl_codehash",
        "expected_current_impl",
        "init_calldata",
        "source_commitment",
    ] {
        assert!(schema["properties"][field].get("$ref").is_some());
    }
    assert_eq!(schema["definitions"]["Hex20"]["type"], "string");
    assert_eq!(schema["definitions"]["Hex20"]["minLength"], 42);
    let params = serde_json::to_value(schemars::schema_for!(Params)).unwrap();
    assert_eq!(params["definitions"]["U128Str"]["type"], "string");
    let _ = schemars::schema_for!(Certificate);
    let _ = schemars::schema_for!(Proposal);
    let _ = schemars::schema_for!(PendingChange);
}

#[test]
fn raw_amount_fields_expose_decimal_strings() {
    let params = Params {
        proposal_deposit: u128::MAX,
        proposal_ttl_s: 1,
        max_cert_ttl_s: 2,
        sign_validity_s: 3,
        gas_limit: 4,
        max_fee_per_gas: 5,
        max_priority_fee_per_gas: 6,
        fee_cap: 7,
        sign_gas_tgas: 15,
        callback_gas_tgas: 8,
    };
    let value = to_value(&params).unwrap();
    assert_eq!(
        value["proposal_deposit"],
        json!("340282366920938463463374607431768211455")
    );
    assert_eq!(value["max_fee_per_gas"], json!("5"));
    assert_eq!(value["max_priority_fee_per_gas"], json!("6"));
    assert_eq!(value["fee_cap"], json!("7"));
    assert_eq!(value["gas_limit"], json!(4));
    assert_eq!(from_value::<Params>(value).unwrap(), params);
}

#[test]
fn borsh_enum_tags_follow_spec_variant_order() {
    for (tag, state) in [
        ProposalState::Pending,
        ProposalState::Rejected,
        ProposalState::Expired,
        ProposalState::SignRequested,
        ProposalState::Signed,
        ProposalState::SignFailed,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(borsh::to_vec(&state).unwrap(), vec![tag as u8]);
    }
    for (tag, status) in [
        ChangeStatus::Pending,
        ChangeStatus::Applied,
        ChangeStatus::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(borsh::to_vec(&status).unwrap(), vec![tag as u8]);
    }
    assert_eq!(borsh::to_vec(&AiReview::NotRun).unwrap(), vec![2]);
    assert_eq!(borsh::to_vec(&Verdict::Pass).unwrap(), vec![0]);
    assert_eq!(
        borsh::to_vec(&Verdict::Reject {
            rule_id: 2,
            reason_code: 22
        })
        .unwrap(),
        vec![1, 2, 22, 0]
    );
    assert_eq!(borsh::to_vec(&ChangeKind::Unpause).unwrap(), vec![7]);
    assert_eq!(
        borsh::to_vec(&SignTarget::Proposal(1)).unwrap(),
        vec![0, 1, 0, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        borsh::to_vec(&SignTarget::Change(1)).unwrap(),
        vec![1, 1, 0, 0, 0, 0, 0, 0, 0]
    );
}

use constitution_types::{bundle::SourceBundle, manifest::StageStatus};
use constitution_verifier::{
    manifest::load_manifest,
    rules::{classify_halmos, filter, initializer},
};

#[test]
fn t_rule_3_skips_push_data_but_rejects_opcodes_and_slots() {
    assert_eq!(filter(&[0x60, 0xf4, 0x00], &[0xf4], &[]), Ok(()));
    assert_eq!(filter(&[0xf4], &[0xf4], &[]), Err(30));
    let slot = constitution_types::Hex32([0x11; 32]);
    let mut code = vec![0x7f];
    code.extend(slot.0);
    assert_eq!(filter(&code, &[], &[slot]), Err(31));
    // EVM PUSH immediates are zero-padded beyond end-of-code, not malformed.
    assert_eq!(filter(&[0x7f, 0x11], &[], &[]), Ok(()));
    assert_eq!(filter(&[0x7f, 0xf4], &[0xf4], &[]), Ok(()));
}
#[test]
fn t_rule_4_enforces_selector_length_and_uint_bound() {
    let m: constitution_types::manifest::RuleSetManifest =
        serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
    assert_eq!(initializer(&[], &m.r4), Ok(()));
    assert_eq!(initializer(&[1, 2, 3, 4], &m.r4), Err(40));
    let mut data = vec![0x5b, 0x4e, 0x12, 0x8c];
    data.extend([0u8; 32]);
    assert_eq!(initializer(&data, &m.r4), Ok(()));
    data[20..].fill(0xff);
    assert_eq!(initializer(&data, &m.r4), Ok(()));
    data[4] = 1;
    assert_eq!(initializer(&data, &m.r4), Err(41));
    data.pop();
    assert_eq!(initializer(&data, &m.r4), Err(41));
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/initializers.json")).unwrap();
    for (key, reason) in [("unknown_selector", 40), ("out_of_range_argument", 41)] {
        let bytes: constitution_types::HexBytes =
            serde_json::from_value(fixtures[key].clone()).unwrap();
        assert_eq!(initializer(&bytes.0, &m.r4), Err(reason));
    }
}
#[test]
fn bundle_paths_cannot_escape_or_inject_configuration() {
    for path in [
        "../src/A.sol",
        "src/../A.sol",
        "src\\A.sol",
        "/src/A.sol",
        "foundry.toml",
        "src//A.sol",
    ] {
        let b = SourceBundle {
            contract_name: "A".into(),
            files: vec![(path.into(), vec![])],
            foundry_profile: "default".into(),
        };
        assert!(b.validate().is_err(), "{path}");
    }
}
#[test]
fn solver_unknown_malformed_and_vacuous_results_are_incomplete() {
    for v in [
        serde_json::Value::Null,
        serde_json::json!({"exitcode":0,"test_results":{}}),
    ] {
        assert_eq!(
            classify_halmos(0, &v, "PassVault", "check_solvency(uint128,uint128)"),
            StageStatus::Incomplete
        );
    }
}
#[test]
fn unresolved_r2_manifest_refuses_startup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest.json");
    let mut baseline: serde_json::Value =
        serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
    baseline["r2"] = serde_json::json!({
        "harness_sha256":format!("0x{}","01".repeat(32)),
        "invariant_depth":2,"loop_bound":2,"array_lengths":"xs=2",
        "solver_timeout_assertion_s":10,"fuzz_runs":16,
        "fuzz_seed":format!("0x{}","02".repeat(32))
    });
    std::fs::write(&path, serde_json::to_vec(&baseline).unwrap()).unwrap();
    load_manifest(&path).unwrap();
    for field in [
        "harness_sha256",
        "invariant_depth",
        "loop_bound",
        "array_lengths",
        "solver_timeout_assertion_s",
        "fuzz_runs",
        "fuzz_seed",
    ] {
        let mut json = baseline.clone();
        json["r2"][field] = serde_json::Value::Null;
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(
            load_manifest(&path)
                .unwrap_err()
                .contains("null fields refuse startup")
        );
    }
}

#[test]
fn sealed_plaintext_limit_duplicate_sources_and_symlinks_fail_closed() {
    use constitution_types::bundle::MAX_BUNDLE;
    use constitution_verifier::{
        bundle::{materialize, open},
        identity::Identity,
    };
    let identity = Identity::new();
    assert!(open(&identity.transport, &vec![0; MAX_BUNDLE + 49]).is_err());
    let mut bundle = SourceBundle {
        contract_name: "A".into(),
        files: vec![("src/A.sol".into(), vec![])],
        foundry_profile: "default".into(),
    };
    bundle.files.push(bundle.files[0].clone());
    assert!(bundle.validate().is_err());
    #[cfg(unix)]
    {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("src/link")).unwrap();
        bundle.files = vec![("src/link/A.sol".into(), b"untrusted".to_vec())];
        assert!(materialize(&bundle, root.path()).is_err());
        assert!(!outside.path().join("A.sol").exists());
    }
}

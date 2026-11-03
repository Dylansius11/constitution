#![allow(dead_code)]
#[path = "../src/appraise.rs"]
mod appraise;
#[path = "../src/failure.rs"]
mod failure;

use dcap_qvl::{QuoteCollateralV3, verify::QuoteVerifier};

const NOW: u64 = 1_750_500_000;
fn collateral(name: &str) -> QuoteCollateralV3 {
    serde_json::from_slice(
        &std::fs::read(format!(
            "{}/tests/fixtures/attest/{name}_collateral.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap()
}
fn quote(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/attest/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}
#[test]
fn attest_real_intel_signed_dcap_chain_and_collateral() {
    let checks = appraise::verify_dcap(&quote("tdx_quote"), &collateral("tdx_quote"), NOW);
    assert!(checks.0.ok, "{}", checks.0.detail);
    assert!(checks.1.ok, "{}", checks.1.detail);
}
#[test]
fn attest_tampered_quote_byte_fails_dcap_signature_chain() {
    let mut quote = quote("tdx_quote");
    quote[100] ^= 1;
    let (dcap, _, _) = appraise::verify_dcap(&quote, &collateral("tdx_quote"), NOW);
    assert_eq!(dcap.name, "dcap_signature_chain");
    assert!(!dcap.ok);
}
#[test]
fn attest_non_up_to_date_tcb_fails_by_name() {
    let verified = QuoteVerifier::new_prod()
        .verify(&quote("sgx_quote"), &collateral("sgx_quote"), NOW)
        .unwrap();
    assert_eq!(verified.status, "ConfigurationAndSWHardeningNeeded");
    let status = appraise::tcb_check(&verified.status);
    assert_eq!(status.name, "tcb_up_to_date");
    assert!(!status.ok);
    assert!(status.detail.contains("ConfigurationAndSWHardeningNeeded"));
}
#[test]
fn attest_unmatched_tdx_platform_tcb_fails_closed() {
    let (dcap, status, _) = appraise::verify_dcap(
        &quote("tdx_quote_outdated"),
        &collateral("tdx_quote_outdated"),
        1_771_632_000,
    );
    assert!(!dcap.ok);
    assert!(
        dcap.detail.contains("No matching TCB level found"),
        "{}",
        dcap.detail
    );
    assert!(!status.ok);
}
#[test]
fn attest_expired_collateral_fails_closed() {
    assert!(
        !appraise::verify_dcap(&quote("tdx_quote"), &collateral("tdx_quote"), 2_000_000_000)
            .0
            .ok
    );
}
#[test]
#[ignore = "opt-in network: fetches current collateral from public Phala PCCS"]
fn attest_network_collateral_fetch() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client =
        dcap_qvl::collateral::CollateralClient::with_default_http(appraise::DEFAULT_PCCS_URL)
            .unwrap();
    let collateral = runtime.block_on(client.fetch(&quote("tdx_quote"))).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(
        QuoteVerifier::new_prod()
            .verify(&quote("tdx_quote"), &collateral, now)
            .is_ok()
    );
}

fn simulator() -> (appraise::Identity, dcap_qvl::quote::TDReport10) {
    let identity: appraise::Identity =
        serde_json::from_str(include_str!("fixtures/attest/simulator_identity.json")).unwrap();
    let raw = hex::decode(identity.quote_hex.trim_start_matches("0x")).unwrap();
    let collateral: QuoteCollateralV3 =
        serde_json::from_str(include_str!("fixtures/attest/simulator_collateral.json")).unwrap();
    let verified =
        QuoteVerifier::new(include_bytes!("fixtures/attest/simulator_root.der").to_vec())
            .verify(&raw, &collateral, 1_793_664_000)
            .unwrap();
    assert_eq!(verified.status, "UpToDate");
    (identity, *verified.report.as_td10().unwrap())
}

#[test]
fn attest_simulator_positive_full_key_preimage_rtmr3_compose_release_binding() {
    let (identity, report) = simulator();
    let checks = appraise::bindings(&identity, &report, identity.compose_hash);
    assert!(checks.iter().all(|check| check.ok), "{checks:?}");
}

fn assert_fails(checks: &[appraise::Check], name: &str) {
    let check = checks.iter().find(|check| check.name == name).unwrap();
    assert!(!check.ok, "{checks:?}");
    assert!(check.next.is_some());
}

#[test]
fn attest_wrong_report_data_key_fails_by_name() {
    let (mut identity, report) = simulator();
    identity.verifier_pubkey.0[0] ^= 1;
    assert_fails(&appraise::bindings(&identity, &report, None), "report_data");
}

#[test]
fn attest_event_log_not_replaying_to_rtmr3_fails_by_name() {
    let (identity, mut report) = simulator();
    report.rt_mr3[0] ^= 1;
    assert_fails(
        &appraise::bindings(&identity, &report, None),
        "rtmr3_replay",
    );
}

#[test]
fn attest_event_payload_tampering_cannot_reuse_original_digest() {
    let (mut identity, report) = simulator();
    let mut event: serde_json::Value = serde_json::from_str(&identity.event_log[0]).unwrap();
    event["event_payload"] = serde_json::json!("11".repeat(32));
    identity.event_log[0] = event.to_string();
    assert_fails(
        &appraise::bindings(&identity, &report, None),
        "rtmr3_replay",
    );
}

#[test]
fn attest_compose_hash_mismatch_fails_by_name() {
    let (mut identity, report) = simulator();
    identity.compose_hash.as_mut().unwrap().0[0] ^= 1;
    assert_fails(
        &appraise::bindings(&identity, &report, None),
        "compose_hash",
    );
}

#[test]
fn attest_expected_compose_hash_mismatch_fails_by_name() {
    let (identity, report) = simulator();
    assert_fails(
        &appraise::bindings(&identity, &report, Some(constitution_types::Hex32([0; 32]))),
        "compose_hash",
    );
}

#[test]
fn attest_wrong_release_hash_fails_by_name() {
    let (mut identity, report) = simulator();
    identity.release_hash.as_mut().unwrap().0[0] ^= 1;
    assert_fails(
        &appraise::bindings(&identity, &report, None),
        "release_hash",
    );
}

#[test]
fn attest_simulator_root_is_not_accepted_by_production_appraisal() {
    let (identity, _) = simulator();
    let collateral: QuoteCollateralV3 =
        serde_json::from_str(include_str!("fixtures/attest/simulator_collateral.json")).unwrap();
    assert_fails(
        &appraise::evaluate(&identity, &collateral, 1_793_664_000, None),
        "dcap_signature_chain",
    );
}

#[test]
fn attest_all_non_up_to_date_statuses_are_explicit_failures() {
    for status in [
        "OutOfDate",
        "SWHardeningNeeded",
        "ConfigurationNeeded",
        "ConfigurationAndSWHardeningNeeded",
        "OutOfDateConfigurationNeeded",
        "Revoked",
        "Unknown",
    ] {
        let check = appraise::tcb_check(status);
        assert!(!check.ok);
        assert!(check.detail.contains(status));
    }
}

#[test]
fn attest_compose_argument_is_exactly_32_bytes() {
    assert!(appraise::parse_compose_hash(&format!("0x{}", "ab".repeat(32))).is_ok());
    assert!(appraise::parse_compose_hash("0xab").is_err());
    assert!(appraise::parse_compose_hash(&"zz".repeat(32)).is_err());
}

#[test]
fn attest_preimage_nonce_length_domain_and_quote_padding_are_enforced() {
    for mutate in 0..4 {
        let (mut identity, mut report) = simulator();
        match mutate {
            0 => {
                identity.report_data_preimage.0.pop();
            }
            1 => {
                identity.report_data_preimage.0[0] ^= 1;
            }
            2 => {
                let last = identity.report_data_preimage.0.len() - 1;
                identity.report_data_preimage.0[last] ^= 1;
            }
            _ => {
                report.report_data[63] = 1;
            }
        }
        assert_fails(&appraise::bindings(&identity, &report, None), "report_data");
    }
}

#[test]
fn attest_missing_or_duplicate_compose_event_is_rejected() {
    for duplicate in [false, true] {
        let (mut identity, report) = simulator();
        if duplicate {
            identity.event_log.push(identity.event_log[0].clone());
        } else {
            identity.event_log.clear();
        }
        assert_fails(
            &appraise::bindings(&identity, &report, None),
            "compose_hash",
        );
    }
}

#[test]
fn attest_v2_canonical_preimage_is_bound_to_replayed_event_fields() {
    use sha2::{Digest, Sha384};
    let (mut identity, mut report) = simulator();
    let payload = hex::encode(identity.compose_hash.unwrap().0);
    let preimage =
        format!("{{\"name\":\"compose-hash\",\"payload\":\"{payload}\",\"type\":134217729}}");
    let digest = Sha384::digest(preimage.as_bytes());
    report.rt_mr3 = Sha384::new()
        .chain_update([0; 48])
        .chain_update(digest)
        .finalize()
        .into();
    let mut event = serde_json::json!({"imr":3,"event_type":134217729,"event":"compose-hash","event_payload":payload,"digest":hex::encode(digest),"version":2,"preimage":hex::encode(preimage)});
    identity.event_log[0] = event.to_string();
    assert!(
        appraise::bindings(&identity, &report, None)
            .iter()
            .all(|check| check.ok)
    );
    event["event"] = serde_json::json!("other-name");
    identity.event_log[0] = event.to_string();
    assert_fails(
        &appraise::bindings(&identity, &report, None),
        "rtmr3_replay",
    );
}

#![cfg(unix)]
use constitution_verifier::identity::Identity;
#[path = "fixtures/dstack_socket.rs"]
mod dstack_socket;
use dstack_socket::guest;

#[tokio::test]
async fn attest_guest_socket_serves_bound_keys_measurements_and_events() {
    let (dir, handle) = guest(false);
    let identity = Identity::new()
        .attest(&dir.path().join("dstack.sock"))
        .await
        .unwrap();
    handle.join().unwrap();
    let view = identity.view([7; 32]);
    assert!(!view.quote_hex.is_empty());
    assert!(!view.event_log.is_empty());
    assert_eq!(view.release_hash.unwrap().0, identity.release_hash());
    let raw = hex::decode(view.quote_hex.trim_start_matches("0x")).unwrap();
    let quote = dcap_qvl::quote::Quote::parse(&raw).unwrap();
    let report = quote.report.as_td10().unwrap();
    assert_eq!(report.report_data, identity.report_data());
    use sha2::{Digest, Sha256};
    let expected: [u8; 32] = Sha256::new()
        .chain_update(b"constitution.v1/release")
        .chain_update(report.mr_td)
        .chain_update(report.rt_mr0)
        .chain_update(report.rt_mr1)
        .chain_update(report.rt_mr2)
        .chain_update(view.compose_hash.unwrap().0)
        .finalize()
        .into();
    assert_eq!(identity.release_hash(), expected);
}

#[tokio::test]
async fn attest_guest_wrong_report_data_fails_without_downgrade() {
    let (dir, handle) = guest(true);
    let error = Identity::new()
        .attest(&dir.path().join("dstack.sock"))
        .await
        .err()
        .unwrap();
    handle.join().unwrap();
    assert!(error.contains("does not bind"), "{error}");
}

#[tokio::test]
async fn attest_existing_broken_socket_fails_without_downgrade() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("dstack.sock");
    std::fs::write(&socket, "not a socket").unwrap();
    assert!(Identity::new().attest(&socket).await.is_err());
}

fn pass_certificate(identity: &Identity) -> constitution_types::Certificate {
    use constitution_types::{AiReview, Certificate, DOMAIN, Verdict};
    Certificate {
        domain: DOMAIN,
        gate: "local.testnet".parse().unwrap(),
        proposal_id: 1,
        attempt: 1,
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
        verifier_pubkey: identity.pubkey(),
        verifier_release_hash: identity.release_hash(),
        evidence_block_number: 1,
        evidence_block_hash: [10; 32],
        evidence_root: [11; 32],
        ai: AiReview::Clean {
            evidence_hash: [12; 32],
            signature_kind: 1,
        },
        verdict: Verdict::Pass,
        issued_at_s: 1,
        expires_at_s: 2,
    }
}

#[tokio::test]
async fn simulator_socket_identity_signs_pass_but_dev_identity_still_refuses() {
    // Simulator quote is deliberately rebound by guest(); not real DCAP/TEE evidence.
    let (dir, handle) = guest(false);
    let identity = Identity::new()
        .attest(&dir.path().join("dstack.sock"))
        .await
        .unwrap();
    handle.join().unwrap();
    let certificate = pass_certificate(&identity);
    let (signature, bytes) = identity.sign_certificate(&certificate).unwrap();
    ed25519_dalek::VerifyingKey::from_bytes(&identity.pubkey())
        .unwrap()
        .verify_strict(
            &constitution_verifier::sha256(&bytes),
            &ed25519_dalek::Signature::from_bytes(&signature),
        )
        .unwrap();
    assert_eq!(
        constitution_types::Certificate::from_canonical_bytes(&bytes).unwrap(),
        certificate
    );
    assert_eq!(
        identity.sign_reject(&certificate).unwrap_err(),
        "local verifier never signs Pass"
    );
    let dev = Identity::new();
    assert_eq!(
        dev.sign_certificate(&pass_certificate(&dev)).unwrap_err(),
        "local verifier never signs Pass"
    );
    let mut vetoed = certificate;
    vetoed.ai = constitution_types::AiReview::Vetoed {
        evidence_hash: [1; 32],
        counterexample_hash: [2; 32],
    };
    assert!(identity.sign_certificate(&vetoed).is_err());
}

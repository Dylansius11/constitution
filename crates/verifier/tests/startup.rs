use std::{path::Path, process::Command};
fn startup(args: &[&str], env: &[(&str, &str)]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_constitution-verifier"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .env_clear()
        .envs(env.iter().copied())
        .args(args)
        .output()
        .unwrap();
    assert!(!output.status.success());
    String::from_utf8(output.stderr).unwrap()
}
#[test]
fn startup_names_missing_origin_and_local_configuration() {
    assert!(startup(&[], &[]).contains("PAGE_ORIGIN"));
    assert!(startup(&[], &[("PAGE_ORIGIN", "http://page.local")]).contains("GATE_ACCOUNT_ID"));
    assert!(
        startup(
            &["--insecure-dev-identity"],
            &[("PAGE_ORIGIN", "http://page.local")]
        )
        .contains("LOCAL_PROPOSAL_REGISTRY")
    );
}
#[test]
fn startup_rejects_unrecognized_flags() {
    assert!(startup(&["--production"], &[]).contains("usage:"));
}
#[tokio::test]
async fn chain_identity_requires_attestation_but_explicit_local_mode_can_start() {
    use constitution_verifier::identity::Identity;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("absent.sock");
    let error = Identity::new()
        .for_mode(false, &socket)
        .await
        .err()
        .unwrap();
    assert!(error.starts_with("attestation_required:"));
    let local = Identity::new().for_mode(true, &socket).await.unwrap();
    assert_eq!(local.release_hash(), [0; 32]);
    assert!(local.view([0; 32]).quote_hex.is_empty());
}
#[tokio::test]
async fn broken_present_socket_never_downgrades_in_either_mode() {
    use constitution_verifier::identity::Identity;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("broken.sock");
    std::fs::write(&socket, b"not a socket").unwrap();
    for local in [true, false] {
        assert!(Identity::new().for_mode(local, &socket).await.is_err());
    }
}

#[cfg(unix)]
#[path = "fixtures/dstack_socket.rs"]
mod dstack_socket;
#[cfg(unix)]
#[tokio::test]
async fn chain_mode_accepts_socket_bound_identity_without_local_opt_out() {
    // Protocol simulator only, not DCAP or deployed attestation evidence.
    let (dir, guest) = dstack_socket::guest(false);
    let identity = constitution_verifier::identity::Identity::new()
        .for_mode(false, &dir.path().join("dstack.sock"))
        .await
        .unwrap();
    assert_ne!(identity.release_hash(), [0; 32]);
    guest.join().unwrap();
}

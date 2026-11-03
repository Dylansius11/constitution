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
fn startup_names_missing_chain_and_local_configuration() {
    for (env, missing) in [
        (vec![], "PAGE_ORIGIN"),
        (vec![("PAGE_ORIGIN", "http://page.local")], "NEAR_RPC_URL"),
        (
            vec![
                ("PAGE_ORIGIN", "http://page.local"),
                ("NEAR_RPC_URL", "http://127.0.0.1:1"),
            ],
            "BASE_SEPOLIA_RPC_URL",
        ),
        (
            vec![
                ("PAGE_ORIGIN", "http://page.local"),
                ("NEAR_RPC_URL", "http://127.0.0.1:1"),
                ("BASE_SEPOLIA_RPC_URL", "http://127.0.0.1:2"),
            ],
            "VERIFIER_RELAY_ACCOUNT_ID",
        ),
        (
            vec![
                ("PAGE_ORIGIN", "http://page.local"),
                ("NEAR_RPC_URL", "http://127.0.0.1:1"),
                ("BASE_SEPOLIA_ARCHIVE_RPC_URL", "http://127.0.0.1:2"),
                ("VERIFIER_RELAY_ACCOUNT_ID", "relay.testnet"),
            ],
            "NEAR_NETWORK_ID",
        ),
    ] {
        assert!(
            startup(&[], &env)
                .contains(&format!("missing required environment variable {missing}")),
            "{missing}"
        );
    }
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

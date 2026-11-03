//! Transport failures must never expose endpoint credentials, including receipt JSON paths.
use std::process::Command;

#[test]
fn failing_keyed_rpc_is_absent_from_audit_relay_status_and_receipt_output() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let url =
        format!("http://user:password@127.0.0.1:{port}/FAKE_RPC_API_KEY_53?token=FAKE_QUERY_KEY");
    let temp = tempfile::tempdir().unwrap();
    let deployment = temp.path().join("deployment.json");
    std::fs::write(
        &deployment,
        serde_json::json!({
            "chain_id":84532,"gate":"gate.testnet","gate_code_hash":"committed",
            "mpc_account":"mpc.testnet","executor":"0x1111111111111111111111111111111111111111",
            "proxy":"0x2222222222222222222222222222222222222222","verifiers":[]
        })
        .to_string(),
    )
    .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let commands = vec![
        vec!["audit", "--deployment", deployment.to_str().unwrap()],
        vec!["relay", "0", "--gate", "gate.testnet"],
        vec!["status", "0", "--gate", "gate.testnet"],
        vec!["receipt", "0", "--gate", "gate.testnet", "--json"],
    ];
    for args in commands {
        let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
            .current_dir(&root)
            .args(&args)
            .args(["--near-rpc", &url])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        for secret in [
            &url[..],
            "FAKE_RPC_API_KEY_53",
            "FAKE_QUERY_KEY",
            "password",
            "http://",
        ] {
            assert!(!text.contains(secret), "{args:?} leaked {secret}: {text}");
        }
        assert!(text.contains("next:"), "{text}");
    }
}

#[test]
fn help_never_prints_rpc_defaults_or_keyed_environment_values() {
    for args in [
        vec!["audit", "--help"],
        vec!["relay", "--help"],
        vec!["status", "--help"],
        vec!["receipt", "--help"],
        vec!["propose", "--help"],
        vec!["govern", "--help"],
        vec!["verifier", "appraise", "--help"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
            .args(&args)
            .env("NEAR_RPC_URL", "https://near.example/FAKE_HELP_KEY")
            .env("BASE_SEPOLIA_RPC_URL", "https://evm.example/FAKE_HELP_KEY")
            .env(
                "BASE_SEPOLIA_SEND_URL",
                "https://send.example/FAKE_HELP_KEY",
            )
            .env("VERIFIER_URL", "https://verifier.example/FAKE_HELP_KEY")
            .env("PCCS_URL", "https://pccs.example/FAKE_HELP_KEY")
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            !text.contains("FAKE_HELP_KEY") && !text.contains("https://"),
            "{args:?}: {text}"
        );
    }
}

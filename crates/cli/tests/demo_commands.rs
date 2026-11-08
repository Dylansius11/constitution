//! Filesystem safety through the actual binary, with no network or credentials.
use serde_json::Value;
use std::{path::Path, process::Command};

fn demo(output: &Path, key: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_constitution"))
        .args([
            "demo",
            "upgrade",
            "--bundle",
            "/constitution-missing-bundle",
            "--contract",
            "src/V.sol:V",
            "--impl",
            "0x1111111111111111111111111111111111111111",
            "--account",
            "alice.testnet",
            "--gate",
            "gate.testnet",
            "--network",
            "local",
            "--near-rpc",
            "http://127.0.0.1:1",
            "--evm-rpc",
            "http://127.0.0.1:1",
            "--verifier",
            "http://127.0.0.1:1",
        ])
        .arg("--key-file")
        .arg(key)
        .arg("--output")
        .arg(output)
        .output()
        .unwrap()
}

#[test]
fn failed_preflight_retains_recovery_and_existing_output_is_never_touched() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("attempt");
    let key = temp.path().join("missing-key.json");
    let first = demo(&output, &key);
    assert!(!first.status.success());
    let failure: Value = serde_json::from_slice(
        &std::fs::read(output.join("failure.json")).expect("failed attempt must retain recovery"),
    )
    .unwrap();
    assert!(failure["cause"].is_string());
    assert!(!output.join("submission.json").exists());
    assert!(!output.join("receipt.json").exists());
    let before = std::fs::read(output.join("failure.json")).unwrap();
    std::fs::write(output.join("receipt.json"), b"previous observed evidence").unwrap();
    let second = demo(&output, &key);
    assert!(!second.status.success());
    assert_eq!(std::fs::read(output.join("failure.json")).unwrap(), before);
    assert_eq!(
        std::fs::read(output.join("receipt.json")).unwrap(),
        b"previous observed evidence"
    );
}

#[cfg(unix)]
#[test]
fn make_preserves_literal_output_paths_with_spaces_dollars_and_quotes() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    // Avoid a nested Cargo build; execute the real CLI with the arguments after Cargo's '--'.
    let cargo = bin.join("cargo");
    std::fs::write(
        &cargo,
        "#!/bin/sh\nwhile [ \"$#\" -gt 0 ] && [ \"$1\" != -- ]; do shift; done\n[ \"$#\" -gt 0 ] || exit 2\nshift\nexec \"$CONSTITUTION_TEST_BINARY\" \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = temp.path().join("attempt ' \" $HOME");
    let result = Command::new("make")
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env(
            "CONSTITUTION_TEST_BINARY",
            env!("CARGO_BIN_EXE_constitution"),
        )
        .args([
            "upgrade",
            "BUNDLE=/constitution-missing-bundle",
            "CONTRACT=src/V.sol:V",
            "IMPLEMENTATION=0x1111111111111111111111111111111111111111",
            "ACCOUNT=alice.testnet",
            "GATE=gate.testnet",
            "KEY_FILE=/missing ' \"$HOME.json",
            "NETWORK=local",
            "NEAR_RPC=http://127.0.0.1:1",
            "EVM_RPC=http://127.0.0.1:1",
            "VERIFIER=http://127.0.0.1:1",
        ])
        .arg(format!("OUTPUT={}", output.display()))
        .output()
        .unwrap();
    assert!(!result.status.success());
    let failure: Value = serde_json::from_slice(
        &std::fs::read(output.join("failure.json"))
            .expect("CLI must reserve the literal output path"),
    )
    .unwrap();
    assert!(failure["cause"].is_string());
}

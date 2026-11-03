//! CLI input failures must use the same actionable Failure contract as RPC failures.
use std::process::Command;

#[test]
fn missing_bundle_is_an_actionable_failure_not_an_unknown_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .args([
            "propose",
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
            "--verifier",
            "http://127.0.0.1:1",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("next: "), "{error}");
    assert!(error.contains("bundle"), "{error}");
}

#[test]
fn argument_errors_are_actionable_but_help_is_successful() {
    let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .args(["status", "not-an-id", "--gate", "gate.testnet"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr).unwrap().contains("next: "));
    let help = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .args(["propose", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("--contract"));
    assert!(help.contains("--verifier"));
}

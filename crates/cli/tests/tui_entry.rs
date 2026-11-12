use std::process::{Command, Stdio};

#[test]
fn tui_no_command_without_terminal_prints_help_and_exits_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .stdin(Stdio::null())
        .output()
        .expect("CLI starts");
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("Usage:"));
    assert!(text.contains("audit"));
    assert!(output.stderr.is_empty());
}

#[test]
fn tui_explicit_without_terminal_fails_with_recovery() {
    let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .arg("tui")
        .stdin(Stdio::null())
        .output()
        .expect("CLI starts");
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("must both be terminals"));
    assert!(text.contains("constitution --help"));
}

#[test]
fn tui_help_remains_noninteractive_success() {
    let output = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .expect("CLI starts");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("tui"));
    assert!(text.contains("propose"));
    assert!(text.contains("audit"));
}

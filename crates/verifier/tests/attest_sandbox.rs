#[cfg(unix)]
#[test]
fn attest_tool_sandbox_cannot_read_host_socket_or_secret() {
    use constitution_verifier::tools::Sandbox;
    use std::os::unix::net::UnixListener;
    let outside = tempfile::tempdir().unwrap();
    let socket_path = outside.path().join("dstack.sock");
    let _socket = UnixListener::bind(&socket_path).unwrap();
    let secret_path = outside.path().join("signing-key");
    std::fs::write(&secret_path, b"isolation-test-not-a-real-key").unwrap();
    let sandbox = Sandbox {
        root: tempfile::tempdir().unwrap(),
        repo: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap(),
        image: "constitution-verifier-spike:local".into(),
    };
    let output = sandbox.run(
        "test ! -e /var/run/dstack.sock && test ! -e /run/dstack.sock && test ! -e \"$1\" && test ! -e \"$2\" && test ! -e /var/run/docker.sock && test -z \"$VERIFIER_RELAY_KEY_FILE\"",
        &[socket_path.to_str().unwrap(), secret_path.to_str().unwrap()],
    ).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

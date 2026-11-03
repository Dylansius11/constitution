use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
};

#[test]
fn attest_command_json_failure_is_machine_readable_and_nonzero() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        let size = stream.read(&mut request).unwrap();
        assert!(String::from_utf8_lossy(&request[..size]).starts_with("GET /v1/identity "));
        let body = serde_json::json!({"verifier_pubkey":format!("0x{}", "01".repeat(32)), "transport_pubkey":format!("0x{}", "02".repeat(32)), "release_hash":null, "report_data_preimage":"0x", "quote_hex":"", "event_log":[], "compose_hash":null});
        let body = body.to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    let result = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .args(["verifier", "appraise", &url, "--json"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(!result.status.success());
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(json["ok"], false);
    assert_eq!(json["checks"].as_array().unwrap().len(), 7);
    for check in json["checks"].as_array().unwrap() {
        assert_eq!(check["ok"], false);
        assert!(check["next"].is_string());
    }
    assert!(String::from_utf8_lossy(&result.stderr).contains("next:"));
}

#[test]
fn attest_help_documents_pccs_and_expected_compose_options() {
    let result = Command::new(env!("CARGO_BIN_EXE_constitution"))
        .args(["verifier", "appraise", "--help"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let text = String::from_utf8(result.stdout).unwrap();
    for flag in [
        "--expect-compose-hash",
        "--pccs-url",
        "https://pccs.phala.network",
        "--json",
    ] {
        assert!(text.contains(flag), "{text}");
    }
}

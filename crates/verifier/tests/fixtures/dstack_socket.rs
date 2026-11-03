//! Local protocol simulator only: rebinding this fixture invalidates its original
//! quote signature. This is not DCAP, provider, or deployed TEE evidence.
use std::{
    io::{Read, Write},
    os::unix::net::UnixListener,
};

pub fn guest(tamper: bool) -> (tempfile::TempDir, std::thread::JoinHandle<()>) {
    let dir = tempfile::tempdir().unwrap();
    let listener = UnixListener::bind(dir.path().join("dstack.sock")).unwrap();
    let handle = std::thread::spawn(move || {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../cli/tests/fixtures/attest/simulator_identity.json"
        ))
        .unwrap();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0; 4096];
                let len = stream.read(&mut buf).unwrap();
                assert_ne!(len, 0);
                request.extend_from_slice(&buf[..len]);
                if let Some(start) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..start]).to_ascii_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap_or("0")
                        .parse()
                        .unwrap();
                    if request.len() >= start + 4 + length {
                        break;
                    }
                }
            }
            let body = if request.starts_with(b"POST /GetQuote ") {
                let start = request.windows(4).position(|bytes| bytes == b"\r\n\r\n").unwrap() + 4;
                let data: serde_json::Value = serde_json::from_slice(&request[start..]).unwrap();
                let mut raw = hex::decode(fixture["quote_hex"].as_str().unwrap().trim_start_matches("0x")).unwrap();
                let mut report_data = hex::decode(data["report_data"].as_str().unwrap()).unwrap();
                if tamper { report_data[0] ^= 1; }
                raw[568..632].copy_from_slice(&report_data);
                let events: Vec<serde_json::Value> = fixture["event_log"].as_array().unwrap().iter().map(|v| serde_json::from_str(v.as_str().unwrap()).unwrap()).collect();
                serde_json::json!({"quote":hex::encode(raw), "event_log":serde_json::to_string(&events).unwrap()})
            } else {
                assert!(request.starts_with(b"GET /Info "));
                serde_json::json!({"compose_hash":fixture["compose_hash"].as_str().unwrap().trim_start_matches("0x")})
            }.to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    (dir, handle)
}

use constitution_types::{bundle::SourceBundle, manifest::RuleSetManifest, *};
use constitution_verifier::{
    api::{Service, router},
    identity::Identity,
    pipeline,
    proposals::{LocalProposal, LocalRegistry},
    sha256,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn test_policy() -> RuleSetManifest {
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo().join("rules/manifest.json")).unwrap())
            .unwrap();
    // [I] This policy exercises prechecks only, with a shallow R2 so the HTTP test stays fast;
    // the committed depth-4 R2 is exercised by tests/sandbox.rs.
    json["r2"] = serde_json::json!({"harness_sha256":format!("0x{}","01".repeat(32)),"invariant_depth":2,"loop_bound":2,"array_lengths":"xs=2","solver_timeout_assertion_s":10,"fuzz_runs":16,"fuzz_depth":8,"fuzz_seed":format!("0x{}","02".repeat(32))});
    serde_json::from_value(json).unwrap()
}
async fn request(
    base: &str,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
    origin: &str,
) -> (u16, String, serde_json::Value) {
    let base = base.to_owned();
    let method = method.to_owned();
    let path = path.to_owned();
    let origin = origin.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut cmd = Command::new("curl");
        cmd.args([
            "--max-time",
            "15",
            "-sS",
            "-i",
            "-X",
            &method,
            "-H",
            &format!("Origin: {origin}"),
            "-H",
            "Content-Type: application/json",
            &format!("{base}{path}"),
        ]);
        if body.is_some() {
            cmd.args(["--data-binary", "@-"]);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(body) = body {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(&serde_json::to_vec(&body).unwrap())
                .unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let (headers, body) = text.split_once("\r\n\r\n").unwrap();
        let code = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
        (
            code,
            headers.to_ascii_lowercase(),
            serde_json::from_str(body).unwrap_or(serde_json::Value::String(body.into())),
        )
    })
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_registry_prechecks_cors_and_no_caller_supplied_hashes() {
    let m = test_policy();
    let mut files = vec![];
    for name in ["VaultV1.sol", "VaultV2Honest.sol", "MockToken.sol"] {
        files.push((
            format!("src/demo/{name}"),
            std::fs::read(repo().join(format!("contracts/src/demo/{name}"))).unwrap(),
        ));
    }
    let b = SourceBundle {
        contract_name: "src/demo/VaultV2Honest.sol:VaultV2Honest".into(),
        files,
        foundry_profile: "default".into(),
    };
    let p = LocalProposal {
        gate: "local.testnet".parse().unwrap(),
        proposal_id: 1,
        chain_id: 84532,
        proxy: Hex20([1; 20]),
        executor: Hex20([2; 20]),
        expected_current_impl: Hex20([3; 20]),
        observed_current_impl: Hex20([3; 20]),
        current_impl_codehash: Hex32([0; 32]),
        new_impl: Hex20([4; 20]),
        new_impl_codehash: Hex32([0; 32]),
        init_calldata: HexBytes(vec![]),
        source_commitment: Hex32(sha256(&borsh::to_vec(&b).unwrap())),
        rule_set_hash: Hex32(constitution_types::manifest::rule_set_hash(&m).unwrap()),
        evidence_block_number: 0,
        evidence_block_hash: Hex32([0; 32]),
        expires_at_s: pipeline::now() + 300,
    };
    let dir = tempfile::tempdir().unwrap();
    let registry = dir.path().join("registry.json");
    std::fs::write(&registry, serde_json::to_vec(&vec![p.clone()]).unwrap()).unwrap();
    let service = Service::new(
        Identity::new(),
        m,
        repo(),
        Box::new(LocalRegistry::load(&registry).unwrap()),
    )
    .unwrap();
    let app = router(service, "https://page.local").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (code, headers, identity) =
        request(&base, "GET", "/v1/identity", None, "https://page.local").await;
    assert_eq!(code, 200);
    assert!(headers.contains("access-control-allow-origin: https://page.local"));
    assert!(headers.contains("access-control-expose-headers: retry-after"));
    assert!(!headers.contains("access-control-allow-credentials"));
    assert_eq!(identity["quote_hex"], "");
    assert!(identity["label"].as_str().unwrap().contains("not attested"));
    let key: Hex32 = serde_json::from_value(identity["transport_pubkey"].clone()).unwrap();
    let sealed = crypto_box::PublicKey::from(key.0)
        .seal(
            &mut crypto_box::aead::rand_core::OsRng,
            &borsh::to_vec(&b).unwrap(),
        )
        .unwrap();
    let body =
        serde_json::json!({"gate":p.gate,"proposal_id":1,"sealed_bundle_b64":Base64Bytes(sealed)});
    let mut injected = body.clone();
    injected["new_impl_codehash"] = serde_json::json!(Hex32([9; 32]));
    assert_ne!(
        request(
            &base,
            "POST",
            "/v1/runs",
            Some(injected),
            "https://page.local"
        )
        .await
        .0,
        202
    );
    assert_eq!(request(&base,"POST","/v1/runs",Some(serde_json::json!({"gate":p.gate,"proposal_id":99,"sealed_bundle_b64":body["sealed_bundle_b64"]})),"https://page.local").await.0,404);
    let (code, _, started) =
        request(&base, "POST", "/v1/runs", Some(body), "https://page.local").await;
    assert_eq!(code, 202);
    let id = started["run_id"].as_u64().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    let result = loop {
        let (code, headers, result) = request(
            &base,
            "GET",
            &format!("/v1/runs/{id}"),
            None,
            "https://untrusted.local",
        )
        .await;
        assert_eq!(code, 200);
        assert!(!headers.contains("access-control-allow-origin"));
        if matches!(result["stage"].as_str(), Some("done" | "failed")) {
            break result;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "HTTP run did not complete"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    assert_eq!(result["verdict"]["Reject"]["reason_code"], 2);
    let bytes: Base64Bytes = serde_json::from_value(result["certificate_b64"].clone()).unwrap();
    assert!(matches!(
        Certificate::from_canonical_bytes(&bytes.0).unwrap().verdict,
        Verdict::Reject { reason_code: 2, .. }
    ));
    let (code, _, evidence) = request(
        &base,
        "GET",
        &format!("/v1/runs/{id}/evidence"),
        None,
        "https://page.local",
    )
    .await;
    assert_eq!(code, 200);
    assert_eq!(evidence["r4"]["status"], "Skipped");
    eprintln!(
        "local HTTP runtime mismatch run: {}ms",
        result["elapsed_ms"]
    );
    server.abort();
}

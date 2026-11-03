//! Local NEAR sandbox + anvil + real no-network tool image. No testnet writes.
#[path = "fixtures/dstack_socket.rs"]
mod dstack_socket;
use constitution_types::{bundle::SourceBundle, manifest::rule_set_hash, *};
use constitution_verifier::{
    api::{Service, router},
    chain::GateSource,
    identity::Identity,
    manifest::load_manifest,
    sha256,
    submit::NearSubmitter,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
struct Anvil(Child);
impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn foundry(tool: &str) -> PathBuf {
    Path::new(&std::env::var("HOME").unwrap())
        .join(".foundry/bin")
        .join(tool)
}
fn command(tool: &str, args: &[&str]) -> String {
    let out = Command::new(foundry(tool))
        .args(args)
        .current_dir(repo().join("contracts"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{tool}: {} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}
fn logged_address(text: &str, label: &str) -> [u8; 20] {
    let word = text
        .lines()
        .find_map(|line| {
            let mut p = line.split_whitespace();
            (p.next() == Some(label)).then(|| p.next()).flatten()
        })
        .unwrap_or_else(|| panic!("missing {label}: {text}"));
    serde_json::from_value::<Hex20>(json!(word.to_ascii_lowercase()))
        .unwrap()
        .0
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_r2_fail_certificate_lands_on_real_gate() -> Result<(), Box<dyn std::error::Error>> {
    local_chain_flow(false).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_honest_r1_to_r5_pass_withheld_without_ai() -> Result<(), Box<dyn std::error::Error>>
{
    local_chain_flow(true).await
}
async fn local_chain_flow(honest: bool) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let worker = near_workspaces::sandbox().await?;
    let wasm = std::fs::read(repo().join("target/near/constitution_gate/constitution_gate.wasm"))?;
    let gate = worker.dev_deploy(&wasm).await?;
    let relay = worker.dev_create_account().await?;
    let proposer = worker.dev_create_account().await?;
    // Local socket simulator, not a genuine attested deployment. The gate now
    // refuses a zero release hash even in local sandbox initialization.
    let (quote_dir, quote_handle) = dstack_socket::guest(false);
    let identity = Identity::new()
        .attest(&quote_dir.path().join("dstack.sock"))
        .await?;
    quote_handle
        .join()
        .map_err(|_| "local socket simulator panicked")?;
    let manifest = load_manifest(repo().join("rules/manifest.json"))?;
    let confirmations = manifest.r5.confirmations;
    let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = socket.local_addr()?.port();
    drop(socket);
    let rpc = format!("http://127.0.0.1:{port}");
    let _anvil = Anvil(
        Command::new(foundry("anvil"))
            .args([
                "--silent",
                "--chain-id",
                "84532",
                "--port",
                &port.to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .post(&rpc)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        assert!(Instant::now() < deadline, "anvil startup timeout");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let rpc_deploy = rpc.clone();
    let (config, new_impl, init) = tokio::task::spawn_blocking(move || {
        // Disposable well-known anvil key, never a real wallet.
        let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let output = Command::new(foundry("forge"))
            .args([
                "script",
                "script/Deploy.s.sol",
                "--rpc-url",
                &rpc_deploy,
                "--broadcast",
                "--private-key",
                key,
            ])
            .env("SIGNER", "0x70997970c51812dc3a010c7d01b50e0d17dc79c8")
            .current_dir(repo().join("contracts"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Deploy.s.sol failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let config = Config {
            chain_id: 84532,
            executor: logged_address(&logs, "executor"),
            proxy: logged_address(&logs, "proxy"),
            proxy_admin: logged_address(&logs, "proxyAdmin"),
            derived_eoa: logged_address(&logs, "signer"),
            mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
            mpc_path: "constitution/base-sepolia/v1".into(),
            mpc_domain_id: 0,
        };
        let init = if honest {
            let actor = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
            let token = json!(Hex20(logged_address(&logs, "token")));
            let token = token.as_str().unwrap();
            let proxy = json!(Hex20(config.proxy));
            let proxy = proxy.as_str().unwrap();
            for (to, sig, args) in [
                (token, "mint(address,uint256)", vec![actor, "1000"]),
                (token, "approve(address,uint256)", vec![proxy, "1000"]),
                (proxy, "deposit(uint256)", vec!["100"]),
            ] {
                let mut command_args = vec!["send", to, sig];
                command_args.extend(args);
                command_args.extend(["--rpc-url", &rpc_deploy, "--unlocked", "--from", actor]);
                command("cast", &command_args);
            }
            let text = command("cast", &["calldata", "initializeV2(uint256)", "1000000"]);
            serde_json::from_value::<HexBytes>(json!(text.trim())).unwrap()
        } else {
            HexBytes(vec![])
        };
        let output = command(
            "forge",
            &[
                "create",
                if honest {
                    "src/demo/VaultV2Honest.sol:VaultV2Honest"
                } else {
                    "src/demo/VaultV2Drain.sol:VaultV2Drain"
                },
                "--rpc-url",
                &rpc_deploy,
                "--broadcast",
                "--private-key",
                key,
            ],
        );
        let address = output
            .lines()
            .find_map(|line| line.strip_prefix("Deployed to: "))
            .unwrap()
            .trim();
        let implementation: Hex20 =
            serde_json::from_value(json!(address.to_ascii_lowercase())).unwrap();
        (config, implementation, init)
    })
    .await?;
    let constitution = Constitution {
        rule_set_hash: rule_set_hash(&manifest)?,
        manifest_uri: "local:rules/manifest.json".into(),
        ai_required: true,
        params: Params {
            proposal_deposit: 100,
            proposal_ttl_s: 7200,
            max_cert_ttl_s: manifest.max_cert_ttl_s,
            sign_validity_s: 3600,
            gas_limit: 500000,
            max_fee_per_gas: 10,
            max_priority_fee_per_gas: 1,
            fee_cap: 100,
            sign_gas_tgas: 15,
            callback_gas_tgas: 20,
        },
    };
    gate.call("new").args_json(json!({"config":config,"constitution":constitution,"roles":Roles {governor:proposer.id().clone(),guardian:proposer.id().clone()},"verifiers":[(Base64(identity.pubkey()),VerifierRecord {release_hash:identity.release_hash(),attestation_uri:"local:socket-simulator-not-real-attestation".into(),added_at_ns:0})],"initial_evm_nonce":0})).transact().await?.into_result()?;
    let mut files = vec![];
    for name in [
        "VaultV1.sol",
        "VaultV2Honest.sol",
        "VaultV2Drain.sol",
        "MockToken.sol",
    ] {
        files.push((
            format!("src/demo/{name}"),
            std::fs::read(repo().join(format!("contracts/src/demo/{name}")))?,
        ));
    }
    let bundle = SourceBundle {
        contract_name: if honest {
            "src/demo/VaultV2Honest.sol:VaultV2Honest"
        } else {
            "src/demo/VaultV2Drain.sol:VaultV2Drain"
        }
        .into(),
        files,
        foundry_profile: "default".into(),
    };
    let bytes = borsh::to_vec(&bundle)?;
    let call = |method: &str, params: Value| json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    let code: Value = client
        .post(&rpc)
        .json(&call("eth_getCode", json!([new_impl, "latest"])))
        .send()
        .await?
        .json()
        .await?;
    let code: HexBytes = serde_json::from_value(code["result"].clone())?;
    let slot: Value = client
        .post(&rpc)
        .json(&call(
            "eth_getStorageAt",
            json!([
                Hex20(config.proxy),
                "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                "latest"
            ]),
        ))
        .send()
        .await?
        .json()
        .await?;
    let slot: Hex32 = serde_json::from_value(slot["result"].clone())?;
    let mut current = [0; 20];
    current.copy_from_slice(&slot.0[12..]);
    proposer
        .call(gate.id(), "deposit_credit")
        .args_json(json!({"account":null}))
        .deposit(near_workspaces::types::NearToken::from_yoctonear(100))
        .transact()
        .await?
        .into_result()?;
    let id:u64=proposer.call(gate.id(),"propose").args_json(json!({"args":ProposeArgs {new_impl,new_impl_codehash:Hex32(constitution_verifier::keccak(&code.0)),expected_current_impl:Hex20(current),init_calldata:init,source_commitment:Hex32(sha256(&bytes))}})).transact().await?.into_result()?.json()?;
    // Workspaces transact returns before this proposal reaches finality.
    // GateSource deliberately reads only final views, so advance the local chain.
    worker.fast_forward(3).await?;
    client
        .post(&rpc)
        .json(&call(
            "anvil_mine",
            json!([format!("0x{:x}", manifest.r5.confirmations + 1)]),
        ))
        .send()
        .await?
        .error_for_status()?;
    let credentials = tempfile::tempdir()?;
    let keyfile = credentials.path().join("relay.json");
    std::fs::write(
        &keyfile,
        serde_json::to_vec(
            &json!({"account_id":relay.id(),"public_key":relay.secret_key().public_key().to_string(),"private_key":relay.secret_key().to_string()}),
        )?,
    )?;
    let submitter = NearSubmitter::new(&worker.rpc_addr(), relay.id().as_str(), &keyfile)?;
    let sealed = identity
        .transport
        .public_key()
        .seal(&mut crypto_box::aead::rand_core::OsRng, &bytes)
        .map_err(|_| "cannot seal local test bundle")?;
    let source = GateSource::new(&worker.rpc_addr(), &rpc, &manifest)?;
    let mut service = Service::with_submitter(
        identity,
        manifest,
        repo(),
        Box::new(source),
        Box::new(submitter),
    )?;
    // Explicit local protocol fake: never contact a possibly running funded sidecar.
    struct LocalUnavailableAiFake;
    impl constitution_verifier::ai::Transport for LocalUnavailableAiFake {
        fn review(&self, _: Vec<u8>, _: u32) -> Result<Vec<u8>, &'static str> {
            Err("local protocol fake: unavailable")
        }
    }
    std::sync::Arc::get_mut(&mut service).unwrap().ai_transport = Box::new(LocalUnavailableAiFake);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let app = router(service, "http://page.local")?;
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let response = client
        .post(format!("{base}/v1/runs"))
        .json(&json!({"gate":gate.id(),"proposal_id":id,"sealed_bundle_b64":Base64Bytes(sealed)}))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    assert_eq!(status, 202, "POST /v1/runs: {body}");
    let started: Value = serde_json::from_str(&body)?;
    let run_id = started["run_id"].as_u64().unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    let run = loop {
        let run: Value = client
            .get(format!("{base}/v1/runs/{run_id}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if run["stage"] == "done" {
            break run;
        }
        assert!(Instant::now() < deadline, "verifier timeout: {run}");
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    let evidence: EvidenceManifest = client
        .get(format!("{base}/v1/runs/{run_id}/evidence"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(
        evidence.archive_provider,
        format!(
            "NEAR 127.0.0.1 final + Base 127.0.0.1 at latest-{}",
            confirmations
        )
    );
    let proposal: Value = gate
        .view("get_proposal")
        .args_json(json!({"id":id}))
        .await?
        .json()?;
    if honest {
        for name in [
            "r1_layout",
            "r2_solvency",
            "r3_filter",
            "r4_initializer",
            "r5_replay",
        ] {
            let stage = run["stages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["name"] == name)
                .unwrap();
            assert_eq!(stage["status"], "Pass", "{run}");
            eprintln!("HONEST_CHAIN_STAGE_{name}_MS={}", stage["elapsed_ms"]);
        }
        assert_eq!(
            evidence.r5.status,
            constitution_types::manifest::StageStatus::Pass
        );
        assert_eq!(evidence.r5.detail_code, 0);
        assert_eq!(
            run["outcome"],
            "withheld: AI review incomplete; no Pass without provider_tee Clean"
        );
        let ai_stage = run["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == "ai_review")
            .unwrap();
        assert_eq!(ai_stage["status"], "Incomplete");
        assert!(evidence.ai.candidates.is_empty());
        for field in [
            "verdict",
            "certificate_b64",
            "signature_b64",
            "near_tx_hash",
        ] {
            assert!(run.get(field).is_none(), "{field}: {run}");
        }
        assert_eq!(proposal["state"], "Pending");
        assert!(proposal["rejection"].is_null());
        assert!(proposal["evm"].is_null());
        assert_eq!(
            run["evidence_root"],
            json!(Hex32(constitution_types::manifest::evidence_root(
                &evidence
            )?))
        );
        eprintln!(
            "HONEST_CHAIN_E2E_RUNTIME_MS={}",
            start.elapsed().as_millis()
        );
    } else {
        assert_eq!(
            run["verdict"],
            json!({"Reject":{"rule_id":2,"reason_code":20}}),
            "{run}"
        );
        assert!(!run["near_tx_hash"].as_str().unwrap().is_empty(), "{run}");
        let certificate: Base64Bytes = serde_json::from_value(run["certificate_b64"].clone())?;
        assert_eq!(
            Certificate::from_canonical_bytes(&certificate.0)?.evidence_root,
            constitution_types::manifest::evidence_root(&evidence)?
        );
        assert_eq!(proposal["state"], "Rejected");
        assert_eq!(proposal["rejection"]["rule_id"], 2);
        assert_eq!(proposal["rejection"]["reason_code"], 20);
        assert!(proposal["evm"].is_null());
        eprintln!("LOCAL_CHAIN_E2E_RUNTIME_MS={}", start.elapsed().as_millis());
    }
    let after: Value = client
        .post(&rpc)
        .json(&call(
            "eth_getStorageAt",
            json!([
                Hex20(config.proxy),
                "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                "latest"
            ]),
        ))
        .send()
        .await?
        .json()
        .await?;
    assert_eq!(after["result"], json!(slot));
    if let Some(hash) = run.get("near_tx_hash") {
        eprintln!("LOCAL_CHAIN_E2E_NEAR_TX={hash}");
    }
    eprintln!("LOCAL_CHAIN_E2E_FINAL_PROPOSAL={proposal}");
    server.abort();
    Ok(())
}

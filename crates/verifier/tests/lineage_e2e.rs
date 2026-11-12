//! Local NEAR sandbox + Anvil + real no-network tool image. Never testnet evidence.
//! AI is an explicit unavailable transport; local signer impersonation, not a gate Pass,
//! advances the proxy between withheld honest proposals to exercise the next reference.
use constitution_types::{
    bundle::SourceBundle,
    manifest::{RuleSetManifest, StageStatus, rule_set_hash},
    *,
};
use constitution_verifier::{
    chain::GateSource,
    identity::Identity,
    manifest::{configured_path, load_manifest},
    pipeline::{self, CanaryHealth, CanaryPolicy, Run, RunContext},
    proposals::ProposalSource,
    tools::DockerBackend,
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
    let output = Command::new(foundry(tool))
        .args(args)
        .current_dir(repo().join("contracts"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{tool}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
fn address(text: &str, label: &str) -> Hex20 {
    let value = text
        .lines()
        .find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some(label))
                .then(|| words.next())
                .flatten()
        })
        .unwrap();
    serde_json::from_value(json!(value.to_ascii_lowercase())).unwrap()
}
fn hex<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .into()
}
const KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"; // Public disposable Anvil key.
const SIGNER: &str = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";
const ACTOR: &str = "0x289541ff1cd4882653a02b049ebb5d6a06374826";
struct UnavailableAi;
impl constitution_verifier::ai::Transport for UnavailableAi {
    fn review(&self, _: Vec<u8>, _: u32) -> Result<Vec<u8>, &'static str> {
        Err("local test: AI unavailable")
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn demo_lineage_honest_upgrades_and_gallery_after_v3()
-> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let manifest = load_manifest(configured_path(
        &repo(),
        std::env::var_os("VERIFIER_MANIFEST").as_deref(),
    )?)?;
    assert_eq!(
        manifest.version, 2,
        "run with VERIFIER_MANIFEST=rules/demo/manifest.json"
    );
    let worker = near_workspaces::sandbox().await?;
    let wasm = std::fs::read(repo().join("target/near/constitution_gate/constitution_gate.wasm"))?;
    let gate = worker.dev_deploy(&wasm).await?;
    let identity = Identity::new(); // Explicit NOT ATTESTED identity; no Pass can be issued.
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
        assert!(Instant::now() < deadline, "Anvil startup timeout");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let deploy_rpc = rpc.clone();
    let (config, token, v1, candidates) = tokio::task::spawn_blocking(move || {
        let output = Command::new(foundry("forge"))
            .args([
                "script",
                "script/Deploy.s.sol",
                "--rpc-url",
                &deploy_rpc,
                "--broadcast",
                "--private-key",
                KEY,
            ])
            .env("SIGNER", SIGNER)
            .current_dir(repo().join("contracts"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let config = Config {
            chain_id: 84532,
            executor: address(&logs, "executor").0,
            proxy: address(&logs, "proxy").0,
            proxy_admin: address(&logs, "proxyAdmin").0,
            derived_eoa: address(&logs, "signer").0,
            mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
            mpc_path: "constitution/base-sepolia/v1".into(),
            mpc_domain_id: 0,
            governance_delay_days: DEFAULT_GOVERNANCE_DELAY_DAYS,
        };
        let token = address(&logs, "token");
        let v1 = address(&logs, "vaultV1Implementation");
        for account in [ACTOR, SIGNER] {
            command(
                "cast",
                &[
                    "rpc",
                    "anvil_setBalance",
                    account,
                    "0x56bc75e2d63100000",
                    "--rpc-url",
                    &deploy_rpc,
                ],
            );
            command(
                "cast",
                &[
                    "rpc",
                    "anvil_impersonateAccount",
                    account,
                    "--rpc-url",
                    &deploy_rpc,
                ],
            );
        }
        let proxy = hex(Hex20(config.proxy));
        let token_hex = hex(token);
        for (to, signature, args) in [
            (&token_hex, "mint(address,uint256)", vec![ACTOR, "1000"]),
            (
                &token_hex,
                "approve(address,uint256)",
                vec![proxy.as_str(), "1000"],
            ),
            (&proxy, "deposit(uint256)", vec!["100"]),
        ] {
            let mut command_args = vec!["send", to, signature];
            command_args.extend(args);
            command_args.extend(["--rpc-url", &deploy_rpc, "--unlocked", "--from", ACTOR]);
            command("cast", &command_args);
        }
        let mut candidates = Vec::new();
        for contract in [
            "VaultV2Honest",
            "VaultV3Honest",
            "VaultV2Drain",
            "VaultV2StorageShuffle",
            "VaultV2SelfDestruct",
        ] {
            let name = format!("src/demo/{contract}.sol:{contract}");
            let output = command(
                "forge",
                &[
                    "create",
                    &name,
                    "--rpc-url",
                    &deploy_rpc,
                    "--broadcast",
                    "--private-key",
                    KEY,
                ],
            );
            let deployed = output
                .lines()
                .find_map(|line| line.strip_prefix("Deployed to: "))
                .unwrap();
            candidates.push((
                name,
                serde_json::from_value::<Hex20>(json!(deployed.to_ascii_lowercase())).unwrap(),
            ));
        }
        (config, token, v1, candidates)
    })
    .await?;
    let owner = worker.dev_create_account().await?;
    let constitution = Constitution {
        rule_set_hash: rule_set_hash(&manifest)?,
        manifest_uri: "local:rules/demo/manifest.json".into(),
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
    // Gate initialization needs a nonzero allowlisted release; this dummy is never used to certify.
    gate.call("new").args_json(json!({"config":config,"constitution":constitution,"roles":Roles {governor:owner.id().clone(), guardian:owner.id().clone()},"verifiers":[(Base64([9;32]),VerifierRecord {release_hash:[8;32],attestation_uri:"local:unused-test-identity".into(),added_at_ns:0})],"initial_evm_nonce":0})).transact().await?.into_result()?;
    let mut current = v1;
    let init: HexBytes = serde_json::from_value(json!(
        command("cast", &["calldata", "initializeV2(uint256)", "1000000"]).trim()
    ))?;
    // Drain retains its historic R2 rejection under the v2 manifest at V1.
    let run = propose_and_run(
        &worker,
        &gate,
        &manifest,
        &identity,
        &config,
        &rpc,
        &candidates[2],
        current,
        HexBytes(vec![]),
    )
    .await?;
    assert_reject(&run, 2, 20);
    // The same proxy is advanced only after each honest proposal passes all deterministic rules.
    for (index, calldata) in [(0, init.clone()), (1, HexBytes(vec![]))] {
        let run = propose_and_run(
            &worker,
            &gate,
            &manifest,
            &identity,
            &config,
            &rpc,
            &candidates[index],
            current,
            calldata.clone(),
        )
        .await?;
        assert_honest(&run);
        let upgrade_rpc = rpc.clone();
        let executor = hex(Hex20(config.executor));
        let next = candidates[index].1;
        tokio::task::spawn_blocking(move || {
            // Explicit local-only authority impersonation. No gate certificate, MPC or AI pass.
            command(
                "cast",
                &[
                    "send",
                    &executor,
                    "execute(bytes32,address,bytes32,address,uint64,bytes)",
                    &format!("0x{:064x}", index + 1),
                    &hex(next),
                    &runtime_hash(&upgrade_rpc, next),
                    &hex(current),
                    "18446744073709551615",
                    &hex(calldata),
                    "--rpc-url",
                    &upgrade_rpc,
                    "--unlocked",
                    "--from",
                    SIGNER,
                ],
            );
        })
        .await?;
        current = next;
    }
    // All four gallery attacks still reach their named first failing rule at V3.
    let forbidden: HexBytes = serde_json::from_value(json!(
        command(
            "cast",
            &[
                "calldata",
                "initializeV2(uint256)",
                "340282366920938463463374607431768211456"
            ]
        )
        .trim()
    ))?;
    for (index, calldata, rule, reason) in [
        (2, HexBytes(vec![]), 2, 20),
        (3, HexBytes(vec![]), 1, 10),
        (4, init, 3, 30),
        (0, forbidden, 4, 41),
    ] {
        let run = propose_and_run(
            &worker,
            &gate,
            &manifest,
            &identity,
            &config,
            &rpc,
            &candidates[index],
            current,
            calldata,
        )
        .await?;
        assert_reject(&run, rule, reason);
    }
    let proxy = hex(Hex20(config.proxy));
    let output = command(
        "cast",
        &[
            "call",
            &proxy,
            "remainingCapacity()(uint256)",
            "--rpc-url",
            &rpc,
        ],
    );
    assert_eq!(output.split_whitespace().next(), Some("999900"));
    let assets = command(
        "cast",
        &[
            "call",
            &hex(token),
            "balanceOf(address)(uint256)",
            &proxy,
            "--rpc-url",
            &rpc,
        ],
    );
    assert_eq!(assets.trim(), "100");
    eprintln!("LINEAGE_E2E_TOTAL_MS={}", started.elapsed().as_millis());
    Ok(())
}
fn runtime_hash(rpc: &str, implementation: Hex20) -> String {
    let code = command("cast", &["code", &hex(implementation), "--rpc-url", rpc]);
    let bytes: HexBytes = serde_json::from_value(json!(code.trim())).unwrap();
    hex(Hex32(constitution_verifier::keccak(&bytes.0)))
}
#[allow(clippy::too_many_arguments)]
async fn propose_and_run(
    worker: &near_workspaces::Worker<near_workspaces::network::Sandbox>,
    gate: &near_workspaces::Contract,
    manifest: &RuleSetManifest,
    identity: &Identity,
    config: &Config,
    rpc: &str,
    candidate: &(String, Hex20),
    current: Hex20,
    init: HexBytes,
) -> Result<Run, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let proposer = worker.dev_create_account().await?;
    let gallery: Value =
        serde_json::from_str(include_str!("../../../deployments/demo-gallery.json"))?;
    let source_files = gallery["attacks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|attack| attack["contract"] == candidate.0)
        .map(|attack| {
            attack["source_files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![candidate.0.split(':').next().unwrap().into()]);
    let files = source_files
        .into_iter()
        .map(|file| std::fs::read(repo().join("contracts").join(&file)).map(|bytes| (file, bytes)))
        .collect::<Result<Vec<_>, _>>()?;
    let bundle = SourceBundle {
        contract_name: candidate.0.clone(),
        files,
        foundry_profile: "default".into(),
    };
    let bytes = borsh::to_vec(&bundle)?;
    let entry: String = gate
        .view("get_credit_entry_fee")
        .args_json(json!({"account":proposer.id()}))
        .await?
        .json()?;
    let fee: String = gate.view("get_proposal_fee").await?.json()?;
    proposer
        .call(gate.id(), "deposit_credit")
        .args_json(json!({"account":null}))
        .deposit(near_workspaces::types::NearToken::from_yoctonear(
            100 + entry.parse::<u128>()? + fee.parse::<u128>()?,
        ))
        .transact()
        .await?
        .into_result()?;
    let candidate_hash: Hex32 = serde_json::from_value(json!(runtime_hash(rpc, candidate.1)))?;
    let id:u64 = proposer.call(gate.id(),"propose").args_json(json!({"args":ProposeArgs {new_impl:candidate.1,new_impl_codehash:candidate_hash,expected_current_impl:current,init_calldata:init,source_commitment:Hex32(constitution_verifier::sha256(&bytes))}})).transact().await?.into_result()?.json()?;
    worker.fast_forward(3).await?;
    command("cast", &["rpc", "anvil_mine", "0x3", "--rpc-url", rpc]);
    let source = GateSource::new(&worker.rpc_addr(), rpc, rpc, manifest, gate.id().clone())?;
    let gate_id = gate.id().clone();
    let proposal = tokio::task::spawn_blocking(move || source.get(&gate_id, id))
        .await??
        .ok_or("missing proposal")?;
    assert_eq!(proposal.observed_current_impl, current);
    assert_eq!(proposal.proxy, Hex20(config.proxy));
    let sealed = identity
        .transport
        .public_key()
        .seal(&mut crypto_box::aead::rand_core::OsRng, &bytes)
        .map_err(|_| "seal failed")?;
    let manifest = manifest.clone();
    let rpc = rpc.to_owned();
    // Identity keys are borrowed through a scoped thread; the async runtime continues serving NEAR.
    let run = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                pipeline::execute_observed(
                    &repo(),
                    &manifest,
                    identity,
                    &proposal,
                    &sealed,
                    1,
                    RunContext {
                        archive_provider: "local-anvil",
                        archive_rpc: Some(&rpc),
                        proof_rpc: Some(&rpc),
                        ai_transport: &UnavailableAi,
                        tools: &DockerBackend,
                        canary: &CanaryPolicy::default(),
                        canary_health: &CanaryHealth::default(),
                        prior_failed_runs: 0,
                        deadline: Instant::now() + pipeline::RUN_DEADLINE,
                        observer: None,
                    },
                )
            })
            .join()
            .unwrap()
    });
    let chain_proposal: Value = gate
        .view("get_proposal")
        .args_json(json!({"id":id}))
        .await?
        .json()?;
    assert_eq!(chain_proposal["state"], "Pending"); // No fake verdict is submitted.
    eprintln!(
        "LINEAGE_CASE={} CURRENT={:?} RUNTIME_MS={} STAGES={}",
        candidate.0,
        current,
        started.elapsed().as_millis(),
        serde_json::to_string(&run.stages)?
    );
    Ok(run)
}
fn assert_reject(run: &Run, rule: u8, reason: u16) {
    assert_eq!(
        run.verdict,
        Some(Verdict::Reject {
            rule_id: rule,
            reason_code: reason
        }),
        "{}",
        serde_json::to_string(run).unwrap()
    );
    assert!(run.certificate_b64.is_some());
    for stage in run
        .stages
        .iter()
        .filter(|s| s.name.starts_with('r') && s.name != "reproducing")
    {
        if stage.name.starts_with(&format!("r{rule}_")) {
            break;
        }
        assert_eq!(stage.status, StageStatus::Pass);
    }
}
fn assert_honest(run: &Run) {
    for name in [
        "r1_layout",
        "r2_solvency",
        "r3_filter",
        "r4_initializer",
        "r5_replay",
    ] {
        assert_eq!(
            run.stages.iter().find(|s| s.name == name).map(|s| s.status),
            Some(StageStatus::Pass),
            "{}",
            serde_json::to_string(run).unwrap()
        );
    }
    assert!(run.verdict.is_none());
    assert!(run.certificate_b64.is_none());
    assert_eq!(
        run.stages
            .iter()
            .find(|s| s.name == "ai_review")
            .map(|s| s.status),
        Some(StageStatus::Incomplete)
    );
}

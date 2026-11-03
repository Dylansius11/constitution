use constitution_types::manifest::StageStatus;
use constitution_verifier::replay::{Corpus, EvidenceBlock, fetch_witness, replay};
use revm::primitives::{Address, B256, U256};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str::FromStr,
    time::{Duration, Instant},
};

struct Anvil(Child);
impl Drop for Anvil {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn foundry(name: &str) -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap())
        .join(".foundry/bin")
        .join(name)
}
fn run(cmd: &mut Command) -> String {
    let out = cmd.output().expect("local Foundry binary");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn rpc(url: &str, method: &str, params: Value) -> Value {
    let body: Value = reqwest::blocking::Client::new()
        .post(url)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert!(body.get("error").is_none(), "{method}: {body}");
    body["result"].clone()
}
fn deployed(out: &str, label: &str) -> Address {
    out.lines()
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            (fields.first() == Some(&label)).then(|| Address::from_str(fields[1]).unwrap())
        })
        .unwrap_or_else(|| panic!("missing deployment label {label}"))
}
fn runtime(repo: &Path, contract: &str) -> Vec<u8> {
    let text = run(Command::new(foundry("forge"))
        .current_dir(repo.join("contracts"))
        .args(["inspect", contract, "deployedBytecode"]));
    hex::decode(text.trim().trim_start_matches("0x")).unwrap()
}

fn deploy(repo: &Path, url: &str, actor: &str, contract: &str) -> Address {
    let code = run(Command::new(foundry("forge"))
        .current_dir(repo.join("contracts"))
        .args(["inspect", contract, "bytecode"]));
    let tx = rpc(
        url,
        "eth_sendTransaction",
        json!([{"from":actor, "data":code.trim(), "gas":"0x7a1200"}]),
    );
    let receipt = rpc(url, "eth_getTransactionReceipt", json!([tx]));
    assert_eq!(receipt["status"], "0x1");
    receipt["contractAddress"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn local_vault_replay_accepts_honest_rejects_changed_logs_and_missing_witness() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}");
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
            .spawn()
            .unwrap(),
    );
    let client = reqwest::blocking::Client::new();
    let start = Instant::now();
    while client
        .post(&url)
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}))
        .send()
        .is_err()
    {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "anvil startup timeout"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let actor = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let deployment = run(Command::new(foundry("forge"))
        .current_dir(repo.join("contracts"))
        .env("SIGNER", actor)
        .args([
            "script",
            "script/Deploy.s.sol",
            "--rpc-url",
            &url,
            "--broadcast",
            "--unlocked",
            "--sender",
            actor,
        ]));
    let proxy = deployed(&deployment, "proxy");
    let token = deployed(&deployment, "token");
    let send = |to: Address, sig: &str, args: &[&str]| {
        run(Command::new(foundry("cast"))
            .args(["send", &to.to_string(), sig])
            .args(args)
            .args(["--rpc-url", &url, "--unlocked", "--from", actor]));
    };
    send(token, "mint(address,uint256)", &[actor, "1000"]);
    send(
        token,
        "approve(address,uint256)",
        &[&proxy.to_string(), "1000"],
    );
    send(proxy, "deposit(uint256)", &["100"]);
    let honest_address = deploy(&repo, &url, actor, "VaultV2Honest");
    let negative_address = deploy(&repo, &url, actor, "VaultV2Drain");
    let header = rpc(&url, "eth_getBlockByNumber", json!(["latest", false]));
    let block = EvidenceBlock {
        number: u64::from_str_radix(
            header["number"].as_str().unwrap().trim_start_matches("0x"),
            16,
        )
        .unwrap(),
        hash: header["hash"].as_str().unwrap().parse().unwrap(),
    };
    rpc(&url, "anvil_mine", json!(["0x2"]));
    let corpus = Corpus::from_json(
        &std::fs::read(repo.join("rules/r5/corpus.json")).unwrap(),
        7,
        2,
    )
    .unwrap();
    let current = runtime(&repo, "VaultV1");
    let honest = runtime(&repo, "VaultV2Honest");
    let honest_hash = B256::from(constitution_verifier::keccak(&honest));
    let init = hex::decode(
        run(Command::new(foundry("cast")).args(["calldata", "initializeV2(uint256)", "1000000"]))
            .trim()
            .trim_start_matches("0x"),
    )
    .unwrap();
    let witness = fetch_witness(
        &url,
        block,
        &corpus,
        proxy,
        honest_address,
        honest_hash,
        &init,
    )
    .expect("anvil prestateTracer and state-root proofs");
    assert!(
        fetch_witness(
            &url,
            block,
            &corpus,
            proxy,
            honest_address,
            B256::ZERO,
            &init
        )
        .is_err()
    );
    let start = Instant::now();
    assert_eq!(
        replay(&witness, &corpus, &current, &honest, &init),
        (StageStatus::Pass, 0)
    );
    eprintln!(
        "local honest replay wall time: {:?}; block {}",
        start.elapsed(),
        block.number
    );
    // This neutral R2 Fail fixture is not assumed to be caught by R5.
    let negative_r2 = runtime(&repo, "VaultV2Drain");
    let negative_hash = B256::from(constitution_verifier::keccak(&negative_r2));
    let candidate_witness = fetch_witness(
        &url,
        block,
        &corpus,
        proxy,
        negative_address,
        negative_hash,
        &init,
    )
    .expect("production candidate access discovery");
    let result = replay(&candidate_witness, &corpus, &current, &negative_r2, &init);
    eprintln!("local R2 Fail fixture R5 outcome: {result:?}");
    assert_eq!(result, (StageStatus::Fail, 50));
    let changed = runtime(
        &repo,
        "../crates/verifier/tests/fixtures/ReplayChangedWithdraw.sol:ReplayChangedWithdraw",
    );
    assert_eq!(
        replay(&witness, &corpus, &current, &changed, &[]),
        (StageStatus::Fail, 50)
    );
    let init_writer = runtime(
        &repo,
        "../crates/verifier/tests/fixtures/ReplayChangedWithdraw.sol:ReplayChangesInitialization",
    );
    assert_eq!(
        replay(&witness, &corpus, &current, &init_writer, &[]),
        (StageStatus::Fail, 50)
    );
    let state_writer = runtime(
        &repo,
        "../crates/verifier/tests/fixtures/ReplayChangedWithdraw.sol:ReplayChangesExistingState",
    );
    let bad_init = constitution_verifier::keccak(b"badInit()")[..4].to_vec();
    assert_eq!(
        replay(&witness, &corpus, &current, &state_writer, &bad_init),
        (StageStatus::Fail, 50)
    );
    assert_eq!(
        replay(&witness, &corpus, &current, &current, &[]),
        (StageStatus::Pass, 0)
    );
    assert_eq!(
        replay(&witness, &corpus, &current, &honest, &[]),
        (StageStatus::Fail, 50)
    );
    let mut low_cap = init.clone();
    *low_cap.last_mut().unwrap() = 1;
    low_cap[4..35].fill(0);
    assert_eq!(
        replay(&witness, &corpus, &current, &honest, &low_cap),
        (StageStatus::Fail, 50)
    );
    let mut missing = witness.clone();
    missing.accounts.get_mut(&proxy).unwrap().storage.remove(
        &U256::from_str_radix(
            "199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416802",
            16,
        )
        .unwrap(),
    );
    assert_eq!(
        replay(&missing, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut too_long = corpus.clone();
    too_long.calls.push(too_long.calls[0].clone());
    assert_eq!(
        replay(&witness, &too_long, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut tampered = witness.clone();
    tampered.accounts.get_mut(&proxy).unwrap().code.push(0);
    assert_eq!(
        replay(&tampered, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut tampered = witness.clone();
    tampered.accounts.get_mut(&proxy).unwrap().balance += U256::ONE;
    assert_eq!(
        replay(&tampered, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut tampered_storage = witness.clone();
    let total_slot = U256::from_str_radix(
        "199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416802",
        16,
    )
    .unwrap();
    *tampered_storage
        .accounts
        .get_mut(&proxy)
        .unwrap()
        .storage
        .get_mut(&total_slot)
        .unwrap() += U256::ONE;
    assert_eq!(
        replay(&tampered_storage, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut tampered = witness.clone();
    tampered
        .accounts
        .get_mut(&proxy)
        .unwrap()
        .account_proof
        .clear();
    assert_eq!(
        replay(&tampered, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut tampered_environment = witness.clone();
    tampered_environment.environment.timestamp += U256::ONE;
    assert_eq!(
        replay(&tampered_environment, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut tampered_header = witness.clone();
    tampered_header.header["stateRoot"] = json!(revm::primitives::B256::ZERO);
    assert_eq!(
        replay(&tampered_header, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut missing_account = witness.clone();
    missing_account.accounts.remove(&token);
    assert_eq!(
        replay(&missing_account, &corpus, &current, &honest, &init),
        (StageStatus::Incomplete, 51)
    );
    let mut wrong_hash = block;
    wrong_hash.hash = revm::primitives::B256::ZERO;
    assert!(
        fetch_witness(
            &url,
            wrong_hash,
            &corpus,
            proxy,
            honest_address,
            honest_hash,
            &init
        )
        .is_err()
    );
    let mut unconfirmed = corpus.clone();
    unconfirmed.confirmations = 3;
    assert!(
        fetch_witness(
            &url,
            block,
            &unconfirmed,
            proxy,
            honest_address,
            honest_hash,
            &init
        )
        .is_err()
    );

    // Independently prove this is a useful corpus, not two equally reverting runtimes.
    // Seeded 100 + deposit 10 - withdrawal 4 = 106 raw token units.
    for (index, call) in corpus.calls.iter().enumerate() {
        let request = json!({"from":call.from, "to":proxy, "value":"0x0", "data":call.data, "gas":"0x2dc6c0"});
        if index < 6 {
            let output = rpc(&url, "eth_call", json!([request, "latest"]));
            let expected = match index {
                1 | 2 => format!("0x{:064x}", 110),
                4 | 5 => format!("0x{:064x}", 106),
                _ => "0x".into(),
            };
            assert_eq!(output.as_str().unwrap(), expected, "{}", call.label);
        }
        let tx = rpc(&url, "eth_sendTransaction", json!([request]));
        let receipt = rpc(&url, "eth_getTransactionReceipt", json!([tx]));
        assert_eq!(
            receipt["status"],
            if index == 6 { "0x0" } else { "0x1" },
            "{}",
            call.label
        );
        if index == 0 || index == 3 {
            assert_eq!(
                receipt["logs"].as_array().unwrap().len(),
                2,
                "token Transfer and vault event"
            );
        }
    }
}

#[test]
fn corpus_rejects_invalid_target_empty_and_over_bound() {
    assert!(Corpus::from_json(b"[]", 7, 2).is_err());
    let call = br#"[{"from":"0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266","to":"elsewhere","value":"0","data":"0x","label":"invalid"}]"#;
    assert!(Corpus::from_json(call, 7, 2).is_err());
    assert!(Corpus::from_json(include_bytes!("../../../rules/r5/corpus.json"), 6, 2).is_err());
}

//! Background boundary: existing read modules and the unchanged demo command own protocol work.
//! A child of this same binary isolates demo stdout and gives cancellation a process boundary.
mod models;
use super::app::*;
use crate::{
    audit, executor_build,
    failure::Failure,
    propose, receipt,
    rpc::{Evm, Near, Verifier},
    rules,
};
use constitution_types::{Config, Constitution, Hex20, U128Str};
use models::*;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// Only demo identity/operator constants live here. Final-gate verifier is deliberately never probed.
pub const DEMO_VERIFIER: &str =
    "https://db06dd366642c074483472e3afcb330ff72e4954-8787.dstack-pha1-prod5.phala.network";
pub const DEMO_COMPOSE: &str = "0xf7571e2688238dd87217f2210789e395c505e3e8eecb3ef026c62051feebe79e";
pub const V2_CONTRACT: &str = "src/demo/VaultV2Honest.sol:VaultV2Honest";
pub const V2_INIT: &str =
    "0x5b4e128c00000000000000000000000000000000000000000000000000000000000003e8";
pub const V3_CONTRACT: &str = "src/demo/VaultV3Honest.sol:VaultV3Honest";
pub const NEAR_RPC: &str = "https://near-testnet.drpc.org";
fn near_url() -> String {
    std::env::var("NEAR_RPC_URL").unwrap_or_else(|_| NEAR_RPC.into())
}
fn evm_url() -> String {
    std::env::var("BASE_SEPOLIA_RPC_URL").unwrap_or_else(|_| "https://sepolia.base.org".into())
}
fn paths(profile: Profile) -> (&'static str, &'static str) {
    match profile {
        Profile::Demo => ("deployments/demo.json", "rules/demo/manifest.json"),
        Profile::Final => ("deployments/testnet.json", "rules/manifest.json"),
    }
}
fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}
/// Missing recovery files are expected before their stage. Other read/parse failures
/// stay visible (a create-only file can briefly be incomplete while the child syncs it).
fn artifact(path: &Path, tx: &Sender<Event>) -> Option<Value> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            let _ = tx.send(Event::Log(format!(
                "Artifact read error {}: {e}; inspect storage",
                path.display()
            )));
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(e) => {
            let _ = tx.send(Event::Log(format!(
                "Artifact JSON not yet readable {}: {e}; inspect file if this persists",
                path.display()
            )));
            None
        }
    }
}
fn failure(error: Failure) -> String {
    format!("{}; recovery: {}", error.cause, error.next)
}
pub fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u64::MAX as u128) as u64
}
pub fn now() -> String {
    utc_from_ns(unix_ns())
}
fn appraise_demo() -> AppraisalResult {
    let pccs =
        std::env::var("PCCS_URL").unwrap_or_else(|_| crate::appraise::DEFAULT_PCCS_URL.into());
    let expected =
        crate::appraise::parse_compose_hash(DEMO_COMPOSE).expect("pinned demo compose is 32 bytes");
    let checks = crate::appraise::collect(DEMO_VERIFIER, &pccs, Some(expected))
        .into_iter()
        .map(|c| AppraisalCheck {
            name: c.name.into(),
            ok: c.ok,
            detail: c.detail,
        })
        .collect();
    AppraisalResult {
        checks,
        observed_at: Some(now()),
        recorded: false,
        recorded_date: None,
    }
}
pub fn presets() -> Result<Vec<Preset>, String> {
    let gallery = read_json(Path::new("deployments/demo-gallery.json"))?;
    let mut all = Vec::new();
    for p in gallery["attacks"]
        .as_array()
        .ok_or("gallery attacks missing")?
    {
        let string = |key: &str| {
            p[key]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("gallery attack missing {key}"))
        };
        all.push(Preset {
            id: string("id")?,
            title: string("title")?,
            contract: string("contract")?,
            implementation: string("impl")?,
            init: string("init_calldata")?,
            rule: p["expected_rule"].as_u64(),
            upgrade: false,
            requires_impl: None,
            previous_run: recorded_duration(&p["observed"])?,
        });
    }
    let mut previous = None;
    for p in gallery["upgrades"]
        .as_array()
        .ok_or("gallery upgrades missing")?
    {
        let name = p["to"].as_str().ok_or("gallery upgrade missing to")?;
        let (contract, init) = match name {
            "VaultV2Honest" => (V2_CONTRACT, V2_INIT),
            "VaultV3Honest" => (V3_CONTRACT, "0x"),
            _ => {
                return Err(format!(
                    "unsupported gallery upgrade {name}; add its explicit initializer"
                ));
            }
        };
        let implementation = p["impl"]
            .as_str()
            .ok_or("gallery upgrade missing impl")?
            .to_owned();
        all.push(Preset {
            id: name.into(),
            title: format!("Honest {name}"),
            contract: p["contract"].as_str().unwrap_or(contract).into(),
            implementation: implementation.clone(),
            init: p["init_calldata"].as_str().unwrap_or(init).into(),
            rule: None,
            upgrade: true,
            requires_impl: previous.clone(),
            previous_run: recorded_duration(&p["observed"])?,
        });
        previous = Some(implementation);
    }
    Ok(all)
}
fn page(near: &Near, gate: &str, method: &str) -> Result<Vec<Value>, String> {
    let mut all = Vec::new();
    let mut from = 0u64;
    loop {
        let items: Vec<Value> = near.view(gate, method, json!({"from_id":from,"limit":50}))?;
        if items.is_empty() {
            break;
        }
        for item in &items {
            let id = item["id"]
                .as_u64()
                .ok_or("gate list item has no numeric id")?;
            if id < from {
                return Err("gate returned unordered/repeated ids".into());
            }
            from = id.checked_add(1).ok_or("gate id overflow")?;
        }
        let done = items.len() < 50;
        all.extend(items);
        if done {
            break;
        }
    }
    Ok(all)
}
pub fn snapshot(profile: Profile) -> Result<Snapshot, String> {
    let (deployment, manifest) = paths(profile);
    let d = audit::read_deployment(Path::new(deployment)).map_err(failure)?;
    let gate = d.gate.as_str();
    let near = Near::with_timeout(&near_url(), Duration::from_secs(15))?;
    let evm = Evm::with_timeout(&evm_url(), Duration::from_secs(15))?;
    let mut s = Snapshot {
        gate: gate.into(),
        gate_url: format!("https://testnet.nearblocks.io/address/{gate}"),
        proxy_url: format!(
            "https://sepolia.basescan.org/address/0x{}",
            hex::encode(d.proxy.0)
        ),
        ..Snapshot::default()
    };
    macro_rules! observe {
        ($expr:expr) => {
            match $expr {
                Ok(v) => Some(v),
                Err(e) => {
                    s.errors.push(e.to_string());
                    None
                }
            }
        };
    }
    s.access_keys = observe!(near.access_key_count(gate));
    s.manifest_hash = observe!(
        rules::hash_file(Path::new(manifest))
            .map(|h| format!("0x{}", hex::encode(h)))
            .map_err(failure)
    );
    let constitution: Option<Constitution> =
        observe!(near.view(gate, "get_constitution", json!({})));
    s.rule_hash = constitution.map(|c| format!("0x{}", hex::encode(c.rule_set_hash)));
    let config: Option<Config> = observe!(near.view(gate, "get_config", json!({})));
    s.delay_days = config.as_ref().map(|c| c.governance_delay_days);
    if let Some(chain) = observe!(evm.chain_id()) {
        if chain == d.chain_id {
            s.implementation = observe!(evm.storage_address(d.proxy.0, audit::IMPLEMENTATION_SLOT))
                .map(|a| format!("0x{}", hex::encode(a)));
        } else {
            s.errors.push(format!(
                "EVM RPC chain {chain} differs from deployment {}",
                d.chain_id
            ));
        }
    }
    s.proposals = observe!(page(&near, gate, "list_proposals").and_then(|items| {
        items
            .into_iter()
            .map(|raw| proposal_view(raw, gate))
            .collect()
    }))
    .unwrap_or_default();
    s.changes = observe!(
        page(&near, gate, "list_changes")
            .and_then(|items| items.into_iter().map(change_view).collect())
    )
    .unwrap_or_default();
    if profile == Profile::Final {
        s.verifier = "retired — final CVM deleted; no probe attempted (recorded 2026-11-12)".into();
        s.identity = observe!(retired_identity());
        s.recorded_appraisal = observe!(retired_appraisal());
    } else {
        match Verifier::new(DEMO_VERIFIER).and_then(|v| v.identity()) {
            Ok(identity) => {
                s.verifier =
                    "online — identity endpoint observed, NOT an attestation appraisal".into();
                s.identity = Some(identity_view(&identity, false, Some(now())));
            }
            Err(e) => {
                s.verifier = format!("offline / identity unavailable: {}", failure(e));
            }
        }
    }
    s.observed_at = now();
    Ok(s)
}
fn collect(profile: Profile, id: u64) -> Result<(Value, constitution_types::AccountId), String> {
    let d = audit::read_deployment(Path::new(paths(profile).0)).map_err(failure)?;
    let value = receipt::collect(
        &Near::with_timeout(&near_url(), Duration::from_secs(15))?,
        d.gate.as_str(),
        id,
        &Evm::with_timeout(&evm_url(), Duration::from_secs(15))?,
        "testnet",
        None,
    )
    .map_err(failure)?;
    Ok((value, d.gate))
}
fn audit_profile(profile: Profile) -> Result<AuditResult, String> {
    let (deployment, manifest) = paths(profile);
    let d = audit::read_deployment(Path::new(deployment)).map_err(failure)?;
    let hash = rules::hash_file(Path::new(manifest)).map_err(failure)?;
    let build = executor_build::ExecutorBuild::read(Path::new(executor_build::DEFAULT_ARTIFACT))
        .map_err(failure)?;
    let observed = audit::observe(
        &d,
        &Near::with_timeout(&near_url(), Duration::from_secs(15))?,
        &Evm::with_timeout(&evm_url(), Duration::from_secs(15))?,
        &build,
    )
    .map_err(failure)?;
    let checks: Vec<_> = audit::evaluate(&d, hash, &observed)
        .into_iter()
        .map(|c| AuditCheck {
            name: c.name.into(),
            ok: c.ok,
            detail: c.detail,
            warning: c.warning,
        })
        .collect();
    let exit = u8::from(checks.iter().any(|c| !c.ok && !c.warning));
    Ok(AuditResult {
        checks,
        exit,
        observed_at: now(),
    })
}
fn prepare(w: &Wizard) -> Result<String, String> {
    w.validate(fs::metadata(&w.key_file).is_ok_and(|m| m.is_file()))?;
    let bytes = propose::canonical_bytes(
        &propose::build_bundle(Path::new(&w.bundle), &w.contract).map_err(failure)?,
    )
    .map_err(failure)?;
    let d = audit::read_deployment(Path::new(paths(Profile::Demo).0)).map_err(failure)?;
    let near = Near::with_timeout(&near_url(), Duration::from_secs(15))?;
    let gate = d.gate.as_str();
    let c: Constitution = near.view(gate, "get_constitution", json!({}))?;
    let credit: U128Str = near.view(gate, "get_credit", json!({"account":w.account}))?;
    let fee: U128Str = near.view(gate, "get_proposal_fee", json!({}))?;
    let required = c
        .params
        .proposal_deposit
        .checked_add(fee.0)
        .ok_or("proposal funding overflow")?;
    let mut deposit = required.saturating_sub(credit.0);
    if credit.0 == 0 {
        let entry: U128Str =
            near.view(gate, "get_credit_entry_fee", json!({"account":w.account}))?;
        deposit = deposit
            .max(1)
            .checked_add(entry.0)
            .ok_or("entry funding overflow")?;
    }
    let evm = Evm::with_timeout(&evm_url(), Duration::from_secs(15))?;
    if evm.chain_id()? != d.chain_id {
        return Err("Wrong EVM chain; configure Base Sepolia RPC".into());
    }
    let current = format!(
        "0x{}",
        hex::encode(evm.storage_address(d.proxy.0, audit::IMPLEMENTATION_SLOT)?)
    );
    for p in presets()?
        .iter()
        .filter(|p| p.upgrade && p.implementation == w.implementation)
    {
        match p.availability(Some(&current)) {
            Availability::Done => {
                return Err("Honest upgrade already applied; select the next preset".into());
            }
            Availability::RequiresPrevious => {
                return Err("V3 requires V2 to be the current proxy implementation".into());
            }
            _ => {}
        }
    }
    let address: Hex20 =
        serde_json::from_value(json!(w.implementation)).map_err(|e| e.to_string())?;
    let code = evm.code(address.0)?;
    if code.is_empty() {
        return Err("Candidate implementation has no deployed code".into());
    }
    use sha3::{Digest, Keccak256};
    Ok(format!(
        "Observed at {}\nSign as {} for gate {} (testnet only).\nSource: {} / {}\nSource commitment: 0x{}\nCandidate: {}\nRuntime codehash: 0x{}\nExpected current: {}\nInit: {}\nCurrent active rules: 0x{}\nCredit top-up: {} yoctoNEAR (re-read by demo before signing).\nReservation: {} yoctoNEAR (refundable on settlement).\nProposal storage fee: {} yoctoNEAR (NON-refundable).\nNEAR calls: deposit_credit (only if shortfall, attaches the top-up) and propose (attaches 0), each with 300 Tgas prepaid; actual network gas is additional. No governance/rule changes.\nVerifier may consume funded AI credits; AI only vetoes. {}\nValues are re-read by the existing demo flow; if chain state changes, funding and bindings may change.\nPress y only if authorized to sign and spend; Esc goes back.",
        now(),
        w.account,
        gate,
        w.bundle,
        w.contract,
        hex::encode(constitution_types::certificate_hash(&bytes)),
        w.implementation,
        hex::encode(Keccak256::digest(code)),
        current,
        w.init,
        hex::encode(c.rule_set_hash),
        deposit,
        c.params.proposal_deposit,
        fee.0,
        if w.upgrade {
            "A certified Signed upgrade will be relayed on Base Sepolia and consume the derived EOA's gas."
        } else {
            "Attack mode NEVER relays, even if unexpectedly signed."
        }
    ))
}
#[derive(Default)]
pub struct Workers {
    cancel: Arc<AtomicBool>,
    process: Arc<Mutex<Option<Child>>>,
}
impl Workers {
    pub fn dispatch(&mut self, action: Action, tx: &Sender<Event>) {
        if matches!(action, Action::CancelRun) {
            self.cancel.store(true, Ordering::SeqCst);
            let process = self.process.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                if let Err(error) = stop_child(&process) {
                    let _ = tx.send(Event::Failure(error));
                }
            });
            return;
        }
        let tx = tx.clone();
        if matches!(action, Action::Run(_)) {
            self.cancel = Arc::new(AtomicBool::new(false));
        }
        let cancel = self.cancel.clone();
        let process = self.process.clone();
        thread::spawn(move || {
            let result: Result<Event, String> = match action {
                Action::Refresh(p) => snapshot(p).map(|s| Event::Snapshot(p, s)),
                Action::Appraise(Profile::Demo) => Ok(Event::Appraised(Profile::Demo,appraise_demo())),
                Action::Appraise(Profile::Final) => retired_appraisal().map(|result|Event::Appraised(Profile::Final,result)),
                Action::Audit(p) => Ok(Event::Audited(p, audit_profile(p).unwrap_or_else(|detail| AuditResult {
                    checks: vec![AuditCheck { name: "read_failed".into(), ok: false, warning: false, detail: format!("{detail}; fix the artifact/RPC configuration and run the audit again") }],
                    exit: 1,
                    observed_at: now(),
                }))),
                Action::Detail(p, id) => collect(p,id).and_then(|(raw,gate)|proposal_view(raw,gate.as_str())).map(|v|Event::Detail(p,v)),
                Action::Receipt(p, id) => collect(p,id).map(|(value,_)|Event::Receipt(p,value)),
                Action::Prepare(w) => prepare(&w).map(|text| Event::Prepared(w, text)),
                Action::Run(w) => {
                    let result = run_demo(w, &tx, &cancel, &process);
                    Ok(Event::RunFinished(result))
                }
                Action::SaveReceipt(path, value) => {
                    save_receipt(&path, &value).map(|()| Event::Saved(path))
                }
                Action::CancelRun => return,
            };
            let _ = tx.send(result.unwrap_or_else(Event::Failure));
        });
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        // Never leave a writable child running when terminal setup/rendering fails.
        let _ = stop_child(&self.process);
    }
}
fn stop_child(process: &Mutex<Option<Child>>) -> Result<(), String> {
    let mut slot = process
        .lock()
        .map_err(|_| "Demo process lock poisoned; inspect the existing process")?;
    if let Some(child) = slot.as_mut()
        && child.try_wait().map_err(|e| e.to_string())?.is_none()
    {
        child.kill().map_err(|e| {
            format!("Cannot stop local demo: {e}; inspect existing process and proposal")
        })?;
    }
    Ok(())
}
struct ReapChild<'a>(&'a Mutex<Option<Child>>);
impl Drop for ReapChild<'_> {
    fn drop(&mut self) {
        let _ = stop_child(self.0);
        if let Ok(mut slot) = self.0.lock()
            && let Some(mut child) = slot.take()
        {
            let _ = child.wait();
        }
    }
}
struct RunClock(Arc<AtomicBool>);
impl RunClock {
    fn start(tx: Sender<Event>, start: Instant) -> Self {
        let stopped = Arc::new(AtomicBool::new(false));
        let signal = stopped.clone();
        thread::spawn(move || {
            while !signal.load(Ordering::SeqCst) {
                if tx
                    .send(Event::Tick(start.elapsed().as_millis() as u64))
                    .is_err()
                {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        Self(stopped)
    }
}
impl Drop for RunClock {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
fn save_receipt(path: &str, value: &Value) -> Result<(), String> {
    if path.is_empty() {
        return Err("Enter a new receipt filename; existing files are never overwritten".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Cannot save receipt: {e}; use a new writable path"))?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Receipt storage: {e}; inspect the file before retrying"))
}
fn send_stage(tx: &Sender<Event>, index: usize, state: StepState, detail: String, elapsed: u64) {
    let _ = tx.send(Event::Stage {
        index,
        state,
        detail,
        elapsed_ms: elapsed,
    });
}
fn stage_index(name: &str) -> Option<usize> {
    match name {
        "queued" | "unsealing" | "compiling" | "canary" => Some(1),
        "r1_layout" | "r2_solvency" | "r3_filter" | "r4_initializer" | "r5_replay" => Some(2),
        "ai_review" | "reproducing" => Some(3),
        "signing" => Some(4),
        "submit" | "submitting" => Some(5),
        _ => None,
    }
}
/// Fold real verifier stage records into the horizontal product stages without treating
/// skipped later rules or an optional reproduction stage as successful execution.
fn verifier_steps(run: &Value) -> Vec<(usize, StepState, String, u64)> {
    let mut result = Vec::new();
    for index in 1..=5 {
        let stages: Vec<_> = run["stages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| stage_index(s["name"].as_str().unwrap_or("")) == Some(index))
            .collect();
        let current = run["stage"]
            .as_str()
            .filter(|name| stage_index(name) == Some(index));
        if stages.is_empty() && current.is_none() {
            continue;
        }
        let elapsed = stages.iter().filter_map(|s| s["elapsed_ms"].as_u64()).sum();
        let refused = stages.iter().any(|s| s["status"] == "Fail");
        let error = stages.iter().any(|s| s["status"] == "Incomplete");
        let complete = match index {
            1 => stages
                .iter()
                .any(|s| s["name"] == "compiling" && s["status"] == "Pass"),
            2 => [
                "r1_layout",
                "r2_solvency",
                "r3_filter",
                "r4_initializer",
                "r5_replay",
            ]
            .iter()
            .all(|name| {
                stages
                    .iter()
                    .any(|s| s["name"] == *name && s["status"] == "Pass")
            }),
            3 => stages
                .iter()
                .any(|s| s["name"] == "ai_review" && s["status"] == "Pass"),
            _ => stages.iter().any(|s| s["status"] == "Pass"),
        };
        let state = if refused {
            StepState::Refused
        } else if error {
            StepState::Error
        } else if complete {
            StepState::Pass
        } else if current.is_some() {
            StepState::Active
        } else {
            StepState::NotRun
        };
        let mut detail = stages
            .iter()
            .map(|s| format!("{}: {} — {}", s["name"], s["status"], s["detail"]))
            .collect::<Vec<_>>()
            .join("; ");
        if state == StepState::Active {
            detail.push_str(&format!(
                "; observed active: {}",
                current.unwrap_or("unknown")
            ));
        }
        result.push((index, state, detail, elapsed));
    }
    result
}
fn run_demo(
    w: Wizard,
    tx: &Sender<Event>,
    cancel: &AtomicBool,
    process: &Mutex<Option<Child>>,
) -> Result<(), String> {
    w.validate(fs::metadata(&w.key_file).is_ok_and(|m| m.is_file()))?;
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelled before launching the demo; no child was started".into());
    }
    let deployment = audit::read_deployment(Path::new(paths(Profile::Demo).0)).map_err(failure)?;
    let gate = deployment.gate.as_str();
    fs::create_dir_all("scratch/tui-runs").map_err(|e| e.to_string())?;
    let scenario = if w.upgrade { "upgrade" } else { "attack" };
    let output =
        PathBuf::from("scratch/tui-runs").join(format!("{}-{}", scenario, now().replace(':', "")));
    let _ = tx.send(Event::RunStarted(output.display().to_string()));
    let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
        .args([
            "demo",
            scenario,
            "--bundle",
            &w.bundle,
            "--contract",
            &w.contract,
            "--impl",
            &w.implementation,
            "--init",
            &w.init,
            "--account",
            &w.account,
            "--key-file",
            &w.key_file,
            "--gate",
            gate,
            "--network",
            "testnet",
            "--near-rpc",
            &near_url(),
            "--evm-rpc",
            &evm_url(),
            "--verifier",
            DEMO_VERIFIER,
            "--output",
        ])
        .arg(&output)
        .args(["--watch", "900"])
        .env("EXPECT_COMPOSE_HASH", DEMO_COMPOSE)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start demo: {e}; check executable permissions"))?;
    let start = Instant::now();
    let _clock = RunClock::start(tx.clone(), start);
    send_stage(
        tx,
        0,
        StepState::Active,
        "Collecting source and appraising verifier before NEAR writes".into(),
        0,
    );
    for pipe in [
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let tx = tx.clone();
        thread::spawn(move || {
            for line in BufReader::new(pipe).lines() {
                match line {
                    Ok(line) => {
                        if line.starts_with("depositing credit:")
                            || line.starts_with("paying the non-refundable")
                        {
                            let _ = tx.send(Event::WriteSent);
                        }
                        let _ = tx.send(Event::Log(crate::rpc::strip_urls(&line)));
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Log(format!("Demo output read error: {e}")));
                        break;
                    }
                }
            }
        });
    }
    *process.lock().map_err(|_| "Demo process lock poisoned")? = Some(child);
    let _reap = ReapChild(process);
    let verifier = Verifier::new(DEMO_VERIFIER).map_err(failure)?;
    let near = Near::with_timeout(&near_url(), Duration::from_secs(15))?;
    let mut last_poll = Instant::now() - Duration::from_secs(3);
    let mut stage_started: [Option<Instant>; 10] = [None; 10];
    stage_started[0] = Some(start);
    let mut completed = [false; 10];
    let mut active = 0;
    let mut monitor_error = None;
    loop {
        if cancel.load(Ordering::SeqCst) {
            stop_child(process)?;
            let detail = format!(
                "Stopped local demo process. Any submitted credit/proposal, verifier job, MPC request or Base transaction is NOT undone. Inspect {}/submission.json and use status/receipt; do not repropose blindly.",
                output.display()
            );
            send_stage(
                tx,
                active,
                StepState::Error,
                detail.clone(),
                stage_started[active].map_or(0, |t| t.elapsed().as_millis() as u64),
            );
            return Err(detail);
        }
        if last_poll.elapsed() >= Duration::from_secs(2) {
            last_poll = Instant::now();
            if let Some(submission) = artifact(&output.join("submission.json"), tx) {
                let _ = tx.send(Event::WriteSent);
                if !completed[0] {
                    completed[0] = true;
                    send_stage(
                        tx,
                        0,
                        StepState::Pass,
                        format!(
                            "Proposal {}; NEAR tx {}",
                            submission["proposal_id"], submission["near_propose_tx"]
                        ),
                        start.elapsed().as_millis() as u64,
                    );
                }
                if let Some(id) = submission["proposal_id"].as_u64() {
                    if let Some(run) = artifact(&output.join("run.json"), tx)
                        && let Some(run_id) = run["run_id"].as_str()
                    {
                        if stage_started[1].is_none() {
                            stage_started[1] = Some(Instant::now());
                            send_stage(
                                tx,
                                1,
                                StepState::Active,
                                format!("Verifier accepted run {run_id}"),
                                0,
                            );
                        }
                        match verifier.run(run_id, gate, id) {
                            Ok(observed) => {
                                monitor_error = None;
                                for (index, state, detail, elapsed) in verifier_steps(&observed) {
                                    send_stage(tx, index, state, detail, elapsed);
                                    if state == StepState::Active {
                                        active = index;
                                    }
                                }
                                if let Err(e) = crate::status::run_complete(&observed) {
                                    monitor_error = Some(failure(e));
                                }
                            }
                            Err(e) => {
                                let message = failure(e);
                                let _ = tx.send(Event::Log(format!(
                                    "Stage observation error: {message}"
                                )));
                                monitor_error = Some(message);
                            }
                        }
                    }
                    match receipt::read_proposal(&near, gate, id) {
                        Ok(p) => {
                            use constitution_types::ProposalState;
                            if p.state == ProposalState::Rejected
                                && let Some(r) = p.rejection
                            {
                                send_stage(
                                    tx,
                                    5,
                                    StepState::Refused,
                                    format!(
                                        "R{} / {}; evidence 0x{}",
                                        r.rule_id,
                                        r.reason_code,
                                        hex::encode(r.evidence_root)
                                    ),
                                    start.elapsed().as_millis() as u64,
                                );
                            }
                            if matches!(
                                p.state,
                                ProposalState::Signed
                                    | ProposalState::SignRequested
                                    | ProposalState::SignFailed
                            ) {
                                send_stage(
                                    tx,
                                    5,
                                    StepState::Pass,
                                    "Gate accepted Pass certificate".into(),
                                    0,
                                );
                                let state = match p.state {
                                    ProposalState::Signed => StepState::Pass,
                                    ProposalState::SignFailed => StepState::Error,
                                    _ => StepState::Active,
                                };
                                stage_started[6].get_or_insert(Instant::now());
                                send_stage(
                                    tx,
                                    6,
                                    state,
                                    format!("{:?}", p.state),
                                    stage_started[6].unwrap().elapsed().as_millis() as u64,
                                );
                                active = if p.state == ProposalState::Signed && w.upgrade {
                                    7
                                } else {
                                    6
                                };
                                if active == 7 {
                                    stage_started[7].get_or_insert(Instant::now());
                                    send_stage(tx,7,StepState::Active,"Demo relay: sender/transaction checks, broadcast, inclusion".into(),stage_started[7].unwrap().elapsed().as_millis() as u64);
                                }
                            }
                        }
                        Err(e) => {
                            let message = failure(e);
                            let _ =
                                tx.send(Event::Log(format!("Gate observation error: {message}")));
                            monitor_error = Some(message);
                        }
                    }
                }
            }
        }
        let status = process
            .lock()
            .map_err(|_| "Demo process lock poisoned")?
            .as_mut()
            .ok_or("Demo process missing")?
            .try_wait()
            .map_err(|e| e.to_string())?;
        if let Some(status) = status {
            let observed = read_json(&output.join("receipt.json"));
            if let Err(error) = &observed {
                let _ = tx.send(Event::Log(format!(
                    "Receipt artifact unavailable: {error}; inspect recovery artifacts"
                )));
            }
            if let Ok(value) = &observed {
                send_stage(
                    tx,
                    8,
                    StepState::Pass,
                    "receipt.json retained from observed public state".into(),
                    0,
                );
                if value["evm"]["status"] == "included" {
                    send_stage(
                        tx,
                        7,
                        StepState::Pass,
                        format!("Base transaction {} included", value["evm"]["tx_hash"]),
                        stage_started[7].map_or(0, |t| t.elapsed().as_millis() as u64),
                    );
                }
                if value["verdict"]["kind"] == "Reject" {
                    send_stage(
                        tx,
                        5,
                        StepState::Refused,
                        format!(
                            "Rule {} / reason {}",
                            value["verdict"]["rule_id"], value["verdict"]["reason_code"]
                        ),
                        0,
                    );
                }
            }
            if status.success() {
                observed.as_ref().map_err(|e| format!("Demo exited 0 but receipt artifact is unreadable: {e}; inspect storage before claiming success"))?;
                if let Some(e) = monitor_error {
                    return Err(format!(
                        "Demo exited 0, but TUI observation failed: {e}; inspect retained receipt"
                    ));
                }
                send_stage(
                    tx,
                    9,
                    StepState::Pass,
                    "Existing demo verified the scenario outcome".into(),
                    start.elapsed().as_millis() as u64,
                );
                return Ok(());
            }
            let message = match read_json(&output.join("failure.json")) {
                Ok(v) => format!("{}; recovery: {}", v["cause"], v["next"]),
                Err(e) => format!(
                    "Demo exited {status}; {e}; inspect the inline log and output directory before retrying"
                ),
            };
            send_stage(
                tx,
                active,
                StepState::Error,
                message.clone(),
                stage_started[active].map_or(0, |t| t.elapsed().as_millis() as u64),
            );
            return Err(message);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tui_v2_initializer_is_abi_encoding() {
        let mut bytes = crate::rpc::selector("initializeV2(uint256)").to_vec();
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&1000u64.to_be_bytes());
        assert_eq!(V2_INIT, format!("0x{}", hex::encode(bytes)));
    }

    #[test]
    fn tui_rule_refusal_is_not_overwritten_by_skipped_rules() {
        let run = json!({"stage":"done","stages":[
            {"name":"r1_layout","status":"Pass","detail":"layout checked","elapsed_ms":5},
            {"name":"r2_solvency","status":"Fail","detail":"reason 20","elapsed_ms":100},
            {"name":"r3_filter","status":"Skipped","detail":"earlier refusal","elapsed_ms":0},
            {"name":"ai_review","status":"NotRun","detail":"deterministic refusal","elapsed_ms":0}
        ]});
        let folded = verifier_steps(&run);
        assert_eq!(folded[0].0, 2);
        assert_eq!(folded[0].1, StepState::Refused);
        assert_eq!(folded[0].3, 105);
        assert_eq!(folded[1].1, StepState::NotRun);
    }

    #[test]
    fn tui_partial_rules_never_display_pass() {
        let run = json!({"stage":"r2_solvency","stages":[
            {"name":"r1_layout","status":"Pass","detail":"layout checked","elapsed_ms":5}
        ]});
        let folded = verifier_steps(&run);
        assert_eq!(folded[0].1, StepState::Active);
    }

    #[test]
    fn tui_ai_clean_without_reproduction_still_records_observed_review() {
        let run = json!({"stage":"done","stages":[
            {"name":"ai_review","status":"Pass","detail":"provider_tee","elapsed_ms":100},
            {"name":"reproducing","status":"NotRun","detail":"no candidates","elapsed_ms":0}
        ]});
        let folded = verifier_steps(&run);
        assert_eq!(folded[0].1, StepState::Pass);
        assert_eq!(folded[0].3, 100);
    }

    #[test]
    fn tui_receipt_save_is_create_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("receipt.json");
        let path = path.to_str().unwrap();
        save_receipt(path, &json!({"schema":"constitution.receipt/1"})).unwrap();
        assert!(save_receipt(path, &json!({"overwritten":true})).is_err());
        assert_eq!(
            read_json(Path::new(path)).unwrap()["schema"],
            "constitution.receipt/1"
        );
    }
}

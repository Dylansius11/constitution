//! Demo orchestration keeps durable recovery separate from observed chain receipts.
use crate::{
    failure::Failure,
    near_tx::Writer,
    propose, receipt, relay,
    rpc::{Evm, Near, Verifier},
    status,
};
use clap::{Args, ValueEnum};
use constitution_types::ProposalState;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Scenario {
    Attack,
    Upgrade,
}

/// Deliberately explicit: demo writes never inherit endpoint or wallet defaults.
#[derive(Args)]
pub struct DemoArgs {
    #[arg(value_enum)]
    pub scenario: Scenario,
    #[arg(long)]
    pub bundle: PathBuf,
    #[arg(long)]
    pub contract: String,
    #[arg(long = "impl")]
    pub implementation: String,
    #[arg(long, default_value = "0x")]
    pub init: String,
    #[arg(long)]
    pub account: String,
    #[arg(long)]
    pub gate: String,
    #[arg(long)]
    pub key_file: PathBuf,
    #[arg(long, value_parser = ["local", "testnet"])]
    pub network: String,
    #[arg(long)]
    pub near_rpc: String,
    #[arg(long)]
    pub evm_rpc: String,
    /// Optional separate transaction broadcaster; defaults to the explicit --evm-rpc.
    #[arg(long)]
    pub send_rpc: Option<String>,
    #[arg(long)]
    pub verifier: String,
    /// New directory, whose parent must already exist. Existing paths are never reused.
    #[arg(long)]
    pub output: PathBuf,
    /// Maximum polling time per gate/relay stage, not an HTTP or transaction deadline.
    #[arg(long, default_value_t = 600)]
    pub watch: u64,
}

fn storage_error(error: std::io::Error) -> Failure {
    Failure::new(
        format!("demo evidence storage: {error}"),
        "inspect the output directory and existing submission before retrying; never rerun a write blindly",
    )
}

fn save(dir: &Path, name: &str, value: &Value) -> Result<(), Failure> {
    let bytes = serde_json::to_vec_pretty(value).expect("JSON Value is serializable");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(dir.join(name)).map_err(storage_error)?;
    file.write_all(&bytes).map_err(storage_error)?;
    file.sync_all().map_err(storage_error)
}

pub fn run(args: DemoArgs) -> Result<(), Failure> {
    let mut directory = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(&args.output).map_err(storage_error)?;
    println!("demo evidence: {}", args.output.display());
    let near = Near::new(&args.near_rpc);
    let evm = Evm::new(&args.evm_rpc);
    let mut accepted: Option<propose::Registered> = None;
    let mut run_id = None;
    let mut verifier = None;
    let operation = (|| {
        save(
            &args.output,
            "attempt.json",
            &json!({
                "schema":"constitution.demo/1", "scenario":format!("{:?}", args.scenario),
                "network":args.network, "gate":args.gate,
            }),
        )?;
        let bytes =
            propose::canonical_bytes(&propose::build_bundle(&args.bundle, &args.contract)?)?;
        let writer = Writer::new(&args.near_rpc, &args.account, &args.key_file)?;
        verifier = Some(Verifier::new(&args.verifier)?);
        let verifier = verifier.as_ref().expect("initialized verifier");
        accepted = Some(propose::propose(
            propose::Input {
                bytes: &bytes,
                implementation: &args.implementation,
                init: &args.init,
                account: &args.account,
            },
            &args.gate,
            &writer,
            &near,
            &evm,
            verifier,
        )?);
        let registered = accepted.as_ref().expect("accepted proposal");
        // Durable identifiers precede the verifier POST. Never automatically repropose.
        save(
            &args.output,
            "submission.json",
            &json!({
                "proposal_id":registered.id, "near_propose_tx":registered.tx_hash,
            }),
        )?;
        run_id = Some(verifier.start(&args.gate, registered.id, &registered.sealed)?);
        let run_id = run_id.as_deref().expect("started run");
        save(&args.output, "run.json", &json!({"run_id":run_id}))?;
        println!("verifier run: {run_id}");
        wait(&args, registered.id, &near, &evm, verifier, run_id)
    })();

    // Even a failed/uncertain POST or timed-out watch retains public observations when available.
    let observed = accepted.as_ref().map(|registered| {
        let run = verifier.as_ref().zip(run_id.as_deref());
        receipt::collect(&near, &args.gate, registered.id, &evm, &args.network, run)
    });
    let mut receipt_error = None;
    let observed = match observed {
        Some(Err(error)) => {
            receipt_error = Some(error);
            accepted.as_ref().and_then(|registered| {
                receipt::collect(&near, &args.gate, registered.id, &evm, &args.network, None).ok()
            })
        }
        Some(Ok(value)) => Some(value),
        None => None,
    };
    let receipt_result = if let Some(mut value) = observed {
        if let Some(registered) = &accepted {
            value["near"]["propose_tx"] = json!(registered.tx_hash);
        }
        let outcome = verify_outcome(args.scenario, &value);
        save(&args.output, "receipt.json", &value).and(outcome)
    } else {
        Err(Failure::new(
            "no complete public receipt was available",
            "inspect submission.json if present, then read the existing proposal with status and receipt",
        ))
    };
    let result = operation
        .and(
            receipt_error
                .as_ref()
                .map_or(Ok(()), |e| Err(Failure::new(&e.cause, &e.next))),
        )
        .and(receipt_result);
    if let Err(error) = &result {
        save(
            &args.output,
            "failure.json",
            &json!({
                "cause":error.cause, "next":error.next,
                "receipt_error":receipt_error.map(|e| e.cause),
            }),
        )?;
        eprintln!(
            "recovery: {}; inspect the existing proposal, do not automatically repropose",
            args.output.display()
        );
    } else {
        println!(
            "verified demo outcome; receipt: {}",
            args.output.join("receipt.json").display()
        );
    }
    result
}

#[derive(Debug, PartialEq)]
enum Next {
    Wait,
    Rejected,
    Relay,
}

fn next(scenario: Scenario, state: &ProposalState, run: &Value) -> Result<Next, Failure> {
    if !status::run_complete(run)? {
        return Ok(Next::Wait);
    }
    match state {
        ProposalState::Rejected => Ok(Next::Rejected),
        ProposalState::Signed => match scenario {
            Scenario::Attack => Err(Failure::new(
                "attack proposal unexpectedly reached Signed; not relaying",
                "inspect the accepted certificate and gate; preserve this evidence",
            )),
            Scenario::Upgrade => Ok(Next::Relay),
        },
        ProposalState::Expired | ProposalState::SignFailed => Err(Failure::new(
            format!("proposal stopped in {state:?}"),
            "inspect the proposal; do not automatically repropose or retry signing",
        )),
        _ => Ok(Next::Wait),
    }
}

fn wait(
    args: &DemoArgs,
    id: u64,
    near: &Near,
    evm: &Evm,
    verifier: &Verifier,
    run_id: &str,
) -> Result<(), Failure> {
    let start = Instant::now();
    loop {
        let proposal = receipt::read_proposal(near, &args.gate, id)?;
        let run = verifier.run(run_id)?;
        match next(args.scenario, &proposal.state, &run)? {
            Next::Rejected => return Ok(()),
            Next::Relay => {
                return relay::relay(
                    near,
                    &args.gate,
                    relay::Target::Proposal(id),
                    &Evm::new(args.send_rpc.as_deref().unwrap_or(&args.evm_rpc)),
                    evm,
                    Some(Duration::from_secs(args.watch)),
                );
            }
            Next::Wait => {}
        }
        println!(
            "proposal {id}: {:?}; verifier stage: {}",
            proposal.state, run["stage"]
        );
        let remaining = Duration::from_secs(args.watch).saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(Failure::new(
                format!("proposal {id} did not settle within {}s", args.watch),
                "read the existing proposal with status and receipt; do not create a duplicate",
            ));
        }
        std::thread::sleep(Duration::from_secs(2).min(remaining));
    }
}

fn verify_outcome(scenario: Scenario, observed: &Value) -> Result<(), Failure> {
    let proposal = &observed["proposal"];
    let after = observed["proxy"]["implementation_after"].as_str();
    let matches_address = |field: &str| {
        proposal[field]
            .as_str()
            .is_some_and(|expected| Some(expected) == after)
    };
    let valid = match scenario {
        Scenario::Attack => {
            proposal["state"] == "Rejected"
                && proposal["evm"].is_null()
                && observed["verdict"]["kind"] == "Reject"
                && observed["evm"]["status"] == "not_signed"
                && matches_address("expected_current_impl")
        }
        Scenario::Upgrade => {
            proposal["state"] == "Signed"
                && observed["evm"]["status"] == "included"
                && matches_address("new_impl")
        }
    };
    if valid {
        Ok(())
    } else {
        Err(Failure::new(
            format!("{scenario:?} outcome was not proven by the observed receipt"),
            "inspect receipt.json and the existing proposal; missing, withheld or divergent state is not success",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn observed(state: &str, evm_status: &str, after: &str) -> serde_json::Value {
        json!({
            "schema":"constitution.receipt/1",
            "proposal":{"state":state,"new_impl":"0x0202020202020202020202020202020202020202","expected_current_impl":"0x0101010101010101010101010101010101010101","evm":null},
            "verdict":{"kind":if state == "Rejected" {"Reject"} else {"Pass"}},
            "evm":{"status":evm_status},
            "proxy":{"implementation_after":after,"observed_at_block":10},
            "labels":{"network":"local"}
        })
    }

    #[test]
    fn attack_requires_rejection_without_any_signing_and_unchanged_proxy() {
        let old = "0x0101010101010101010101010101010101010101";
        assert!(verify_outcome(Scenario::Attack, &observed("Rejected", "not_signed", old)).is_ok());
        for state in [
            "Pending",
            "SignRequested",
            "SignFailed",
            "Signed",
            "Expired",
        ] {
            assert!(verify_outcome(Scenario::Attack, &observed(state, "not_signed", old)).is_err());
        }
        let mut signed = observed("Rejected", "not_signed", old);
        signed["proposal"]["evm"] = json!({"nonce":0});
        assert!(verify_outcome(Scenario::Attack, &signed).is_err());
        assert!(
            verify_outcome(
                Scenario::Attack,
                &observed(
                    "Rejected",
                    "not_signed",
                    "0x0202020202020202020202020202020202020202"
                )
            )
            .is_err()
        );
    }

    #[test]
    fn upgrade_requires_inclusion_and_candidate_readback_not_merely_signed() {
        let candidate = "0x0202020202020202020202020202020202020202";
        assert!(
            verify_outcome(
                Scenario::Upgrade,
                &observed("Signed", "included", candidate)
            )
            .is_ok()
        );
        for status in ["not_signed", "signed", "pending", "reverted"] {
            assert!(
                verify_outcome(Scenario::Upgrade, &observed("Signed", status, candidate)).is_err()
            );
        }
        assert!(
            verify_outcome(
                Scenario::Upgrade,
                &observed(
                    "Signed",
                    "included",
                    "0x0101010101010101010101010101010101010101"
                )
            )
            .is_err()
        );
        assert!(
            verify_outcome(
                Scenario::Upgrade,
                &observed("Pending", "included", candidate)
            )
            .is_err()
        );
    }

    #[test]
    fn settled_gate_never_bypasses_failed_withheld_or_uncertain_verifier_run() {
        let incomplete = json!({"stage":"done","started_at":1,"certificate_b64":"retained",
            "stages":[{"name":"submit","status":"Incomplete","detail":"response lost","elapsed_ms":1}]});
        let failed =
            json!({"stage":"failed","started_at":1,"stages":[],"error_code":"rpc_unavailable"});
        let withheld = json!({"stage":"done","started_at":1,"stages":[]});
        for run in [incomplete, failed, withheld] {
            assert!(next(Scenario::Upgrade, &ProposalState::Signed, &run).is_err());
            assert!(next(Scenario::Attack, &ProposalState::Rejected, &run).is_err());
        }
        let submitting =
            json!({"stage":"submitting","started_at":1,"stages":[],"certificate_b64":"retained"});
        assert_eq!(
            next(Scenario::Upgrade, &ProposalState::Signed, &submitting).unwrap(),
            Next::Wait
        );
        let done = json!({"stage":"done","started_at":1,"stages":[],"certificate_b64":"retained"});
        assert_eq!(
            next(Scenario::Upgrade, &ProposalState::Pending, &done).unwrap(),
            Next::Wait
        );
        assert_eq!(
            next(Scenario::Upgrade, &ProposalState::Signed, &done).unwrap(),
            Next::Relay
        );
        assert_eq!(
            next(Scenario::Attack, &ProposalState::Rejected, &done).unwrap(),
            Next::Rejected
        );
    }
}

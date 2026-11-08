//! Read-only gate and verifier status, with bounded two-second polling.
use crate::{
    failure::Failure,
    receipt,
    rpc::{Evm, Near, Verifier},
};
use constitution_types::ProposalState;
use std::time::{Duration, Instant};

/// Shared run policy: a settled gate never turns an incomplete verifier run into success.
pub fn run_complete(run: &serde_json::Value) -> Result<bool, Failure> {
    let stages = run["stages"].as_array().ok_or_else(|| {
        Failure::new(
            "verifier returned no stages",
            "inspect the verifier; incomplete is not a pass",
        )
    })?;
    if run["stage"] == "failed" {
        return Err(Failure::verifier_failed(
            run["error_code"].as_str().unwrap_or(""),
        ));
    }
    if run["stage"] != "done" {
        return Ok(false);
    }
    if stages
        .iter()
        .any(|stage| stage["name"] == "submit" && stage["status"] == "Incomplete")
    {
        return Err(Failure::new(
            "verifier submission incomplete; certificate retained; the submission may have landed",
            "inspect relay transactions before retrying; check the gate with `constitution status <id>`; do not automatically resubmit or start another run",
        ));
    }
    if run
        .get("certificate_b64")
        .and_then(serde_json::Value::as_str)
        .is_none()
    {
        return Err(Failure::retry_verifier(
            "verifier withheld: AI review incomplete; no certificate; the proposal is still Pending",
            10,
        ));
    }
    Ok(true)
}

pub fn status(
    near: &Near,
    gate: &str,
    id: u64,
    evm: &Evm,
    network: &str,
    run: Option<(&Verifier, &str)>,
    watch: Option<Duration>,
) -> Result<(), Failure> {
    let start = Instant::now();
    loop {
        let proposal = receipt::read_proposal(near, gate, id)?;
        let run_value = run.map(|(client, run_id)| client.run(run_id)).transpose()?;
        let observed = receipt::observe(near, gate, &proposal, evm, network, run_value.as_ref())?;
        println!("proposal {id}: {:?}", proposal.state);
        if let Some(rejection) = &proposal.rejection {
            println!(
                "rejected: rule {}, reason {}",
                rejection.rule_id, rejection.reason_code
            );
        }
        if let Some(nonce) = observed["evm"].get("nonce") {
            println!("EVM nonce: {nonce}");
        }
        println!(
            "EVM tx state: {}",
            observed["evm"]["status"]
                .as_str()
                .expect("receipt status is a string")
        );
        if let Some(hash) = observed["evm"]["tx_hash"].as_str() {
            println!("EVM tx: {hash}");
        }
        let mut run_terminal = false;
        if let Some(run) = run_value {
            println!(
                "verifier stage: {}",
                run["stage"].as_str().expect("validated run stage")
            );
            for stage in run["stages"].as_array().expect("validated stages") {
                let name = stage["name"].as_str();
                let status = stage["status"].as_str();
                let detail = stage["detail"].as_str();
                let (Some(name), Some(status), Some(detail)) = (name, status, detail) else {
                    return Err(Failure::new(
                        "verifier returned an incomplete stage",
                        "check the verifier run endpoint and retry; incomplete is not a pass",
                    ));
                };
                println!("  {name}: {status} - {detail}");
            }
            run_terminal = run_complete(&run)?;
            if run_terminal {
                println!(
                    "verifier run done; certificate issued; gate state and relay inclusion are reported separately"
                );
            }
        }
        let Some(timeout) = watch else {
            return Ok(());
        };
        if run_terminal
            || matches!(
                proposal.state,
                ProposalState::Rejected | ProposalState::Expired | ProposalState::Signed
            )
        {
            return Ok(());
        }
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(Failure::new(
                format!(
                    "proposal {id} did not reach a terminal gate state within {}s",
                    timeout.as_secs()
                ),
                "rerun status later; if SignFailed inspect the MPC failure before retry_sign",
            ));
        }
        std::thread::sleep(Duration::from_secs(2).min(remaining));
    }
}

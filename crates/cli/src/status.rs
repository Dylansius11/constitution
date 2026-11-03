//! Read-only gate and verifier status, with bounded two-second polling.
use crate::{
    failure::Failure,
    receipt,
    rpc::{Evm, Near, Verifier},
};
use constitution_types::ProposalState;
use std::time::{Duration, Instant};

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
        }
        let Some(timeout) = watch else {
            return Ok(());
        };
        if matches!(
            proposal.state,
            ProposalState::Rejected | ProposalState::Expired | ProposalState::Signed
        ) {
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

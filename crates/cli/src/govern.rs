//! Governance writes (F2). Every change waits the gate's fixed 7-day timelock; the CLI only
//! submits and reports, it cannot shorten anything.
use crate::failure::Failure;
use crate::near_tx::Writer;
use constitution_types::ChangeKind;
use serde_json::json;

pub fn schedule(w: &Writer, gate: &str, kind_json: &str) -> Result<(), Failure> {
    let kind: ChangeKind = serde_json::from_str(kind_json).map_err(|e| {
        Failure::new(
            format!("--kind is not a ChangeKind: {e}"),
            r#"pass JSON such as '"Unpause"' or '{"SetParams":{...}}' (SMART-CONTRACT "Governance")"#,
        )
    })?;
    let called = w.call(
        gate,
        "schedule_change",
        json!({ "kind": kind }),
        0,
        "only the governor may schedule; check --account and the gate roles",
    )?;
    println!(
        "scheduled change {} in tx {} (applies after the 7-day timelock)",
        called.value, called.tx_hash
    );
    Ok(())
}

pub fn apply(w: &Writer, gate: &str, change_id: u64) -> Result<(), Failure> {
    let called = w.call(
        gate,
        "apply_change",
        json!({ "change_id": change_id }),
        0,
        "E53 means the 7-day timelock has not elapsed; nobody can shorten it, apply after eta",
    )?;
    println!("applied change {change_id} in tx {}", called.tx_hash);
    Ok(())
}

pub fn cancel(w: &Writer, gate: &str, change_id: u64) -> Result<(), Failure> {
    let called = w.call(
        gate,
        "cancel_change",
        json!({ "change_id": change_id }),
        0,
        "only the guardian may cancel a pending change",
    )?;
    println!("cancelled change {change_id} in tx {}", called.tx_hash);
    Ok(())
}

//! Strict pinned Halmos result plus protocol reason code.
use constitution_types::manifest::StageStatus;
pub fn classify(
    status: i32,
    json: &serde_json::Value,
    contract: &str,
    function: &str,
) -> (StageStatus, u16) {
    let stage = crate::rules::classify_halmos(status, json, contract, function);
    let reason = match stage {
        StageStatus::Pass => 0,
        StageStatus::Fail => 20,
        _ => 23,
    };
    (stage, reason)
}

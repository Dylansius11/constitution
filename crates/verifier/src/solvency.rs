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

/// Tool/setup errors are not evidence of a candidate counterexample or solver timeout.
pub fn classify_tool_output(
    status: i32,
    json: &serde_json::Value,
    contract: &str,
    function: &str,
) -> Result<(StageStatus, u16), String> {
    let invalid = || "infrastructure:tool_output_invalid".to_owned();
    let results = json
        .get("test_results")
        .and_then(serde_json::Value::as_object)
        .filter(|results| results.len() == 1)
        .ok_or_else(invalid)?;
    let (name, tests) = results.iter().next().ok_or_else(invalid)?;
    let tests = tests
        .as_array()
        .filter(|tests| tests.len() == 1)
        .ok_or_else(invalid)?;
    let test = &tests[0];
    // [V] Pinned Halmos 0.3.3 Exitcode: PASS=0, COUNTEREXAMPLE=1,
    // TIMEOUT=2, STUCK=3, REVERT_ALL=4, EXCEPTION=5. Timeout/exception
    // TestResult can omit num_paths; the process exit is 0 or 1, not this enum.
    let test_code = test
        .get("exitcode")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(invalid)?;
    if !name.ends_with(&format!(":{contract}"))
        || test.get("name").and_then(serde_json::Value::as_str) != Some(function)
        || !(0..=1).contains(&status)
        || json.get("exitcode").and_then(serde_json::Value::as_i64) != Some(i64::from(status))
        || !(0..=4).contains(&test_code)
        || (test_code <= 1
            && (test
                .get("num_models")
                .and_then(serde_json::Value::as_u64)
                .is_none()
                || test
                    .get("num_bounded_loops")
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
                || test
                    .get("num_paths")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|paths| {
                        paths.len() != 3 || paths.iter().any(|n| n.as_u64().is_none())
                    })))
    {
        return Err(invalid());
    }
    Ok(classify(status, json, contract, function))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_solver_incomplete_remains_reason_23_but_setup_or_malformed_output_is_a_fault() {
        let incomplete = serde_json::json!({
            "exitcode": 1,
            "test_results": {"test/Solvency.t.sol:Solvency": [{
                "name": "invariant_solvency()", "exitcode": 2, "num_models": 0,
                "num_bounded_loops": null, "num_paths": null, "models": null, "time": null
            }]}
        });
        assert_eq!(
            classify_tool_output(1, &incomplete, "Solvency", "invariant_solvency()").unwrap(),
            (StageStatus::Incomplete, 23)
        );
        for bad in [
            serde_json::Value::Null,
            serde_json::json!({"exitcode":2,"test_results":{}}),
        ] {
            let error =
                classify_tool_output(2, &bad, "Solvency", "invariant_solvency()").unwrap_err();
            assert_eq!(
                crate::tools::infrastructure_error(&error),
                Some("tool_output_invalid")
            );
        }
        let mut exception = incomplete;
        exception["test_results"]["test/Solvency.t.sol:Solvency"][0]["exitcode"] = 5.into();
        assert!(classify_tool_output(1, &exception, "Solvency", "invariant_solvency()").is_err());
    }
}

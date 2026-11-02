use constitution_types::{
    Hex32,
    manifest::{InitializerRule, StageStatus},
};
use serde_json::Value;

pub fn filter(code: &[u8], forbidden: &[u8], constants: &[Hex32]) -> Result<(), u16> {
    let mut pc = 0;
    while pc < code.len() {
        let opcode = code[pc];
        pc += 1;
        if forbidden.contains(&opcode) {
            return Err(30);
        }
        if (0x60..=0x7f).contains(&opcode) {
            let n = usize::from(opcode - 0x5f);
            // EVM permits a final PUSH with missing immediate bytes (zero padding).
            // Do not misclassify Solidity's trailing data as an invalid opcode.
            let data = code.get(pc..pc + n);
            if n == 32 && data.is_some_and(|data| constants.iter().any(|c| data == c.0)) {
                return Err(31);
            }
            pc += n;
        }
    }
    Ok(())
}
pub fn initializer(data: &[u8], rule: &InitializerRule) -> Result<(), u16> {
    if data.is_empty() && rule.allow_empty {
        return Ok(());
    }
    let selector = data.get(..4).ok_or(40u16)?;
    let item = rule
        .allowlist
        .iter()
        .find(|a| a.selector.0 == selector)
        .ok_or(40u16)?;
    if data.len() != 4 + 32 * item.args.len() {
        return Err(41);
    }
    for (word, schema) in data[4..].as_chunks::<32>().0.iter().zip(&item.args) {
        if schema.kind != "uint256" || schema.max != "2^128-1" || word[..16].iter().any(|b| *b != 0)
        {
            return Err(41);
        }
    }
    Ok(())
}
/// Exact strict semantics of spikes/verifier-min/classify.py for Halmos 0.3.3.
pub fn classify_halmos(status: i32, data: &Value, contract: &str, function: &str) -> StageStatus {
    let incomplete = StageStatus::Incomplete;
    let Some(exit) = data.get("exitcode").and_then(Value::as_i64) else {
        return incomplete;
    };
    let Some(results) = data.get("test_results").and_then(Value::as_object) else {
        return incomplete;
    };
    if results.len() != 1 {
        return incomplete;
    }
    let Some((name, tests)) = results.iter().next() else {
        return incomplete;
    };
    if !name.ends_with(&format!(":{contract}")) {
        return incomplete;
    }
    let Some(tests) = tests.as_array().filter(|a| a.len() == 1) else {
        return incomplete;
    };
    let t = &tests[0];
    if t.get("name").and_then(Value::as_str) != Some(function) {
        return incomplete;
    }
    let (Some(code), Some(models)) = (
        t.get("exitcode").and_then(Value::as_i64),
        t.get("num_models").and_then(Value::as_i64),
    ) else {
        return incomplete;
    };
    if status == 1 && exit == 1 && code == 1 && models > 0 {
        return StageStatus::Fail;
    }
    let paths = t.get("num_paths").and_then(Value::as_array);
    if status == 0
        && exit == 0
        && code == 0
        && models == 0
        && t.get("num_bounded_loops").and_then(Value::as_i64) == Some(0)
        && paths.is_some_and(|p| {
            p.len() == 3
                && p[0].as_i64().is_some_and(|v| v > 0)
                && p[1].as_i64().is_some_and(|v| v > 0)
                && p[2].as_i64() == Some(0)
        })
    {
        return StageStatus::Pass;
    }
    incomplete
}

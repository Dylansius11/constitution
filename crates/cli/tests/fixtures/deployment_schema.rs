//! Small checker for the schema keywords used by the committed record; no new dependency.
use base64::Engine;
use serde_json::{Value, json};

pub fn sample() -> Value {
    json!({"chain_id":84532,"gate":"constitution-gate.testnet",
        "gate_code_hash":"11111111111111111111111111111111","mpc_account":"v1.signer-prod.testnet",
        "executor":"0xe0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0e0",
        "proxy":"0x7070707070707070707070707070707070707070",
        "executor_runtime_hash":format!("0x{}","ab".repeat(32)),
        "verifiers":[{"pubkey":base64::engine::general_purpose::STANDARD.encode([7;32]),"release_hash":format!("0x{}","06".repeat(32))}],
        "planned_changes":[{"id":0,"kind":{"MigrateAuthority":{"new_signer":"0x3333333333333333333333333333333333333333"}},"eta_ns":604800000000001u64}]})
}

fn schema() -> Value {
    serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../deployments/testnet.schema.json"),
        )
        .expect("committed deployment schema must exist"),
    )
    .unwrap()
}

pub fn validate(record: &Value) {
    let schema = schema();
    assert!(
        valid(record, &schema, &schema),
        "record does not match deployment schema: {record}"
    );
}

fn valid(v: &Value, s: &Value, root: &Value) -> bool {
    if let Some(reference) = s["$ref"].as_str() {
        return valid(
            v,
            root.pointer(reference.strip_prefix('#').unwrap()).unwrap(),
            root,
        );
    }
    if let Some(choices) = s["oneOf"].as_array() {
        return choices.iter().filter(|s| valid(v, s, root)).count() == 1;
    }
    if let Some(expected) = s.get("const") {
        return v == expected;
    }
    match s["type"].as_str() {
        Some("object") => {
            let Some(object) = v.as_object() else {
                return false;
            };
            if s["required"].as_array().is_some_and(|keys| {
                keys.iter()
                    .any(|k| !object.contains_key(k.as_str().unwrap()))
            }) {
                return false;
            }
            object
                .iter()
                .all(|(key, value)| match s["properties"].get(key) {
                    Some(property) => valid(value, property, root),
                    None => s["additionalProperties"] != false,
                })
        }
        Some("array") => v
            .as_array()
            .is_some_and(|items| items.iter().all(|v| valid(v, &s["items"], root))),
        Some("integer") => v.as_u64().is_some_and(|n| {
            n >= s["minimum"].as_u64().unwrap_or(0)
                && n <= s["maximum"].as_u64().unwrap_or(u64::MAX)
        }),
        Some("boolean") => v.is_boolean(),
        Some("null") => v.is_null(),
        Some("string") => v.as_str().is_some_and(|text| {
            let pattern_ok = match s["pattern"].as_str() {
                None => true,
                Some("^0x[0-9a-f]{40}$") => lowercase_hex(text, 40),
                Some("^0x[0-9a-f]{64}$") => lowercase_hex(text, 64),
                Some("^[1-9A-HJ-NP-Za-km-z]{32,44}$") => {
                    bs58::decode(text).into_vec().is_ok_and(|v| v.len() == 32)
                }
                Some("^[A-Za-z0-9+/]{42}[AEIMQUYcgkosw048]=$") => {
                    base64::engine::general_purpose::STANDARD
                        .decode(text)
                        .is_ok_and(|v| v.len() == 32)
                }
                Some("^[a-z0-9]+([._-][a-z0-9]+)*$") => {
                    text.parse::<constitution_types::AccountId>().is_ok()
                }
                Some("^[0-9]+$") => !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
                Some(other) => panic!("add checker for schema pattern {other}"),
            };
            pattern_ok
                && text.len() >= s["minLength"].as_u64().unwrap_or(0) as usize
                && text.len() <= s["maxLength"].as_u64().unwrap_or(u64::MAX) as usize
        }),
        Some(other) => panic!("add checker for schema type {other}"),
        None => panic!("unsupported schema: {s}"),
    }
}

fn lowercase_hex(text: &str, digits: usize) -> bool {
    text.strip_prefix("0x").is_some_and(|s| {
        s.len() == digits
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

#[test]
fn cli_sample_record_matches_schema_and_rejects_record_mistakes() {
    let record = sample();
    validate(&record);
    let schema = schema();
    for (field, bad) in [
        ("chain_id", json!(-1)),
        ("gate", json!("Invalid")),
        ("gate_code_hash", json!("not a NEAR code hash")),
        ("executor", json!("0xABCDEF")),
        ("executor_runtime_hash", json!("0x01")),
        (
            "verifiers",
            json!([{"pubkey":"bad","release_hash":format!("0x{}","06".repeat(32))}]),
        ),
        (
            "planned_changes",
            json!([{"id":0,"kind":"MigrateAuthority","eta_ns":1}]),
        ),
        ("typo", json!(true)),
    ] {
        let mut bad_record = record.clone();
        bad_record[field] = bad;
        assert!(
            !valid(&bad_record, &schema, &schema),
            "accepted invalid {field}"
        );
    }
    for field in [
        "chain_id",
        "gate",
        "gate_code_hash",
        "mpc_account",
        "executor",
        "proxy",
        "verifiers",
    ] {
        let mut missing = record.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            !valid(&missing, &schema, &schema),
            "accepted missing {field}"
        );
    }
    let mut legacy = record;
    legacy
        .as_object_mut()
        .unwrap()
        .remove("executor_runtime_hash");
    legacy.as_object_mut().unwrap().remove("planned_changes");
    validate(&legacy); // CLI decodes this only to report the named executor commitment FAIL.
    legacy["executor_runtime_hash"] = Value::Null;
    validate(&legacy);
}

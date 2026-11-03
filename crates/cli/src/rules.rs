use crate::failure::Failure;
use constitution_types::manifest::{RuleSetManifest, rule_set_hash, validate_rule_set};
use std::path::Path;

pub fn hash_file(path: &Path) -> Result<[u8; 32], Failure> {
    let bytes = std::fs::read(path).map_err(|e| {
        Failure::new(
            format!("cannot read {}: {e}", path.display()),
            "pass the path of a rule-set manifest, for example rules/manifest.json",
        )
    })?;
    let manifest: RuleSetManifest = serde_json::from_slice(&bytes).map_err(|e| {
        Failure::new(
            format!("{} is not a rule-set manifest: {e}", path.display()),
            "fix the field named above; the schema is ARCHITECTURE \"Rule-set manifest\"",
        )
    })?;
    validate_rule_set(&manifest).map_err(|e| {
        Failure::new(
            e,
            "commit every R2 value and the pinned tool versions, then hash again",
        )
    })?;
    rule_set_hash(&manifest).map_err(|e| Failure::new(e.to_string(), "report this as a bug"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn resolved() -> Value {
        let mut m: Value =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        m["r2"] = json!({
            "harness_sha256": format!("0x{}", "11".repeat(32)),
            "invariant_depth": 4,
            "loop_bound": 4,
            "array_lengths": "default",
            "solver_timeout_assertion_s": 60,
            "fuzz_runs": 1000,
            "fuzz_depth": 128,
            "fuzz_seed": format!("0x{}", "22".repeat(32)),
        });
        m
    }

    fn hash(value: &Value) -> Result<[u8; 32], Failure> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        std::fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        hash_file(&path)
    }

    #[test]
    fn hash_is_stable_across_runs_and_json_formatting() {
        let m = resolved();
        let first = hash(&m).unwrap();
        assert_eq!(first, hash(&m).unwrap());
        let dir = tempfile::tempdir().unwrap();
        let compact = dir.path().join("compact.json");
        std::fs::write(&compact, serde_json::to_vec(&m).unwrap()).unwrap();
        assert_eq!(first, hash_file(&compact).unwrap());
    }

    #[test]
    fn any_committed_bound_changes_the_hash() {
        let base = hash(&resolved()).unwrap();
        let mut deeper = resolved();
        deeper["r2"]["invariant_depth"] = json!(5);
        assert_ne!(base, hash(&deeper).unwrap());
        let mut ttl = resolved();
        ttl["max_cert_ttl_s"] = json!(301);
        assert_ne!(base, hash(&ttl).unwrap());
    }

    #[test]
    fn unresolved_unknown_or_unreadable_manifests_fail_with_next_step() {
        let mut unresolved = resolved();
        unresolved["r2"]["fuzz_seed"] = Value::Null;
        let e = hash(&unresolved).unwrap_err();
        assert!(e.cause.contains("R2 commitment unresolved"), "{e}");
        assert!(e.next.contains("R2"));

        let mut extra = resolved();
        extra["surprise"] = json!(1);
        assert!(hash(&extra).unwrap_err().cause.contains("unknown field"));

        let e = hash_file(Path::new("does/not/exist.json")).unwrap_err();
        assert!(e.to_string().starts_with("error: cannot read"));
        assert!(e.to_string().contains("\nnext: "));
    }
}

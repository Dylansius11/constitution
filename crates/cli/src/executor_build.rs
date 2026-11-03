//! Hash the executor runtime after masking compiler-declared immutable references.
use crate::failure::Failure;
use serde::Deserialize;
use serde_json::Value;
use sha3::{Digest, Keccak256};
use std::{collections::BTreeMap, path::Path};

pub const DEFAULT_ARTIFACT: &str =
    "contracts/out/ConstitutionExecutor.sol/ConstitutionExecutor.json";

#[derive(Deserialize)]
struct Region {
    start: usize,
    length: usize,
}

pub struct ExecutorBuild {
    runtime: Vec<u8>,
    regions: Vec<Region>,
}

impl ExecutorBuild {
    pub fn read(path: &Path) -> Result<Self, Failure> {
        let read = || -> Result<Self, String> {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            Self::from_json(serde_json::from_slice(&bytes).map_err(|e| e.to_string())?)
        };
        read().map_err(|e| Failure::new(
            format!("executor build artifact {}: {e}", path.display()),
            "build the committed contracts with `forge build --root contracts --offline` and pass --executor-artifact if needed",
        ))
    }

    fn from_json(value: Value) -> Result<Self, String> {
        let deployed = &value["deployedBytecode"];
        let object = deployed["object"]
            .as_str()
            .ok_or("missing deployedBytecode.object")?;
        let runtime = hex::decode(object.strip_prefix("0x").unwrap_or(object))
            .map_err(|e| format!("runtime is not linked hex: {e}"))?;
        if runtime.is_empty() {
            return Err("empty runtime".into());
        }
        let references: BTreeMap<String, Vec<Region>> =
            serde_json::from_value(deployed["immutableReferences"].clone())
                .map_err(|e| format!("missing or invalid immutableReferences: {e}"))?;
        let mut regions: Vec<_> = references.into_values().flatten().collect();
        regions.sort_unstable_by_key(|r| r.start);
        let mut end = 0;
        for r in &regions {
            let next = r
                .start
                .checked_add(r.length)
                .ok_or("immutable region overflow")?;
            if r.length == 0 || r.start < end || next > runtime.len() {
                return Err("overlapping, empty or out-of-bounds immutable region".into());
            }
            end = next;
        }
        Ok(Self { runtime, regions })
    }

    pub fn masked_hash(&self, runtime: &[u8]) -> Option<[u8; 32]> {
        if runtime.len() != self.runtime.len() {
            return None;
        }
        // Stream unchanged sections and zero words, avoiding a full runtime copy.
        let mut hash = Keccak256::new();
        let mut end = 0;
        for r in &self.regions {
            hash.update(&runtime[end..r.start]);
            for _ in 0..r.length {
                hash.update([0]);
            }
            end = r.start + r.length;
        }
        hash.update(&runtime[end..]);
        Some(hash.finalize().into())
    }

    pub fn hash(&self) -> [u8; 32] {
        self.masked_hash(&self.runtime).expect("artifact length")
    }

    #[cfg(test)]
    fn matches(&self, runtime: &[u8], committed: [u8; 32]) -> bool {
        committed == self.hash() && self.masked_hash(runtime) == Some(committed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn immutable_values_are_ignored_but_code_and_length_are_not() {
        let build = ExecutorBuild::from_json(json!({"deployedBytecode": {
            "object":"0x6000000061", "immutableReferences":{"1":[{"start":1,"length":3}]}
        }}))
        .unwrap();
        assert!(build.matches(&[0x60, 1, 2, 3, 0x61], build.hash()));
        assert!(!build.matches(&[0x60, 1, 2, 3, 0x62], build.hash()));
        assert!(!build.matches(&[0x60, 1, 2, 3, 0x61, 0], build.hash()));
        assert!(!build.matches(&[], build.hash()));
        assert!(!build.matches(&[0x60, 1, 2, 3, 0x61], [9; 32]));
    }

    #[test]
    fn missing_metadata_overlaps_and_out_of_bounds_fail_closed() {
        for references in [
            json!({"1":[{"start":1,"length":3}]}),
            json!({"1":[{"start":0,"length":1},{"start":0,"length":1}]}),
            json!({"1":[{"start":0,"length":0}]}),
        ] {
            assert!(
                ExecutorBuild::from_json(json!({"deployedBytecode": {
                    "object":"0x6000", "immutableReferences":references
                }}))
                .is_err()
            );
        }
        assert!(ExecutorBuild::from_json(json!({"deployedBytecode":{"object":"0x6000"}})).is_err());
    }
}

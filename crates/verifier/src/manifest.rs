use constitution_types::manifest::{RuleSetManifest, validate_rule_set};
use std::path::Path;

pub fn load_manifest(path: impl AsRef<Path>) -> Result<RuleSetManifest, String> {
    let manifest: RuleSetManifest =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate_rule_set(&manifest)?;
    Ok(manifest)
}

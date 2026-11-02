use constitution_types::manifest::RuleSetManifest;
use std::path::Path;

pub fn load_manifest(path: impl AsRef<Path>) -> Result<RuleSetManifest, String> {
    let manifest: RuleSetManifest =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate(&manifest)?;
    Ok(manifest)
}
pub fn validate(m: &RuleSetManifest) -> Result<(), String> {
    let r = &m.r2;
    if r.harness_sha256.is_none()
        || r.invariant_depth.is_none()
        || r.loop_bound.is_none()
        || r.array_lengths.is_none()
        || r.solver_timeout_assertion_s.is_none()
        || r.fuzz_runs.is_none()
        || r.fuzz_seed.is_none()
    {
        return Err("R2 commitment unresolved: null fields refuse startup".into());
    }
    if m.version != 1
        || m.solc != "0.8.38"
        || m.evm_version != "prague"
        || m.tools.foundry != "1.8.6"
        || m.tools.halmos != "0.3.3"
        || m.tools.upgrades_core != "1.47.0"
        || r.solver_timeout_assertion_s == Some(0)
        || r.fuzz_runs == Some(0)
        || m.max_cert_ttl_s == 0
    {
        return Err("unsupported compiler, tool version or zero safety bound".into());
    }
    for item in &m.r4.allowlist {
        if item.selector.0.len() != 4
            || item
                .args
                .iter()
                .any(|a| a.kind != "uint256" || a.max != "2^128-1")
        {
            return Err("unsupported R4 schema".into());
        }
    }
    if ![255, 242, 244]
        .iter()
        .all(|o| m.r3.forbidden_opcodes.contains(o))
        || m.r3.forbidden_constants.len() != 2
    {
        return Err("missing mandatory R3 filters".into());
    }
    Ok(())
}

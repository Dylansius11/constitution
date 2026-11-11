use constitution_types::manifest::{RuleSetManifest, validate_rule_set};
use std::path::Path;

pub fn load_manifest(path: impl AsRef<Path>) -> Result<RuleSetManifest, String> {
    let manifest: RuleSetManifest =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate_rule_set(&manifest)?;
    Ok(manifest)
}

/// Operator input is repo-relative, and canonicalization also refuses symlink escapes.
pub fn configured_path(
    repo: &Path,
    value: Option<&std::ffi::OsStr>,
) -> Result<std::path::PathBuf, String> {
    let value = value.unwrap_or_else(|| std::ffi::OsStr::new("rules/manifest.json"));
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("VERIFIER_MANIFEST must be a repo-relative path without traversal".into());
    }
    let repo = repo
        .canonicalize()
        .map_err(|_| "manifest repository unavailable")?;
    let path = repo
        .join(path)
        .canonicalize()
        .map_err(|_| "VERIFIER_MANIFEST unavailable")?;
    if !path.starts_with(&repo) {
        return Err("VERIFIER_MANIFEST escapes repository".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_filters_cannot_be_replaced_or_reordered() {
        let m: RuleSetManifest =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        validate_rule_set(&m).unwrap();
        for index in 0..m.r3.forbidden_constants.len() {
            let mut changed = m.clone();
            changed.r3.forbidden_constants[index] = constitution_types::Hex32([0; 32]);
            assert!(validate_rule_set(&changed).is_err());
        }
        let mut changed = m.clone();
        changed.r3.forbidden_constants.swap(0, 1);
        assert!(validate_rule_set(&changed).is_err());
        let mut changed = m;
        changed.r3.forbidden_constants.pop();
        assert!(validate_rule_set(&changed).is_err());
    }

    #[test]
    fn committed_slots_and_cheatcodes_match_their_protocol_derivations() {
        let m: RuleSetManifest =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        for (index, label) in ["eip1967.proxy.implementation", "eip1967.proxy.admin"]
            .iter()
            .enumerate()
        {
            let hash = crate::keccak(label.as_bytes());
            let mut slot = hash;
            for byte in slot.iter_mut().rev() {
                let (value, borrow) = byte.overflowing_sub(1);
                *byte = value;
                if !borrow {
                    break;
                }
            }
            assert_eq!(m.r3.forbidden_constants[index].0, slot);
            assert_eq!(m.r3.forbidden_constants[3 + index * 2].0, hash);
            let mut below = slot;
            below[31] -= 1;
            assert_eq!(m.r3.forbidden_constants[2 + index * 2].0, below);
        }
        for (index, label) in ["hevm cheat code", "svm cheat code"].iter().enumerate() {
            let hash = crate::keccak(label.as_bytes());
            let mut address = [0; 32];
            address[12..].copy_from_slice(&hash[12..]);
            assert_eq!(m.r3.forbidden_constants[6 + index].0, address);
        }
    }
}

#[cfg(test)]
mod configured_tests {
    use super::*;
    #[test]
    fn configured_manifest_refuses_absolute_traversal_and_symlink_escape() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir(repo.path().join("rules")).unwrap();
        std::fs::write(repo.path().join("rules/manifest.json"), "{}").unwrap();
        assert!(
            configured_path(repo.path(), None)
                .unwrap()
                .ends_with("rules/manifest.json")
        );
        for path in [
            "",
            "../manifest.json",
            "rules/../rules/manifest.json",
            "/etc/passwd",
        ] {
            assert!(configured_path(repo.path(), Some(std::ffi::OsStr::new(path))).is_err());
        }
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::fs::write(outside.path().join("manifest.json"), "{}").unwrap();
            std::os::unix::fs::symlink(outside.path(), repo.path().join("escape")).unwrap();
            assert!(
                configured_path(
                    repo.path(),
                    Some(std::ffi::OsStr::new("escape/manifest.json"))
                )
                .is_err()
            );
        }
    }
}

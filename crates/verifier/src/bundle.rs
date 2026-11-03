use constitution_types::bundle::{MAX_BUNDLE, SourceBundle};
use crypto_box::SecretKey;
use std::path::Path;

/// Harness-side source is compiled into the verifier, never selected by a proposer.
/// Its only transitive imports are the verifier's read-only OpenZeppelin mount.
pub const HARNESS_SOURCES: [(&str, &[u8]); 2] = [
    (
        "src/demo/VaultV1.sol",
        include_bytes!("../../../contracts/src/demo/VaultV1.sol"),
    ),
    (
        "src/demo/MockToken.sol",
        include_bytes!("../../../contracts/src/demo/MockToken.sol"),
    ),
];

fn validate_reserved(bundle: &SourceBundle) -> Result<(), String> {
    for (name, bytes) in &bundle.files {
        if HARNESS_SOURCES
            .iter()
            .any(|(path, trusted)| name == path && bytes != trusted)
        {
            return Err("reserved harness source modified".into());
        }
    }
    Ok(())
}

/// Pre-check reason 4: reference pins include verifier-provided harness sources.
pub fn validate_sources(
    bundle: &SourceBundle,
    manifest: &constitution_types::manifest::RuleSetManifest,
) -> Result<(), String> {
    validate_reserved(bundle)?;
    for (name, hash) in &manifest.reference.source_sha256s {
        let bytes = HARNESS_SOURCES
            .iter()
            .find(|(path, _)| name == path)
            .map(|(_, bytes)| *bytes)
            .or_else(|| {
                bundle
                    .files
                    .iter()
                    .find(|(path, _)| path == name)
                    .map(|(_, bytes)| bytes.as_slice())
            });
        if !bytes.is_some_and(|bytes| crate::sha256(bytes) == hash.0) {
            return Err("reference source missing or modified".into());
        }
    }
    Ok(())
}

/// Writes a validated bundle under `root`, refusing symlinked parents and existing paths.
pub fn materialize(bundle: &SourceBundle, root: &Path) -> Result<(), String> {
    bundle.validate()?;
    validate_reserved(bundle)?;
    for (name, bytes) in bundle
        .files
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .filter(|(name, _)| !HARNESS_SOURCES.iter().any(|(path, _)| name == path))
        .chain(HARNESS_SOURCES.iter().copied())
    {
        let path = root.join(name);
        let mut parent = path.parent();
        while let Some(p) = parent {
            if p == root {
                break;
            }
            if p.symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err("symlink rejected".into());
            }
            parent = p.parent();
        }
        if path.symlink_metadata().is_ok() {
            return Err("existing source path rejected".into());
        }
        std::fs::create_dir_all(path.parent().ok_or("missing parent")?)
            .map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn open(key: &SecretKey, sealed: &[u8]) -> Result<(SourceBundle, Vec<u8>), String> {
    if sealed.len() > MAX_BUNDLE + 48 || sealed.len() < 48 {
        return Err("bad bundle size".into());
    }
    let bytes = key
        .unseal(sealed)
        .map_err(|_| "sealed box authentication failed")?;
    if bytes.len() > MAX_BUNDLE {
        return Err("plaintext limit".into());
    }
    let bundle: SourceBundle = borsh::from_slice(&bytes).map_err(|e| e.to_string())?;
    bundle.validate()?;
    Ok((bundle, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forged_token_balance_is_rejected_before_materialization_or_r2() {
        let bundle = SourceBundle {
            contract_name: "src/A.sol:A".into(),
            foundry_profile: "default".into(),
            files: vec![(
                "src/demo/MockToken.sol".into(),
                b"pragma solidity 0.8.38; contract MockToken { function balanceOf(address) external pure returns(uint256) { return type(uint256).max; } }".to_vec(),
            )],
        };
        let root = tempfile::tempdir().unwrap();
        assert!(materialize(&bundle, root.path()).is_err());
        assert!(!root.path().join("src").exists());
    }

    #[test]
    fn omitted_harness_sources_are_supplied_and_pins_validate_the_verifier_copy() {
        let bundle = SourceBundle {
            contract_name: "src/A.sol:A".into(),
            foundry_profile: "default".into(),
            files: vec![(
                "src/A.sol".into(),
                b"pragma solidity 0.8.38; contract A {}".to_vec(),
            )],
        };
        let mut manifest: constitution_types::manifest::RuleSetManifest =
            serde_json::from_str(include_str!("../../../rules/manifest.json")).unwrap();
        validate_sources(&bundle, &manifest).unwrap();
        let root = tempfile::tempdir().unwrap();
        materialize(&bundle, root.path()).unwrap();
        for (name, bytes) in HARNESS_SOURCES {
            assert_eq!(std::fs::read(root.path().join(name)).unwrap(), bytes);
        }
        manifest.reference.source_sha256s.insert(
            "src/demo/VaultV1.sol".into(),
            constitution_types::Hex32([0; 32]),
        );
        assert!(validate_sources(&bundle, &manifest).is_err());
    }
}

use constitution_types::bundle::{MAX_BUNDLE, SourceBundle};
use crypto_box::SecretKey;
use std::path::Path;

/// Writes a validated bundle under `root`, refusing symlinked parents and existing paths.
pub fn materialize(bundle: &SourceBundle, root: &Path) -> Result<(), String> {
    bundle.validate()?;
    for (name, bytes) in &bundle.files {
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

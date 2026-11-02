use crypto_box::SecretKey;
use std::{collections::BTreeSet, path::Path};
pub const MAX_BUNDLE: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct SourceBundle {
    pub contract_name: String,
    pub files: Vec<(String, Vec<u8>)>,
    pub foundry_profile: String,
}
impl SourceBundle {
    pub fn validate(&self) -> Result<(), String> {
        if self.foundry_profile != "default"
            || self.files.is_empty()
            || self.files.len() > 4096
            || !self
                .contract_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_/:.".contains(c))
        {
            return Err("unsupported bundle profile or contract".into());
        }
        let mut names = BTreeSet::new();
        for (name, _) in &self.files {
            if !name.starts_with("src/")
                || name.contains('\\')
                || name.contains('\0')
                || name
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || !name.ends_with(".sol")
                || !names.insert(name)
            {
                return Err("unsafe or duplicate source path".into());
            }
        }
        Ok(())
    }
    pub fn materialize(&self, root: &Path) -> Result<(), String> {
        self.validate()?;
        for (name, bytes) in &self.files {
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

//! Canonical source bundle bytes. `source_commitment = sha256(borsh(SourceBundle))`; the CLI and
//! the page build it, the verifier opens it. Paths are validated here so every producer and the
//! consumer refuse the same bundles.
use std::collections::BTreeSet;

/// Plaintext limit of one bundle (ARCHITECTURE "Verifier HTTP API").
pub const MAX_BUNDLE: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct SourceBundle {
    pub contract_name: String,
    pub files: Vec<(String, Vec<u8>)>,
    pub foundry_profile: String,
}

impl SourceBundle {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.foundry_profile != "default"
            || self.files.is_empty()
            || self.files.len() > 4096
            || !self
                .contract_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_/:.".contains(c))
        {
            return Err("unsupported bundle profile or contract");
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
                return Err("unsafe or duplicate source path");
            }
        }
        Ok(())
    }
}

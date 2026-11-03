use constitution_types::manifest::{RuleSetManifest, validate_rule_set};
use std::path::Path;

pub fn load_manifest(path: impl AsRef<Path>) -> Result<RuleSetManifest, String> {
    let manifest: RuleSetManifest =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate_rule_set(&manifest)?;
    Ok(manifest)
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

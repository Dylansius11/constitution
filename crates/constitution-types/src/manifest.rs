//! [I] Canonical manifest field order follows ARCHITECTURE. Optional R2 fields encode
//! unresolved commitments, not defaults. The verifier refuses them at startup.
use crate::{Hex20, Hex32, HexBytes};
macro_rules! record {
    ($name:ident { $($fields:tt)* }) => {
        #[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize,
            borsh::BorshSchema, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $($fields)* }
    };
}
record!(RuleSetManifest {
    pub version: u32, pub name: String, pub solc: String, pub evm_version: String,
    pub optimizer_runs: u32, pub tools: Tools, pub reference: Reference,
    pub r1: LayoutRule, pub r2: SolvencyRule, pub r3: FilterRule, pub r4: InitializerRule,
    pub r5: ReplayRule, pub ai: AiPolicy, pub max_cert_ttl_s: u64,
});
record!(Tools { pub foundry: String, pub halmos: String, pub upgrades_core: String, pub z3: String, pub yices: String });
record!(Reference { pub contract_name: String, pub source_sha256s: std::collections::BTreeMap<String, Hex32> });
// [I] R1 exceptions are committed; R3 independently rejects dangerous opcodes.
record!(LayoutRule { pub unsafe_allow: Vec<String> });
record!(SolvencyRule {
    pub harness_sha256: Option<Hex32>, pub invariant_depth: Option<u32>, pub loop_bound: Option<u32>,
    pub array_lengths: Option<String>, pub solver_timeout_assertion_s: Option<u32>,
    pub fuzz_runs: Option<u32>, pub fuzz_depth: Option<u32>, pub fuzz_seed: Option<Hex32>,
});
record!(FilterRule { pub forbidden_opcodes: Vec<u8>, pub forbidden_constants: Vec<Hex32> });
record!(InitializerRule { pub allowlist: Vec<Initializer>, pub allow_empty: bool });
record!(Initializer { pub selector: HexBytes, pub args: Vec<Argument> });
record!(Argument { #[serde(rename = "type")] pub kind: String, pub max: String });
record!(ReplayRule { pub corpus_sha256: Hex32, pub max_txs: u32, pub confirmations: u32 });
record!(AiPolicy { pub required: bool, pub model: String, pub signature_kind: String, pub max_candidates: u32, pub timeout_s: u32 });

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum StageStatus {
    Pass,
    Fail,
    Incomplete,
    NotRun,
    Skipped,
}
record!(EvidenceManifest {
    pub certificate_fields_digest: Hex32, pub compile: CompileEvidence, pub current: CurrentEvidence,
    pub r1: RuleEvidence, pub r2: RuleEvidence, pub r3: RuleEvidence, pub r4: RuleEvidence, pub r5: RuleEvidence,
    pub ai: AiEvidence, pub evidence_block: EvidenceBlock, pub archive_provider: String,
});
record!(CompileEvidence { pub solc: String, pub settings_hash: Hex32, pub runtime_hash: Hex32 });
record!(CurrentEvidence { pub address: Hex20, pub runtime_hash: Hex32, pub reproduced: bool });
record!(RuleEvidence { pub status: StageStatus, pub artifact_sha256: Hex32, pub detail_code: u16 });
record!(AiEvidence {
    pub model: String, pub completion_id: String, pub request_sha256: Hex32, pub response_sha256: Hex32,
    pub signature_kind: String, pub signature: HexBytes, pub preflight_digest: Hex32, pub candidates: Vec<AiCandidate>,
});
record!(AiCandidate { pub sha256: Hex32, pub reproduced: bool });
record!(EvidenceBlock { pub number: u64, pub hash: Hex32 });

/// The rule-set schema both the verifier and the CLI accept. A manifest that fails here has
/// no meaningful `rule_set_hash`: unresolved R2 commitments and unsupported tools are refused.
pub fn validate_rule_set(m: &RuleSetManifest) -> Result<(), &'static str> {
    let r = &m.r2;
    if r.harness_sha256.is_none()
        || r.invariant_depth.is_none()
        || r.loop_bound.is_none()
        || r.array_lengths.is_none()
        || r.solver_timeout_assertion_s.is_none()
        || r.fuzz_runs.is_none()
        || r.fuzz_depth.is_none()
        || r.fuzz_seed.is_none()
    {
        return Err("R2 commitment unresolved: null fields refuse startup");
    }
    if m.version != 1
        || m.solc != "0.8.38"
        || m.evm_version != "prague"
        || m.tools.foundry != "1.8.6"
        || m.tools.halmos != "0.3.3"
        || m.tools.upgrades_core != "1.47.0"
        || r.solver_timeout_assertion_s == Some(0)
        || r.fuzz_runs == Some(0)
        || r.fuzz_depth == Some(0)
        || r.invariant_depth == Some(0)
        || m.max_cert_ttl_s == 0
    {
        return Err("unsupported compiler, tool version or zero safety bound");
    }
    for item in &m.r4.allowlist {
        if item.selector.0.len() != 4
            || item
                .args
                .iter()
                .any(|a| a.kind != "uint256" || a.max != "2^128-1")
        {
            return Err("unsupported R4 schema");
        }
    }
    // Ordered list: implementation/admin slots, each slot -1/+1, then the
    // zero-padded Foundry/Halmos addresses. Vec encoding and Borsh field order
    // remain unchanged; changing this supported list changes the rule-set hash.
    const REQUIRED_R3: [Hex32; 8] = [
        Hex32([
            0x36, 0x08, 0x94, 0xa1, 0x3b, 0xa1, 0xa3, 0x21, 0x06, 0x67, 0xc8, 0x28, 0x49, 0x2d,
            0xb9, 0x8d, 0xca, 0x3e, 0x20, 0x76, 0xcc, 0x37, 0x35, 0xa9, 0x20, 0xa3, 0xca, 0x50,
            0x5d, 0x38, 0x2b, 0xbc,
        ]),
        Hex32([
            0xb5, 0x31, 0x27, 0x68, 0x4a, 0x56, 0x8b, 0x31, 0x73, 0xae, 0x13, 0xb9, 0xf8, 0xa6,
            0x01, 0x6e, 0x24, 0x3e, 0x63, 0xb6, 0xe8, 0xee, 0x11, 0x78, 0xd6, 0xa7, 0x17, 0x85,
            0x0b, 0x5d, 0x61, 0x03,
        ]),
        Hex32([
            0x36, 0x08, 0x94, 0xa1, 0x3b, 0xa1, 0xa3, 0x21, 0x06, 0x67, 0xc8, 0x28, 0x49, 0x2d,
            0xb9, 0x8d, 0xca, 0x3e, 0x20, 0x76, 0xcc, 0x37, 0x35, 0xa9, 0x20, 0xa3, 0xca, 0x50,
            0x5d, 0x38, 0x2b, 0xbb,
        ]),
        Hex32([
            0x36, 0x08, 0x94, 0xa1, 0x3b, 0xa1, 0xa3, 0x21, 0x06, 0x67, 0xc8, 0x28, 0x49, 0x2d,
            0xb9, 0x8d, 0xca, 0x3e, 0x20, 0x76, 0xcc, 0x37, 0x35, 0xa9, 0x20, 0xa3, 0xca, 0x50,
            0x5d, 0x38, 0x2b, 0xbd,
        ]),
        Hex32([
            0xb5, 0x31, 0x27, 0x68, 0x4a, 0x56, 0x8b, 0x31, 0x73, 0xae, 0x13, 0xb9, 0xf8, 0xa6,
            0x01, 0x6e, 0x24, 0x3e, 0x63, 0xb6, 0xe8, 0xee, 0x11, 0x78, 0xd6, 0xa7, 0x17, 0x85,
            0x0b, 0x5d, 0x61, 0x02,
        ]),
        Hex32([
            0xb5, 0x31, 0x27, 0x68, 0x4a, 0x56, 0x8b, 0x31, 0x73, 0xae, 0x13, 0xb9, 0xf8, 0xa6,
            0x01, 0x6e, 0x24, 0x3e, 0x63, 0xb6, 0xe8, 0xee, 0x11, 0x78, 0xd6, 0xa7, 0x17, 0x85,
            0x0b, 0x5d, 0x61, 0x04,
        ]),
        Hex32([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x71, 0x09,
            0x70, 0x9e, 0xcf, 0xa9, 0x1a, 0x80, 0x62, 0x6f, 0xf3, 0x98, 0x9d, 0x68, 0xf6, 0x7f,
            0x5b, 0x1d, 0xd1, 0x2d,
        ]),
        Hex32([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf3, 0x99,
            0x3a, 0x62, 0x37, 0x7b, 0xcd, 0x56, 0xae, 0x39, 0xd7, 0x73, 0x74, 0x0a, 0x53, 0x90,
            0x41, 0x1e, 0x8b, 0xc9,
        ]),
    ];
    if ![255, 242, 244]
        .iter()
        .all(|o| m.r3.forbidden_opcodes.contains(o))
        || m.r3.forbidden_constants.as_slice() != REQUIRED_R3
    {
        return Err("missing mandatory R3 filters");
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn rule_set_hash(manifest: &RuleSetManifest) -> Result<[u8; 32], borsh::io::Error> {
    use sha2::{Digest, Sha256};
    Ok(Sha256::digest(borsh::to_vec(manifest)?).into())
}
#[cfg(not(target_arch = "wasm32"))]
pub fn evidence_root(manifest: &EvidenceManifest) -> Result<[u8; 32], borsh::io::Error> {
    use sha2::{Digest, Sha256};
    Ok(Sha256::digest(borsh::to_vec(manifest)?).into())
}

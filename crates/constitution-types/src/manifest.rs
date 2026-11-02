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
    pub fuzz_runs: Option<u32>, pub fuzz_seed: Option<Hex32>,
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

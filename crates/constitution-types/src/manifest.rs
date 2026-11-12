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
record!(RuleSetManifestV1 {
    pub version: u32, pub name: String, pub solc: String, pub evm_version: String,
    pub optimizer_runs: u32, pub tools: Tools, pub reference: Reference,
    pub r1: LayoutRule, pub r2: SolvencyRule, pub r3: FilterRule, pub r4: InitializerRule,
    pub r5: ReplayRule, pub ai: AiPolicy, pub max_cert_ttl_s: u64,
});

/// Version two appends a committed, ordered lineage to the unchanged v1 policy fields.
/// `reference` remains the harness baseline and must equal the first lineage entry.
#[derive(
    Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize, serde::Serialize,
)]
pub struct RuleSetManifestV2 {
    #[serde(flatten)]
    pub policy: RuleSetManifestV1,
    pub reference_lineage: Vec<Reference>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum RuleSetManifest {
    V1(RuleSetManifestV1),
    V2(RuleSetManifestV2),
}

impl<'de> serde::Deserialize<'de> for RuleSetManifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // One strict wire schema preserves unknown-field diagnostics. Serde flatten
        // plus an untagged enum would silently accept unknown v2 policy fields.
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            version: u32,
            name: String,
            solc: String,
            evm_version: String,
            optimizer_runs: u32,
            tools: Tools,
            reference: Reference,
            r1: LayoutRule,
            r2: SolvencyRule,
            r3: FilterRule,
            r4: InitializerRule,
            r5: ReplayRule,
            ai: AiPolicy,
            max_cert_ttl_s: u64,
            #[serde(default, deserialize_with = "present_lineage")]
            reference_lineage: Option<Vec<Reference>>,
        }
        fn present_lineage<'de, D: serde::Deserializer<'de>>(
            d: D,
        ) -> Result<Option<Vec<Reference>>, D::Error> {
            <Vec<Reference> as serde::Deserialize>::deserialize(d).map(Some)
        }
        let wire = Wire::deserialize(deserializer)?;
        let policy = RuleSetManifestV1 {
            version: wire.version,
            name: wire.name,
            solc: wire.solc,
            evm_version: wire.evm_version,
            optimizer_runs: wire.optimizer_runs,
            tools: wire.tools,
            reference: wire.reference,
            r1: wire.r1,
            r2: wire.r2,
            r3: wire.r3,
            r4: wire.r4,
            r5: wire.r5,
            ai: wire.ai,
            max_cert_ttl_s: wire.max_cert_ttl_s,
        };
        match (policy.version, wire.reference_lineage) {
            (1, None) => Ok(Self::V1(policy)),
            (2, Some(reference_lineage)) => Ok(Self::V2(RuleSetManifestV2 {
                policy,
                reference_lineage,
            })),
            (1, Some(_)) => Err(serde::de::Error::custom(
                "unknown field `reference_lineage` in v1 manifest",
            )),
            (2, None) => Err(serde::de::Error::missing_field("reference_lineage")),
            _ => Err(serde::de::Error::custom("unsupported manifest version")),
        }
    }
}

impl std::ops::Deref for RuleSetManifest {
    type Target = RuleSetManifestV1;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::V1(m) => m,
            Self::V2(m) => &m.policy,
        }
    }
}
impl std::ops::DerefMut for RuleSetManifest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::V1(m) => m,
            Self::V2(m) => &mut m.policy,
        }
    }
}
impl RuleSetManifest {
    pub fn references(&self) -> &[Reference] {
        match self {
            Self::V1(m) => std::slice::from_ref(&m.reference),
            Self::V2(m) => &m.reference_lineage,
        }
    }
}
impl borsh::BorshSerialize for RuleSetManifest {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> Result<(), borsh::io::Error> {
        match self {
            Self::V1(m) if m.version == 1 => borsh::BorshSerialize::serialize(m, writer),
            Self::V2(m) if m.policy.version == 2 => borsh::BorshSerialize::serialize(m, writer),
            _ => Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "unsupported manifest version",
            )),
        }
    }
}
impl borsh::BorshDeserialize for RuleSetManifest {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> Result<Self, borsh::io::Error> {
        let policy = RuleSetManifestV1::deserialize_reader(reader)?;
        match policy.version {
            1 => Ok(Self::V1(policy)),
            2 => Ok(Self::V2(RuleSetManifestV2 {
                policy,
                reference_lineage: Vec::<Reference>::deserialize_reader(reader)?,
            })),
            _ => Err(borsh::io::Error::new(
                borsh::io::ErrorKind::InvalidData,
                "unsupported manifest version",
            )),
        }
    }
}

pub fn validate_rule_set(m: &RuleSetManifest) -> Result<(), &'static str> {
    match m {
        RuleSetManifest::V1(m) if m.version != 1 => return Err("unsupported manifest version"),
        RuleSetManifest::V2(m) => {
            if m.policy.version != 2 || m.reference_lineage.first() != Some(&m.policy.reference) {
                return Err("lineage must start with the reference baseline");
            }
            let mut names = std::collections::BTreeSet::new();
            for reference in &m.reference_lineage {
                if reference.contract_name.is_empty()
                    || reference.source_sha256s.is_empty()
                    || !names.insert(&reference.contract_name)
                {
                    return Err("empty or duplicate lineage reference");
                }
            }
        }
        _ => {}
    }
    validate_policy(m)
}
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

fn validate_policy(m: &RuleSetManifestV1) -> Result<(), &'static str> {
    if m.reference.contract_name.is_empty() || m.reference.source_sha256s.is_empty() {
        return Err("empty reference");
    }
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
    if m.solc != "0.8.38"
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

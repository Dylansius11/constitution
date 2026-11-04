//! Proposal observations consumed by the pipeline; POST never supplies these fields.
use constitution_types::{AccountId, Hex20, Hex32, HexBytes};
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
// Kept as LocalProposal because the existing pipeline consumes this interface.
// GateSource populates it from chain reads; LocalRegistry is explicitly local-only.
pub struct LocalProposal {
    pub gate: AccountId,
    pub proposal_id: u64,
    pub chain_id: u64,
    pub proxy: Hex20,
    pub executor: Hex20,
    pub expected_current_impl: Hex20,
    pub observed_current_impl: Hex20,
    pub current_impl_codehash: Hex32,
    pub new_impl: Hex20,
    pub new_impl_codehash: Hex32,
    pub init_calldata: HexBytes,
    pub source_commitment: Hex32,
    pub rule_set_hash: Hex32,
    pub evidence_block_number: u64,
    pub evidence_block_hash: Hex32,
    pub expires_at_s: u64,
}
/// Why a proposal observation was refused; the API maps each kind to a stable status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// Plaintext does not match the on-chain source commitment.
    BadBundle,
    WrongGate,
    /// The gate proposal is no longer Pending.
    NotPending,
    /// Rule set or proposal shape this verifier cannot certify (will not change by retrying).
    Unsupported(&'static str),
    /// Chain state is not yet usable (too few blocks, no current implementation); retry later.
    NotReady(&'static str),
    /// Transport, protocol or lag faults of a provider.
    Rpc(String),
    /// The admission callback refused; the caller holds the reason.
    Refused,
}
impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadBundle => f.write_str("bad_bundle"),
            Self::WrongGate => f.write_str("wrong_gate"),
            Self::NotPending => f.write_str("proposal must be Pending"),
            Self::Unsupported(text) | Self::NotReady(text) => f.write_str(text),
            Self::Rpc(text) => f.write_str(text),
            Self::Refused => f.write_str("admission refused"),
        }
    }
}
impl std::error::Error for SourceError {}
impl From<String> for SourceError {
    fn from(text: String) -> Self {
        Self::Rpc(text)
    }
}
impl From<&str> for SourceError {
    fn from(text: &str) -> Self {
        Self::Rpc(text.into())
    }
}
/// Called once the source commitment matched, before further reads; an error aborts.
pub type Admit<'a> = &'a mut dyn FnMut() -> Result<(), SourceError>;
pub trait ProposalSource: Send + Sync {
    fn get(&self, gate: &AccountId, id: u64) -> Result<Option<LocalProposal>, SourceError>;
    /// Chain sources pin one gate; local registries remain explicitly offline.
    fn gate_account(&self) -> Option<&AccountId> {
        None
    }
    /// Verify the secret source commitment before expensive chain observations.
    /// `admit` runs exactly once, after the commitment matched and before any further
    /// read; its error aborts the observation (callers charge limiters there).
    fn get_for_bundle(
        &self,
        gate: &AccountId,
        id: u64,
        commitment: [u8; 32],
        admit: Admit<'_>,
    ) -> Result<Option<LocalProposal>, SourceError> {
        let proposal = self.get(gate, id)?;
        if let Some(p) = &proposal {
            if p.source_commitment.0 != commitment {
                return Err(SourceError::BadBundle);
            }
            admit()?;
        }
        Ok(proposal)
    }
    fn archive_provider(&self) -> String;
    /// Used only for archive calls; never include this credential-bearing URL in evidence.
    fn archive_rpc(&self) -> Option<String>;
}
pub struct LocalRegistry {
    entries: Vec<LocalProposal>,
}
impl LocalRegistry {
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let entries: Vec<LocalProposal> =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let mut keys = std::collections::BTreeSet::new();
        for p in &entries {
            if !keys.insert((p.gate.clone(), p.proposal_id)) {
                return Err("duplicate local proposal".into());
            }
        }
        Ok(Self { entries })
    }
}
impl ProposalSource for LocalRegistry {
    fn archive_rpc(&self) -> Option<String> {
        None
    }
    fn archive_provider(&self) -> String {
        "local registry, not chain observed".into()
    }
    fn get(&self, gate: &AccountId, id: u64) -> Result<Option<LocalProposal>, SourceError> {
        Ok(self
            .entries
            .iter()
            .find(|p| &p.gate == gate && p.proposal_id == id)
            .cloned())
    }
}

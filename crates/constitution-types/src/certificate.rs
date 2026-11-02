//! Canonical certificate field and enum order. Encoding does not validate gate policy.

pub const DOMAIN: [u8; 16] = *b"constitution.v1\0";

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct Certificate {
    pub domain: [u8; 16],
    pub gate: crate::AccountId,
    pub proposal_id: u64,
    pub attempt: u32,
    pub chain_id: u64,
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub proxy: [u8; 20],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub executor: [u8; 20],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub current_impl: [u8; 20],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub current_impl_codehash: [u8; 32],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub new_impl: [u8; 20],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub new_impl_codehash: [u8; 32],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub init_calldata_hash: [u8; 32],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub source_commitment: [u8; 32],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub rule_set_hash: [u8; 32],
    #[serde(with = "crate::bytes::base64_32")]
    #[schemars(with = "crate::Base64<32>")]
    pub verifier_pubkey: [u8; 32],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub verifier_release_hash: [u8; 32],
    pub evidence_block_number: u64,
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub evidence_block_hash: [u8; 32],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub evidence_root: [u8; 32],
    pub ai: AiReview,
    pub verdict: Verdict,
    pub issued_at_s: u64,
    pub expires_at_s: u64,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum AiReview {
    Clean {
        #[serde(with = "crate::bytes::hex32")]
        #[schemars(with = "crate::Hex32")]
        evidence_hash: [u8; 32],
        signature_kind: u8,
    },
    Vetoed {
        #[serde(with = "crate::bytes::hex32")]
        #[schemars(with = "crate::Hex32")]
        evidence_hash: [u8; 32],
        #[serde(with = "crate::bytes::hex32")]
        #[schemars(with = "crate::Hex32")]
        counterexample_hash: [u8; 32],
    },
    NotRun,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum Verdict {
    Pass,
    Reject { rule_id: u8, reason_code: u16 },
}

impl Certificate {
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, borsh::io::Error> {
        borsh::to_vec(self)
    }

    /// Decode exactly one certificate, rejecting truncated or trailing bytes.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, borsh::io::Error> {
        borsh::from_slice(bytes)
    }
}

/// Host-side SHA-256 of already encoded canonical bytes. The gate uses env::sha256_array.
#[cfg(not(target_arch = "wasm32"))]
pub fn certificate_hash(canonical_bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(canonical_bytes).into()
}

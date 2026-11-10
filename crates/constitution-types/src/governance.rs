//! Timelocked governance storage. Each gate fixes its delay at initialization.

pub const DEFAULT_GOVERNANCE_DELAY_DAYS: u32 = 3;
pub const MIN_GOVERNANCE_DELAY_DAYS: u32 = 1;
pub const MAX_GOVERNANCE_DELAY_DAYS: u32 = 30;

fn default_governance_delay_days() -> u32 {
    DEFAULT_GOVERNANCE_DELAY_DAYS
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct Config {
    pub chain_id: u64,
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub executor: [u8; 20],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub proxy: [u8; 20],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub proxy_admin: [u8; 20],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub derived_eoa: [u8; 20],
    pub mpc_account: crate::AccountId,
    pub mpc_path: String,
    pub mpc_domain_id: u32,
    /// Whole days, selected at initialization; no governance change can alter this value.
    #[serde(default = "default_governance_delay_days")]
    pub governance_delay_days: u32,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct Constitution {
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub rule_set_hash: [u8; 32],
    pub manifest_uri: String,
    pub ai_required: bool,
    pub params: Params,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct Params {
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub proposal_deposit: u128,
    pub proposal_ttl_s: u64,
    pub max_cert_ttl_s: u64,
    pub sign_validity_s: u64,
    pub gas_limit: u64,
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub max_fee_per_gas: u128,
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub max_priority_fee_per_gas: u128,
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub fee_cap: u128,
    pub sign_gas_tgas: u64,
    pub callback_gas_tgas: u64,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct Roles {
    pub governor: crate::AccountId,
    pub guardian: crate::AccountId,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct VerifierRecord {
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub release_hash: [u8; 32],
    pub attestation_uri: String,
    pub added_at_ns: u64,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum ChangeKind {
    SetRuleSet {
        #[serde(with = "crate::bytes::hex32")]
        #[schemars(with = "crate::Hex32")]
        rule_set_hash: [u8; 32],
        manifest_uri: String,
        ai_required: bool,
    },
    AddVerifier {
        #[serde(with = "crate::bytes::base64_32")]
        #[schemars(with = "crate::Base64<32>")]
        pubkey: [u8; 32],
        #[serde(with = "crate::bytes::hex32")]
        #[schemars(with = "crate::Hex32")]
        release_hash: [u8; 32],
        attestation_uri: String,
    },
    RemoveVerifier {
        #[serde(with = "crate::bytes::base64_32")]
        #[schemars(with = "crate::Base64<32>")]
        pubkey: [u8; 32],
    },
    SetParams {
        params: Params,
    },
    SetGovernor {
        account: crate::AccountId,
    },
    SetGuardian {
        account: crate::AccountId,
    },
    MigrateAuthority {
        #[serde(with = "crate::bytes::hex20")]
        #[schemars(with = "crate::Hex20")]
        new_signer: [u8; 20],
    },
    Unpause,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum ChangeStatus {
    Pending,
    Applied,
    Cancelled,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub struct PendingChange {
    pub id: u64,
    pub kind: ChangeKind,
    pub scheduled_by: crate::AccountId,
    pub scheduled_at_ns: u64,
    pub eta_ns: u64,
    pub status: ChangeStatus,
    pub evm: Option<crate::EvmTx>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    borsh::BorshSerialize,
    borsh::BorshDeserialize,
    borsh::BorshSchema,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
pub enum SignTarget {
    Proposal(u64),
    Change(u64),
}

/// Hash the exact rotateSigner change-id preimage without an intermediate allocation.
pub fn change_id_hash(gate: &crate::AccountId, change_id: u64) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"constitution.v1/change");
    // AccountId's Borsh representation is its string: u32 LE length, then UTF-8.
    hash.update((gate.as_str().len() as u32).to_le_bytes());
    hash.update(gate.as_str().as_bytes());
    hash.update(change_id.to_le_bytes());
    hash.finalize().into()
}

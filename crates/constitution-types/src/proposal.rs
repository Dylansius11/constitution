//! Proposal storage and public registration arguments.

pub const MAX_INIT_CALLDATA: usize = 1024;

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
pub enum ProposalState {
    Pending,
    Rejected,
    Expired,
    SignRequested,
    Signed,
    SignFailed,
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
pub struct Proposal {
    pub id: u64,
    pub proposer: crate::AccountId,
    pub created_at_ns: u64,
    pub expires_at_ns: u64,
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub rule_set_hash: [u8; 32],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub new_impl: [u8; 20],
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub new_impl_codehash: [u8; 32],
    #[serde(with = "crate::bytes::hex20")]
    #[schemars(with = "crate::Hex20")]
    pub expected_current_impl: [u8; 20],
    #[serde(with = "crate::bytes::hex_bytes")]
    #[schemars(with = "crate::HexBytes")]
    pub init_calldata: Vec<u8>,
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub source_commitment: [u8; 32],
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub reserved: u128,
    pub state: ProposalState,
    #[serde(with = "crate::bytes::optional_hex32")]
    #[schemars(with = "Option<crate::Hex32>")]
    pub certificate_hash: Option<[u8; 32]>,
    pub rejection: Option<Rejection>,
    pub evm: Option<EvmTx>,
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
pub struct Rejection {
    pub rule_id: u8,
    pub reason_code: u16,
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub evidence_root: [u8; 32],
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
pub struct EvmTx {
    pub nonce: u64,
    pub deadline_s: u64,
    pub gas_limit: u64,
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub max_fee_per_gas: u128,
    #[serde(with = "crate::bytes::u128_str")]
    #[schemars(with = "crate::U128Str")]
    pub max_priority_fee_per_gas: u128,
    #[serde(with = "crate::bytes::hex32")]
    #[schemars(with = "crate::Hex32")]
    pub signing_hash: [u8; 32],
    #[serde(with = "crate::bytes::optional_hex_bytes")]
    #[schemars(with = "Option<crate::HexBytes>")]
    pub signed_tx: Option<Vec<u8>>,
    pub attempts: u32,
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
pub struct ProposeArgs {
    pub new_impl: crate::Hex20,
    pub new_impl_codehash: crate::Hex32,
    pub expected_current_impl: crate::Hex20,
    pub init_calldata: crate::HexBytes,
    pub source_commitment: crate::Hex32,
}

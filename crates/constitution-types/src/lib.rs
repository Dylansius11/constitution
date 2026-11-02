//! Canonical bytes shared by the gate, verifier and CLI. See docs/technical/SMART-CONTRACT.md.
//!
//! [I] SMART-CONTRACT storage fields retain raw arrays, vectors and amounts, in canonical
//! Borsh order. Serde adapters expose strict lowercase hex, padded base64 keys, and decimal
//! amount strings without altering storage bytes. Encoding is separate from gate policy:
//! callers still validate domains, bindings, TTLs, roles and calldata limits.
//!
//! [I] JsonSchema is always available, not feature-gated: cargo-near enables the gate's
//! near-sdk/__abi-generate, not arbitrary dependency features. Its shared argument and
//! result types therefore must already implement schemars 0.8 JsonSchema.
//!
//! [V] AccountId is near-account-id 2.6.0. near-sdk 5.29.1 re-exports its core types,
//! whose AccountId is the same near-account-id 2.x type (checked 2026-11-03):
//! <https://docs.rs/near-sdk/5.29.1/src/near_sdk/types/mod.rs.html>,
//! <https://docs.rs/near-sdk-core/4.2.3/src/near_sdk_core/types/mod.rs.html>.
//! The SDK's ^2.3.0 dependency admits this crate's exact 2.6.0 pin.

pub mod bytes;
pub mod certificate;
pub mod governance;
pub mod proposal;
pub mod reasons;
pub mod time;

pub use bytes::{Base64, Base64Bytes, Hex20, Hex32, HexBytes, U128Str};
#[cfg(not(target_arch = "wasm32"))]
pub use certificate::certificate_hash;
pub use certificate::{AiReview, Certificate, DOMAIN, Verdict};
pub use governance::{
    ChangeKind, ChangeStatus, Config, Constitution, Params, PendingChange, Roles, SignTarget,
    TIMELOCK_NS, VerifierRecord, change_id_hash,
};
pub use near_account_id::AccountId;
pub use proposal::{EvmTx, MAX_INIT_CALLDATA, Proposal, ProposalState, ProposeArgs, Rejection};
pub use reasons::{GateError, ReasonCode, RuleId};
pub use time::{ns_to_s, s_to_ns};

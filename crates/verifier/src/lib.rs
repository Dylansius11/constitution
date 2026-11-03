//! [I] Local deterministic verifier plumbing. No attestation or Pass issuance.
pub mod api;
pub mod bundle;
pub mod chain;
pub mod identity;
pub mod manifest;
pub mod pipeline;
pub mod proposals;
pub mod replay;
pub mod rules;
pub mod solvency;
pub mod submit;
pub mod tools;
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).into()
}
pub fn keccak(bytes: &[u8]) -> [u8; 32] {
    use sha3::{Digest, Keccak256};
    Keccak256::digest(bytes).into()
}

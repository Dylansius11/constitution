//! Exact executor calls only. No caller-controlled transaction or signing hash.
use super::*;
use alloy_sol_types::{SolCall, sol};
use constitution_types::{EvmTx, SignTarget, change_id_hash};
use omni_transaction::{EVM, TransactionBuilder, TxBuilder, evm::EVMTransaction};

sol! {
    function execute(bytes32 certId, address newImpl, bytes32 newImplCodehash,
        address expectedCurrentImpl, uint64 deadline, bytes initCalldata);
    function rotateSigner(address newSigner, bytes32 changeId, uint64 deadline);
}

pub(crate) fn proposal_transaction(
    config: &Config,
    proposal: &Proposal,
    evm: &EvmTx,
) -> Result<EVMTransaction, &'static str> {
    let calldata = executeCall {
        certId: proposal
            .certificate_hash
            .ok_or("missing certificate hash")?
            .into(),
        newImpl: proposal.new_impl.into(),
        newImplCodehash: proposal.new_impl_codehash.into(),
        expectedCurrentImpl: proposal.expected_current_impl.into(),
        deadline: evm.deadline_s,
        initCalldata: proposal.init_calldata.clone().into(),
    }
    .abi_encode();
    Ok(transaction(config, evm, calldata))
}
pub(crate) fn rotate_transaction(
    config: &Config,
    evm: &EvmTx,
    new_signer: [u8; 20],
    change_id: [u8; 32],
) -> EVMTransaction {
    let calldata = rotateSignerCall {
        newSigner: new_signer.into(),
        changeId: change_id.into(),
        deadline: evm.deadline_s,
    }
    .abi_encode();
    transaction(config, evm, calldata)
}
fn transaction(config: &Config, evm: &EvmTx, calldata: Vec<u8>) -> EVMTransaction {
    TransactionBuilder::new::<EVM>()
        .chain_id(config.chain_id)
        .nonce(evm.nonce)
        .max_priority_fee_per_gas(evm.max_priority_fee_per_gas)
        .max_fee_per_gas(evm.max_fee_per_gas)
        .gas_limit(evm.gas_limit.into())
        .to(config.executor)
        .value(0)
        .input(calldata)
        .access_list(vec![])
        .build()
}
impl Gate {
    pub(crate) fn target_transaction(
        &self,
        target: &SignTarget,
    ) -> Result<EVMTransaction, &'static str> {
        match target {
            SignTarget::Proposal(id) => {
                let proposal = self.proposals.get(id).ok_or("missing proposal")?;
                proposal_transaction(
                    &self.config,
                    proposal,
                    proposal.evm.as_ref().ok_or("missing EVM transaction")?,
                )
            }
            SignTarget::Change(id) => {
                let change = self.changes.get(id).ok_or("missing change")?;
                let evm = change.evm.as_ref().ok_or("missing migration transaction")?;
                let ChangeKind::MigrateAuthority { new_signer } = change.kind else {
                    return Err("not a migration");
                };
                Ok(rotate_transaction(
                    &self.config,
                    evm,
                    new_signer,
                    change_id_hash(&env::current_account_id(), *id),
                ))
            }
        }
    }
    pub(crate) fn new_evm(&self, deadline_s: u64) -> EvmTx {
        let p = &self.constitution.params;
        EvmTx {
            nonce: self.next_evm_nonce,
            deadline_s,
            gas_limit: p.gas_limit,
            max_fee_per_gas: p.max_fee_per_gas,
            max_priority_fee_per_gas: p.max_priority_fee_per_gas,
            signing_hash: [0; 32],
            signed_tx: None,
            attempts: 1,
        }
    }
}

#[derive(serde::Deserialize)]
struct SignatureResponse {
    scheme: Scheme,
    big_r: Point,
    s: Scalar,
    recovery_id: u8,
}
#[derive(serde::Deserialize)]
enum Scheme {
    Secp256k1,
}
#[derive(serde::Deserialize)]
struct Point {
    affine_point: String,
}
#[derive(serde::Deserialize)]
struct Scalar {
    scalar: String,
}

pub(crate) fn assemble_signed(
    tx: &EVMTransaction,
    response: &[u8],
    expected_address: [u8; 20],
) -> Result<Vec<u8>, &'static str> {
    let response: SignatureResponse =
        serde_json::from_slice(response).map_err(|_| "invalid MPC response JSON shape")?;
    let Scheme::Secp256k1 = response.scheme;
    if response.recovery_id > 1 {
        return Err("invalid recovery parity");
    }
    let point = response.big_r.affine_point;
    if point.len() != 66 || !(point.starts_with("02") || point.starts_with("03")) {
        return Err("invalid compressed R point");
    }
    fn scalar(text: &str, max: &str) -> Result<Vec<u8>, &'static str> {
        if text.len() != 64
            || !text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || text > max
        {
            return Err("invalid scalar");
        }
        let mut bytes = [0; 32];
        hex::decode_to_slice(text, &mut bytes).map_err(|_| "invalid scalar hex")?;
        let first = bytes.iter().position(|b| *b != 0).ok_or("zero scalar")?;
        Ok(bytes[first..].to_vec())
    }
    let r = scalar(
        &point[2..],
        "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140",
    )?;
    let s = scalar(
        &response.s.scalar,
        "7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0",
    )?;
    let mut r_full = [0u8; 32];
    r_full[32 - r.len()..].copy_from_slice(&r);
    let mut s_full = [0u8; 32];
    s_full[32 - s.len()..].copy_from_slice(&s);
    let signature =
        k256::ecdsa::Signature::from_scalars(r_full, s_full).map_err(|_| "invalid signature")?;
    let recovery = k256::ecdsa::RecoveryId::from_byte(response.recovery_id)
        .ok_or("invalid recovery parity")?;
    let digest = env::keccak256_array(tx.build_for_signing());
    let key = k256::ecdsa::VerifyingKey::recover_from_prehash(&digest, &signature, recovery)
        .map_err(|_| "invalid signature")?;
    let point = key.to_encoded_point(false);
    if env::keccak256_array(&point.as_bytes()[1..])[12..] != expected_address {
        return Err("signature signer mismatch");
    }
    Ok(
        tx.build_with_signature(&omni_transaction::evm::types::Signature {
            v: response.recovery_id.into(),
            r,
            s,
        }),
    )
}

#[cfg(test)]
#[path = "enc_vectors.rs"]
mod enc_vectors;

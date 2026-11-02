use super::*;
use crate::GateExt;
use constitution_types::{
    AiReview, Base64Bytes, Certificate, DOMAIN, Rejection, SignTarget, Verdict,
};
use near_sdk::PromiseOrValue;

#[near]
impl Gate {
    pub fn submit_verdict(
        &mut self,
        certificate: Base64Bytes,
        signature: Base64<64>,
    ) -> PromiseOrValue<Proposal> {
        // The order below is part of the public rejection contract. No writes before step 8.
        let certificate_bytes = certificate.0;
        let certificate = Certificate::from_canonical_bytes(&certificate_bytes)
            .unwrap_or_else(|_| env::panic_str(GateError::BadEncoding.code()));
        check(
            certificate.domain == DOMAIN && certificate.gate == env::current_account_id(),
            GateError::WrongDomain,
            "certificate domain or gate",
        );
        check(
            self.verifiers
                .get(&certificate.verifier_pubkey)
                .is_some_and(|v| v.release_hash == certificate.verifier_release_hash),
            GateError::UnknownVerifier,
            "key or release hash",
        );
        let message = env::sha256_array(&certificate_bytes);
        check(
            env::ed25519_verify(&signature.0, message, &certificate.verifier_pubkey),
            GateError::BadSignature,
            "certificate signature",
        );
        let id = certificate.proposal_id;
        let proposal = self
            .proposals
            .get(&id)
            .unwrap_or_else(|| env::panic_str(GateError::ProposalNotFound.code()));
        check(
            proposal.state == ProposalState::Pending,
            GateError::WrongState,
            "verdict requires Pending",
        );
        let now_ns = env::block_timestamp();
        let now_s = ns_to_s(now_ns);
        check(
            now_ns <= proposal.expires_at_ns,
            GateError::ProposalExpired,
            "proposal expiry",
        );
        macro_rules! bind {
            ($field:ident,$want:expr) => {
                check(
                    certificate.$field == $want,
                    GateError::BindingMismatch,
                    stringify!($field),
                );
            };
        }
        bind!(chain_id, self.config.chain_id);
        bind!(proxy, self.config.proxy);
        bind!(executor, self.config.executor);
        bind!(new_impl, proposal.new_impl);
        bind!(new_impl_codehash, proposal.new_impl_codehash);
        bind!(source_commitment, proposal.source_commitment);
        bind!(current_impl, proposal.expected_current_impl);
        bind!(
            init_calldata_hash,
            env::keccak256_array(&proposal.init_calldata)
        );
        check(
            certificate.rule_set_hash == proposal.rule_set_hash
                && proposal.rule_set_hash == self.constitution.rule_set_hash,
            GateError::BindingMismatch,
            "rule_set_hash",
        );
        check(
            certificate.issued_at_s <= now_s.saturating_add(60)
                && now_s <= certificate.expires_at_s
                && certificate
                    .expires_at_s
                    .checked_sub(certificate.issued_at_s)
                    .is_some_and(|ttl| ttl <= self.constitution.params.max_cert_ttl_s),
            GateError::CertTime,
            "certificate time",
        );
        if let Verdict::Reject {
            rule_id,
            reason_code,
        } = certificate.verdict
        {
            let mut proposal = self.proposals.remove(&id).unwrap();
            proposal.certificate_hash = Some(message);
            proposal.rejection = Some(Rejection {
                rule_id,
                reason_code,
                evidence_root: certificate.evidence_root,
            });
            self.release_reservation(&mut proposal);
            proposal.state = ProposalState::Rejected;
            emit(
                "verdict_rejected",
                json!({"proposal_id":id,"certificate_hash":constitution_types::Hex32(message),"rejection":&proposal.rejection}),
            );
            let view = proposal.clone();
            self.proposals.insert(id, proposal);
            return PromiseOrValue::Value(view);
        }
        // A Vetoed review can never accompany Pass, even when the rule set does not require AI.
        check(
            !matches!(certificate.ai, AiReview::Vetoed { .. })
                && (!self.constitution.ai_required
                    || matches!(certificate.ai, AiReview::Clean { .. })),
            GateError::AiReviewRequired,
            "Clean AI review required",
        );
        check(
            !self.signing_paused,
            GateError::SigningPaused,
            "signing paused",
        );
        let next_nonce = self
            .next_evm_nonce
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
        let mut evm = self.new_evm(
            certificate
                .expires_at_s
                .min(now_s.saturating_add(self.constitution.params.sign_validity_s)),
        );
        let mut proposal = self.proposals.remove(&id).unwrap();
        proposal.certificate_hash = Some(message);
        evm.signing_hash = env::keccak256_array(
            crate::evm::proposal_transaction(&self.config, &proposal, &evm)
                .expect("certificate hash assigned before encoding")
                .build_for_signing(),
        );
        proposal.evm = Some(evm);
        proposal.state = ProposalState::SignRequested;
        self.proposals.insert(id, proposal);
        self.next_evm_nonce = next_nonce;
        self.outstanding_signatures.insert(id);
        PromiseOrValue::Promise(self.request_signature(SignTarget::Proposal(id)))
    }
}

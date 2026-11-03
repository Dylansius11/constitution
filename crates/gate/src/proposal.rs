use super::*;
use crate::{GateExt, credit::active_key};

#[near]
impl Gate {
    pub fn propose(&mut self, args: ProposeArgs) -> u64 {
        let proposer = env::predecessor_account_id();
        check(
            !self.active_by_account.contains_key(&proposer),
            GateError::ActiveProposalExists,
            "account active",
        );
        let balance = self.credit.get(&proposer).copied().unwrap_or(0);
        let reserved = self.constitution.params.proposal_deposit;
        check(
            balance >= reserved,
            GateError::InsufficientCredit,
            "proposal reservation",
        );
        check(
            args.init_calldata.0.len() <= MAX_INIT_CALLDATA,
            GateError::InitCalldataTooLong,
            "maximum 1024 bytes",
        );
        check(
            args.new_impl.0 != [0; 20] && args.expected_current_impl.0 != [0; 20],
            GateError::ZeroAddress,
            "implementation",
        );
        let key = active_key(&proposer, &args.new_impl_codehash.0, &args.init_calldata.0);
        check(
            !self.active_by_key.contains_key(&key),
            GateError::DuplicateActive,
            "proposer, codehash and calldata active",
        );
        let now = env::block_timestamp();
        let expires_at_ns = s_to_ns(self.constitution.params.proposal_ttl_s)
            .and_then(|ttl| now.checked_add(ttl))
            .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
        let id = self.next_proposal_id;
        let next = id
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
        let proposal = Proposal {
            id,
            proposer: proposer.clone(),
            created_at_ns: now,
            expires_at_ns,
            rule_set_hash: self.constitution.rule_set_hash,
            new_impl: args.new_impl.0,
            new_impl_codehash: args.new_impl_codehash.0,
            expected_current_impl: args.expected_current_impl.0,
            init_calldata: args.init_calldata.0,
            source_commitment: args.source_commitment.0,
            reserved,
            state: ProposalState::Pending,
            certificate_hash: None,
            rejection: None,
            evm: None,
        };
        emit(
            "proposal_created",
            json!({"proposal_id": id, "proposal": &proposal,
            "rule_set_hash": constitution_types::Hex32(proposal.rule_set_hash)}),
        );
        self.credit.insert(proposer.clone(), balance - reserved);
        self.active_by_account.insert(proposer, id);
        self.active_by_key.insert(key, id);
        self.proposals.insert(id, proposal);
        self.next_proposal_id = next;
        self.assert_solvent(0);
        id
    }

    pub fn expire(&mut self, proposal_id: u64) {
        let proposal = self
            .proposals
            .get(&proposal_id)
            .unwrap_or_else(|| env::panic_str(GateError::ProposalNotFound.code()));
        check(
            proposal.state == ProposalState::Pending,
            GateError::WrongState,
            "expire requires Pending",
        );
        check(
            env::block_timestamp() > proposal.expires_at_ns,
            GateError::NotExpired,
            "proposal still valid",
        );
        let mut proposal = self.proposals.remove(&proposal_id).unwrap();
        self.release_reservation(&mut proposal);
        proposal.state = ProposalState::Expired;
        self.proposals.insert(proposal_id, proposal);
        self.assert_solvent(0);
        emit("proposal_expired", json!({"proposal_id": proposal_id}));
    }
}

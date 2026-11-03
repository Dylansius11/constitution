use super::*;
use crate::GateExt;
use constitution_types::{Hex32, HexBytes, SignTarget};
use near_sdk::{Gas, PromiseOrValue};

#[derive(borsh::BorshSerialize, borsh::BorshDeserialize, PartialEq, Eq)]
pub(crate) enum MigrationSigning {
    InFlight,
    Failed,
    Signed,
}

#[near]
impl Gate {
    /// Raw results are parsed here, not by generated callback argument deserialization.
    #[private]
    pub fn on_signed(&mut self, target: SignTarget, attempt: u32) -> Option<&Proposal> {
        let current = match &target {
            SignTarget::Proposal(id) => self.proposals.get(id).is_some_and(|p| {
                p.state == ProposalState::SignRequested
                    && p.evm.as_ref().is_some_and(|e| e.attempts == attempt)
            }),
            SignTarget::Change(id) => {
                self.migration_signing.get(id) == Some(&MigrationSigning::InFlight)
                    && self
                        .changes
                        .get(id)
                        .and_then(|c| c.evm.as_ref())
                        .is_some_and(|e| e.attempts == attempt)
            }
        };
        if !current {
            env::log_str("Ignoring stale signing callback");
            return None;
        }
        self.flush_storage();
        let result = if env::promise_results_count() != 1 {
            Err("expected one MPC result")
        } else {
            env::promise_result_checked(0, 4096)
                .map_err(|_| "MPC promise failed or oversized")
                .and_then(|bytes| {
                    self.target_transaction(&target)
                        .and_then(|tx| crate::evm::assemble_signed(&tx, &bytes))
                })
                .and_then(|raw| {
                    // A signed transaction adds bytes to an existing record. Fail
                    // closed without panicking if the gate needs an operating top-up.
                    let extra = u128::try_from(raw.len() + 128)
                        .unwrap_or(u128::MAX)
                        .saturating_mul(env::storage_byte_cost().as_yoctonear());
                    if !self.has_liquid_backing(extra) {
                        Err("insufficient operating storage balance")
                    } else {
                        Ok(raw)
                    }
                })
        };
        match target {
            SignTarget::Proposal(id) => {
                let mut proposal = self.proposals.remove(&id)?;
                let result = match result {
                    Ok(raw) if self.try_release_reservation(&mut proposal) => Ok(raw),
                    Ok(_) => Err("reservation accounting failed"),
                    Err(error) => Err(error),
                };
                match result {
                    Ok(raw) => {
                        proposal.state = ProposalState::Signed;
                        if let Some(evm) = proposal.evm.as_mut() {
                            emit(
                                "signed",
                                json!({"proposal_id":id,"nonce":evm.nonce,"tx_hash":Hex32(env::keccak256_array(&raw)),"raw_tx":HexBytes(raw.clone())}),
                            );
                            evm.signed_tx = Some(raw);
                        }
                    }
                    Err(error) => {
                        proposal.state = ProposalState::SignFailed;
                        emit(
                            "sign_failed",
                            json!({"proposal_id":id,"attempt":attempt,"error":error}),
                        );
                    }
                }
                self.proposals.insert(id, proposal);
                self.proposals.get(&id)
            }
            SignTarget::Change(id) => {
                match result {
                    Ok(raw) => {
                        if let Some(evm) = self.changes.get_mut(&id).and_then(|c| c.evm.as_mut()) {
                            emit(
                                "signed",
                                json!({"change_id":id,"nonce":evm.nonce,"tx_hash":Hex32(env::keccak256_array(&raw)),"raw_tx":HexBytes(raw.clone())}),
                            );
                            evm.signed_tx = Some(raw);
                        }
                        self.migration_signing.insert(id, MigrationSigning::Signed);
                    }
                    Err(error) => {
                        self.migration_signing.insert(id, MigrationSigning::Failed);
                        emit(
                            "sign_failed",
                            json!({"change_id":id,"attempt":attempt,"error":error}),
                        );
                    }
                }
                None
            }
        }
    }

    pub fn retry_sign(&mut self, proposal_id: u64) -> Promise {
        let proposal = self
            .proposals
            .get(&proposal_id)
            .unwrap_or_else(|| env::panic_str(GateError::ProposalNotFound.code()));
        check(
            proposal.state == ProposalState::SignFailed
                || (proposal.state == ProposalState::SignRequested
                    && self
                        .sign_requested_at
                        .get(&proposal_id)
                        .is_some_and(|started| {
                            env::block_timestamp().saturating_sub(*started) >= SIGN_REQUEST_TIMEOUT
                        })),
            GateError::WrongState,
            "retry requires SignFailed or timed-out SignRequested",
        );
        self.check_resigning(proposal);
        let attempt = proposal
            .evm
            .as_ref()
            .unwrap()
            .attempts
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str(GateError::TooManyBumps.code()));
        let proposal = self.proposals.get_mut(&proposal_id).unwrap();
        proposal.evm.as_mut().unwrap().attempts = attempt;
        proposal.state = ProposalState::SignRequested;
        self.outstanding_signatures.insert(proposal_id);
        self.sign_requested_at
            .insert(proposal_id, env::block_timestamp());
        self.assert_solvent(1);
        self.request_signature(SignTarget::Proposal(proposal_id))
    }

    pub fn bump_fee(&mut self, target: SignTarget) -> Promise {
        let (evm, bumps) = match &target {
            SignTarget::Proposal(id) => {
                let proposal = self
                    .proposals
                    .get(id)
                    .unwrap_or_else(|| env::panic_str(GateError::ProposalNotFound.code()));
                check(
                    proposal.state == ProposalState::Signed,
                    GateError::WrongState,
                    "fee bump requires Signed",
                );
                self.check_resigning(proposal);
                (
                    proposal.evm.as_ref().unwrap(),
                    self.fee_bumps.get(id).copied().unwrap_or(0),
                )
            }
            SignTarget::Change(id) => {
                let change = self
                    .changes
                    .get(id)
                    .unwrap_or_else(|| env::panic_str(GateError::ChangeNotFound.code()));
                check(
                    self.migration_signing.get(id) == Some(&MigrationSigning::Signed),
                    GateError::WrongState,
                    "fee bump requires Signed migration",
                );
                check(
                    !self.signing_paused,
                    GateError::SigningPaused,
                    "signing paused",
                );
                (
                    change.evm.as_ref().unwrap(),
                    self.migration_fee_bumps.get(id).copied().unwrap_or(0),
                )
            }
        };
        check(
            bumps < 8,
            GateError::TooManyBumps,
            "eight fee bumps maximum",
        );
        let mut evm = evm.clone();
        let bump = |fee: u128| {
            fee.checked_add(fee.div_ceil(4))
                .unwrap_or_else(|| env::panic_str(GateError::FeeCap.code()))
        };
        evm.max_fee_per_gas = bump(evm.max_fee_per_gas);
        evm.max_priority_fee_per_gas = bump(evm.max_priority_fee_per_gas);
        check(
            evm.max_fee_per_gas <= self.constitution.params.fee_cap,
            GateError::FeeCap,
            "timelocked fee cap",
        );
        evm.attempts = evm
            .attempts
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str(GateError::TooManyBumps.code()));
        let tx = match &target {
            SignTarget::Proposal(id) => crate::evm::proposal_transaction(
                &self.config,
                self.proposals.get(id).unwrap(),
                &evm,
            )
            .unwrap(),
            SignTarget::Change(id) => {
                let ChangeKind::MigrateAuthority { new_signer } =
                    self.changes.get(id).unwrap().kind
                else {
                    env::panic_str(GateError::WrongState.code());
                };
                crate::evm::rotate_transaction(
                    &self.config,
                    &evm,
                    new_signer,
                    constitution_types::change_id_hash(&env::current_account_id(), *id),
                )
            }
        };
        evm.signing_hash = env::keccak256_array(tx.build_for_signing());
        match &target {
            SignTarget::Proposal(id) => {
                let proposal = self.proposals.get_mut(id).unwrap();
                proposal.evm = Some(evm);
                proposal.state = ProposalState::SignRequested;
                self.fee_bumps.insert(*id, bumps + 1);
                self.outstanding_signatures.insert(*id);
                self.sign_requested_at.insert(*id, env::block_timestamp());
            }
            SignTarget::Change(id) => {
                self.changes.get_mut(id).unwrap().evm = Some(evm);
                self.migration_fee_bumps.insert(*id, bumps + 1);
                self.migration_signing
                    .insert(*id, MigrationSigning::InFlight);
                self.migration_requested_at
                    .insert(*id, env::block_timestamp());
            }
        }
        self.assert_solvent(1);
        self.request_signature(target)
    }

    pub fn signing_preimage(&self, proposal_id: u64) -> HexBytes {
        check(
            self.proposals.contains_key(&proposal_id),
            GateError::ProposalNotFound,
            "proposal",
        );
        let tx = self
            .target_transaction(&SignTarget::Proposal(proposal_id))
            .unwrap_or_else(|_| env::panic_str(GateError::WrongState.code()));
        HexBytes(tx.build_for_signing())
    }
}

impl Gate {
    fn check_resigning(&self, proposal: &Proposal) {
        check(
            !self.signing_paused,
            GateError::SigningPaused,
            "signing paused",
        );
        let evm = proposal
            .evm
            .as_ref()
            .unwrap_or_else(|| env::panic_str(GateError::WrongState.code()));
        check(
            proposal.rule_set_hash == self.constitution.rule_set_hash
                || evm.deadline_s <= ns_to_s(env::block_timestamp()),
            GateError::BindingMismatch,
            "rule_set_hash",
        );
    }

    pub(crate) fn request_signature(&self, target: SignTarget) -> Promise {
        let evm = match &target {
            SignTarget::Proposal(id) => self.proposals.get(id).and_then(|p| p.evm.as_ref()),
            SignTarget::Change(id) => self.changes.get(id).and_then(|c| c.evm.as_ref()),
        }
        .expect("signing request follows stored transaction");
        let mut data = json!({"attempt":evm.attempts,"signing_hash":Hex32(evm.signing_hash),"nonce":evm.nonce});
        match &target {
            SignTarget::Proposal(id) => data["proposal_id"] = json!(id),
            SignTarget::Change(id) => data["change_id"] = json!(id),
        }
        emit("sign_requested", data);
        Promise::new(self.config.mpc_account.clone()).function_call("sign",serde_json::to_vec(&json!({"request":{
            "payload_v2":{"Ecdsa":hex::encode(evm.signing_hash)},"path":&self.config.mpc_path,"domain_id":self.config.mpc_domain_id
        }})).expect("serializable MPC request"),NearToken::from_yoctonear(1),Gas::from_tgas(self.constitution.params.sign_gas_tgas))
            .then(Self::ext(env::current_account_id()).with_static_gas(Gas::from_tgas(self.constitution.params.callback_gas_tgas)).on_signed(target,evm.attempts))
    }

    pub(crate) fn apply_migration(&mut self, id: u64) -> PromiseOrValue<()> {
        check(
            !self.signing_paused,
            GateError::SigningPaused,
            "signing paused",
        );
        let change = self.changes.get(&id).unwrap();
        if change.status == ChangeStatus::Applied {
            check(
                self.migration_signing.get(&id) == Some(&MigrationSigning::Failed)
                    || (self.migration_signing.get(&id) == Some(&MigrationSigning::InFlight)
                        && self.migration_requested_at.get(&id).is_some_and(|started| {
                            env::block_timestamp().saturating_sub(*started) >= SIGN_REQUEST_TIMEOUT
                        })),
                GateError::NotPending,
                "migration is not retryable",
            );
            let evm = self.changes.get_mut(&id).unwrap().evm.as_mut().unwrap();
            evm.attempts = evm
                .attempts
                .checked_add(1)
                .unwrap_or_else(|| env::panic_str(GateError::TooManyBumps.code()));
        } else {
            let next = self
                .next_evm_nonce
                .checked_add(1)
                .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
            let deadline = ns_to_s(env::block_timestamp())
                .saturating_add(self.constitution.params.sign_validity_s);
            let evm = self.new_evm(deadline);
            let change = self.changes.get_mut(&id).unwrap();
            change.evm = Some(evm);
            change.status = ChangeStatus::Applied;
            let hash = env::keccak256_array(
                self.target_transaction(&SignTarget::Change(id))
                    .unwrap()
                    .build_for_signing(),
            );
            self.changes
                .get_mut(&id)
                .unwrap()
                .evm
                .as_mut()
                .unwrap()
                .signing_hash = hash;
            self.next_evm_nonce = next;
            self.pending_changes.retain(|pending| *pending != id);
            emit("change_applied", json!({"change_id":id}));
        }
        self.migration_signing
            .insert(id, MigrationSigning::InFlight);
        self.migration_requested_at
            .insert(id, env::block_timestamp());
        self.assert_solvent(1);
        PromiseOrValue::Promise(self.request_signature(SignTarget::Change(id)))
    }
}

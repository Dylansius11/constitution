use super::*;
use crate::GateExt;
use near_sdk::PromiseOrValue;

#[near]
impl Gate {
    pub fn schedule_change(&mut self, kind: ChangeKind) -> u64 {
        check(
            env::predecessor_account_id() == self.roles.governor,
            GateError::NotGovernor,
            "schedule only",
        );
        self.validate_change(&kind);
        let now = env::block_timestamp();
        let eta_ns = now
            .checked_add(TIMELOCK_NS)
            .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
        let id = self.next_change_id;
        let next = id
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
        let change = PendingChange {
            id,
            kind,
            scheduled_by: env::predecessor_account_id(),
            scheduled_at_ns: now,
            eta_ns,
            status: ChangeStatus::Pending,
            evm: None,
        };
        emit(
            "change_scheduled",
            json!({"change_id":id, "kind": &change.kind, "eta_ns":eta_ns}),
        );
        self.changes.insert(id, change);
        self.next_change_id = next;
        id
    }

    pub fn apply_change(&mut self, change_id: u64) -> PromiseOrValue<()> {
        let change = self
            .changes
            .get(&change_id)
            .unwrap_or_else(|| env::panic_str(GateError::ChangeNotFound.code()));
        if change.status == ChangeStatus::Applied
            && matches!(change.kind, ChangeKind::MigrateAuthority { .. })
        {
            return self.apply_migration(change_id);
        }
        check(
            change.status == ChangeStatus::Pending,
            GateError::NotPending,
            "change status",
        );
        let now = env::block_timestamp();
        if now < change.eta_ns {
            env::panic_str(&format!(
                "{}: eta_ns={} now_ns={now}",
                GateError::TimelockNotElapsed.code(),
                change.eta_ns
            ));
        }
        self.validate_change(&change.kind);
        if matches!(change.kind, ChangeKind::MigrateAuthority { .. }) {
            return self.apply_migration(change_id);
        }
        if let ChangeKind::RemoveVerifier { .. } = change.kind {
            check(
                self.verifiers.len() > 1,
                GateError::LastVerifier,
                "cannot remove last verifier",
            );
        }
        if let ChangeKind::SetRuleSet { .. } = change.kind {
            let now_s = ns_to_s(now);
            let mut expired = Vec::new();
            for id in self.outstanding_signatures.iter() {
                let proposal = self
                    .proposals
                    .get(id)
                    .unwrap_or_else(|| env::panic_str(GateError::SignaturesOutstanding.code()));
                match proposal.state {
                    ProposalState::SignRequested => {
                        env::panic_str(GateError::SignaturesOutstanding.code())
                    }
                    ProposalState::SignFailed | ProposalState::Signed => {
                        let evm = proposal.evm.as_ref().unwrap_or_else(|| {
                            env::panic_str(GateError::SignaturesOutstanding.code())
                        });
                        check(
                            evm.deadline_s <= now_s,
                            GateError::SignaturesOutstanding,
                            "signed deadline not elapsed",
                        );
                        expired.push(*id);
                    }
                    _ => expired.push(*id),
                }
            }
            for id in expired {
                self.outstanding_signatures.remove(&id);
            }
        }
        // All validation precedes state mutation. Move rather than clone the queued payload.
        let mut change = self.changes.remove(&change_id).unwrap();
        match &change.kind {
            ChangeKind::SetRuleSet {
                rule_set_hash,
                manifest_uri,
                ai_required,
            } => {
                self.constitution.rule_set_hash = *rule_set_hash;
                self.constitution.manifest_uri = manifest_uri.clone();
                self.constitution.ai_required = *ai_required;
            }
            ChangeKind::AddVerifier {
                pubkey,
                release_hash,
                attestation_uri,
            } => {
                self.verifiers.insert(
                    *pubkey,
                    VerifierRecord {
                        release_hash: *release_hash,
                        attestation_uri: attestation_uri.clone(),
                        added_at_ns: now,
                    },
                );
            }
            ChangeKind::RemoveVerifier { pubkey } => {
                self.verifiers.remove(pubkey);
            }
            ChangeKind::SetParams { params } => self.constitution.params = params.clone(),
            ChangeKind::SetGovernor { account } => self.roles.governor = account.clone(),
            ChangeKind::SetGuardian { account } => self.roles.guardian = account.clone(),
            ChangeKind::Unpause => self.signing_paused = false,
            ChangeKind::MigrateAuthority { .. } => unreachable!(),
        }
        change.status = ChangeStatus::Applied;
        self.changes.insert(change_id, change);
        emit("change_applied", json!({"change_id":change_id}));
        PromiseOrValue::Value(())
    }

    pub fn cancel_change(&mut self, change_id: u64) {
        self.assert_guardian();
        let change = self
            .changes
            .get_mut(&change_id)
            .unwrap_or_else(|| env::panic_str(GateError::ChangeNotFound.code()));
        check(
            change.status == ChangeStatus::Pending,
            GateError::NotPending,
            "change status",
        );
        change.status = ChangeStatus::Cancelled;
        emit("change_cancelled", json!({"change_id":change_id}));
    }

    pub fn pause_signing(&mut self) {
        self.assert_guardian();
        self.signing_paused = true;
        emit("signing_paused", json!({"change_id":null}));
    }

    pub fn remove_verifier_now(&mut self, pubkey: Base64<32>) {
        self.assert_guardian();
        check(
            self.verifiers.contains_key(&pubkey.0),
            GateError::InvalidChange,
            "unknown verifier",
        );
        check(
            self.verifiers.len() > 1,
            GateError::LastVerifier,
            "cannot remove last verifier",
        );
        self.verifiers.remove(&pubkey.0);
        emit(
            "verifier_removed",
            json!({"change_id":null,"pubkey":pubkey}),
        );
    }
}

impl Gate {
    fn assert_guardian(&self) {
        check(
            env::predecessor_account_id() == self.roles.guardian,
            GateError::NotGuardian,
            "guardian only",
        );
    }

    fn validate_change(&self, kind: &ChangeKind) {
        match kind {
            ChangeKind::SetRuleSet { manifest_uri, .. } => validate_uri(manifest_uri),
            ChangeKind::AddVerifier {
                pubkey,
                attestation_uri,
                ..
            } => {
                validate_uri(attestation_uri);
                check(
                    !self.verifiers.contains_key(pubkey),
                    GateError::InvalidChange,
                    "verifier already exists",
                );
            }
            ChangeKind::RemoveVerifier { pubkey } => {
                check(
                    self.verifiers.contains_key(pubkey),
                    GateError::InvalidChange,
                    "unknown verifier",
                );
            }
            ChangeKind::SetParams { params } => validate_params(params),
            ChangeKind::MigrateAuthority { new_signer } => check(
                *new_signer != [0; 20],
                GateError::InvalidChange,
                "zero signer",
            ),
            ChangeKind::SetGovernor { .. }
            | ChangeKind::SetGuardian { .. }
            | ChangeKind::Unpause => {}
        }
    }
}

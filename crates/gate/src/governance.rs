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
        if self.pending_changes.len() >= MAX_PENDING_CHANGES {
            env::panic_str(PENDING_CHANGE_LIMIT);
        }
        // Authority kinds are unique; every other kind has a small cap so one kind cannot take
        // every slot of the queue.
        let same_kind = self
            .pending_changes
            .iter()
            .filter(|id| {
                self.changes.get(id).is_some_and(|c| {
                    std::mem::discriminant(&c.kind) == std::mem::discriminant(&kind)
                })
            })
            .count();
        let cap = if is_authority_kind(&kind) {
            1
        } else {
            MAX_PENDING_PER_KIND
        };
        if same_kind >= cap {
            env::panic_str(DUPLICATE_AUTHORITY_CHANGE);
        }
        let now = env::block_timestamp();
        let delay_ns = u64::from(self.config.governance_delay_days)
            .checked_mul(86_400)
            .and_then(s_to_ns)
            .unwrap_or_else(|| env::panic_str(GateError::InvalidChange.code()));
        let eta_ns = now
            .checked_add(delay_ns)
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
        self.pending_changes.push(id);
        self.assert_solvent(0);
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
        if matches!(change.kind, ChangeKind::Unpause) {
            check(
                self.paused_at_ns
                    .is_none_or(|paused_at| change.scheduled_at_ns > paused_at),
                GateError::InvalidChange,
                "unpause must be scheduled after latest pause",
            );
        }
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
                    ProposalState::SignRequested
                    | ProposalState::SignFailed
                    | ProposalState::Signed => {
                        let evm = proposal.evm.as_ref().unwrap_or_else(|| {
                            env::panic_str(GateError::SignaturesOutstanding.code())
                        });
                        let timed_out = self.sign_requested_at.get(id).is_some_and(|started| {
                            now.saturating_sub(*started) >= SIGN_REQUEST_TIMEOUT
                        });
                        check(
                            evm.deadline_s < now_s
                                && (proposal.state != ProposalState::SignRequested || timed_out),
                            GateError::SignaturesOutstanding,
                            "signature deadline or request timeout not elapsed",
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
        let change = self.changes.remove(&change_id).unwrap();
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
        // The body is deleted, not kept: terminal records would be permanent storage that the
        // public governor key could create for free. Events carry the history.
        self.pending_changes.retain(|id| *id != change_id);
        self.assert_solvent(0);
        emit("change_applied", json!({"change_id":change_id}));
        PromiseOrValue::Value(())
    }

    pub fn cancel_change(&mut self, change_id: u64) {
        self.assert_guardian();
        let change = self
            .changes
            .get(&change_id)
            .unwrap_or_else(|| env::panic_str(GateError::ChangeNotFound.code()));
        check(
            change.status == ChangeStatus::Pending,
            GateError::NotPending,
            "change status",
        );
        self.changes.remove(&change_id);
        self.pending_changes.retain(|id| *id != change_id);
        emit("change_cancelled", json!({"change_id":change_id}));
    }

    pub fn cancel_changes(&mut self, ids: Vec<u64>) {
        self.assert_guardian();
        if ids.len() > MAX_PENDING_CHANGES {
            env::panic_str(PENDING_CHANGE_LIMIT);
        }
        for (index, id) in ids.iter().enumerate() {
            check(
                !ids[..index].contains(id),
                GateError::InvalidChange,
                "duplicate change id",
            );
            let change = self
                .changes
                .get(id)
                .unwrap_or_else(|| env::panic_str(GateError::ChangeNotFound.code()));
            check(
                change.status == ChangeStatus::Pending,
                GateError::NotPending,
                "change status",
            );
        }
        for id in ids {
            self.cancel_change(id);
        }
    }

    pub fn pause_signing(&mut self) {
        self.assert_guardian();
        self.signing_paused = true;
        self.paused_at_ns = Some(env::block_timestamp());
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
                release_hash,
                attestation_uri,
            } => {
                validate_uri(attestation_uri);
                check(
                    *release_hash != [0; 32],
                    GateError::InvalidChange,
                    "zero release_hash",
                );
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

fn is_authority_kind(kind: &ChangeKind) -> bool {
    matches!(
        kind,
        ChangeKind::MigrateAuthority { .. }
            | ChangeKind::SetGuardian { .. }
            | ChangeKind::SetGovernor { .. }
    )
}

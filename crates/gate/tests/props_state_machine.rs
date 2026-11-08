mod props_support;

use constitution_gate::Gate;
use constitution_types::*;
use near_sdk::{env, json_types::U128, test_utils::get_created_receipts};
use props_support::*;
use proptest::prelude::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Debug)]
enum Op {
    Deposit(u8, u16),
    Withdraw(u8, u16),
    Propose(u8, u8, u8),
    Verdict(u8, u8, u8),
    Expire(u8),
    Callback(u8, u8, bool),
    Retry(u8),
    Bump(u8),
    Pause(bool),
    RemoveVerifier(u8, bool),
    Schedule(u8, bool),
    Apply(u8),
    Cancel(u8, bool),
    Advance(u8),
}

fn operations() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..3, 0u16..301).prop_map(|(a, n)| Op::Deposit(a, n)),
        (0u8..3, 0u16..301).prop_map(|(a, n)| Op::Withdraw(a, n)),
        (0u8..3, 0u8..4, 0u8..3).prop_map(|(a, t, bad)| Op::Propose(a, t, bad)),
        (any::<u8>(), 0u8..9, 0u8..2).prop_map(|(p, v, k)| Op::Verdict(p, v, k)),
        any::<u8>().prop_map(Op::Expire),
        (any::<u8>(), 0u8..3, any::<bool>()).prop_map(|(p, r, stale)| Op::Callback(p, r, stale)),
        any::<u8>().prop_map(Op::Retry),
        any::<u8>().prop_map(Op::Bump),
        any::<bool>().prop_map(Op::Pause),
        (0u8..2, any::<bool>()).prop_map(|(k, authorized)| Op::RemoveVerifier(k, authorized)),
        (0u8..6, any::<bool>()).prop_map(|(k, authorized)| Op::Schedule(k, authorized)),
        any::<u8>().prop_map(Op::Apply),
        (any::<u8>(), any::<bool>()).prop_map(|(c, authorized)| Op::Cancel(c, authorized)),
        (0u8..10).prop_map(Op::Advance),
    ]
}

#[derive(Debug)]
struct ModelProposal {
    owner: u8,
    token: u8,
    state: ProposalState,
    expires: u64,
    reserved: u128,
    rules: [u8; 32],
    nonce: Option<u64>,
    deadline: u64,
    attempts: u32,
    requested_at: u64,
    bumps: u8,
    fee: u128,
    priority: u128,
    certified: bool,
    // A failed bump retains the previous signed transaction.
    signed: bool,
    hash: Option<[u8; 32]>,
}

struct Machine {
    gate: Option<Gate>,
    now: u64,
    credit: [u128; 3],
    entries: [bool; 3],
    deposits: u128,
    withdrawals: u128,
    storage_fees: u128,
    proposals: Vec<ModelProposal>,
    changes: Vec<Option<PendingChange>>,
    verifiers: [bool; 2],
    paused: bool,
    rules: [u8; 32],
    parameters: Params,
    next_nonce: u64,
}

impl Machine {
    fn new(nonce: u64) -> Self {
        Self {
            gate: Some(fresh_gate(nonce)),
            now: START,
            credit: [0; 3],
            entries: [false; 3],
            deposits: 0,
            withdrawals: 0,
            storage_fees: 0,
            proposals: vec![],
            changes: vec![],
            verifiers: [true, false],
            paused: false,
            rules: [5; 32],
            parameters: params(),
            next_nonce: nonce,
        }
    }

    fn proposal_id(&self, slot: u8) -> u64 {
        u64::from(slot) % (self.proposals.len() as u64 + 1)
    }

    fn change_id(&self, slot: u8) -> u64 {
        u64::from(slot) % (self.changes.len() as u64 + 1)
    }

    fn release(&mut self, id: usize) {
        let p = &mut self.proposals[id];
        self.credit[usize::from(p.owner)] += p.reserved;
        p.reserved = 0;
    }

    fn resign_error(&self, id: usize, state: ProposalState) -> Option<&'static str> {
        let p = &self.proposals[id];
        let timed_out = state == ProposalState::SignFailed
            && p.state == ProposalState::SignRequested
            && self.now >= p.requested_at + 1_200_000_000_000;
        if p.state != state && !timed_out {
            Some("E31_WRONG_STATE")
        } else if self.paused {
            Some("E20_SIGNING_PAUSED")
        } else if p.rules != self.rules && p.deadline > ns_to_s(self.now) {
            Some("E45_BINDING_MISMATCH")
        } else {
            None
        }
    }

    fn kind(&self, kind: u8) -> ChangeKind {
        match kind {
            0 => ChangeKind::Unpause,
            1 => ChangeKind::AddVerifier {
                pubkey: key(1).verifying_key().to_bytes(),
                release_hash: [6; 32],
                attestation_uri: "ipfs://second-quote".into(),
            },
            2 => ChangeKind::RemoveVerifier {
                pubkey: key(1).verifying_key().to_bytes(),
            },
            3 => {
                let mut p = self.parameters.clone();
                p.proposal_deposit = if p.proposal_deposit == 100 { 73 } else { 100 };
                p.fee_cap = if p.fee_cap == 1_000 { 20 } else { 1_000 };
                ChangeKind::SetParams { params: p }
            }
            4 => ChangeKind::SetRuleSet {
                rule_set_hash: [self.rules[0] ^ 1; 32],
                manifest_uri: "ipfs://next-rules".into(),
                ai_required: true,
            },
            _ => ChangeKind::SetRuleSet {
                rule_set_hash: [0; 32],
                manifest_uri: String::new(),
                ai_required: true,
            },
        }
    }

    fn invalid_change(&self, kind: &ChangeKind) -> bool {
        match kind {
            ChangeKind::AddVerifier { .. } => self.verifiers[1],
            ChangeKind::RemoveVerifier { .. } => !self.verifiers[1],
            ChangeKind::SetRuleSet { manifest_uri, .. } => manifest_uri.is_empty(),
            _ => false,
        }
    }

    fn step(&mut self, op: &Op) {
        // Taking the gate avoids aliasing the model with the implementation.
        let mut g = self.gate.take().unwrap();
        context(account(0), self.now, 0, None);
        let mut requested = None;
        let mut accepted_callback = false;
        match *op {
            Op::Deposit(owner, amount) => {
                let recipient = (owner + 1) % 3;
                let fee = if amount > 0 && !self.entries[usize::from(recipient)] {
                    g.get_credit_entry_fee(account(recipient)).0
                } else {
                    0
                };
                context(account(owner), self.now, u128::from(amount) + fee, None);
                if amount == 0 {
                    expect_code("E10_ZERO_DEPOSIT", || g.deposit_credit(None));
                } else {
                    g.deposit_credit(Some(account(recipient)));
                    self.entries[usize::from(recipient)] = true;
                    self.credit[usize::from(recipient)] += u128::from(amount);
                    self.deposits += u128::from(amount);
                }
            }
            Op::Withdraw(owner, amount) => {
                context(account(owner), self.now, 0, None);
                let amount = u128::from(amount);
                if amount == 0 {
                    expect_code("E10_ZERO_DEPOSIT", || g.withdraw_credit(U128(amount)));
                } else if amount > self.credit[usize::from(owner)] {
                    expect_code("E11_INSUFFICIENT_CREDIT", || {
                        g.withdraw_credit(U128(amount))
                    });
                } else {
                    drop(g.withdraw_credit(U128(amount)));
                    self.credit[usize::from(owner)] -= amount;
                    self.withdrawals += amount;
                    let receipts = get_created_receipts();
                    assert_eq!(receipts.len(), 2);
                    assert_eq!(receipts[0].receiver_id, account(owner));
                    assert_eq!(receipts[1].receiver_id.as_str(), "gate.testnet");
                    assert!(
                        matches!(receipts[0].actions[0], near_sdk::mock::MockAction::Transfer { deposit, .. } if deposit.as_yoctonear() == amount)
                    );
                }
            }
            Op::Propose(owner, token, bad) => {
                let fee = g.get_proposal_fee().0;
                // A proposer may prepay storage before attempting the zero-deposit call.
                if self.entries[usize::from(owner)] {
                    context(account(owner), self.now, fee, None);
                    g.deposit_credit(None);
                    self.credit[usize::from(owner)] += fee;
                    self.deposits += fee;
                }
                context(account(owner), self.now, 0, None);
                let mut a = args(token);
                if bad == 1 {
                    a.init_calldata.0.resize(1025, 0);
                }
                if bad == 2 {
                    a.new_impl.0 = [0; 20];
                }
                let error = if self
                    .proposals
                    .iter()
                    .any(|p| p.owner == owner && p.reserved > 0)
                {
                    Some("E21_ACTIVE_PROPOSAL_EXISTS")
                } else if self.credit[usize::from(owner)] < self.parameters.proposal_deposit + fee {
                    Some("E11_INSUFFICIENT_CREDIT")
                } else if bad == 1 {
                    Some("E22_INIT_CALLDATA_TOO_LONG")
                } else if bad == 2 {
                    Some("E23_ZERO_ADDRESS")
                } else {
                    None
                };
                if let Some(code) = error {
                    expect_code(code, || g.propose(a));
                } else {
                    assert_eq!(g.propose(a), self.proposals.len() as u64);
                    self.credit[usize::from(owner)] -= self.parameters.proposal_deposit + fee;
                    self.storage_fees += fee;
                    self.proposals.push(ModelProposal {
                        owner,
                        token,
                        state: ProposalState::Pending,
                        expires: self.now + self.parameters.proposal_ttl_s * 1_000_000_000,
                        reserved: self.parameters.proposal_deposit,
                        rules: self.rules,
                        nonce: None,
                        deadline: 0,
                        attempts: 0,
                        requested_at: 0,
                        bumps: 0,
                        fee: 0,
                        priority: 0,
                        certified: false,
                        signed: false,
                        hash: None,
                    });
                }
            }
            Op::Verdict(slot, verdict, verifier) => {
                let id = self.proposal_id(slot);
                // A missing proposal still receives an otherwise authentic certificate.
                let template = self.proposals.first().map(|_| 0);
                let mut c = if g.get_proposal(id).is_some() {
                    certificate(&g, id, self.now, verifier)
                } else if let Some(template) = template {
                    let mut c = certificate(&g, template, self.now, verifier);
                    c.proposal_id = id;
                    c
                } else {
                    // Constructing a certificate needs no production-state mutation.
                    empty_certificate(self.now, verifier, id)
                };
                if verdict == 1 || verdict == 2 {
                    c.verdict = Verdict::Reject {
                        rule_id: 2,
                        reason_code: 22,
                    };
                }
                if verdict == 2 || verdict == 3 {
                    c.ai = AiReview::Vetoed {
                        evidence_hash: [1; 32],
                        counterexample_hash: [2; 32],
                    };
                }
                if verdict == 5 {
                    c.expires_at_s = c.issued_at_s - 1;
                }
                if verdict == 6 {
                    c.new_impl[0] ^= 1;
                }
                if verdict == 7 {
                    c.ai = AiReview::NotRun;
                }
                if verdict == 8 {
                    c.ai = AiReview::Clean {
                        evidence_hash: [4; 32],
                        signature_kind: 2,
                    };
                }
                let p = self.proposals.get(id as usize);
                let error = if !self.verifiers[usize::from(verifier)] {
                    Some("E42_UNKNOWN_VERIFIER")
                } else if verdict == 4 {
                    Some("E43_BAD_SIGNATURE")
                } else {
                    match p {
                        None => Some("E30_NOT_FOUND"),
                        Some(p) if p.state != ProposalState::Pending => Some("E31_WRONG_STATE"),
                        Some(p) if self.now > p.expires => Some("E44_PROPOSAL_EXPIRED"),
                        Some(p) if verdict == 6 || p.rules != self.rules => {
                            Some("E45_BINDING_MISMATCH")
                        }
                        Some(_) if verdict == 5 => Some("E46_CERT_TIME"),
                        Some(_) if verdict == 3 || verdict == 7 => Some("E47_AI_REVIEW_REQUIRED"),
                        Some(_) if verdict == 8 => Some("E58_AI_SIGNATURE_KIND"),
                        Some(_) if verdict != 1 && verdict != 2 && self.paused => {
                            Some("E20_SIGNING_PAUSED")
                        }
                        Some(_) => None,
                    }
                };
                if let Some(code) = error {
                    expect_code(code, || submit(&mut g, &c, verdict == 4));
                } else {
                    submit(&mut g, &c, false);
                    assert_eq!(
                        g.get_proposal(id).unwrap().certificate_hash,
                        Some(env::sha256_array(c.to_canonical_bytes().unwrap()))
                    );
                    if verdict == 1 || verdict == 2 {
                        self.proposals[id as usize].state = ProposalState::Rejected;
                        self.release(id as usize);
                    } else {
                        let p = &mut self.proposals[id as usize];
                        p.state = ProposalState::SignRequested;
                        p.certified = true;
                        p.nonce = Some(self.next_nonce);
                        self.next_nonce += 1;
                        p.deadline = c
                            .expires_at_s
                            .min(ns_to_s(self.now) + self.parameters.sign_validity_s);
                        p.attempts = 1;
                        p.requested_at = self.now;
                        p.fee = self.parameters.max_fee_per_gas;
                        p.priority = self.parameters.max_priority_fee_per_gas;
                        requested = Some(id);
                    }
                }
            }
            Op::Expire(slot) => {
                let id = self.proposal_id(slot);
                let error = match self.proposals.get(id as usize) {
                    None => Some("E30_NOT_FOUND"),
                    Some(p) if p.state != ProposalState::Pending => Some("E31_WRONG_STATE"),
                    Some(p) if self.now <= p.expires => Some("E32_NOT_EXPIRED"),
                    _ => None,
                };
                if let Some(code) = error {
                    expect_code(code, || g.expire(id));
                } else {
                    g.expire(id);
                    self.proposals[id as usize].state = ProposalState::Expired;
                    self.release(id as usize);
                }
            }
            Op::Callback(slot, outcome, stale) => {
                let id = self.proposal_id(slot);
                let attempt = self.proposals.get(id as usize).map_or(1, |p| p.attempts);
                callback(
                    &mut g,
                    id,
                    if stale { attempt + 1 } else { attempt },
                    self.now,
                    outcome,
                );
                if let Some(p) = self.proposals.get_mut(id as usize)
                    && p.state == ProposalState::SignRequested
                    && !stale
                {
                    p.state = if outcome == 1 {
                        ProposalState::Signed
                    } else {
                        ProposalState::SignFailed
                    };
                    if outcome == 1 {
                        accepted_callback = true;
                        p.signed = true;
                        self.release(id as usize);
                    }
                }
            }
            Op::Retry(slot) | Op::Bump(slot) => {
                let id = self.proposal_id(slot);
                let bump = matches!(op, Op::Bump(_));
                let error = if id as usize == self.proposals.len() {
                    Some("E30_NOT_FOUND")
                } else {
                    self.resign_error(
                        id as usize,
                        if bump {
                            ProposalState::Signed
                        } else {
                            ProposalState::SignFailed
                        },
                    )
                    .or_else(|| {
                        let p = &self.proposals[id as usize];
                        if bump && self.now.saturating_sub(p.requested_at) < 600_000_000_000 {
                            Some("E62_BUMP_TOO_EARLY")
                        } else if bump && p.bumps == 8 {
                            Some("E49_TOO_MANY_BUMPS")
                        } else if bump && p.fee + p.fee.div_ceil(4) > self.parameters.fee_cap {
                            Some("E48_FEE_CAP")
                        } else {
                            None
                        }
                    })
                };
                let call = |g: &mut Gate| {
                    if bump {
                        drop(g.bump_fee(SignTarget::Proposal(id)));
                    } else {
                        drop(g.retry_sign(id));
                    }
                };
                if let Some(code) = error {
                    expect_code(code, || call(&mut g));
                } else {
                    call(&mut g);
                    let p = &mut self.proposals[id as usize];
                    p.state = ProposalState::SignRequested;
                    p.attempts += 1;
                    p.requested_at = self.now;
                    if bump {
                        p.bumps += 1;
                        p.fee += p.fee.div_ceil(4);
                        p.priority += p.priority.div_ceil(4);
                    }
                    requested = Some(id);
                }
            }
            Op::Pause(authorized) => {
                context(
                    if authorized {
                        "guardian.testnet".parse().unwrap()
                    } else {
                        account(0)
                    },
                    self.now,
                    0,
                    None,
                );
                if authorized {
                    g.pause_signing();
                    self.paused = true;
                } else {
                    expect_code("E56_NOT_GUARDIAN", || g.pause_signing());
                }
            }
            Op::RemoveVerifier(index, authorized) => {
                context(
                    if authorized {
                        "guardian.testnet".parse().unwrap()
                    } else {
                        account(0)
                    },
                    self.now,
                    0,
                    None,
                );
                let error = if !authorized {
                    Some("E56_NOT_GUARDIAN")
                } else if !self.verifiers[usize::from(index)] {
                    Some("E51_INVALID_CHANGE")
                } else if self.verifiers.iter().filter(|v| **v).count() == 1 {
                    Some("E57_LAST_VERIFIER")
                } else {
                    None
                };
                let pubkey = Base64(key(index).verifying_key().to_bytes());
                if let Some(code) = error {
                    expect_code(code, || g.remove_verifier_now(pubkey));
                } else {
                    g.remove_verifier_now(pubkey);
                    self.verifiers[usize::from(index)] = false;
                }
            }
            Op::Schedule(kind, authorized) => {
                context(
                    if authorized {
                        "governor.testnet".parse().unwrap()
                    } else {
                        account(0)
                    },
                    self.now,
                    0,
                    None,
                );
                let kind = self.kind(kind);
                let error = if !authorized {
                    Some("E50_NOT_GOVERNOR")
                } else if self.invalid_change(&kind) {
                    Some("E51_INVALID_CHANGE")
                } else if self.changes.iter().filter(|c| c.is_some()).count() == 8 {
                    Some("E59_PENDING_CHANGE_LIMIT")
                } else if self
                    .changes
                    .iter()
                    .flatten()
                    .filter(|c| std::mem::discriminant(&c.kind) == std::mem::discriminant(&kind))
                    .count()
                    >= 2
                {
                    Some("E60_DUPLICATE_AUTHORITY_CHANGE")
                } else {
                    None
                };
                if let Some(code) = error {
                    expect_code(code, || g.schedule_change(kind));
                } else {
                    let id = g.schedule_change(kind.clone());
                    assert_eq!(id, self.changes.len() as u64);
                    self.changes.push(Some(PendingChange {
                        id,
                        kind,
                        scheduled_by: "governor.testnet".parse().unwrap(),
                        scheduled_at_ns: self.now,
                        eta_ns: self.now + 604_800_000_000_000,
                        status: ChangeStatus::Pending,
                        evm: None,
                    }));
                }
            }
            Op::Apply(slot) => {
                let id = self.change_id(slot);
                let change = self.changes.get(id as usize).and_then(Option::as_ref);
                let error = if let Some(c) = change {
                    if self.now < c.eta_ns {
                        Some("E53_TIMELOCK_NOT_ELAPSED")
                    } else if self.invalid_change(&c.kind) {
                        Some("E51_INVALID_CHANGE")
                    } else if matches!(c.kind, ChangeKind::RemoveVerifier { .. })
                        && !self.verifiers[0]
                    {
                        Some("E57_LAST_VERIFIER")
                    } else if matches!(c.kind, ChangeKind::SetRuleSet { .. })
                        && self.proposals.iter().any(|p| {
                            matches!(
                                p.state,
                                ProposalState::SignRequested
                                    | ProposalState::Signed
                                    | ProposalState::SignFailed
                            ) && (p.deadline > ns_to_s(self.now)
                                || (p.state == ProposalState::SignRequested
                                    && self.now.saturating_sub(p.requested_at) < 1_200_000_000_000))
                        })
                    {
                        Some("E55_SIGNATURES_OUTSTANDING")
                    } else {
                        None
                    }
                } else {
                    Some("E52_NOT_FOUND")
                };
                if let Some(code) = error {
                    expect_code(code, || g.apply_change(id));
                } else {
                    drop(g.apply_change(id));
                    let c = self.changes[id as usize].take().unwrap();
                    match &c.kind {
                        ChangeKind::Unpause => self.paused = false,
                        ChangeKind::AddVerifier { .. } => self.verifiers[1] = true,
                        ChangeKind::RemoveVerifier { .. } => self.verifiers[1] = false,
                        ChangeKind::SetParams { params } => self.parameters = params.clone(),
                        ChangeKind::SetRuleSet { rule_set_hash, .. } => self.rules = *rule_set_hash,
                        _ => unreachable!(),
                    }
                }
            }
            Op::Cancel(slot, authorized) => {
                let id = self.change_id(slot);
                context(
                    if authorized {
                        "guardian.testnet".parse().unwrap()
                    } else {
                        account(0)
                    },
                    self.now,
                    0,
                    None,
                );
                let error = if !authorized {
                    Some("E56_NOT_GUARDIAN")
                } else if self
                    .changes
                    .get(id as usize)
                    .and_then(Option::as_ref)
                    .is_none()
                {
                    Some("E52_NOT_FOUND")
                } else {
                    None
                };
                if let Some(code) = error {
                    expect_code(code, || g.cancel_change(id));
                } else {
                    g.cancel_change(id);
                    self.changes[id as usize] = None;
                }
            }
            Op::Advance(delta) => {
                self.now += [
                    0,
                    1,
                    1_000_000_000,
                    40_000_000_000,
                    60_000_000_000,
                    120_000_000_000,
                    604_800_000_000_000 - 1,
                    604_800_000_000_000,
                    1_200_000_000_000 - 1,
                    1_200_000_000_000,
                ][usize::from(delta)];
            }
        }
        let signed_events = near_sdk::test_utils::get_logs()
            .into_iter()
            .filter(|log| {
                log.strip_prefix("EVENT_JSON:").is_some_and(|json| {
                    serde_json::from_str::<serde_json::Value>(json).unwrap()["event"] == "signed"
                })
            })
            .count();
        assert_eq!(
            signed_events,
            usize::from(accepted_callback),
            "callback accepted more than once"
        );
        let requests = sign_requests();
        if let Some(id) = requested {
            assert_eq!(requests.len(), 1);
            let e = g.get_proposal(id).unwrap().evm.as_ref().unwrap();
            assert_eq!(
                requests[0]["request"]["payload_v2"]["Ecdsa"],
                hex::encode(e.signing_hash)
            );
            assert_eq!(requests[0]["request"]["domain_id"], 0);
            assert_eq!(
                requests[0]["request"]["path"],
                "constitution/base-sepolia/v1"
            );
            let p = &mut self.proposals[id as usize];
            assert!(p.certified, "no signing without a validated Pass");
            // Retries preserve the full preimage; fee bumps alone may change its hash.
            if !matches!(op, Op::Bump(_))
                && let Some(hash) = p.hash
            {
                assert_eq!(hash, e.signing_hash);
            }
            p.hash = Some(e.signing_hash);
        } else {
            assert!(requests.is_empty(), "unexpected MPC request after {op:?}");
        }
        self.gate = Some(roundtrip(g));
        self.check();
    }

    fn check(&self) {
        let g = self.gate.as_ref().unwrap();
        let mut escrow = 0;
        let mut nonces = std::collections::BTreeSet::new();
        for (id, p) in self.proposals.iter().enumerate() {
            let actual = g.get_proposal(id as u64).unwrap();
            assert_eq!(actual.state, p.state, "proposal {id}");
            assert_eq!(actual.reserved, p.reserved);
            assert_eq!(actual.expires_at_ns, p.expires);
            assert_eq!(actual.rule_set_hash, p.rules);
            assert_eq!(actual.proposer, account(p.owner));
            let a = args(p.token);
            assert_eq!(actual.new_impl, a.new_impl.0);
            assert_eq!(actual.new_impl_codehash, a.new_impl_codehash.0);
            assert_eq!(actual.expected_current_impl, a.expected_current_impl.0);
            assert_eq!(actual.init_calldata, a.init_calldata.0);
            assert_eq!(actual.source_commitment, a.source_commitment.0);
            assert_eq!(
                borsh::from_slice::<Proposal>(&borsh::to_vec(actual).unwrap()).unwrap(),
                *actual
            );
            escrow += actual.reserved;
            assert_eq!(actual.evm.as_ref().map(|e| e.nonce), p.nonce);
            if let Some(e) = &actual.evm {
                assert!(p.certified);
                assert!(
                    nonces.insert(e.nonce),
                    "nonce reused for a different proposal"
                );
                assert!(e.nonce < self.next_nonce);
                assert_eq!(e.deadline_s, p.deadline);
                assert_eq!(e.attempts, p.attempts);
                assert_eq!(e.max_fee_per_gas, p.fee);
                assert_eq!(e.max_priority_fee_per_gas, p.priority);
                assert_eq!(e.signed_tx.is_some(), p.signed);
                assert_eq!(
                    e.signing_hash,
                    env::keccak256_array(&g.signing_preimage(id as u64).0)
                );
            }
            if matches!(p.state, ProposalState::Rejected | ProposalState::Expired) {
                assert!(
                    actual.evm.is_none(),
                    "terminal rejection/expiry obtained signing authority"
                );
            }
        }
        assert_eq!(
            self.deposits,
            self.credit.iter().sum::<u128>() + escrow + self.withdrawals + self.storage_fees
        );
        let liabilities = self.credit.iter().sum::<u128>() + escrow;
        assert_eq!(g.get_total_liabilities().0, liabilities);
        let storage_cost =
            u128::from(env::storage_usage()) * env::storage_byte_cost().as_yoctonear();
        let liquid = env::account_balance()
            .as_yoctonear()
            .saturating_sub(storage_cost);
        assert!(
            liabilities <= liquid,
            "model liabilities exceed liquid balance"
        );
        // This model leaves withdrawals in flight, so both liabilities and the pending
        // withdrawal total remain excluded from operating surplus.
        assert_eq!(
            g.get_operating_surplus().0,
            liquid.saturating_sub(liabilities + self.withdrawals)
        );
        assert_eq!(g.is_signing_paused(), self.paused);
        for owner in 0..3 {
            assert_eq!(
                g.get_credit(account(owner)).0,
                self.credit[usize::from(owner)]
            );
            let active: Vec<_> = self
                .proposals
                .iter()
                .enumerate()
                .filter(|(_, p)| p.owner == owner && p.reserved > 0)
                .map(|(id, _)| id as u64)
                .collect();
            assert!(
                active.len() <= 1,
                "more than one active reservation per account"
            );
            assert_eq!(g.get_active(account(owner)), active.first().copied());
        }
        assert_eq!(g.get_constitution().params, self.parameters);
        assert_eq!(g.get_constitution().rule_set_hash, self.rules);
        assert_eq!(g.get_roles().governor.as_str(), "governor.testnet");
        assert_eq!(g.get_roles().guardian.as_str(), "guardian.testnet");
        assert_eq!(g.get_config().executor, [1; 20]);
        let listed = g.list_verifiers(0, 50);
        assert_eq!(listed.len(), self.verifiers.iter().filter(|v| **v).count());
        assert!(self.changes.iter().filter(|c| c.is_some()).count() <= 8);
        let live = g.list_changes(0, 50);
        assert!(live.len() <= 8);
        for change in &live {
            let same_kind = live
                .iter()
                .filter(|other| {
                    std::mem::discriminant(&other.kind) == std::mem::discriminant(&change.kind)
                })
                .count();
            let cap = if matches!(
                change.kind,
                ChangeKind::SetGovernor { .. }
                    | ChangeKind::SetGuardian { .. }
                    | ChangeKind::MigrateAuthority { .. }
            ) {
                1
            } else {
                2
            };
            assert!(same_kind <= cap);
        }
        for index in 0..2 {
            assert_eq!(
                listed.iter().any(|(k, v)| {
                    assert_eq!(v.release_hash, [6; 32]);
                    !v.attestation_uri.is_empty() && k.0 == key(index).verifying_key().to_bytes()
                }),
                self.verifiers[usize::from(index)]
            );
        }
        assert_eq!(TIMELOCK_NS, 604_800_000_000_000);
        for (id, expected) in self.changes.iter().enumerate() {
            let Some(expected) = expected else {
                assert!(g.get_change(id as u64).is_none());
                continue;
            };
            let actual = g.get_change(id as u64).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.eta_ns - actual.scheduled_at_ns, 604_800_000_000_000);
            assert_eq!(
                borsh::from_slice::<PendingChange>(&borsh::to_vec(actual).unwrap()).unwrap(),
                *actual
            );
        }
    }
}

fn empty_certificate(now: u64, verifier: u8, id: u64) -> Certificate {
    Certificate {
        domain: DOMAIN,
        gate: "gate.testnet".parse().unwrap(),
        proposal_id: id,
        attempt: 0,
        chain_id: 84532,
        proxy: [2; 20],
        executor: [1; 20],
        current_impl: [10; 20],
        current_impl_codehash: [1; 32],
        new_impl: [8; 20],
        new_impl_codehash: [9; 32],
        init_calldata_hash: env::keccak256_array([]),
        source_commitment: [11; 32],
        rule_set_hash: [5; 32],
        verifier_pubkey: key(verifier).verifying_key().to_bytes(),
        verifier_release_hash: [6; 32],
        evidence_block_number: 1,
        evidence_block_hash: [2; 32],
        evidence_root: [3; 32],
        ai: AiReview::Clean {
            evidence_hash: [4; 32],
            signature_kind: 1,
        },
        verdict: Verdict::Pass,
        issued_at_s: ns_to_s(now),
        expires_at_s: ns_to_s(now) + 60,
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn generated_operations_match_reference_model(initial_nonce in 0u64..1_000_000, ops in prop::collection::vec(operations(), 1..33)) {
        let mut machine = Machine::new(initial_nonce);
        // Fund all actors; the generated trace can withdraw credit and reserve/refund it.
        for owner in 0..3 { machine.step(&Op::Deposit(owner, 300)); }
        machine.step(&Op::Propose(0, 0, 0));
        for (step, op) in ops.iter().enumerate() {
            let result = catch_unwind(AssertUnwindSafe(|| machine.step(op)));
            prop_assert!(result.is_ok(), "step {step}: {op:?}; minimal trace: {ops:#?}");
        }
    }

    #[test]
    fn signing_retries_bumps_and_stale_callbacks_preserve_nonce_and_credit(
        initial_nonce in 0u64..1_000_000,
        failures in 0usize..4,
        bumps in 0usize..10,
        malformed in any::<bool>(),
        low_fee_cap in any::<bool>(),
    ) {
        let mut m = Machine::new(initial_nonce);
        if low_fee_cap {
            m.step(&Op::Schedule(3, true));
            m.step(&Op::Advance(7));
            m.step(&Op::Apply(0));
        }
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Verdict(0, 0, 0));
        for _ in 0..failures {
            m.step(&Op::Callback(0, if malformed { 2 } else { 0 }, false));
            m.step(&Op::Retry(0));
        }
        m.step(&Op::Callback(0, 1, false));
        // Duplicate delivery cannot sign twice, refund twice, or clear a newer proposal.
        m.step(&Op::Callback(0, 1, false));
        m.step(&Op::Propose(0, 1, 0));
        m.step(&Op::Verdict(1, 0, 0));
        for _ in 0..bumps {
            m.now += 600_000_000_000;
            m.step(&Op::Bump(0));
            m.step(&Op::Callback(0, 1, true));
            m.step(&Op::Callback(0, 0, false));
            m.step(&Op::Retry(0));
            m.step(&Op::Callback(0, 1, false));
        }
        prop_assert_eq!(m.proposals[1].nonce, Some(initial_nonce + 1));
    }

    #[test]
    fn timelocked_verifiers_and_guardian_actions_never_expand_authority(offset in 0u64..1_000_000_000, cancel in any::<bool>()) {
        let mut m = Machine::new(42);
        m.now += offset;
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Pause(true));
        m.step(&Op::Schedule(1, true));
        m.step(&Op::Schedule(0, true));
        m.step(&Op::Apply(0)); // E53, no allowlist expansion.
        m.step(&Op::Verdict(0, 0, 1)); // E42, scheduled is not registered.
        m.step(&Op::Advance(6));
        m.step(&Op::Apply(0)); // eta - 1ns: E53.
        m.step(&Op::Advance(1));
        m.step(&Op::Apply(0)); // Exactly eta, permissionless add.
        m.step(&Op::Verdict(0, 0, 1)); // Proposal expired; no sign request.
        m.step(&Op::Apply(1)); // Timelocked unpause.
        m.step(&Op::Expire(0));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Schedule(2, true));
        m.step(&Op::Apply(2)); // Early removal also E53.
        if cancel { m.step(&Op::Cancel(2, true)); }
        m.step(&Op::Advance(7));
        m.step(&Op::Apply(2));
        if cancel { m.step(&Op::RemoveVerifier(1, true)); }
        m.step(&Op::RemoveVerifier(0, true)); // E57: cannot remove last verifier.
        m.step(&Op::Verdict(1, 0, 1)); // E42 even with a valid signature.
        m.step(&Op::Schedule(3, true));
        m.step(&Op::Apply(3)); // Parameter updates cannot shorten governance delay.
        m.step(&Op::Advance(7));
        m.step(&Op::Apply(3));
        m.step(&Op::Schedule(0, true));
        m.step(&Op::Cancel(4, false));
        m.step(&Op::Cancel(4, true));
        m.step(&Op::Apply(4));
    }
}

/// Each mutation is independently invalid; the valid fields are derived from a real
/// proposal. Fixed examples below pin every rejection class, not just sampled ones.
fn certificate_mutation(mutation: u8, token: u8) {
    let mut m = Machine::new(17);
    m.step(&Op::Deposit(2, 300));
    m.step(&Op::Propose(0, token, 0));
    let mut g = m.gate.take().unwrap();
    context(account(0), m.now, 0, None);
    let before = g.get_proposal(0).unwrap().clone();
    let mut c = certificate(&g, 0, m.now, 0);
    let expected = match mutation {
        0 | 1 => "E40_BAD_ENCODING",
        2 | 3 => "E41_WRONG_DOMAIN",
        4 | 5 => "E42_UNKNOWN_VERIFIER",
        6 => "E43_BAD_SIGNATURE",
        7..=15 => "E45_BINDING_MISMATCH",
        16..=19 => "E46_CERT_TIME",
        20 | 21 => "E47_AI_REVIEW_REQUIRED",
        22 => "E58_AI_SIGNATURE_KIND",
        _ => unreachable!(),
    };
    match mutation {
        2 => c.domain[0] ^= 1,
        3 => c.gate = "wrong.testnet".parse().unwrap(),
        4 => c.verifier_pubkey = key(1).verifying_key().to_bytes(),
        5 => c.verifier_release_hash[0] ^= 1,
        7 => c.chain_id ^= 1,
        8 => c.proxy[0] ^= 1,
        9 => c.executor[0] ^= 1,
        10 => c.new_impl[0] ^= 1,
        11 => c.new_impl_codehash[0] ^= 1,
        12 => c.source_commitment[0] ^= 1,
        13 => c.current_impl[0] ^= 1,
        14 => c.init_calldata_hash[0] ^= 1,
        15 => c.rule_set_hash[0] ^= 1,
        16 => {
            c.issued_at_s += 61;
            c.expires_at_s = c.issued_at_s;
        }
        17 => c.expires_at_s = c.issued_at_s - 1,
        18 => c.expires_at_s = c.issued_at_s + 61,
        19 => {
            c.expires_at_s = c.issued_at_s;
            c.issued_at_s += 1;
        }
        20 => c.ai = AiReview::NotRun,
        21 => {
            c.ai = AiReview::Vetoed {
                evidence_hash: [1; 32],
                counterexample_hash: [2; 32],
            }
        }
        22 => {
            c.ai = AiReview::Clean {
                evidence_hash: [4; 32],
                signature_kind: 2,
            }
        }
        _ => {}
    }
    if mutation <= 1 {
        let mut bytes = c.to_canonical_bytes().unwrap();
        if mutation == 0 {
            bytes.truncate(bytes.len() - 1);
        } else {
            bytes.push(0);
        }
        expect_code(expected, || {
            g.submit_verdict(Base64Bytes(bytes), Base64([0; 64]))
        });
    } else {
        expect_code(expected, || submit(&mut g, &c, mutation == 6));
    }
    assert!(sign_requests().is_empty(), "mutation {mutation} signed");
    assert_eq!(
        g.get_proposal(0).unwrap(),
        &before,
        "mutation {mutation} wrote state"
    );
    m.gate = Some(roundtrip(g));
    m.check();
    m.step(&Op::Verdict(0, 0, 0)); // Invalid delivery did not consume nonce 17.
    assert_eq!(m.proposals[0].nonce, Some(17));
}

#[test]
fn every_certificate_rejection_class_has_exact_error_and_no_effect() {
    for mutation in 0..23 {
        certificate_mutation(mutation, mutation % 4);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn certificate_mutations_never_authorize_signing(mutation in 0u8..23, token in 0u8..4) {
        certificate_mutation(mutation, token);
    }

    #[test]
    fn certificate_time_boundaries_and_terminal_rejections(
        ttl in prop_oneof![Just(0u64), Just(1), Just(59), Just(60)],
        future_issue in any::<bool>(),
        veto in any::<bool>(),
    ) {
        let mut m = Machine::new(42);
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Propose(0, 0, 0));
        let mut g = m.gate.take().unwrap();
        let mut c = certificate(&g, 0, m.now, 0);
        c.issued_at_s += if future_issue { 60 } else { 0 };
        c.expires_at_s = c.issued_at_s + ttl;
        // The allowed +60s clock skew and maximum TTL are inclusive.
        context(account(0), m.now, 0, None);
        submit(&mut g, &c, false);
        prop_assert_eq!(sign_requests().len(), 1);
        prop_assert_eq!(g.get_proposal(0).unwrap().evm.as_ref().unwrap().deadline_s,
            c.expires_at_s.min(ns_to_s(m.now) + params().sign_validity_s));
        g = roundtrip(g);
        drop(g);

        let mut m = Machine::new(42);
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Pause(true));
        m.step(&Op::Verdict(0, if veto { 2 } else { 1 }, 0));
        m.step(&Op::Verdict(0, 0, 0));
        m.step(&Op::Retry(0));
        m.step(&Op::Bump(0));
        m.step(&Op::Callback(0, 1, false));
        prop_assert_eq!(&m.proposals[0].state, &ProposalState::Rejected);
        prop_assert_eq!(m.credit[0], 300);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn rule_changes_wait_for_inflight_signatures_and_expired_retries_only_fill_nonce(outcome in 0u8..3) {
        let mut m = Machine::new(42);
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Schedule(4, true));
        m.step(&Op::Advance(7));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Verdict(0, 0, 0));
        m.step(&Op::Apply(0)); // E55: live deadline and a fresh in-flight request.
        m.step(&Op::Advance(4));
        m.step(&Op::Apply(0)); // E55: deadline elapsed, but request timeout has not.
        m.step(&Op::Advance(9));
        m.step(&Op::Apply(0)); // Both elapsed: a lost callback cannot lock governance.
        m.step(&Op::Callback(0, outcome, false));
        m.step(&Op::Apply(0));
        if outcome != 1 {
            m.step(&Op::Retry(0)); // Same expired bytes may only consume the reserved nonce.
        } else {
            m.step(&Op::Bump(0)); // Same deadline and calldata, only fees may change.
        }
        m.step(&Op::Callback(0, 1, false));
        prop_assert_eq!(m.proposals[0].nonce, Some(42));
        prop_assert!(m.proposals[0].deadline < ns_to_s(m.now));
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn proposal_expiry_and_calldata_boundaries_release_exactly_once(
        calldata_len in prop_oneof![Just(0usize), Just(1), Just(1024)],
        offset_ns in 0u64..1_000_000_000,
    ) {
        let mut g = fresh_gate(42);
        let now = START + offset_ns;
        deposit(&mut g, account(0), now, 100);
        let mut input = args(0);
        input.init_calldata.0.resize(calldata_len, 0);
        proposal_context(&mut g, account(0), now);
        let id = g.propose(input.clone());
        g = roundtrip(g);
        deposit(&mut g, account(1), now, 100);
        proposal_context(&mut g, account(1), now);
        let other = g.propose(input.clone());
        prop_assert_eq!(other, id + 1);
        prop_assert_eq!(g.get_active(account(0)), Some(id));
        prop_assert_eq!(g.get_active(account(1)), Some(other));
        g = roundtrip(g);
        let expiry = g.get_proposal(id).unwrap().expires_at_ns;
        for time in [expiry - 1, expiry] {
            context(account(1), time, 0, None);
            expect_code("E32_NOT_EXPIRED", || g.expire(id));
            prop_assert_eq!(g.get_credit(account(0)).0, 0);
            prop_assert_eq!(g.get_proposal(id).unwrap().reserved, 100);
            g = roundtrip(g);
        }
        context(account(1), expiry + 1, 0, None);
        let c = certificate(&g, id, expiry + 1, 0);
        expect_code("E44_PROPOSAL_EXPIRED", || submit(&mut g, &c, false));
        g = roundtrip(g);
        g.expire(id);
        prop_assert!(sign_requests().is_empty());
        prop_assert_eq!(g.get_credit(account(0)).0, 100);
        prop_assert_eq!(g.get_proposal(id).unwrap().reserved, 0);
        prop_assert_eq!(g.get_active(account(0)), None);
        g = roundtrip(g);
        expect_code("E31_WRONG_STATE", || g.expire(id));
        g = roundtrip(g);
        proposal_context(&mut g, account(0), expiry + 1);
        let reopened = g.propose(input);
        prop_assert_eq!(reopened, id + 2);
        prop_assert_eq!(g.get_active(account(1)), Some(other));
        g = roundtrip(g);
        prop_assert_eq!(g.get_credit(account(0)).0 + g.get_proposal(reopened).unwrap().reserved, 100);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn migration_and_upgrade_nonces_never_collide(
        initial_nonce in 0u64..1_000_000,
        failures in 0u32..4,
    ) {
        let mut g = fresh_gate(initial_nonce);
        context("governor.testnet".parse().unwrap(), START, 0, None);
        let change = g.schedule_change(ChangeKind::MigrateAuthority { new_signer: [12; 20] });
        g = roundtrip(g);
        context(account(0), START + TIMELOCK_NS - 1, 0, None);
        expect_code("E53_TIMELOCK_NOT_ELAPSED", || g.apply_change(change));
        prop_assert!(sign_requests().is_empty());
        g = roundtrip(g);
        let now = START + TIMELOCK_NS;
        context(account(0), now, 0, None);
        drop(g.apply_change(change));
        prop_assert_eq!(sign_requests().len(), 1);
        let original = g.get_change(change).unwrap().evm.clone().unwrap();
        prop_assert_eq!(original.nonce, initial_nonce);
        g = roundtrip(g);
        for attempt in 1..=failures {
            context("gate.testnet".parse().unwrap(), now, 0, Some(near_sdk::PromiseResult::Failed));
            g.on_signed(SignTarget::Change(change), attempt);
            g = roundtrip(g);
            context(account(0), now, 0, None);
            drop(g.apply_change(change));
            prop_assert_eq!(sign_requests().len(), 1);
            let e = g.get_change(change).unwrap().evm.as_ref().unwrap();
            prop_assert_eq!(e.nonce, original.nonce);
            prop_assert_eq!(e.signing_hash, original.signing_hash);
            prop_assert_eq!(e.deadline_s, original.deadline_s);
            prop_assert_eq!(e.attempts, attempt + 1);
            g = roundtrip(g);
        }
        context("gate.testnet".parse().unwrap(), now, 0, Some(near_sdk::PromiseResult::Successful(
            include_bytes!("../../../vectors/mpc-response.json").to_vec(),
        )));
        g.on_signed(SignTarget::Change(change), failures + 1);
        g = roundtrip(g);
        let signed = g.get_change(change).unwrap().evm.as_ref().unwrap().signed_tx.clone();
        prop_assert!(signed.is_some());
        // Stale duplicate callbacks cannot overwrite the accepted migration signature.
        context("gate.testnet".parse().unwrap(), now, 0, Some(near_sdk::PromiseResult::Failed));
        g.on_signed(SignTarget::Change(change), failures + 1);
        g = roundtrip(g);
        prop_assert_eq!(&g.get_change(change).unwrap().evm.as_ref().unwrap().signed_tx, &signed);
        context(account(0), now, 0, None);
        expect_code("E54_NOT_PENDING", || g.apply_change(change));
        prop_assert!(sign_requests().is_empty());
        g = roundtrip(g);
        deposit(&mut g, account(0), now, 100);
        proposal_context(&mut g, account(0), now);
        let id = g.propose(args(0));
        g = roundtrip(g);
        context(account(0), now, 0, None);
        let c = certificate(&g, id, now, 0);
        submit(&mut g, &c, false);
        prop_assert_eq!(sign_requests().len(), 1);
        g = roundtrip(g);
        prop_assert_eq!(g.get_proposal(id).unwrap().evm.as_ref().unwrap().nonce, initial_nonce + 1);
        prop_assert_eq!(g.get_proposal(id).unwrap().reserved + g.get_credit(account(0)).0, 100);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn requested_signature_retries_only_after_timeout_and_ignores_old_attempt(pause in any::<bool>()) {
        let mut m = Machine::new(42);
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Verdict(0, 0, 0));
        let original = m.gate.as_ref().unwrap().get_proposal(0).unwrap().evm.clone().unwrap();
        m.step(&Op::Retry(0)); // In flight, no timeout: E31.
        m.step(&Op::Advance(8));
        m.step(&Op::Retry(0)); // Timeout minus one nanosecond: E31.
        m.step(&Op::Advance(1));
        if pause {
            m.step(&Op::Pause(true));
            m.step(&Op::Retry(0)); // Timeout cannot bypass the guardian's pause.
            m.step(&Op::Schedule(0, true));
            m.step(&Op::Advance(7));
            m.step(&Op::Apply(0));
        }
        m.step(&Op::Retry(0));
        let mut g = m.gate.take().unwrap();
        let retried = g.get_proposal(0).unwrap().evm.as_ref().unwrap();
        prop_assert_eq!(retried.nonce, original.nonce);
        prop_assert_eq!(retried.signing_hash, original.signing_hash);
        prop_assert_eq!(retried.deadline_s, original.deadline_s);
        prop_assert_eq!(retried.attempts, 2);
        callback(&mut g, 0, 1, m.now, 1);
        prop_assert!(near_sdk::test_utils::get_logs().iter().all(|log| !log.contains("\"event\":\"signed\"")));
        m.gate = Some(roundtrip(g));
        m.check();
        m.step(&Op::Callback(0, 1, false));
        prop_assert_eq!(m.credit[0], 300);
    }

    #[test]
    fn non_provider_clean_never_authorizes_pass_even_when_ai_optional(
        encoded_kind in 0u16..255,
        ai_required in any::<bool>(),
    ) {
        let mut g = fresh_gate(42);
        let mut now = START;
        if !ai_required {
            context("governor.testnet".parse().unwrap(), now, 0, None);
            let change = g.schedule_change(ChangeKind::SetRuleSet {
                rule_set_hash: [5; 32], manifest_uri: "ipfs://optional-ai".into(), ai_required: false,
            });
            g = roundtrip(g);
            now += TIMELOCK_NS;
            context(account(0), now, 0, None);
            drop(g.apply_change(change));
            g = roundtrip(g);
        }
        deposit(&mut g, account(0), now, 100);
        proposal_context(&mut g, account(0), now);
        let id = g.propose(args(0));
        g = roundtrip(g);
        context(account(0), now, 0, None);
        let mut c = certificate(&g, id, now, 0);
        let signature_kind = if encoded_kind == 0 { 0 } else { (encoded_kind + 1) as u8 };
        c.ai = AiReview::Clean { evidence_hash: [4; 32], signature_kind };
        expect_code("E58_AI_SIGNATURE_KIND", || submit(&mut g, &c, false));
        prop_assert!(sign_requests().is_empty());
        g = roundtrip(g);
        prop_assert_eq!(&g.get_proposal(id).unwrap().state, &ProposalState::Pending);
        prop_assert_eq!(g.get_total_liabilities().0, 100);
        c.verdict = Verdict::Reject { rule_id: 2, reason_code: 22 };
        submit(&mut g, &c, false); // Signature-kind policy cannot block a fail-closed rejection.
        prop_assert!(sign_requests().is_empty());
        g = roundtrip(g);
        prop_assert_eq!(&g.get_proposal(id).unwrap().state, &ProposalState::Rejected);
        prop_assert_eq!(g.get_credit(account(0)).0, 100);
    }

    #[test]
    fn pending_changes_are_bounded_unique_and_batch_cancel_is_atomic(reverse in any::<bool>()) {
        let mut g = fresh_gate(42);
        let mut kinds = vec![
            ChangeKind::MigrateAuthority { new_signer: [12; 20] },
            ChangeKind::SetGuardian { account: "next-guardian.testnet".parse().unwrap() },
            ChangeKind::SetGovernor { account: "next-governor.testnet".parse().unwrap() },
        ];
        if reverse { kinds.reverse(); }
        let mut ids = vec![];
        for kind in &kinds {
            context("governor.testnet".parse().unwrap(), START, 0, None);
            let id = g.schedule_change(kind.clone());
            ids.push(id);
            g = roundtrip(g);
            expect_code("E60_DUPLICATE_AUTHORITY_CHANGE", || g.schedule_change(kind.clone()));
            g = roundtrip(g);
        }
        for kind in [
            ChangeKind::SetParams { params: params() },
            ChangeKind::SetParams { params: params() },
            ChangeKind::Unpause,
            ChangeKind::Unpause,
            ChangeKind::SetRuleSet { rule_set_hash: [8; 32], manifest_uri: "ipfs://rules".into(), ai_required: true },
        ] {
            context("governor.testnet".parse().unwrap(), START, 0, None);
            ids.push(g.schedule_change(kind));
            g = roundtrip(g);
        }
        prop_assert_eq!(g.list_changes(0, 50).len(), 8);
        expect_code("E59_PENDING_CHANGE_LIMIT", || g.schedule_change(kinds[0].clone()));
        g = roundtrip(g);
        expect_code("E51_INVALID_CHANGE", || g.schedule_change(ChangeKind::SetRuleSet {
            rule_set_hash: [0; 32], manifest_uri: String::new(), ai_required: true,
        }));
        g = roundtrip(g);
        context(account(0), START, 0, None);
        expect_code("E50_NOT_GOVERNOR", || g.schedule_change(kinds[0].clone()));
        expect_code("E56_NOT_GUARDIAN", || g.cancel_changes(ids.clone()));
        context("guardian.testnet".parse().unwrap(), START, 0, None);
        expect_code("E59_PENDING_CHANGE_LIMIT", || g.cancel_changes(vec![ids[0]; 9]));
        expect_code("E51_INVALID_CHANGE", || g.cancel_changes(vec![ids[0], ids[0]]));
        expect_code("E52_NOT_FOUND", || g.cancel_changes(vec![ids[0], 99]));
        g = roundtrip(g);
        for id in &ids {
            prop_assert_eq!(&g.get_change(*id).unwrap().status, &ChangeStatus::Pending);
        }
        g.cancel_changes(ids.clone());
        g = roundtrip(g);
        for id in &ids {
            prop_assert!(g.get_change(*id).is_none());
        }
        expect_code("E52_NOT_FOUND", || g.cancel_changes(vec![ids[0]]));
        g = roundtrip(g);
        context("governor.testnet".parse().unwrap(), START, 0, None);
        for kind in kinds { g.schedule_change(kind); g = roundtrip(g); }
        prop_assert_eq!(g.list_changes(0, 50).iter().filter(|c| c.status == ChangeStatus::Pending).count(), 3);
        prop_assert_eq!(g.get_roles().guardian.as_str(), "guardian.testnet");
        prop_assert_eq!(g.get_roles().governor.as_str(), "governor.testnet");
        prop_assert!(sign_requests().is_empty());
    }

    #[test]
    fn insolvent_liquid_balance_rejects_credit_growth_and_outgoing_actions(
        operation in 0u8..5,
        amount in 100u128..1_001,
    ) {
        let mut g = fresh_gate(42);
        let amount = if operation == 1 { amount + g.get_proposal_fee().0 } else { amount };
        deposit(&mut g, account(0), START, amount);
        g = roundtrip(g);
        prop_assert_eq!(g.get_total_liabilities().0, amount);
        if operation == 4 {
            proposal_context(&mut g, account(0), START);
            g.propose(args(0));
            g = roundtrip(g);
        }
        let storage_cost = u128::from(env::storage_usage()) * env::storage_byte_cost().as_yoctonear();
        let who = if operation == 2 { "governor.testnet".parse().unwrap() } else { account(0) };
        let attached = if operation == 0 { 1 } else { 0 };
        context_with_balance(who, START, attached, storage_cost);
        match operation {
            0 => expect_code("E12_STORAGE_INSOLVENT", || g.deposit_credit(None)),
            1 => expect_code("E12_STORAGE_INSOLVENT", || g.propose(args(0))),
            2 => expect_code("E12_STORAGE_INSOLVENT", || g.schedule_change(ChangeKind::Unpause)),
            3 => expect_code("E12_STORAGE_INSOLVENT", || g.withdraw_credit(U128(amount))),
            _ => {
                let c = certificate(&g, 0, START, 0);
                expect_code("E12_STORAGE_INSOLVENT", || submit(&mut g, &c, false));
            }
        }
        // E12 is a post-write receipt failure. The mocked host does not implement
        // NEAR rollback; discard this failed instance, rather than treating its
        // transient Rust fields as committed state. Machine::check constrains every
        // successfully committed step against the same liquid-balance equation.
        drop(g);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn migration_bumps_and_timeout_retries_preserve_nonce_deadline_and_signed_fallback(
        bumps in 1usize..10,
        failed_result in prop_oneof![Just(0u8), Just(2)],
    ) {
        let mut g = fresh_gate(42);
        context("governor.testnet".parse().unwrap(), START, 0, None);
        let id = g.schedule_change(ChangeKind::MigrateAuthority { new_signer: [12; 20] });
        g = roundtrip(g);
        let mut now = START + TIMELOCK_NS;
        context(account(0), now, 0, None);
        drop(g.apply_change(id));
        g = roundtrip(g);
        let original = g.get_change(id).unwrap().evm.clone().unwrap();
        target_callback(&mut g, SignTarget::Change(id), 1, now, 1);
        g = roundtrip(g);
        let mut prior_signed = g.get_change(id).unwrap().evm.as_ref().unwrap().signed_tx.clone();
        for index in 0..bumps {
            now += 600_000_000_000;
            context(account(0), now, 0, None);
            let before = g.get_change(id).unwrap().evm.clone().unwrap();
            if index == 8 {
                expect_code("E49_TOO_MANY_BUMPS", || g.bump_fee(SignTarget::Change(id)));
                prop_assert!(sign_requests().is_empty());
                break;
            }
            drop(g.bump_fee(SignTarget::Change(id)));
            prop_assert_eq!(sign_requests().len(), 1);
            g = roundtrip(g);
            let bumped = g.get_change(id).unwrap().evm.clone().unwrap();
            prop_assert_eq!(bumped.nonce, original.nonce);
            prop_assert_eq!(bumped.deadline_s, original.deadline_s);
            prop_assert_eq!(bumped.max_fee_per_gas, before.max_fee_per_gas + before.max_fee_per_gas.div_ceil(4));
            prop_assert_eq!(bumped.max_priority_fee_per_gas, before.max_priority_fee_per_gas + before.max_priority_fee_per_gas.div_ceil(4));
            prop_assert_ne!(bumped.signing_hash, before.signing_hash);
            prop_assert_eq!(&bumped.signed_tx, &prior_signed);
            target_callback(&mut g, SignTarget::Change(id), bumped.attempts, now, failed_result);
            g = roundtrip(g);
            prop_assert_eq!(&g.get_change(id).unwrap().evm.as_ref().unwrap().signed_tx, &prior_signed);
            context(account(0), now, 0, None);
            drop(g.apply_change(id));
            prop_assert_eq!(sign_requests().len(), 1);
            g = roundtrip(g);
            let attempt = g.get_change(id).unwrap().evm.as_ref().unwrap().attempts;
            // A lost retry callback can be retried only at the fixed timeout.
            now += 1_200_000_000_000 - 1;
            context(account(0), now, 0, None);
            expect_code("E54_NOT_PENDING", || g.apply_change(id));
            prop_assert!(sign_requests().is_empty());
            now += 1;
            context(account(0), now, 0, None);
            drop(g.apply_change(id));
            prop_assert_eq!(sign_requests().len(), 1);
            g = roundtrip(g);
            let e = g.get_change(id).unwrap().evm.as_ref().unwrap();
            prop_assert_eq!(e.attempts, attempt + 1);
            prop_assert_eq!(e.signing_hash, bumped.signing_hash);
            prop_assert_eq!(e.nonce, original.nonce);
            prop_assert_eq!(e.deadline_s, original.deadline_s);
            let current_attempt = e.attempts;
            target_callback(&mut g, SignTarget::Change(id), attempt, now, 1);
            g = roundtrip(g);
            prop_assert_eq!(&g.get_change(id).unwrap().evm.as_ref().unwrap().signed_tx, &prior_signed);
            target_callback(&mut g, SignTarget::Change(id), current_attempt, now, 1);
            g = roundtrip(g);
            prior_signed = g.get_change(id).unwrap().evm.as_ref().unwrap().signed_tx.clone();
            prop_assert!(prior_signed.is_some());
            prop_assert_eq!(&g.get_change(id).unwrap().kind, &ChangeKind::MigrateAuthority { new_signer: [12; 20] });
            prop_assert_eq!(g.get_total_liabilities().0, 0);
        }
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn withdrawal_callback_preserves_credit_and_liability_conservation(
        deposit in 1u128..1_001,
        fraction in 0u128..101,
        succeeds in any::<bool>(),
    ) {
        let mut g = fresh_gate(42);
        props_support::deposit(&mut g, account(0), START, deposit);
        g = roundtrip(g);
        let withdrawn = 1 + (deposit - 1) * fraction / 100;
        context(account(0), START, 0, None);
        drop(g.withdraw_credit(U128(withdrawn)));
        prop_assert_eq!(get_created_receipts().len(), 2);
        g = roundtrip(g);
        prop_assert_eq!(g.get_credit(account(0)).0, deposit - withdrawn);
        prop_assert_eq!(g.get_total_liabilities().0, deposit - withdrawn);
        let result = if succeeds {
            near_sdk::PromiseResult::Successful(vec![])
        } else {
            near_sdk::PromiseResult::Failed
        };
        context("gate.testnet".parse().unwrap(), START, 0, Some(result));
        prop_assert_eq!(g.on_withdraw_credit(account(0), U128(withdrawn)), succeeds);
        g = roundtrip(g);
        let settled_outgoing = if succeeds { withdrawn } else { 0 };
        prop_assert_eq!(g.get_credit(account(0)).0 + settled_outgoing, deposit);
        prop_assert_eq!(g.get_total_liabilities().0, g.get_credit(account(0)).0);
        let liquid = env::account_balance().as_yoctonear().saturating_sub(
            u128::from(env::storage_usage()) * env::storage_byte_cost().as_yoctonear(),
        );
        prop_assert!(g.get_total_liabilities().0 <= liquid);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn prepaid_proposal_rejects_unfunded_credit_and_nonzero_attachment(delta in 1u128..1_000_000) {
        let mut g = fresh_gate(42);
        context(account(0), START, 0, None);
        expect_code("E11_INSUFFICIENT_CREDIT", || g.propose(args(0)));
        let fee = g.get_proposal_fee().0;
        deposit(&mut g, account(0), START, 100 + fee);
        g = roundtrip(g);
        context(account(0), START, 0, None);
        let mut too_long = args(0);
        too_long.init_calldata.0.resize(1025, 0);
        expect_code("E22_INIT_CALLDATA_TOO_LONG", || g.propose(too_long));
        let mut zero = args(0);
        zero.new_impl.0 = [0; 20];
        expect_code("E23_ZERO_ADDRESS", || g.propose(zero));
        for attached in [delta, fee + delta] {
            context(account(0), START, attached, None);
            expect_code("E61_STORAGE_FEE", || g.propose(args(0)));
            prop_assert_eq!(g.get_total_liabilities().0, 100 + fee);
            prop_assert_eq!(g.get_active(account(0)), None);
        }
        context(account(0), START, 0, None);
        prop_assert_eq!(g.propose(args(0)), 0);
        context(account(0), START, 0, None);
        expect_code("E21_ACTIVE_PROPOSAL_EXISTS", || g.propose(args(0)));
    }

    #[test]
    fn bump_age_boundary_is_inclusive(offset in 0u64..1_000_000_000) {
        let mut m = Machine::new(42);
        m.now += offset;
        m.step(&Op::Deposit(2, 300));
        m.step(&Op::Propose(0, 0, 0));
        m.step(&Op::Verdict(0, 0, 0));
        m.step(&Op::Callback(0, 1, false));
        m.now += 600_000_000_000 - 1;
        m.step(&Op::Bump(0));
        prop_assert_eq!(m.proposals[0].bumps, 0);
        m.now += 1;
        m.step(&Op::Bump(0));
        prop_assert_eq!(m.proposals[0].bumps, 1);
    }
}

proptest! {
    #![proptest_config(property_config())]
    #[test]
    fn zero_withdrawal_never_creates_credit_entry(owner in 0u8..3) {
        let mut g = fresh_gate(42);
        g = roundtrip(g);
        let before = env::storage_usage();
        context(account(owner), START, 0, None);
        expect_code("E10_ZERO_DEPOSIT", || g.withdraw_credit(U128(0)));
        g = roundtrip(g);
        prop_assert_eq!(g.get_credit(account(owner)).0, 0);
        prop_assert_eq!(g.get_total_liabilities().0, 0);
        prop_assert_eq!(env::storage_usage(), before);
    }
}

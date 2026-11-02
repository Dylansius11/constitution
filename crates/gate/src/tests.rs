use super::*;
use constitution_types::{EvmTx, Hex20, Hex32, HexBytes};
use near_sdk::{
    NearToken,
    test_utils::{VMContextBuilder, get_created_receipts, get_logs},
    testing_env,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn context(who: &str, now: u64, deposit: u128) {
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id(who.parse().unwrap())
            .signer_account_id(who.parse().unwrap())
            .block_timestamp(now)
            .attached_deposit(NearToken::from_yoctonear(deposit))
            .build()
    );
}
fn params() -> Params {
    Params {
        proposal_deposit: 100,
        proposal_ttl_s: 7200,
        max_cert_ttl_s: 1800,
        sign_validity_s: 3600,
        gas_limit: 500000,
        max_fee_per_gas: 10,
        max_priority_fee_per_gas: 1,
        fee_cap: 100,
        sign_gas_tgas: 15,
        callback_gas_tgas: 20,
    }
}
fn config() -> Config {
    Config {
        chain_id: 84532,
        executor: [1; 20],
        proxy: [2; 20],
        proxy_admin: [3; 20],
        derived_eoa: [4; 20],
        mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
        mpc_path: "constitution/base-sepolia/v1".into(),
        mpc_domain_id: 0,
    }
}
fn constitution() -> Constitution {
    Constitution {
        rule_set_hash: [5; 32],
        manifest_uri: "ipfs://rules".into(),
        ai_required: true,
        params: params(),
    }
}
fn roles() -> Roles {
    Roles {
        governor: "governor.testnet".parse().unwrap(),
        guardian: "guardian.testnet".parse().unwrap(),
    }
}
fn verifier() -> VerifierRecord {
    VerifierRecord {
        release_hash: [6; 32],
        attestation_uri: "ipfs://quote".into(),
        added_at_ns: 0,
    }
}
fn gate() -> Gate {
    context("alice.testnet", 1_000_000_000, 0);
    Gate::new(
        config(),
        constitution(),
        roles(),
        vec![(Base64([7; 32]), verifier())],
        42,
    )
}
fn args() -> ProposeArgs {
    ProposeArgs {
        new_impl: Hex20([8; 20]),
        new_impl_codehash: Hex32([9; 32]),
        expected_current_impl: Hex20([10; 20]),
        init_calldata: HexBytes(vec![]),
        source_commitment: Hex32([11; 32]),
    }
}
fn fund(g: &mut Gate, who: &str, amount: u128) {
    context(who, 1_000_000_000, amount);
    g.deposit_credit(None);
    context(who, 1_000_000_000, 0);
}
fn fails(code: &str, f: impl FnOnce()) -> String {
    let error = catch_unwind(AssertUnwindSafe(f)).expect_err("expected rejection");
    let message = error
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| error.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap();
    // near-sdk 5.29.1 wraps GuestPanic in its mocked host's Result::unwrap panic.
    let message = if let Some((_, wrapped)) = message.split_once("panic_msg: ") {
        serde_json::from_str::<String>(wrapped.strip_suffix(" })").unwrap()).unwrap()
    } else {
        message
    };
    assert!(
        message.starts_with(code),
        "{message:?} does not start with {code}"
    );
    message
}
fn rule_change() -> ChangeKind {
    ChangeKind::SetRuleSet {
        rule_set_hash: [12; 32],
        manifest_uri: "ipfs://next".into(),
        ai_required: false,
    }
}
fn schedule(g: &mut Gate, kind: ChangeKind) -> u64 {
    context("governor.testnet", 1_000_000_000, 0);
    g.schedule_change(kind)
}
fn matured(g: &mut Gate, kind: ChangeKind) -> u64 {
    let id = schedule(g, kind);
    context("anyone.testnet", 1_000_000_000 + TIMELOCK_NS, 0);
    g.apply_change(id);
    id
}

#[test]
fn t_gate_1_proposal_reserves_credit_and_binds_rules() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 150);
    let id = g.propose(args());
    let p = g.get_proposal(id).unwrap();
    assert_eq!(id, 0);
    assert_eq!(p.state, ProposalState::Pending);
    assert_eq!(p.reserved, 100);
    assert_eq!(p.rule_set_hash, [5; 32]);
    assert_eq!(p.expires_at_ns, 7_201_000_000_000);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 50);
    assert_eq!(g.get_active("alice.testnet".parse().unwrap()), Some(id));
    fails("E21_ACTIVE_PROPOSAL_EXISTS", || {
        g.propose(args());
    });
    assert_eq!(g.next_evm_nonce, 42);
}
#[test]
fn credit_funding_recipient_and_withdrawal_effects() {
    let mut g = gate();
    context("bob.testnet", 1_000_000_000, 150);
    g.deposit_credit(Some("alice.testnet".parse().unwrap()));
    context("alice.testnet", 1_000_000_000, 0);
    g.propose(args());
    fails("E11_INSUFFICIENT_CREDIT", || {
        drop(g.withdraw_credit(U128(51)));
    });
    drop(g.withdraw_credit(U128(50)));
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 0);
    let receipts = get_created_receipts();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].receiver_id.as_str(), "alice.testnet");
    assert!(
        matches!(receipts[0].actions[0], near_sdk::mock::MockAction::Transfer { deposit, .. } if deposit.as_yoctonear()==50)
    );
    fails("E10_ZERO_DEPOSIT", || g.deposit_credit(None));
}
#[test]
fn proposals_reject_insufficient_credit_long_calldata_zero_addresses_and_duplicates() {
    let mut g = gate();
    fails("E11_INSUFFICIENT_CREDIT", || {
        g.propose(args());
    });
    fund(&mut g, "alice.testnet", 100);
    let mut a = args();
    a.init_calldata.0 = vec![0; 1025];
    fails("E22_INIT_CALLDATA_TOO_LONG", || {
        g.propose(a);
    });
    let mut a = args();
    a.new_impl.0 = [0; 20];
    fails("E23_ZERO_ADDRESS", || {
        g.propose(a);
    });
    let mut a = args();
    a.expected_current_impl.0 = [0; 20];
    fails("E23_ZERO_ADDRESS", || {
        g.propose(a);
    });
    g.propose(args());
    fund(&mut g, "bob.testnet", 100);
    fails("E24_DUPLICATE_ACTIVE", || {
        g.propose(args());
    });
    let mut a = args();
    a.init_calldata.0 = vec![0; 1024];
    assert_eq!(g.propose(a), 1);
}
#[test]
fn expire_boundary_releases_once_and_reopens_duplicate_slot_while_paused() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    let id = g.propose(args());
    fails("E30_NOT_FOUND", || g.expire(99));
    context("guardian.testnet", 7_201_000_000_000, 0);
    g.pause_signing();
    fails("E32_NOT_EXPIRED", || g.expire(id));
    context("anyone.testnet", 7_201_000_000_001, 0);
    g.expire(id);
    assert_eq!(g.get_proposal(id).unwrap().state, ProposalState::Expired);
    assert_eq!(g.get_proposal(id).unwrap().reserved, 0);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 100);
    assert_eq!(g.get_active("alice.testnet".parse().unwrap()), None);
    fails("E31_WRONG_STATE", || g.expire(id));
    context("alice.testnet", 7_201_000_000_001, 0);
    assert_eq!(g.propose(args()), 1);
    drop(g.withdraw_credit(U128(0)));
}
#[test]
fn t_gate_6_timelock_before_eta_minus_one_and_at_eta() {
    let mut g = gate();
    let id = schedule(&mut g, rule_change());
    assert_eq!(g.get_change(id).unwrap().eta_ns, 604_801_000_000_000);
    let msg = fails("E53_TIMELOCK_NOT_ELAPSED", || g.apply_change(id));
    assert!(msg.contains("eta_ns=604801000000000"));
    assert!(msg.contains("now_ns=1000000000"));
    context("anyone.testnet", 604_800_999_999_999, 0);
    fails("E53_TIMELOCK_NOT_ELAPSED", || g.apply_change(id));
    context("anyone.testnet", 604_801_000_000_000, 0);
    g.apply_change(id);
    assert_eq!(g.get_constitution().rule_set_hash, [12; 32]);
    assert!(!g.get_constitution().ai_required);
    assert_eq!(g.get_change(id).unwrap().status, ChangeStatus::Applied);
    fails("E54_NOT_PENDING", || g.apply_change(id));
}
#[test]
fn t_gate_7_roles_and_cancellation_fail_closed() {
    let mut g = gate();
    context("guardian.testnet", 1_000_000_000, 0);
    fails("E50_NOT_GOVERNOR", || {
        g.schedule_change(rule_change());
    });
    let id = schedule(&mut g, rule_change());
    fails("E56_NOT_GUARDIAN", || g.cancel_change(id));
    fails("E56_NOT_GUARDIAN", || g.pause_signing());
    fails("E56_NOT_GUARDIAN", || {
        g.remove_verifier_now(Base64([7; 32]))
    });
    fails("E52_NOT_FOUND", || g.apply_change(99));
    context("guardian.testnet", 1_000_000_000, 0);
    fails("E52_NOT_FOUND", || g.cancel_change(99));
    g.cancel_change(id);
    assert_eq!(g.get_change(id).unwrap().status, ChangeStatus::Cancelled);
    fails("E54_NOT_PENDING", || g.cancel_change(id));
    context("anyone.testnet", 604_801_000_000_000, 0);
    fails("E54_NOT_PENDING", || g.apply_change(id));
}
#[test]
fn t_gate_8_rules_block_requested_and_unexpired_signed_but_allow_deadline_equality() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    let pid = g.propose(args());
    let id = schedule(&mut g, rule_change());
    context("anyone.testnet", 604_801_000_000_000, 0);
    g.proposals.get_mut(&pid).unwrap().state = ProposalState::SignRequested;
    g.outstanding_signatures.insert(pid);
    fails("E55_SIGNATURES_OUTSTANDING", || g.apply_change(id));
    let p = g.proposals.get_mut(&pid).unwrap();
    p.state = ProposalState::Signed;
    p.evm = Some(EvmTx {
        nonce: 42,
        deadline_s: 604802,
        gas_limit: 500000,
        max_fee_per_gas: 10,
        max_priority_fee_per_gas: 1,
        signing_hash: [1; 32],
        signed_tx: None,
        attempts: 0,
    });
    fails("E55_SIGNATURES_OUTSTANDING", || g.apply_change(id));
    g.proposals
        .get_mut(&pid)
        .unwrap()
        .evm
        .as_mut()
        .unwrap()
        .deadline_s = 604801;
    g.apply_change(id);
    assert_eq!(g.get_constitution().rule_set_hash, [12; 32]);
    assert!(g.outstanding_signatures.is_empty());
}
#[test]
fn governance_applies_all_unsigned_changes_and_guards_last_verifier() {
    let mut g = gate();
    context("guardian.testnet", 1_000_000_000, 0);
    g.pause_signing();
    assert!(g.signing_paused);
    fails("E57_LAST_VERIFIER", || {
        g.remove_verifier_now(Base64([7; 32]))
    });
    fails("E51_INVALID_CHANGE", || {
        g.remove_verifier_now(Base64([99; 32]))
    });
    matured(
        &mut g,
        ChangeKind::AddVerifier {
            pubkey: [8; 32],
            release_hash: [9; 32],
            attestation_uri: "ipfs://second".into(),
        },
    );
    assert_eq!(g.list_verifiers(0, 50).len(), 2);
    context("guardian.testnet", 1_000_000_000, 0);
    g.remove_verifier_now(Base64([8; 32]));
    assert_eq!(g.list_verifiers(0, 50).len(), 1);
    matured(
        &mut g,
        ChangeKind::AddVerifier {
            pubkey: [8; 32],
            release_hash: [9; 32],
            attestation_uri: "ipfs://second".into(),
        },
    );
    matured(&mut g, ChangeKind::RemoveVerifier { pubkey: [7; 32] });
    assert_eq!(g.list_verifiers(0, 50)[0].0, Base64([8; 32]));
    let id = schedule(&mut g, ChangeKind::RemoveVerifier { pubkey: [8; 32] });
    context("anyone.testnet", 604_801_000_000_000, 0);
    fails("E57_LAST_VERIFIER", || g.apply_change(id));
    let mut p = params();
    p.proposal_deposit = 200;
    matured(&mut g, ChangeKind::SetParams { params: p.clone() });
    assert_eq!(g.get_constitution().params, p);
    matured(&mut g, ChangeKind::Unpause);
    assert!(!g.signing_paused);
    matured(
        &mut g,
        ChangeKind::SetGuardian {
            account: "next-guardian.testnet".parse().unwrap(),
        },
    );
    assert_eq!(g.get_roles().guardian.as_str(), "next-guardian.testnet");
    matured(
        &mut g,
        ChangeKind::SetGovernor {
            account: "next-governor.testnet".parse().unwrap(),
        },
    );
    assert_eq!(g.get_roles().governor.as_str(), "next-governor.testnet");
}
#[test]
fn invalid_changes_and_migration_sequencing() {
    let mut g = gate();
    context("governor.testnet", 1_000_000_000, 0);
    for uri in [String::new(), "x".repeat(257)] {
        fails("E51_INVALID_CHANGE", || {
            g.schedule_change(ChangeKind::SetRuleSet {
                rule_set_hash: [1; 32],
                manifest_uri: uri.clone(),
                ai_required: true,
            });
        });
        fails("E51_INVALID_CHANGE", || {
            g.schedule_change(ChangeKind::AddVerifier {
                pubkey: [9; 32],
                release_hash: [1; 32],
                attestation_uri: uri,
            });
        });
    }
    let mut p = params();
    p.sign_gas_tgas = 14;
    fails("E51_INVALID_CHANGE", || {
        g.schedule_change(ChangeKind::SetParams { params: p });
    });
    fails("E51_INVALID_CHANGE", || {
        g.schedule_change(ChangeKind::MigrateAuthority {
            new_signer: [0; 20],
        });
    });
    fails("E51_INVALID_CHANGE", || {
        g.schedule_change(ChangeKind::AddVerifier {
            pubkey: [7; 32],
            release_hash: [1; 32],
            attestation_uri: "ipfs://quote".into(),
        });
    });
    fails("E51_INVALID_CHANGE", || {
        g.schedule_change(ChangeKind::RemoveVerifier { pubkey: [99; 32] });
    });
    let id = g.schedule_change(ChangeKind::MigrateAuthority {
        new_signer: [1; 20],
    });
    context("anyone.testnet", 604_801_000_000_000, 0);
    assert!(
        fails("E51_INVALID_CHANGE", || g.apply_change(id))
            .ends_with("MigrateAuthority apply lands with signing")
    );
    assert_eq!(g.get_change(id).unwrap().status, ChangeStatus::Pending);
    context("guardian.testnet", 604_801_000_000_000, 0);
    g.cancel_change(id);
}
#[test]
fn initialization_rejects_every_invalid_binding() {
    context("alice.testnet", 1_000_000_000, 0);
    let init = |c, k, v| {
        Gate::new(c, k, roles(), v, 0);
    };
    let vs = || vec![(Base64([7; 32]), verifier())];
    let mut c = config();
    c.chain_id = 1;
    fails("E51_INVALID_CHANGE", || init(c, constitution(), vs()));
    let mut c = config();
    c.mpc_account = "evil.testnet".parse().unwrap();
    fails("E51_INVALID_CHANGE", || init(c, constitution(), vs()));
    for field in 0..4 {
        let mut c = config();
        match field {
            0 => c.executor = [0; 20],
            1 => c.proxy = [0; 20],
            2 => c.proxy_admin = [0; 20],
            _ => c.derived_eoa = [0; 20],
        };
        fails("E51_INVALID_CHANGE", || init(c, constitution(), vs()));
    }
    let mut k = constitution();
    k.params.sign_gas_tgas = 14;
    fails("E51_INVALID_CHANGE", || init(config(), k, vs()));
    fails("E51_INVALID_CHANGE", || {
        init(config(), constitution(), vec![])
    });
    let mut c = config();
    c.mpc_path = "wrong".into();
    fails("E51_INVALID_CHANGE", || init(c, constitution(), vs()));
    let mut c = config();
    c.mpc_domain_id = 1;
    fails("E51_INVALID_CHANGE", || init(c, constitution(), vs()));
    let mut k = constitution();
    k.manifest_uri.clear();
    fails("E51_INVALID_CHANGE", || init(config(), k, vs()));
    let mut k = constitution();
    k.params.proposal_ttl_s = u64::MAX;
    fails("E51_INVALID_CHANGE", || init(config(), k, vs()));
    let mut v = verifier();
    v.attestation_uri.clear();
    fails("E51_INVALID_CHANGE", || {
        init(config(), constitution(), vec![(Base64([7; 32]), v)])
    });
    fails("E51_INVALID_CHANGE", || {
        init(
            config(),
            constitution(),
            vec![(Base64([7; 32]), verifier()), (Base64([7; 32]), verifier())],
        )
    });
}
#[test]
fn views_paginate_and_events_have_canonical_hashes() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    g.propose(args());
    let logs = get_logs();
    let event: serde_json::Value =
        serde_json::from_str(logs[0].strip_prefix("EVENT_JSON:").unwrap()).unwrap();
    assert_eq!(event["standard"], "constitution");
    assert_eq!(event["version"], "1.0.0");
    assert_eq!(event["event"], "proposal_created");
    assert_eq!(
        event["data"][0]["rule_set_hash"],
        format!("0x{}", "05".repeat(32))
    );
    assert_eq!(g.get_config(), &config());
    assert_eq!(g.list_proposals(0, 51).len(), 1);
    assert!(g.list_proposals(1, 50).is_empty());
    assert!(g.list_proposals(u64::MAX, 50).is_empty());
    assert!(g.list_proposals(0, 0).is_empty());
    for _ in 0..51 {
        schedule(&mut g, ChangeKind::Unpause);
    }
    assert_eq!(g.list_changes(0, 51).len(), 50);
    assert_eq!(g.list_changes(50, 50)[0].id, 50);
    assert!(g.list_changes(u64::MAX, 50).is_empty());
    assert!(g.list_verifiers(0, 0).is_empty());
}

fn last_event(name: &str) -> serde_json::Value {
    let logs = get_logs();
    let value: serde_json::Value =
        serde_json::from_str(logs.last().unwrap().strip_prefix("EVENT_JSON:").unwrap()).unwrap();
    assert_eq!(value["standard"], "constitution");
    assert_eq!(value["version"], "1.0.0");
    assert_eq!(value["event"], name);
    assert_eq!(value["data"].as_array().unwrap().len(), 1);
    value["data"][0].clone()
}

#[test]
fn lifecycle_and_guardian_events_identify_the_actual_transition() {
    let mut g = gate();
    let id = schedule(&mut g, rule_change());
    let event = last_event("change_scheduled");
    assert_eq!(event["change_id"], id);
    assert_eq!(event["eta_ns"], 604_801_000_000_000u64);
    assert_eq!(
        event["kind"]["SetRuleSet"]["rule_set_hash"],
        format!("0x{}", "0c".repeat(32))
    );
    context("guardian.testnet", 1_000_000_000, 0);
    g.cancel_change(id);
    assert_eq!(last_event("change_cancelled")["change_id"], id);
    g.pause_signing();
    assert!(last_event("signing_paused")["change_id"].is_null());
    let id = matured(
        &mut g,
        ChangeKind::AddVerifier {
            pubkey: [8; 32],
            release_hash: [9; 32],
            attestation_uri: "ipfs://next".into(),
        },
    );
    assert_eq!(last_event("change_applied")["change_id"], id);
    assert_eq!(
        g.verifiers.get(&[8; 32]).unwrap().added_at_ns,
        604_801_000_000_000
    );
    context("guardian.testnet", 604_801_000_000_000, 0);
    g.remove_verifier_now(Base64([8; 32]));
    assert_eq!(
        last_event("verifier_removed")["pubkey"],
        serde_json::to_value(Base64([8; 32])).unwrap()
    );
    fund(&mut g, "alice.testnet", 100);
    let id = g.propose(args());
    context("anyone.testnet", 7_201_000_000_001, 0);
    g.expire(id);
    assert_eq!(last_event("proposal_expired")["proposal_id"], id);
}

#[test]
fn queued_verifier_changes_revalidate_after_guardian_removal() {
    let mut g = gate();
    matured(
        &mut g,
        ChangeKind::AddVerifier {
            pubkey: [8; 32],
            release_hash: [9; 32],
            attestation_uri: "ipfs://next".into(),
        },
    );
    let id = schedule(&mut g, ChangeKind::RemoveVerifier { pubkey: [8; 32] });
    context("guardian.testnet", 1_000_000_000, 0);
    g.remove_verifier_now(Base64([8; 32]));
    context("anyone.testnet", 604_801_000_000_000, 0);
    fails("E51_INVALID_CHANGE", || g.apply_change(id));
    assert_eq!(g.get_change(id).unwrap().status, ChangeStatus::Pending);
}

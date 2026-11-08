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
    near_sdk::mock::with_mocked_blockchain(|b| b.take_storage());
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
    // The first deposit of an account also pays its credit entry's storage, so the credit that
    // results equals `amount`.
    let account: AccountId = who.parse().unwrap();
    let entry_fee = if g.credit.contains_key(&account) {
        0
    } else {
        Gate::credit_entry_fee(&account)
    };
    context(who, 1_000_000_000, amount + entry_fee);
    g.deposit_credit(None);
    context(who, 1_000_000_000, 0);
}
/// Prepay one proposal's storage from an existing entry, then call without a deposit.
fn propose_fee(g: &mut Gate, args: ProposeArgs) -> u64 {
    let who = env::predecessor_account_id();
    let now = env::block_timestamp();
    if g.credit.contains_key(&who) {
        context(who.as_str(), now, Gate::proposal_storage_fee());
        g.deposit_credit(None);
    }
    context(who.as_str(), now, 0);
    let id = g.propose(args);
    context(who.as_str(), now, 0);
    id
}
fn fails<T>(code: &str, f: impl FnOnce() -> T) -> String {
    let error = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(_) => panic!("expected rejection"),
        Err(error) => error,
    };
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
    drop(g.apply_change(id));
    id
}

#[test]
fn prepaid_proposal_fee_requires_full_credit_and_zero_attachment() {
    let mut g = gate();
    let fee = Gate::proposal_storage_fee();
    fund(&mut g, "alice.testnet", fee + 99);
    fails("E11_INSUFFICIENT_CREDIT", || g.propose(args()));
    assert_eq!(g.total_liabilities, fee + 99);
    assert!(g.get_proposal(0).is_none());
    context("alice.testnet", 1_000_000_000, 1);
    g.deposit_credit(None);
    context("alice.testnet", 1_000_000_000, fee);
    fails("E61_STORAGE_FEE", || g.propose(args()));
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, fee + 100);
    context("alice.testnet", 1_000_000_000, 0);
    let id = g.propose(args());
    assert_eq!(g.total_liabilities, 100);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 0);
    assert_eq!(g.get_proposal(id).unwrap().reserved, 100);
    context("anyone.testnet", 7_201_000_000_001, 0);
    g.expire(id);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 100);
    assert_eq!(g.total_liabilities, 100, "storage fee is never refunded");
}

#[test]
fn t_gate_1_proposal_reserves_credit_and_binds_rules() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 150);
    let id = propose_fee(&mut g, args());
    let p = g.get_proposal(id).unwrap();
    assert_eq!(id, 0);
    assert_eq!(p.state, ProposalState::Pending);
    assert_eq!(p.reserved, 100);
    assert_eq!(p.rule_set_hash, [5; 32]);
    assert_eq!(p.expires_at_ns, 7_201_000_000_000);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 50);
    assert_eq!(g.get_active("alice.testnet".parse().unwrap()), Some(id));
    fails("E21_ACTIVE_PROPOSAL_EXISTS", || {
        propose_fee(&mut g, args());
    });
    assert_eq!(g.next_evm_nonce, 42);
}
#[test]
fn credit_funding_recipient_and_withdrawal_effects() {
    let mut g = gate();
    let alice: AccountId = "alice.testnet".parse().unwrap();
    let entry_fee = Gate::credit_entry_fee(&alice);
    // A first deposit that does not exceed the entry fee is refused and records nothing.
    context("bob.testnet", 1_000_000_000, entry_fee);
    let msg = fails("E61_STORAGE_FEE", || g.deposit_credit(Some(alice.clone())));
    assert!(msg.contains(&entry_fee.to_string()));
    assert_eq!(g.total_liabilities, 0);
    // The fee is taken from the first deposit only: credit is the deposit minus the fee.
    context("bob.testnet", 1_000_000_000, 150 + entry_fee);
    g.deposit_credit(Some(alice.clone()));
    assert_eq!(g.get_credit(alice.clone()).0, 150);
    assert_eq!(g.total_liabilities, 150);
    assert_eq!(g.get_operating_surplus().0, g.operating_surplus());
    context("alice.testnet", 1_000_000_000, 0);
    propose_fee(&mut g, args());
    fails("E11_INSUFFICIENT_CREDIT", || {
        drop(g.withdraw_credit(U128(51)));
    });
    drop(g.withdraw_credit(U128(50)));
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 0);
    let receipts = get_created_receipts();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].receiver_id.as_str(), "alice.testnet");
    assert!(
        matches!(receipts[0].actions[0], near_sdk::mock::MockAction::Transfer { deposit, .. } if deposit.as_yoctonear()==50)
    );
    fails("E10_ZERO_DEPOSIT", || g.deposit_credit(None));
    // Later deposits for an existing entry pay no fee.
    context("bob.testnet", 1_000_000_000, 7);
    g.deposit_credit(Some(alice.clone()));
    assert_eq!(g.get_credit(alice).0, 7);
}
#[test]
fn exactly_backed_gate_can_pay_out_a_full_withdrawal() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    g.flush_storage();
    let storage = env::storage_usage();
    let balance = u128::from(storage) * env::storage_byte_cost().as_yoctonear() + 100;
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("alice.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(balance))
            .storage_usage(storage)
            .build()
    );
    assert_eq!(g.operating_surplus(), 0);
    drop(g.withdraw_credit(U128(100)));
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 0);
    assert_eq!(g.pending_withdrawals, 100);
}
#[test]
fn proposals_reject_insufficient_credit_long_calldata_zero_addresses_and_duplicates() {
    let mut g = gate();
    fails("E11_INSUFFICIENT_CREDIT", || {
        propose_fee(&mut g, args());
    });
    fund(&mut g, "alice.testnet", 100);
    let mut a = args();
    a.init_calldata.0 = vec![0; 1025];
    fails("E22_INIT_CALLDATA_TOO_LONG", || {
        propose_fee(&mut g, a);
    });
    let mut a = args();
    a.new_impl.0 = [0; 20];
    fails("E23_ZERO_ADDRESS", || {
        propose_fee(&mut g, a);
    });
    let mut a = args();
    a.expected_current_impl.0 = [0; 20];
    fails("E23_ZERO_ADDRESS", || {
        propose_fee(&mut g, a);
    });
    propose_fee(&mut g, args());
    fund(&mut g, "bob.testnet", 100);
    assert_eq!(propose_fee(&mut g, args()), 1);
    fund(&mut g, "carol.testnet", 100);
    let mut a = args();
    a.init_calldata.0 = vec![0; 1024];
    assert_eq!(propose_fee(&mut g, a), 2);
}
#[test]
fn expire_boundary_releases_once_and_reopens_duplicate_slot_while_paused() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    let id = propose_fee(&mut g, args());
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
    assert_eq!(propose_fee(&mut g, args()), 1);
    fails("E10_ZERO_DEPOSIT", || g.withdraw_credit(U128(0)));
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
    drop(g.apply_change(id));
    assert_eq!(g.get_constitution().rule_set_hash, [12; 32]);
    assert!(!g.get_constitution().ai_required);
    assert!(g.get_change(id).is_none());
    fails("E52_NOT_FOUND", || g.apply_change(id));
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
    assert!(g.get_change(id).is_none());
    fails("E52_NOT_FOUND", || g.cancel_change(id));
    context("anyone.testnet", 604_801_000_000_000, 0);
    fails("E52_NOT_FOUND", || g.apply_change(id));
}
#[test]
fn t_gate_8_rules_block_through_inclusive_executor_deadline() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    let pid = propose_fee(&mut g, args());
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
    fails("E55_SIGNATURES_OUTSTANDING", || g.apply_change(id));
    g.proposals.get_mut(&pid).unwrap().state = ProposalState::SignFailed;
    fails("E55_SIGNATURES_OUTSTANDING", || g.apply_change(id));
    context("anyone.testnet", 604_802_000_000_000, 0);
    drop(g.apply_change(id));
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
fn invalid_changes_and_migration_can_be_cancelled() {
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
    fund(&mut g, "alice.testnet", 100 + Gate::proposal_storage_fee());
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
        let id = schedule(&mut g, ChangeKind::Unpause);
        context("guardian.testnet", 1_000_000_000, 0);
        g.cancel_change(id);
    }
    // Cancelled changes are deleted; only the two live ones are listed and ids stay stable.
    schedule(&mut g, ChangeKind::Unpause);
    schedule(&mut g, ChangeKind::Unpause);
    assert_eq!(g.list_changes(0, 51).len(), 2);
    assert_eq!(g.list_changes(0, 51)[0].id, 51);
    assert_eq!(g.list_changes(52, 50)[0].id, 52);
    assert!(g.list_changes(u64::MAX, 50).is_empty());
    assert!(g.list_changes(0, 0).is_empty());
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
    let id = propose_fee(&mut g, args());
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

#[test]
fn unsafe_params_and_zero_release_are_rejected_at_both_boundaries() {
    let mut g = gate();
    let mut invalid = Vec::new();
    for (callback, sign) in [(9, 15), (101, 15), (20, 14), (100, 201)] {
        let mut p = params();
        p.callback_gas_tgas = callback;
        p.sign_gas_tgas = sign;
        invalid.push(p);
    }
    for field in 0..8 {
        let mut p = params();
        match field {
            0 => p.max_cert_ttl_s = 29,
            1 => p.max_cert_ttl_s = 86401,
            2 => p.sign_validity_s = 59,
            3 => p.sign_validity_s = 86401,
            4 => p.proposal_ttl_s = 299,
            5 => p.proposal_ttl_s = 604801,
            6 => p.gas_limit = 0,
            _ => p.max_priority_fee_per_gas = 11,
        }
        invalid.push(p);
    }
    let mut p = params();
    p.max_fee_per_gas = 101;
    invalid.push(p);
    for p in invalid {
        let mut c = constitution();
        c.params = p.clone();
        fails("E51_INVALID_CHANGE", || {
            Gate::new(config(), c, roles(), vec![(Base64([7; 32]), verifier())], 0)
        });
        context("governor.testnet", 1_000_000_000, 0);
        fails("E51_INVALID_CHANGE", || {
            g.schedule_change(ChangeKind::SetParams { params: p })
        });
    }
    let mut v = verifier();
    v.release_hash = [0; 32];
    fails("E51_INVALID_CHANGE", || {
        Gate::new(
            config(),
            constitution(),
            roles(),
            vec![(Base64([7; 32]), v)],
            0,
        )
    });
    fails("E51_INVALID_CHANGE", || {
        g.schedule_change(ChangeKind::AddVerifier {
            pubkey: [8; 32],
            release_hash: [0; 32],
            attestation_uri: "ipfs://quote".into(),
        })
    });
    let mut p = params();
    p.gas_limit = 300000;
    p.max_priority_fee_per_gas = 1000000;
    p.max_fee_per_gas = 50000000;
    p.fee_cap = 1000000000;
    assert_eq!(g.schedule_change(ChangeKind::SetParams { params: p }), 0);
}

#[test]
fn pending_queue_is_bounded_unique_and_guardian_can_cancel_batch() {
    let mut g = gate();
    for kind in [
        ChangeKind::MigrateAuthority {
            new_signer: [1; 20],
        },
        ChangeKind::SetGuardian {
            account: "next.testnet".parse().unwrap(),
        },
        ChangeKind::SetGovernor {
            account: "next.testnet".parse().unwrap(),
        },
    ] {
        let id = schedule(&mut g, kind.clone());
        fails("E60_DUPLICATE_AUTHORITY_CHANGE", || g.schedule_change(kind));
        assert_eq!(id, g.next_change_id - 1);
    }
    // Non-authority kinds are capped at two each, so one kind cannot take the whole queue.
    for (cap_kind, extra) in [
        (ChangeKind::Unpause, 2usize),
        (
            ChangeKind::SetRuleSet {
                rule_set_hash: [3; 32],
                manifest_uri: "ipfs://next".into(),
                ai_required: true,
            },
            2,
        ),
    ] {
        for _ in 0..extra {
            schedule(&mut g, cap_kind.clone());
        }
        fails("E60_DUPLICATE_AUTHORITY_CHANGE", || {
            g.schedule_change(cap_kind.clone())
        });
    }
    schedule(&mut g, ChangeKind::SetParams { params: params() });
    assert_eq!(g.pending_changes.len(), 8);
    fails("E59_PENDING_CHANGE_LIMIT", || {
        g.schedule_change(ChangeKind::SetParams { params: params() })
    });
    fails("E56_NOT_GUARDIAN", || g.cancel_changes(vec![0, 1, 2]));
    context("guardian.testnet", 1_000_000_000, 0);
    fails("E59_PENDING_CHANGE_LIMIT", || g.cancel_changes(vec![0; 9]));
    fails("E52_NOT_FOUND", || g.cancel_changes(vec![0, 99]));
    assert_eq!(g.get_change(0).unwrap().status, ChangeStatus::Pending);
    g.cancel_changes((0..8).collect());
    // Cancelled changes are deleted, not kept as permanent storage.
    assert!(g.list_changes(0, 50).is_empty());
    assert!(g.pending_changes.is_empty());
    assert_eq!(
        schedule(
            &mut g,
            ChangeKind::MigrateAuthority {
                new_signer: [2; 20]
            }
        ),
        8
    );
}

#[test]
fn proposal_storage_growth_is_the_solvency_limiter_and_measurement() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100 + Gate::proposal_storage_fee());
    g.flush_storage();
    let before = env::storage_usage();
    let mut a = args();
    a.init_calldata.0 = vec![0x55; 1024];
    context("alice.testnet", 1_000_000_000, 0);
    g.propose(a);
    g.flush_storage();
    let delta = env::storage_usage() - before;
    println!("max_calldata_proposal_storage_bytes={delta}");
    assert!(delta > 1024);
    assert!(
        delta <= PROPOSAL_STORAGE_BYTES,
        "the committed storage fee must cover the pending record"
    );
    assert_eq!(g.total_liabilities, 100);
    // With no operating surplus, prepaid storage must still fund the next record.
    context("alice.testnet", 7_201_000_000_001, 0);
    g.expire(0);
    fund(&mut g, "alice.testnet", Gate::proposal_storage_fee());
    g.flush_storage();
    let storage = env::storage_usage();
    let fee = Gate::proposal_storage_fee();
    let balance = u128::from(storage) * env::storage_byte_cost().as_yoctonear() + 100 + fee;
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("alice.testnet".parse().unwrap())
            .block_timestamp(7_201_000_000_002)
            .account_balance(NearToken::from_yoctonear(balance))
            .attached_deposit(NearToken::from_yoctonear(0))
            .storage_usage(storage)
            .build()
    );
    let mut again = args();
    again.init_calldata.0 = vec![0x66; 1024];
    g.propose(again);
    // Accepted (no E12) with zero prior surplus: the prepaid fee covered the record.
    g.flush_storage();
}

#[test]
fn governance_writes_need_operating_surplus() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 100);
    g.flush_storage();
    let storage = env::storage_usage();
    // Exactly backed: liquid balance equals liabilities, so a transient governance write is
    // refused instead of eating user credit.
    let balance = u128::from(storage) * env::storage_byte_cost().as_yoctonear() + 100;
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("governor.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .account_balance(NearToken::from_yoctonear(balance))
            .storage_usage(storage)
            .build()
    );
    fails("E12_STORAGE_INSOLVENT", || g.schedule_change(rule_change()));
}

#[test]
fn failed_withdrawal_restores_credit_and_liabilities_success_does_not() {
    let mut g = gate();
    fund(&mut g, "alice.testnet", 150);
    drop(g.withdraw_credit(U128(50)));
    assert_eq!(g.total_liabilities, 100);
    for (result, expected) in [
        (near_sdk::PromiseResult::Failed, 150),
        (near_sdk::PromiseResult::Successful(vec![]), 100),
    ] {
        testing_env!(
            VMContextBuilder::new()
                .current_account_id("gate.testnet".parse().unwrap())
                .predecessor_account_id("gate.testnet".parse().unwrap())
                .build(),
            near_sdk::test_vm_config(),
            near_sdk::RuntimeFeesConfig::test(),
            Default::default(),
            vec![result]
        );
        g.on_withdraw_credit("alice.testnet".parse().unwrap(), U128(50));
        assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, expected);
        assert_eq!(g.total_liabilities, expected);
        if expected == 150 {
            context("alice.testnet", 1_000_000_000, 0);
            drop(g.withdraw_credit(U128(50)));
        }
    }
}

#[test]
fn refunded_withdrawal_cannot_be_spent_before_failure_callback() {
    let mut g = gate();
    // Below the reserve cap, so the whole refund stays protected.
    let amount = 10u128.pow(23);
    assert!(amount < Gate::withdrawal_reserve(u128::MAX));
    fund(&mut g, "alice.testnet", amount);
    drop(g.withdraw_credit(U128(amount)));
    let storage = env::storage_usage();
    // Transfer failed, its refund has arrived, but callback has not run.
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("governor.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(
                u128::from(storage) * env::storage_byte_cost().as_yoctonear() + amount
            ))
            .storage_usage(storage)
            .build()
    );
    fails("E12_STORAGE_INSOLVENT", || {
        g.schedule_change(ChangeKind::Unpause)
    });
}

#[test]
fn large_withdrawal_in_flight_does_not_lock_writes() {
    // R3-1: reserving the whole amount would need surplus >= amount, so anyone holding more than the
    // operating surplus could open E12 windows on every write. The reserve is capped.
    let mut g = gate();
    let amount = 10u128.pow(24);
    fund(&mut g, "alice.testnet", amount);
    drop(g.withdraw_credit(U128(amount)));
    assert_eq!(g.pending_withdrawals, Gate::withdrawal_reserve(amount));
    assert!(g.pending_withdrawals < amount);
    let storage = env::storage_usage();
    let surplus = 5 * 10u128.pow(23);
    // The transfer has left; the gate holds storage plus a 0.5 NEAR operating surplus only.
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("governor.testnet".parse().unwrap())
            .account_balance(NearToken::from_yoctonear(
                u128::from(storage) * env::storage_byte_cost().as_yoctonear() + surplus
            ))
            .storage_usage(storage)
            .build()
    );
    assert_eq!(g.operating_surplus(), surplus - g.pending_withdrawals);
    g.schedule_change(ChangeKind::Unpause);
    // The callback releases exactly the reserve it took.
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("gate.testnet".parse().unwrap())
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![near_sdk::PromiseResult::Successful(vec![])]
    );
    assert!(g.on_withdraw_credit("alice.testnet".parse().unwrap(), U128(amount)));
    assert_eq!(g.pending_withdrawals, 0);
}

#[path = "signing_tests.rs"]
mod signing_tests;

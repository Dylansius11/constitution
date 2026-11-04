use super::*;
use constitution_types::{AiReview, Base64Bytes, Certificate, DOMAIN, SignTarget, Verdict};
use ed25519_dalek::{Signer, SigningKey};
use near_sdk::{PromiseOrValue, PromiseResult};

fn key() -> SigningKey {
    SigningKey::from_bytes(&[42; 32])
}
fn setup() -> (Gate, Certificate) {
    let mut g = gate();
    g.verifiers.clear();
    g.verifiers
        .insert(key().verifying_key().to_bytes(), verifier());
    fund(&mut g, "alice.testnet", 100);
    let id = propose_fee(&mut g, args());
    let c = Certificate {
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
        verifier_pubkey: key().verifying_key().to_bytes(),
        verifier_release_hash: [6; 32],
        evidence_block_number: 1,
        evidence_block_hash: [2; 32],
        evidence_root: [3; 32],
        ai: AiReview::Clean {
            evidence_hash: [4; 32],
            signature_kind: 1,
        },
        verdict: Verdict::Pass,
        issued_at_s: 1,
        expires_at_s: 1801,
    };
    (g, c)
}
fn signed(c: &Certificate) -> (Base64Bytes, Base64<64>) {
    let bytes = c.to_canonical_bytes().unwrap();
    let signature = key().sign(&env::sha256_array(&bytes)).to_bytes();
    (Base64Bytes(bytes), Base64(signature))
}
fn submit(g: &mut Gate, c: &Certificate) {
    let (bytes, sig) = signed(c);
    drop(g.submit_verdict(bytes, sig));
}
fn callback(g: &mut Gate, id: u64, attempt: u32, result: PromiseResult) {
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("gate.testnet".parse().unwrap())
            .block_timestamp(1_000_000_000)
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![result]
    );
    g.on_signed(SignTarget::Proposal(id), attempt);
}
fn captured() -> PromiseResult {
    PromiseResult::Successful(include_bytes!("../../../vectors/mpc-response.json").to_vec())
}

macro_rules! binding_test {
    ($name:ident,$field:ident,$value:expr) => {
        #[test]
        fn $name() {
            let (mut g, mut c) = setup();
            c.$field = $value;
            let message = fails("E45_BINDING_MISMATCH", || submit(&mut g, &c));
            assert!(message.ends_with(stringify!($field)));
            assert_eq!(g.next_evm_nonce, 42);
            assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::Pending);
        }
    };
}
binding_test!(t_gate_2_chain_id, chain_id, 1);
binding_test!(t_gate_2_proxy, proxy, [99; 20]);
binding_test!(t_gate_2_executor, executor, [99; 20]);
binding_test!(t_gate_2_new_impl, new_impl, [99; 20]);
binding_test!(t_gate_2_codehash, new_impl_codehash, [99; 32]);
binding_test!(t_gate_2_source, source_commitment, [99; 32]);
binding_test!(t_gate_2_current, current_impl, [99; 20]);
binding_test!(t_gate_2_calldata, init_calldata_hash, [99; 32]);
binding_test!(t_gate_2_rule_set, rule_set_hash, [99; 32]);
#[test]
fn t_gate_2_active_constitution_must_match_proposal() {
    let (mut g, c) = setup();
    g.constitution.rule_set_hash = [99; 32];
    assert!(fails("E45_BINDING_MISMATCH", || submit(&mut g, &c)).ends_with("rule_set_hash"));
}
#[test]
fn t_gate_3_check_order_encoding_domain_verifier_and_signature() {
    let (mut g, mut c) = setup();
    fails("E40_BAD_ENCODING", || {
        drop(g.submit_verdict(Base64Bytes(vec![0]), Base64([0; 64])));
    });
    let (mut bytes, sig) = signed(&c);
    bytes.0.push(0);
    fails("E40_BAD_ENCODING", || {
        drop(g.submit_verdict(bytes, sig));
    });
    c.domain = [0; 16];
    fails("E41_WRONG_DOMAIN", || submit(&mut g, &c));
    c.domain = DOMAIN;
    c.gate = "other.testnet".parse().unwrap();
    fails("E41_WRONG_DOMAIN", || submit(&mut g, &c));
    c.gate = "gate.testnet".parse().unwrap();
    c.verifier_pubkey = [99; 32];
    fails("E42_UNKNOWN_VERIFIER", || submit(&mut g, &c));
    c.verifier_pubkey = key().verifying_key().to_bytes();
    c.verifier_release_hash = [99; 32];
    fails("E42_UNKNOWN_VERIFIER", || submit(&mut g, &c));
    c.verifier_release_hash = [6; 32];
    let (bytes, _) = signed(&c);
    fails("E43_BAD_SIGNATURE", || {
        drop(g.submit_verdict(bytes, Base64([0; 64])));
    });
    c.proposal_id = 99;
    fails("E30_NOT_FOUND", || submit(&mut g, &c));
    c.proposal_id = 0;
    g.proposals.get_mut(&0).unwrap().state = ProposalState::Expired;
    fails("E31_WRONG_STATE", || submit(&mut g, &c));
}
#[test]
fn t_gate_3_time_rejections_and_inclusive_boundaries() {
    let (mut g, mut c) = setup();
    c.issued_at_s = 62;
    fails("E46_CERT_TIME", || submit(&mut g, &c));
    c.issued_at_s = 1;
    c.expires_at_s = 0;
    fails("E46_CERT_TIME", || submit(&mut g, &c));
    c.expires_at_s = 1802;
    fails("E46_CERT_TIME", || submit(&mut g, &c));
    c.issued_at_s = 2;
    c.expires_at_s = 1;
    fails("E46_CERT_TIME", || submit(&mut g, &c));
    c.issued_at_s = 61;
    c.expires_at_s = 1861;
    submit(&mut g, &c);
    assert_eq!(
        g.get_proposal(0).unwrap().state,
        ProposalState::SignRequested
    );
}
#[test]
fn certificate_at_expiry_is_accepted_but_expired_proposal_is_not() {
    let (mut g, mut c) = setup();
    c.issued_at_s = 1;
    c.expires_at_s = 1;
    submit(&mut g, &c);
    let (mut g, c) = setup();
    context("relay.testnet", 7_201_000_000_001, 0);
    fails("E44_PROPOSAL_EXPIRED", || submit(&mut g, &c));
}
#[test]
fn t_gate_4_ai_and_pause_cannot_authorize_pass() {
    let (mut g, mut c) = setup();
    c.ai = AiReview::NotRun;
    fails("E47_AI_REVIEW_REQUIRED", || submit(&mut g, &c));
    c.ai = AiReview::Vetoed {
        evidence_hash: [1; 32],
        counterexample_hash: [2; 32],
    };
    fails("E47_AI_REVIEW_REQUIRED", || submit(&mut g, &c));
    c.ai = AiReview::Clean {
        evidence_hash: [1; 32],
        signature_kind: 1,
    };
    g.signing_paused = true;
    fails("E20_SIGNING_PAUSED", || submit(&mut g, &c));
}
#[test]
fn vetoed_review_never_authorizes_pass_even_when_ai_not_required() {
    let (mut g, mut c) = setup();
    g.constitution.ai_required = false;
    c.ai = AiReview::Vetoed {
        evidence_hash: [1; 32],
        counterexample_hash: [2; 32],
    };
    fails("E47_AI_REVIEW_REQUIRED", || submit(&mut g, &c));
}
#[test]
fn t_gate_5_rejection_while_paused_releases_without_mpc_or_nonce() {
    let (mut g, mut c) = setup();
    g.signing_paused = true;
    c.ai = AiReview::NotRun;
    c.verdict = Verdict::Reject {
        rule_id: 2,
        reason_code: 22,
    };
    let (bytes, sig) = signed(&c);
    context("relay.testnet", 1_000_000_000, 0);
    assert!(matches!(
        g.submit_verdict(bytes, sig),
        PromiseOrValue::Value(_)
    ));
    assert!(get_created_receipts().is_empty());
    assert_eq!(g.next_evm_nonce, 42);
    let p = g.get_proposal(0).unwrap();
    assert_eq!(p.state, ProposalState::Rejected);
    assert_eq!(p.reserved, 0);
    assert_eq!(p.rejection.as_ref().unwrap().reason_code, 22);
    assert_eq!(p.rejection.as_ref().unwrap().evidence_root, [3; 32]);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 100);
    assert_eq!(g.get_active("alice.testnet".parse().unwrap()), None);
    assert!(!g.active_by_key.contains_key(&crate::credit::active_key(
        &"alice.testnet".parse().unwrap(),
        &[9; 32],
        &[]
    )));
    assert_eq!(last_event("verdict_rejected")["proposal_id"], 0);
}
#[test]
fn pass_mpc_request_and_callback_bind_stored_preimage() {
    let (mut g, c) = setup();
    context("relay.testnet", 1_000_000_000, 0);
    submit(&mut g, &c);
    let p = g.get_proposal(0).unwrap();
    let e = p.evm.as_ref().unwrap();
    assert_eq!(e.nonce, 42);
    assert_eq!(e.attempts, 1);
    assert_eq!(e.deadline_s, 1801);
    assert_eq!(
        e.signing_hash,
        env::keccak256_array(&g.signing_preimage(0).0)
    );
    assert_eq!(g.next_evm_nonce, 43);
    let receipts = get_created_receipts();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[0].receiver_id.as_str(), "v1.signer-prod.testnet");
    match &receipts[0].actions[0] {
        near_sdk::mock::MockAction::FunctionCallWeight {
            method_name,
            args,
            attached_deposit,
            prepaid_gas,
            ..
        } => {
            assert_eq!(method_name, b"sign");
            assert_eq!(attached_deposit.as_yoctonear(), 1);
            assert_eq!(prepaid_gas.as_tgas(), 15);
            let value: serde_json::Value = serde_json::from_slice(args).unwrap();
            assert_eq!(
                value,
                json!({"request":{"payload_v2":{"Ecdsa":hex::encode(e.signing_hash)},"path":"constitution/base-sepolia/v1","domain_id":0}})
            );
        }
        other => panic!("{other:?}"),
    }
    match &receipts[1].actions[0] {
        near_sdk::mock::MockAction::FunctionCallWeight {
            method_name,
            args,
            prepaid_gas,
            ..
        } => {
            assert_eq!(method_name, b"on_signed");
            assert_eq!(prepaid_gas.as_tgas(), 20);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(args).unwrap(),
                json!({"target":{"Proposal":0},"attempt":1})
            );
        }
        other => panic!("{other:?}"),
    }
}
#[test]
fn t_gate_10_callback_failure_retry_is_identical_and_stale_attempt_ignored() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    let original = g.get_proposal(0).unwrap().evm.clone().unwrap();
    callback(&mut g, 0, 1, PromiseResult::Failed);
    assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::SignFailed);
    context("relay.testnet", 1_000_000_000, 0);
    drop(g.retry_sign(0));
    let retry = g.get_proposal(0).unwrap().evm.as_ref().unwrap();
    assert_eq!(retry.signing_hash, original.signing_hash);
    assert_eq!(retry.nonce, original.nonce);
    assert_eq!(retry.attempts, 2);
    callback(&mut g, 0, 1, captured());
    assert_eq!(
        g.get_proposal(0).unwrap().state,
        ProposalState::SignRequested
    );
    callback(&mut g, 0, 2, captured());
    assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::Signed);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 100);
    assert_eq!(g.next_evm_nonce, 43);
}
#[test]
fn malformed_mpc_results_record_failure_without_panicking() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    for bytes in [
        b"not json".to_vec(),
        br#"{}"#.to_vec(),
        br#"{"scheme":"Ed25519"}"#.to_vec(),
        vec![0; 4097],
    ] {
        let attempt = g.get_proposal(0).unwrap().evm.as_ref().unwrap().attempts;
        callback(&mut g, 0, attempt, PromiseResult::Successful(bytes));
        assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::SignFailed);
        context("relay.testnet", 1_000_000_000, 0);
        drop(g.retry_sign(0));
    }
}
#[test]
fn t_gate_9_fee_bumps_round_up_keep_nonce_and_stop_at_eight() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    callback(&mut g, 0, 1, captured());
    let original = g.get_proposal(0).unwrap().evm.clone().unwrap();
    g.constitution.params.fee_cap = u128::MAX;
    for bump in 1..=8 {
        context(
            "relay.testnet",
            1_000_000_000 + u64::from(bump) * (MIN_BUMP_AGE + 1),
            0,
        );
        drop(g.bump_fee(SignTarget::Proposal(0)));
        let e = g.get_proposal(0).unwrap().evm.as_ref().unwrap();
        assert_eq!(e.nonce, original.nonce);
        assert_eq!(e.deadline_s, original.deadline_s);
        assert_eq!(g.fee_bumps.get(&0), Some(&bump));
        if bump == 1 {
            assert_eq!(e.max_fee_per_gas, 13);
            assert_eq!(e.max_priority_fee_per_gas, 2);
        }
        let attempt = e.attempts;
        callback(&mut g, 0, attempt, captured());
    }
    context("relay.testnet", 1_000_000_000 + 9 * (MIN_BUMP_AGE + 1), 0);
    fails("E49_TOO_MANY_BUMPS", || {
        drop(g.bump_fee(SignTarget::Proposal(0)));
    });
}
#[test]
fn fee_cap_and_superseded_rules_fail_before_state_mutation() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    callback(&mut g, 0, 1, captured());
    context("relay.testnet", 1_000_000_000 + MIN_BUMP_AGE, 0);
    g.constitution.params.fee_cap = 12;
    fails("E48_FEE_CAP", || {
        drop(g.bump_fee(SignTarget::Proposal(0)));
    });
    g.constitution.params.fee_cap = 100;
    g.constitution.rule_set_hash = [99; 32];
    fails("E45_BINDING_MISMATCH", || {
        drop(g.bump_fee(SignTarget::Proposal(0)));
    });
    context("relay.testnet", 1_801_000_000_000, 0);
    drop(g.bump_fee(SignTarget::Proposal(0)));
}
#[test]
fn failed_bump_preserves_prior_raw_and_never_releases_a_new_active_proposal() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    callback(&mut g, 0, 1, captured());
    let raw = g
        .get_proposal(0)
        .unwrap()
        .evm
        .as_ref()
        .unwrap()
        .signed_tx
        .clone()
        .unwrap();
    context("alice.testnet", 1_000_000_000, 0);
    let mut a = args();
    a.new_impl_codehash = Hex32([88; 32]);
    let next = propose_fee(&mut g, a);
    context("relay.testnet", 1_000_000_000 + MIN_BUMP_AGE, 0);
    drop(g.bump_fee(SignTarget::Proposal(0)));
    callback(&mut g, 0, 2, PromiseResult::Failed);
    assert_eq!(
        g.get_proposal(0)
            .unwrap()
            .evm
            .as_ref()
            .unwrap()
            .signed_tx
            .as_ref(),
        Some(&raw)
    );
    drop(g.retry_sign(0));
    callback(&mut g, 0, 3, captured());
    assert_eq!(g.get_active("alice.testnet".parse().unwrap()), Some(next));
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 0);
}
#[test]
fn retry_and_bump_require_their_states_and_respect_pause() {
    let (mut g, c) = setup();
    fails("E30_NOT_FOUND", || {
        drop(g.retry_sign(99));
    });
    fails("E31_WRONG_STATE", || {
        drop(g.retry_sign(0));
    });
    fails("E31_WRONG_STATE", || {
        drop(g.bump_fee(SignTarget::Proposal(0)));
    });
    submit(&mut g, &c);
    callback(&mut g, 0, 1, PromiseResult::Failed);
    g.signing_paused = true;
    fails("E20_SIGNING_PAUSED", || {
        drop(g.retry_sign(0));
    });
    g.signing_paused = false;
    drop(g.retry_sign(0));
    callback(&mut g, 0, 2, captured());
    g.signing_paused = true;
    fails("E20_SIGNING_PAUSED", || {
        drop(g.bump_fee(SignTarget::Proposal(0)));
    });
}
#[test]
fn sign_failed_blocks_rule_change_until_its_deadline_then_only_nonce_filler_is_allowed() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    callback(&mut g, 0, 1, PromiseResult::Failed);
    let id = schedule(&mut g, rule_change());
    g.changes.get_mut(&id).unwrap().eta_ns = 1_000_000_000;
    fails("E55_SIGNATURES_OUTSTANDING", || {
        drop(g.apply_change(id));
    });
    context("relay.testnet", 1_801_000_000_000, 0);
    drop(g.apply_change(id));
    drop(g.retry_sign(0));
    assert_eq!(g.get_proposal(0).unwrap().evm.as_ref().unwrap().nonce, 42);
}

fn fixed_spike_proposal(g: &mut Gate) {
    g.config.executor = [0x11; 20];
    let p = g.proposals.get_mut(&0).unwrap();
    p.certificate_hash = Some([0x22; 32]);
    p.new_impl = [0x33; 20];
    p.new_impl_codehash = [0x44; 32];
    p.expected_current_impl = [0x55; 20];
    p.init_calldata = vec![0xde, 0xad, 0xbe, 0xef];
    p.state = ProposalState::SignRequested;
    p.evm = Some(EvmTx {
        nonce: 7,
        deadline_s: 1_800_000_000,
        gas_limit: 300_000,
        max_fee_per_gas: 2_000_000_000,
        max_priority_fee_per_gas: 1_000_000,
        signing_hash: [0; 32],
        signed_tx: None,
        attempts: 1,
    });
}

// Independent ABI and RLP oracle used by the spike's fixed-input test.
fn independent_spike_preimage() -> Vec<u8> {
    let word = |n: u128| {
        let mut out = vec![0; 16];
        out.extend(n.to_be_bytes());
        out
    };
    let mut call = vec![0x90, 0xc7, 0x0c, 0xd8];
    call.extend([0x22; 32]);
    call.extend([0; 12]);
    call.extend([0x33; 20]);
    call.extend([0x44; 32]);
    call.extend([0; 12]);
    call.extend([0x55; 20]);
    call.extend(word(1_800_000_000));
    call.extend(word(192));
    call.extend(word(4));
    call.extend([0xde, 0xad, 0xbe, 0xef]);
    call.extend([0; 28]);
    let mut stream = rlp::RlpStream::new_list(9);
    stream
        .append(&84532u64)
        .append(&7u64)
        .append(&1_000_000u64)
        .append(&2_000_000_000u64)
        .append(&300_000u64)
        .append(&[0x11; 20].as_slice())
        .append(&0u64)
        .append(&call)
        .begin_list(0);
    let mut preimage = vec![2];
    preimage.extend(stream.out());
    preimage
}

#[test]
fn signing_preimage_matches_spike_independent_oracle_and_captured_hash() {
    let (mut g, _) = setup();
    fixed_spike_proposal(&mut g);
    let actual = g.signing_preimage(0).0;
    assert_eq!(actual, independent_spike_preimage());
    assert_eq!(
        hex::encode(env::keccak256_array(&actual)),
        "f4cabf1bbeafae5b1b847737b03d31ac62d5805cb3204b04faefe5e22bb750c3"
    );
}

#[test]
fn captured_signature_raw_transaction_recovers_to_vector_derived_eoa() {
    let (mut g, _) = setup();
    fixed_spike_proposal(&mut g);
    callback(&mut g, 0, 1, captured());
    let raw = g
        .get_proposal(0)
        .unwrap()
        .evm
        .as_ref()
        .unwrap()
        .signed_tx
        .as_ref()
        .unwrap();
    assert_eq!(raw[0], 2);
    let fields = rlp::Rlp::new(&raw[1..]);
    assert_eq!(fields.item_count().unwrap(), 12);
    let mut stream = rlp::RlpStream::new_list(9);
    for i in 0..9 {
        stream.append_raw(fields.at(i).unwrap().as_raw(), 1);
    }
    let mut unsigned = vec![2];
    unsigned.extend(stream.out());
    assert_eq!(unsigned, independent_spike_preimage());
    let scalar = |index| {
        let bytes: Vec<u8> = fields.val_at(index).unwrap();
        let mut out = [0; 32];
        out[32 - bytes.len()..].copy_from_slice(&bytes);
        out
    };
    let signature = k256::ecdsa::Signature::from_scalars(scalar(10), scalar(11)).unwrap();
    let parity = fields.val_at::<u8>(9).unwrap();
    let recovery = k256::ecdsa::RecoveryId::from_byte(parity).unwrap();
    let key = k256::ecdsa::VerifyingKey::recover_from_prehash(
        &env::keccak256_array(&unsigned),
        &signature,
        recovery,
    )
    .unwrap();
    let point = key.to_encoded_point(false);
    let address = format!(
        "0x{}",
        hex::encode(&env::keccak256_array(&point.as_bytes()[1..])[12..])
    );
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("../../../vectors/mpc-derivation.json")).unwrap();
    assert_eq!(address, vectors["vectors"][2]["evm_address"]);
}

#[test]
fn migration_signs_rotate_signer_and_retries_failed_request_without_new_nonce() {
    let mut g = gate();
    let id = schedule(
        &mut g,
        ChangeKind::MigrateAuthority {
            new_signer: [0x77; 20],
        },
    );
    context("anyone.testnet", 604_801_000_000_000, 0);
    drop(g.apply_change(id));
    assert_eq!(g.get_change(id).unwrap().status, ChangeStatus::Applied);
    let before = g.get_change(id).unwrap().evm.clone().unwrap();
    assert_eq!(before.nonce, 42);
    assert_eq!(before.deadline_s, 608401);
    let tx = g.target_transaction(&SignTarget::Change(id)).unwrap();
    let preimage = tx.build_for_signing();
    let fields = rlp::Rlp::new(&preimage[1..]);
    let calldata: Vec<u8> = fields.val_at(7).unwrap();
    assert_eq!(&calldata[..4], &[0xfa, 0x2e, 0x9b, 0x8f]);
    assert_eq!(&calldata[16..36], &[0x77; 20]);
    fails("E54_NOT_PENDING", || g.apply_change(id));
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("gate.testnet".parse().unwrap())
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![PromiseResult::Failed]
    );
    g.on_signed(SignTarget::Change(id), 1);
    drop(g.apply_change(id));
    let retried = g.get_change(id).unwrap().evm.as_ref().unwrap();
    assert_eq!(retried.nonce, before.nonce);
    assert_eq!(retried.signing_hash, before.signing_hash);
    assert_eq!(retried.attempts, 2);
    assert_eq!(g.next_evm_nonce, 43);
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("gate.testnet".parse().unwrap())
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![captured()]
    );
    g.on_signed(SignTarget::Change(id), 2);
    assert!(
        g.get_change(id)
            .unwrap()
            .evm
            .as_ref()
            .unwrap()
            .signed_tx
            .is_some()
    );
    fails("E54_NOT_PENDING", || g.apply_change(id));
}

#[test]
fn lost_callback_can_be_retried_at_timeout_and_late_original_is_ignored() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    let original = g.get_proposal(0).unwrap().evm.clone().unwrap();
    context("relay.testnet", 1_200_999_999_999, 0);
    fails("E31_WRONG_STATE", || drop(g.retry_sign(0)));
    context("relay.testnet", 1_201_000_000_000, 0);
    drop(g.retry_sign(0));
    let retried = g.get_proposal(0).unwrap().evm.as_ref().unwrap();
    assert_eq!(retried.signing_hash, original.signing_hash);
    assert_eq!(retried.nonce, original.nonce);
    assert_eq!(retried.attempts, 2);
    callback(&mut g, 0, 1, captured());
    assert_eq!(
        g.get_proposal(0).unwrap().state,
        ProposalState::SignRequested
    );
    callback(&mut g, 0, 2, captured());
    callback(&mut g, 0, 1, captured());
    assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::Signed);
    assert_eq!(g.get_credit("alice.testnet".parse().unwrap()).0, 100);
}

#[test]
fn expired_lost_callback_does_not_block_rule_change_forever() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    let id = schedule(&mut g, rule_change());
    context("relay.testnet", 604_801_000_000_000, 0);
    drop(g.apply_change(id));
    assert_eq!(g.get_constitution().rule_set_hash, [12; 32]);
    drop(g.retry_sign(0));
    assert_eq!(g.get_proposal(0).unwrap().evm.as_ref().unwrap().nonce, 42);
}

#[test]
fn clean_review_requires_provider_tee_even_when_ai_optional() {
    let (mut g, mut c) = setup();
    for required in [true, false] {
        g.constitution.ai_required = required;
        for kind in [0, 2, 255] {
            c.ai = AiReview::Clean {
                evidence_hash: [4; 32],
                signature_kind: kind,
            };
            fails("E58_AI_SIGNATURE_KIND", || submit(&mut g, &c));
            assert_eq!(g.next_evm_nonce, 42);
        }
    }
}

#[test]
fn migration_fee_bump_preserves_nonce_deadline_and_calldata_and_caps() {
    let mut g = gate();
    let id = schedule(
        &mut g,
        ChangeKind::MigrateAuthority {
            new_signer: [0x77; 20],
        },
    );
    context("relay.testnet", 604_801_000_000_000, 0);
    drop(g.apply_change(id));
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("gate.testnet".parse().unwrap())
            .block_timestamp(604_801_000_000_000 + MIN_BUMP_AGE)
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![captured()]
    );
    g.on_signed(SignTarget::Change(id), 1);
    let before = g.get_change(id).unwrap().evm.clone().unwrap();
    let old_tx = g
        .target_transaction(&SignTarget::Change(id))
        .unwrap()
        .build_for_signing();
    g.constitution.params.fee_cap = 12;
    fails("E48_FEE_CAP", || drop(g.bump_fee(SignTarget::Change(id))));
    g.constitution.params.fee_cap = 1000;
    drop(g.bump_fee(SignTarget::Change(id)));
    let after = g.get_change(id).unwrap().evm.as_ref().unwrap();
    assert_eq!(after.nonce, before.nonce);
    assert_eq!(after.deadline_s, before.deadline_s);
    assert_eq!(after.max_fee_per_gas, 13);
    assert_eq!(after.max_priority_fee_per_gas, 2);
    assert_eq!(after.signed_tx, before.signed_tx);
    let new_tx = g
        .target_transaction(&SignTarget::Change(id))
        .unwrap()
        .build_for_signing();
    assert_eq!(
        rlp::Rlp::new(&old_tx[1..]).at(7).unwrap().as_raw(),
        rlp::Rlp::new(&new_tx[1..]).at(7).unwrap().as_raw()
    );
    assert_eq!(g.next_evm_nonce, 43);
}

#[test]
fn signed_max_calldata_storage_measurement() {
    let (template, mut c) = setup();
    drop(template);
    let mut g = gate();
    g.verifiers.clear();
    g.verifiers
        .insert(key().verifying_key().to_bytes(), verifier());
    fund(&mut g, "alice.testnet", 100);
    g.flush_storage();
    let before = env::storage_usage();
    let mut a = args();
    a.init_calldata.0 = vec![0x55; 1024];
    c.init_calldata_hash = env::keccak256_array(&a.init_calldata.0);
    propose_fee(&mut g, a);
    submit(&mut g, &c);
    let usage = env::storage_usage();
    testing_env!(
        VMContextBuilder::new()
            .current_account_id("gate.testnet".parse().unwrap())
            .predecessor_account_id("gate.testnet".parse().unwrap())
            .storage_usage(usage)
            .block_timestamp(1_000_000_000)
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![captured()]
    );
    g.on_signed(SignTarget::Proposal(0), 1);
    g.flush_storage();
    assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::Signed);
    println!(
        "signed_max_calldata_proposal_storage_bytes={}",
        env::storage_usage() - before
    );
    assert_eq!(g.total_liabilities, 100);
}

#[test]
fn late_failed_callback_after_rule_cleanup_does_not_allocate_outstanding_entry() {
    let (mut g, c) = setup();
    submit(&mut g, &c);
    let id = schedule(&mut g, rule_change());
    context("relay.testnet", 604_801_000_000_000, 0);
    drop(g.apply_change(id));
    assert!(g.outstanding_signatures.is_empty());
    callback(&mut g, 0, 1, PromiseResult::Failed);
    assert_eq!(g.get_proposal(0).unwrap().state, ProposalState::SignFailed);
    assert!(g.outstanding_signatures.is_empty());
}

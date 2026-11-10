use constitution_gate::Gate;
use constitution_types::*;
use ed25519_dalek::{Signer, SigningKey};
use near_sdk::{
    NearToken, PromiseResult, env,
    test_utils::{VMContextBuilder, get_created_receipts},
    testing_env,
};
use proptest::test_runner::{Config as ProptestConfig, RngSeed};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub const START: u64 = 1_000_000_000_000;
pub const SEED: u64 = 0x434f_4e53_5449_5455;
pub const CASES: u32 = 64;
pub const TEST_BALANCE: u128 = 1_000_000 * 10u128.pow(24);

pub fn property_config() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        rng_seed: RngSeed::Fixed(SEED),
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

pub fn account(index: u8) -> AccountId {
    format!("user{index}.testnet").parse().unwrap()
}

// RuntimeConfigStore::test() reparses the protocol history. Reuse its immutable
// configuration, while installing fresh gas/log/receipt state for every operation.
type ContextInstaller = Box<dyn Fn(near_sdk::VMContext, Vec<PromiseResult>)>;
thread_local! {
    static INSTALL_CONTEXT: ContextInstaller = {
        let config = near_sdk::test_vm_config();
        let fees = near_sdk::RuntimeFeesConfig::test();
        Box::new(move |ctx, results| {
            testing_env!(ctx, config.clone(), fees.clone(), Default::default(), results);
        })
    };
}

pub fn context(who: AccountId, now: u64, deposit: u128, result: Option<PromiseResult>) {
    context_with_resources(
        who,
        now,
        deposit,
        result,
        TEST_BALANCE,
        env::storage_usage(),
    );
}

pub fn context_with_balance(who: AccountId, now: u64, deposit: u128, balance: u128) {
    context_with_resources(who, now, deposit, None, balance, env::storage_usage());
}

fn context_with_resources(
    who: AccountId,
    now: u64,
    deposit: u128,
    result: Option<PromiseResult>,
    balance: u128,
    storage_usage: u64,
) {
    let ctx = VMContextBuilder::new()
        .current_account_id("gate.testnet".parse().unwrap())
        .predecessor_account_id(who.clone())
        .signer_account_id(who)
        .block_timestamp(now)
        .attached_deposit(NearToken::from_yoctonear(deposit))
        .account_balance(NearToken::from_yoctonear(balance))
        .storage_usage(storage_usage)
        .build();
    INSTALL_CONTEXT.with(|install| install(ctx, result.into_iter().collect()));
}

pub fn deposit(gate: &mut Gate, who: AccountId, now: u64, amount: u128) {
    let fee = gate.get_credit_entry_fee(who.clone()).0;
    context(who, now, amount + fee, None);
    gate.deposit_credit(None);
}

pub fn proposal_context(gate: &mut Gate, who: AccountId, now: u64) {
    // These fixture accounts already paid their entry fee; prepay this proposal's storage.
    context(who.clone(), now, gate.get_proposal_fee().0, None);
    gate.deposit_credit(None);
    context(who, now, 0, None);
}

pub fn key(index: u8) -> SigningKey {
    SigningKey::from_bytes(&[42 + index; 32])
}

pub fn params() -> Params {
    Params {
        proposal_deposit: 100,
        proposal_ttl_s: 300,
        max_cert_ttl_s: 60,
        sign_validity_s: 60,
        gas_limit: 300_000,
        max_fee_per_gas: 10,
        max_priority_fee_per_gas: 1,
        fee_cap: 1_000,
        sign_gas_tgas: 15,
        callback_gas_tgas: 20,
    }
}

pub fn fresh_gate(initial_nonce: u64) -> Gate {
    // testing_env! normally preserves storage. Property cases must not share it.
    near_sdk::mock::with_mocked_blockchain(|blockchain| {
        drop(blockchain.take_storage());
    });
    context_with_resources(account(0), START, 0, None, TEST_BALANCE, 0);
    Gate::new(
        Config {
            chain_id: 84532,
            executor: [1; 20],
            proxy: [2; 20],
            proxy_admin: [3; 20],
            derived_eoa: [4; 20],
            mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
            mpc_path: "constitution/base-sepolia/v1".into(),
            mpc_domain_id: 0,
            governance_delay_days: DEFAULT_GOVERNANCE_DELAY_DAYS,
        },
        Constitution {
            rule_set_hash: [5; 32],
            manifest_uri: "ipfs://rules".into(),
            ai_required: true,
            params: params(),
        },
        Roles {
            governor: "governor.testnet".parse().unwrap(),
            guardian: "guardian.testnet".parse().unwrap(),
        },
        vec![(
            Base64(key(0).verifying_key().to_bytes()),
            VerifierRecord {
                release_hash: [6; 32],
                attestation_uri: "ipfs://quote".into(),
                added_at_ns: START,
            },
        )],
        initial_nonce,
    )
}

pub fn args(token: u8) -> ProposeArgs {
    ProposeArgs {
        new_impl: Hex20([8 + token; 20]),
        new_impl_codehash: Hex32([9 + token; 32]),
        expected_current_impl: Hex20([10; 20]),
        init_calldata: HexBytes(vec![token; usize::from(token)]),
        source_commitment: Hex32([11 + token; 32]),
    }
}

pub fn certificate(gate: &Gate, id: u64, now: u64, verifier: u8) -> Certificate {
    let p = gate.get_proposal(id).unwrap();
    Certificate {
        domain: DOMAIN,
        gate: "gate.testnet".parse().unwrap(),
        proposal_id: id,
        attempt: 0,
        chain_id: 84532,
        proxy: [2; 20],
        executor: [1; 20],
        current_impl: p.expected_current_impl,
        current_impl_codehash: [1; 32],
        new_impl: p.new_impl,
        new_impl_codehash: p.new_impl_codehash,
        init_calldata_hash: env::keccak256_array(&p.init_calldata),
        source_commitment: p.source_commitment,
        rule_set_hash: p.rule_set_hash,
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

pub fn submit(gate: &mut Gate, cert: &Certificate, corrupt_signature: bool) {
    let bytes = cert.to_canonical_bytes().unwrap();
    let mut signature = key(
        if cert.verifier_pubkey == key(0).verifying_key().to_bytes() {
            0
        } else {
            1
        },
    )
    .sign(&env::sha256_array(&bytes))
    .to_bytes();
    if corrupt_signature {
        signature[0] ^= 1;
    }
    drop(gate.submit_verdict(Base64Bytes(bytes), Base64(signature)));
}

pub fn expect_code<T>(code: &str, call: impl FnOnce() -> T) {
    let panic = match catch_unwind(AssertUnwindSafe(call)) {
        Ok(_) => panic!("expected {code}, call succeeded"),
        Err(panic) => panic,
    };
    let message = panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .expect("string panic");
    let message = if let Some((_, wrapped)) = message.split_once("panic_msg: ") {
        serde_json::from_str::<String>(wrapped.strip_suffix(" })").unwrap()).unwrap()
    } else {
        message
    };
    assert_eq!(message.split(':').next().unwrap(), code, "{message}");
}

pub fn callback(gate: &mut Gate, id: u64, attempt: u32, now: u64, outcome: u8) {
    target_callback(gate, SignTarget::Proposal(id), attempt, now, outcome);
}

pub fn target_callback(gate: &mut Gate, target: SignTarget, attempt: u32, now: u64, outcome: u8) {
    let result = match outcome {
        0 => PromiseResult::Failed,
        1 => PromiseResult::Successful(
            include_bytes!("../../../../vectors/mpc-response.json").to_vec(),
        ),
        _ => PromiseResult::Successful(b"{\"wrong\":true}".to_vec()),
    };
    context("gate.testnet".parse().unwrap(), now, 0, Some(result));
    gate.on_signed(target, attempt);
}

pub fn sign_requests() -> Vec<serde_json::Value> {
    get_created_receipts()
        .into_iter()
        .filter(|receipt| receipt.receiver_id.as_str() == "v1.signer-prod.testnet")
        .flat_map(|receipt| receipt.actions)
        .filter_map(|action| match action {
            near_sdk::mock::MockAction::FunctionCallWeight {
                method_name,
                args,
                attached_deposit,
                ..
            } => {
                assert_eq!(method_name, b"sign");
                assert_eq!(attached_deposit.as_yoctonear(), 1);
                Some(serde_json::from_slice(&args).unwrap())
            }
            _ => None,
        })
        .collect()
}

pub fn roundtrip(gate: Gate) -> Gate {
    let bytes = borsh::to_vec(&gate).unwrap();
    env::state_write(&gate);
    // Drop flushes lazy collections before the deserialized handles read storage.
    drop(gate);
    let restored: Gate = borsh::from_slice(&bytes).unwrap();
    assert_eq!(borsh::to_vec(&restored).unwrap(), bytes);
    restored
}

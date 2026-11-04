//! Local sandbox with an explicitly labeled mock MPC. Captured signatures are replayed,
//! not signatures for these generated certificates, and are never broadcast to EVM.
use constitution_types::*;
use ed25519_dalek::{Signer, SigningKey};
use near_workspaces::{
    Account, Contract, Worker,
    network::Sandbox,
    types::{AccountDetailsPatch, NearToken},
};
use serde_json::json;
use std::path::PathBuf;
type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;
fn artifact(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(path)
}
fn key() -> SigningKey {
    SigningKey::from_bytes(&[42; 32])
}
struct Harness {
    worker: Worker<Sandbox>,
    gate: Contract,
    actor: Account,
    /// Accounts that already hold a credit entry (the first deposit pays the entry fee).
    funded: std::sync::Mutex<std::collections::HashSet<String>>,
}
impl Harness {
    async fn new(fee_cap: u128) -> Result<Self> {
        let worker = near_workspaces::sandbox().await?;
        let gate = worker
            .dev_deploy(&std::fs::read(artifact(
                "near/constitution_gate/constitution_gate.wasm",
            ))?)
            .await?;
        let actor = worker.dev_create_account().await?;
        let mpc: AccountId = "v1.signer-prod.testnet".parse()?;
        worker
            .patch(&mpc)
            .account(AccountDetailsPatch::default().balance(NearToken::from_near(100)))
            .code(&std::fs::read(artifact(
                "mock-mpc/wasm32-unknown-unknown/release/mock_mpc.wasm",
            ))?)
            .transact()
            .await?;
        actor
            .call(&mpc, "new")
            .args_json(json!({}))
            .transact()
            .await?
            .into_result()?;
        gate.call("new").args_json(json!({
            "config":Config{chain_id:84532,executor:[1;20],proxy:[2;20],proxy_admin:[3;20],derived_eoa:[4;20],mpc_account:mpc,mpc_path:"constitution/base-sepolia/v1".into(),mpc_domain_id:0},
            "constitution":Constitution{rule_set_hash:[5;32],manifest_uri:"ipfs://rules".into(),ai_required:true,
                params:Params{proposal_deposit:100,proposal_ttl_s:7200,max_cert_ttl_s:1800,sign_validity_s:3600,gas_limit:500000,max_fee_per_gas:10,max_priority_fee_per_gas:1,fee_cap,sign_gas_tgas:15,callback_gas_tgas:30}},
            "roles":Roles{governor:actor.id().clone(),guardian:actor.id().clone()},
            "verifiers":[(Base64(key().verifying_key().to_bytes()),VerifierRecord{release_hash:[6;32],attestation_uri:"ipfs://quote".into(),added_at_ns:0})],"initial_evm_nonce":42
        })).transact().await?.into_result()?;
        Ok(Self {
            worker,
            gate,
            actor,
            funded: Default::default(),
        })
    }
    async fn mode(&self, mode: u8) -> Result<()> {
        self.actor
            .call(&"v1.signer-prod.testnet".parse()?, "set_mode")
            .args_json(json!({"mode":mode}))
            .transact()
            .await?
            .into_result()?;
        Ok(())
    }
    async fn propose(&self, who: &Account, tag: u8) -> Result<Certificate> {
        let first = self.funded.lock().unwrap().insert(who.id().to_string());
        let entry_fee: u128 = if first {
            self.gate
                .view("get_credit_entry_fee")
                .args_json(json!({"account":who.id()}))
                .await?
                .json::<String>()?
                .parse()?
        } else {
            0
        };
        let proposal_fee: u128 = self
            .gate
            .view("get_proposal_fee")
            .await?
            .json::<String>()?
            .parse()?;
        who.call(self.gate.id(), "deposit_credit")
            .args_json(json!({"account":null}))
            .deposit(NearToken::from_yoctonear(100 + entry_fee))
            .transact()
            .await?
            .into_result()?;
        let id:u64=who.call(self.gate.id(),"propose").args_json(json!({"args":ProposeArgs{new_impl:Hex20([8;20]),new_impl_codehash:Hex32([tag;32]),expected_current_impl:Hex20([10;20]),init_calldata:HexBytes(vec![]),source_commitment:Hex32([11;32])}})).deposit(NearToken::from_yoctonear(proposal_fee)).transact().await?.into_result()?.json()?;
        let now = ns_to_s(self.worker.view_block().await?.timestamp());
        let mut empty_hash = [0; 32];
        hex::decode_to_slice(
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
            &mut empty_hash,
        )?;
        Ok(Certificate {
            domain: DOMAIN,
            gate: self.gate.id().clone(),
            proposal_id: id,
            attempt: 0,
            chain_id: 84532,
            proxy: [2; 20],
            executor: [1; 20],
            current_impl: [10; 20],
            current_impl_codehash: [1; 32],
            new_impl: [8; 20],
            new_impl_codehash: [tag; 32],
            init_calldata_hash: empty_hash,
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
            issued_at_s: now,
            expires_at_s: now + 1800,
        })
    }
    async fn send(&self, c: &Certificate) -> Result<near_workspaces::result::ExecutionFinalResult> {
        let bytes = c.to_canonical_bytes()?;
        let sig = key().sign(&certificate_hash(&bytes)).to_bytes();
        Ok(self
            .actor
            .call(self.gate.id(), "submit_verdict")
            .args_json(json!({"certificate":Base64Bytes(bytes),"signature":Base64(sig)}))
            .max_gas()
            .transact()
            .await?)
    }
    async fn refused(&self, c: &Certificate, code: &str, field: Option<&str>) -> Result<()> {
        let result = self.send(c).await?;
        assert!(result.is_failure());
        let error = format!("{:?}", result.failures());
        assert!(error.contains(code), "{error}");
        if let Some(field) = field {
            assert!(error.contains(field), "{error}")
        };
        Ok(())
    }
    async fn proposal(&self, id: u64) -> Result<Proposal> {
        Ok(self
            .gate
            .view("get_proposal")
            .args_json(json!({"id":id}))
            .await?
            .json()?)
    }
    async fn call(
        &self,
        method: &str,
        id: u64,
    ) -> Result<near_workspaces::result::ExecutionFinalResult> {
        if method == "bump_fee" {
            // Sandbox fast_forward skips heights, not wall-clock time. Backdate only the
            // existing request timestamp to exercise later bumps without a ten-minute wait.
            let now = self.worker.view_block().await?.timestamp();
            let mut key = vec![9]; // StorageKey::SignRequestedAt
            key.extend_from_slice(&id.to_le_bytes());
            let state = self.worker.view_state(self.gate.id()).await?;
            assert!(state.contains_key(&key), "missing request timestamp");
            self.worker
                .patch_state(
                    self.gate.id(),
                    &key,
                    &now.saturating_sub(600_000_000_000).to_le_bytes(),
                )
                .await?;
        }
        Ok(self
            .actor
            .call(self.gate.id(), method)
            .args_json(if method == "bump_fee" {
                json!({"target":SignTarget::Proposal(id)})
            } else {
                json!({"proposal_id":id})
            })
            .max_gas()
            .transact()
            .await?)
    }
}

type Mutation = Box<dyn Fn(&mut Certificate)>;

#[tokio::test]
async fn mock_mpc_sandbox_t_gate_2_3_4_5_certificate_rejections_never_request_signing() -> Result<()>
{
    let h = Harness::new(100).await?;
    let good = h.propose(&h.actor, 9).await?;
    let mutate: Vec<(&str, Mutation)> = vec![
        ("chain_id", Box::new(|c| c.chain_id = 1)),
        ("proxy", Box::new(|c| c.proxy = [99; 20])),
        ("executor", Box::new(|c| c.executor = [99; 20])),
        ("new_impl", Box::new(|c| c.new_impl = [99; 20])),
        (
            "new_impl_codehash",
            Box::new(|c| c.new_impl_codehash = [99; 32]),
        ),
        (
            "source_commitment",
            Box::new(|c| c.source_commitment = [99; 32]),
        ),
        ("current_impl", Box::new(|c| c.current_impl = [99; 20])),
        (
            "init_calldata_hash",
            Box::new(|c| c.init_calldata_hash = [99; 32]),
        ),
        ("rule_set_hash", Box::new(|c| c.rule_set_hash = [99; 32])),
    ];
    for (field, mutate) in mutate {
        let mut c = good.clone();
        mutate(&mut c);
        h.refused(&c, "E45_BINDING_MISMATCH", Some(field)).await?;
    }
    for (code, mutate) in [
        (
            "E41_WRONG_DOMAIN",
            Box::new(|c: &mut Certificate| c.domain = [0; 16]) as Box<dyn Fn(&mut Certificate)>,
        ),
        (
            "E42_UNKNOWN_VERIFIER",
            Box::new(|c| c.verifier_pubkey = [99; 32]),
        ),
        (
            "E42_UNKNOWN_VERIFIER",
            Box::new(|c| c.verifier_release_hash = [99; 32]),
        ),
        (
            "E46_CERT_TIME",
            Box::new(|c| {
                c.issued_at_s += 10_000;
                c.expires_at_s += 10_000;
            }),
        ),
        (
            "E46_CERT_TIME",
            Box::new(|c| {
                c.issued_at_s = 0;
                c.expires_at_s = 1;
            }),
        ),
        ("E46_CERT_TIME", Box::new(|c| c.expires_at_s += 1)),
        (
            "E47_AI_REVIEW_REQUIRED",
            Box::new(|c| c.ai = AiReview::NotRun),
        ),
        (
            "E47_AI_REVIEW_REQUIRED",
            Box::new(|c| {
                c.ai = AiReview::Vetoed {
                    evidence_hash: [1; 32],
                    counterexample_hash: [2; 32],
                }
            }),
        ),
    ] {
        let mut c = good.clone();
        mutate(&mut c);
        h.refused(&c, code, None).await?;
    }
    let bytes = good.to_canonical_bytes()?;
    let bad = h
        .actor
        .call(h.gate.id(), "submit_verdict")
        .args_json(json!({"certificate":Base64Bytes(bytes),"signature":Base64([0;64])}))
        .transact()
        .await?;
    assert!(format!("{:?}", bad.failures()).contains("E43_BAD_SIGNATURE"));
    let bad = h
        .actor
        .call(h.gate.id(), "submit_verdict")
        .args_json(json!({"certificate":Base64Bytes(vec![0]),"signature":Base64([0;64])}))
        .transact()
        .await?;
    assert!(format!("{:?}", bad.failures()).contains("E40_BAD_ENCODING"));
    h.actor
        .call(h.gate.id(), "pause_signing")
        .args_json(json!({}))
        .transact()
        .await?
        .into_result()?;
    h.refused(&good, "E20_SIGNING_PAUSED", None).await?;
    let mut reject = good;
    reject.verdict = Verdict::Reject {
        rule_id: 2,
        reason_code: 22,
    };
    reject.ai = AiReview::NotRun;
    let rejected = h.send(&reject).await?.into_result()?;
    assert!(
        !rejected
            .logs()
            .iter()
            .any(|log| log.contains("sign_requested"))
    );
    assert_eq!(h.proposal(0).await?.state, ProposalState::Rejected);
    let credit: String = h
        .gate
        .view("get_credit")
        .args_json(json!({"account":h.actor.id()}))
        .await?
        .json()?;
    assert_eq!(credit, "100");
    Ok(())
}

#[tokio::test]
async fn mock_mpc_sandbox_t_gate_9_10_11_retry_fee_bumps_and_nonce_assignment() -> Result<()> {
    let h = Harness::new(100).await?;
    let first = h.propose(&h.actor, 9).await?;
    h.mode(1).await?;
    let _ = h.send(&first).await?;
    let failed = h.proposal(0).await?;
    assert_eq!(failed.state, ProposalState::SignFailed);
    let hash = failed.evm.as_ref().unwrap().signing_hash;
    h.mode(0).await?;
    h.call("retry_sign", 0).await?.into_result()?;
    let signed = h.proposal(0).await?;
    assert_eq!(signed.state, ProposalState::Signed);
    let early = h
        .actor
        .call(h.gate.id(), "bump_fee")
        .args_json(json!({"target":SignTarget::Proposal(0)}))
        .max_gas()
        .transact()
        .await?;
    assert!(early.is_failure());
    assert!(format!("{:?}", early.failures()).contains("E62_BUMP_TOO_EARLY"));
    let e = signed.evm.unwrap();
    assert_eq!(e.signing_hash, hash);
    assert_eq!(e.nonce, 42);
    let bob = h.worker.dev_create_account().await?;
    let mut rejected = h.propose(&bob, 10).await?;
    rejected.verdict = Verdict::Reject {
        rule_id: 2,
        reason_code: 22,
    };
    h.send(&rejected).await?.into_result()?;
    let carol = h.worker.dev_create_account().await?;
    let second = h.propose(&carol, 11).await?;
    h.send(&second).await?.into_result()?;
    assert_eq!(h.proposal(2).await?.evm.unwrap().nonce, 43);
    assert!(h.proposal(1).await?.evm.is_none());
    for _ in 0..8 {
        h.call("bump_fee", 0).await?.into_result()?;
    }
    let refused = h.call("bump_fee", 0).await?;
    assert!(format!("{:?}", refused.failures()).contains("E49_TOO_MANY_BUMPS"));
    let bumped = h.proposal(0).await?.evm.unwrap();
    assert_eq!(bumped.nonce, 42);
    assert_eq!(bumped.deadline_s, e.deadline_s);
    assert_eq!(bumped.max_fee_per_gas, 69);
    let forged = h
        .actor
        .call(h.gate.id(), "on_signed")
        .args_json(json!({"target":{"Proposal":0},"attempt":99}))
        .transact()
        .await?;
    assert!(forged.is_failure());
    assert_eq!(h.proposal(0).await?.state, ProposalState::Signed);
    Ok(())
}

#[tokio::test]
async fn mock_mpc_sandbox_malformed_callbacks_fail_closed_and_fee_cap_is_enforced() -> Result<()> {
    let h = Harness::new(12).await?;
    let c = h.propose(&h.actor, 9).await?;
    h.mode(2).await?;
    h.send(&c).await?.into_result()?;
    assert_eq!(h.proposal(0).await?.state, ProposalState::SignFailed);
    h.mode(3).await?;
    h.call("retry_sign", 0).await?.into_result()?;
    assert_eq!(h.proposal(0).await?.state, ProposalState::SignFailed);
    h.mode(0).await?;
    h.call("retry_sign", 0).await?.into_result()?;
    assert_eq!(h.proposal(0).await?.state, ProposalState::Signed);
    let cap = h.call("bump_fee", 0).await?;
    assert!(format!("{:?}", cap.failures()).contains("E48_FEE_CAP"));
    Ok(())
}

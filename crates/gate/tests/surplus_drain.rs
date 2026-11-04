//! Local sandbox regressions. Storage patches backdate timestamps, not balances or credits.
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
}
impl Harness {
    async fn new() -> Result<Self> {
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
            "constitution":Constitution{rule_set_hash:[5;32],manifest_uri:"ipfs://rules".into(),ai_required:true,params:Params{proposal_deposit:100,proposal_ttl_s:300,max_cert_ttl_s:1800,sign_validity_s:3600,gas_limit:500000,max_fee_per_gas:10,max_priority_fee_per_gas:1,fee_cap:1000,sign_gas_tgas:15,callback_gas_tgas:30}},
            "roles":Roles{governor:actor.id().clone(),guardian:actor.id().clone()},
            "verifiers":[(Base64(key().verifying_key().to_bytes()),VerifierRecord{release_hash:[6;32],attestation_uri:"ipfs://quote".into(),added_at_ns:0})],"initial_evm_nonce":42
        })).transact().await?.into_result()?;
        Ok(Self {
            worker,
            gate,
            actor,
        })
    }
    async fn amount(&self, method: &str) -> Result<u128> {
        Ok(self.gate.view(method).await?.json::<String>()?.parse()?)
    }
    async fn fund(&self, who: &Account) -> Result<()> {
        let fee: u128 = self
            .gate
            .view("get_credit_entry_fee")
            .args_json(json!({"account":who.id()}))
            .await?
            .json::<String>()?
            .parse()?;
        who.call(self.gate.id(), "deposit_credit")
            .args_json(json!({"account":null}))
            .deposit(NearToken::from_yoctonear(fee + 100))
            .transact()
            .await?
            .into_result()?;
        Ok(())
    }
    async fn propose(&self, who: &Account, data: Vec<u8>, tag: u8) -> Result<u64> {
        let args = ProposeArgs {
            new_impl: Hex20([8; 20]),
            new_impl_codehash: Hex32([tag; 32]),
            expected_current_impl: Hex20([10; 20]),
            init_calldata: HexBytes(data),
            source_commitment: Hex32([11; 32]),
        };
        Ok(who
            .call(self.gate.id(), "propose")
            .args_json(json!({"args":args}))
            .deposit(NearToken::from_yoctonear(
                self.amount("get_proposal_fee").await?,
            ))
            .max_gas()
            .transact()
            .await?
            .into_result()?
            .json()?)
    }
    async fn backdate(&self, prefix: u8, id: u64, value: &[u8]) -> Result<()> {
        let mut key = vec![prefix];
        key.extend_from_slice(&id.to_le_bytes());
        let state = self.worker.view_state(self.gate.id()).await?;
        assert!(state.contains_key(&key), "missing gate storage key {key:?}");
        self.worker.patch_state(self.gate.id(), &key, value).await?;
        Ok(())
    }
}

#[tokio::test]
async fn zero_withdrawals_and_expired_proposals_cannot_drain_surplus() -> Result<()> {
    let h = Harness::new().await?;
    let details = h.gate.view_account().await?;
    // The 0.1 NEAR slack funds transient governor records; proposal and entry fees
    // cover their own records, and cancellation releases the governance storage.
    let initial_balance = u128::from(details.storage_usage) * 10u128.pow(19) + 10u128.pow(23);
    h.worker
        .patch(h.gate.id())
        .account(
            AccountDetailsPatch::from(details).balance(NearToken::from_yoctonear(initial_balance)),
        )
        .transact()
        .await?;
    let before = h.amount("get_operating_surplus").await?;
    let balance_before = h.gate.view_account().await?.balance.as_yoctonear();
    let mut attached = 0u128;
    let proposal_fee = h.amount("get_proposal_fee").await?;
    for tag in 1u8..=8 {
        let before_zero = h.amount("get_operating_surplus").await?;
        let account_before = h.gate.view_account().await?;
        let liabilities_before = h.amount("get_total_liabilities").await?;
        let caller = h.worker.dev_create_account().await?;
        let rejected = caller
            .call(h.gate.id(), "withdraw_credit")
            .args_json(json!({"amount":"0"}))
            .transact()
            .await?;
        assert!(rejected.is_failure());
        let account_after = h.gate.view_account().await?;
        assert_eq!(account_after.storage_usage, account_before.storage_usage);
        assert_eq!(h.amount("get_total_liabilities").await?, liabilities_before);
        // Contract gas rewards increase account balance; excluding that increase,
        // the refused zero withdrawal leaves uncredited surplus exactly unchanged.
        assert_eq!(
            h.amount("get_operating_surplus").await? - before_zero,
            account_after.balance.as_yoctonear() - account_before.balance.as_yoctonear()
        );
        assert!(format!("{:?}", rejected.failures()).contains("E10_ZERO_DEPOSIT"));
        let entry_fee: u128 = h
            .gate
            .view("get_credit_entry_fee")
            .args_json(json!({"account":caller.id()}))
            .await?
            .json::<String>()?
            .parse()?;
        attached += entry_fee + 100 + proposal_fee;
        h.fund(&caller).await?;
        let id = h.propose(&caller, vec![], tag).await?;
        // Sandbox skips heights but not time: backdate the stored expiration to test
        // the real expire transition without altering the stored proposal's size.
        let mut proposal: Proposal = h
            .gate
            .view("get_proposal")
            .args_json(json!({"id":id}))
            .await?
            .json()?;
        proposal.expires_at_ns = 1;
        h.backdate(2, id, &borsh::to_vec(&proposal)?).await?;
        caller
            .call(h.gate.id(), "expire")
            .args_json(json!({"proposal_id":id}))
            .transact()
            .await?
            .into_result()?;
        let change: u64 = h
            .actor
            .call(h.gate.id(), "schedule_change")
            .args_json(json!({"kind":"Unpause"}))
            .transact()
            .await?
            .into_result()?
            .json()?;
        h.actor
            .call(h.gate.id(), "cancel_change")
            .args_json(json!({"change_id":change}))
            .transact()
            .await?
            .into_result()?;
    }
    // Net surplus cannot fall: every permanent user record has its own storage fee,
    // and each unfunded governance record has been removed before measuring.
    let rewards = h.gate.view_account().await?.balance.as_yoctonear() - balance_before - attached;
    assert!(h.amount("get_operating_surplus").await? >= before + rewards);
    Ok(())
}

#[tokio::test]
async fn max_calldata_signed_and_eight_bumps_fit_prepaid_storage() -> Result<()> {
    let h = Harness::new().await?;
    h.fund(&h.actor).await?;
    let baseline = h.gate.view_account().await?.storage_usage;
    let data = vec![0u8; 1024];
    let id = h.propose(&h.actor, data.clone(), 9).await?;
    assert!(
        h.gate
            .view_account()
            .await?
            .storage_usage
            .saturating_sub(baseline)
            <= 4096
    );
    let now = ns_to_s(h.worker.view_block().await?.timestamp());
    let cert = Certificate {
        domain: DOMAIN,
        gate: h.gate.id().clone(),
        proposal_id: id,
        attempt: 0,
        chain_id: 84532,
        proxy: [2; 20],
        executor: [1; 20],
        current_impl: [10; 20],
        current_impl_codehash: [1; 32],
        new_impl: [8; 20],
        new_impl_codehash: [9; 32],
        init_calldata_hash: alloy_sol_types::private::keccak256(&data).0,
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
    };
    let bytes = cert.to_canonical_bytes()?;
    let sig = key().sign(&certificate_hash(&bytes)).to_bytes();
    h.actor
        .call(h.gate.id(), "submit_verdict")
        .args_json(json!({"certificate":Base64Bytes(bytes),"signature":Base64(sig)}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    let signed: Proposal = h
        .gate
        .view("get_proposal")
        .args_json(json!({"id":id}))
        .await?
        .json()?;
    assert_eq!(signed.state, ProposalState::Signed);
    assert!(
        h.gate
            .view_account()
            .await?
            .storage_usage
            .saturating_sub(baseline)
            <= 4096
    );
    for _ in 0..8 {
        let now = h.worker.view_block().await?.timestamp();
        h.backdate(9, id, &now.saturating_sub(600_000_000_000).to_le_bytes())
            .await?;
        h.actor
            .call(h.gate.id(), "bump_fee")
            .args_json(json!({"target":SignTarget::Proposal(id)}))
            .max_gas()
            .transact()
            .await?
            .into_result()?;
        let usage = h.gate.view_account().await?.storage_usage;
        assert!(
            usage.saturating_sub(baseline) <= 4096,
            "proposal used {} bytes",
            usage - baseline
        );
    }
    Ok(())
}

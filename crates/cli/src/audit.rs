//! F7: prove the claim's preconditions from public state only. Read-only; any read failure
//! aborts the audit (fail closed), any violated precondition is a named FAIL line.
use crate::failure::Failure;
use crate::rpc::{Evm, Near};
use constitution_types::{
    AccountId, Base64, ChangeKind, ChangeStatus, Config, Constitution, Hex20, Hex32, PendingChange,
    Roles, VerifierRecord,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;

pub const ADMIN_SLOT: &str = "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103";
pub const IMPLEMENTATION_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";

/// The committed deployment record the audit compares public state against (P2.8 writes it).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    pub chain_id: u64,
    pub gate: AccountId,
    /// NEAR `view_account` code hash (base58) of the reproducible gate build.
    pub gate_code_hash: String,
    pub mpc_account: AccountId,
    pub executor: Hex20,
    /// Keccak256 of the committed executor runtime with compiler immutable regions zeroed.
    /// Optional at decode only so a legacy/missing commitment produces a named FAIL.
    #[serde(default)]
    pub executor_runtime_hash: Option<Hex32>,
    pub proxy: Hex20,
    pub verifiers: Vec<DeployedVerifier>,
    #[serde(default)]
    pub planned_changes: Vec<PlannedChange>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployedVerifier {
    pub pubkey: Base64<32>,
    pub release_hash: Hex32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedChange {
    pub id: u64,
    pub kind: ChangeKind,
    pub eta_ns: u64,
}

/// Everything the audit reads. Filled only by `observe`, never defaulted.
#[derive(Debug, Clone)]
pub struct Observed {
    pub access_keys: usize,
    pub code_hash: String,
    pub config: Config,
    pub constitution: Constitution,
    pub verifiers: BTreeSet<([u8; 32], [u8; 32])>,
    pub mpc_derived_eoa: [u8; 20],
    pub evm_chain_id: u64,
    pub executor_signer: [u8; 20],
    pub executor_proxy: [u8; 20],
    pub executor_proxy_admin: [u8; 20],
    pub executor_current_impl: [u8; 20],
    pub proxy_admin_owner: [u8; 20],
    pub proxy_admin_slot: [u8; 20],
    pub proxy_impl_slot: [u8; 20],
    pub executor_build_hash: [u8; 32],
    pub executor_runtime_hash: Option<[u8; 32]>,
    pub roles: Roles,
    pub signing_paused: bool,
    pub changes: Vec<PendingChange>,
}

#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    pub warning: bool,
}

fn addr(a: &[u8; 20]) -> String {
    format!("0x{}", hex::encode(a))
}

fn check(name: &'static str, ok: bool, detail: String) -> Check {
    Check {
        name,
        ok,
        detail,
        warning: false,
    }
}

pub fn evaluate(d: &Deployment, manifest_hash: [u8; 32], o: &Observed) -> Vec<Check> {
    let c = &o.config;
    let expected_verifiers: BTreeSet<_> = d
        .verifiers
        .iter()
        .map(|v| (v.pubkey.0, v.release_hash.0))
        .collect();
    let mut checks = vec![
        check(
            "gate_has_no_access_keys",
            o.access_keys == 0,
            format!(
                "{} has {} access key(s); must be 0, any key could redeploy the gate",
                d.gate, o.access_keys
            ),
        ),
        check(
            "gate_code_is_committed_build",
            o.code_hash == d.gate_code_hash,
            format!("code hash {} (committed {})", o.code_hash, d.gate_code_hash),
        ),
        check(
            "executor_code_is_committed_build",
            d.executor_runtime_hash.as_ref().is_some_and(|h| {
                h.0 == o.executor_build_hash && o.executor_runtime_hash == Some(h.0)
            }),
            format!(
                "masked runtime {}; committed {}; artifact 0x{}",
                o.executor_runtime_hash
                    .map(|h| format!("0x{}", hex::encode(h)))
                    .unwrap_or_else(|| "length mismatch".into()),
                d.executor_runtime_hash
                    .as_ref()
                    .map(|h| format!("0x{}", hex::encode(h.0)))
                    .unwrap_or_else(|| "MISSING executor_runtime_hash".into()),
                hex::encode(o.executor_build_hash),
            ),
        ),
        check(
            "gate_binds_this_deployment",
            c.chain_id == d.chain_id
                && o.evm_chain_id == d.chain_id
                && c.executor == d.executor.0
                && c.proxy == d.proxy.0
                && c.mpc_account == d.mpc_account,
            format!(
                "gate config chain {} executor {} proxy {} mpc {}; EVM RPC chain {}",
                c.chain_id,
                addr(&c.executor),
                addr(&c.proxy),
                c.mpc_account,
                o.evm_chain_id
            ),
        ),
        check(
            "derived_eoa_is_executor_signer",
            o.mpc_derived_eoa == c.derived_eoa && o.executor_signer == o.mpc_derived_eoa,
            format!(
                "MPC derives {} for ({}, {}); gate records {}; executor accepts {}",
                addr(&o.mpc_derived_eoa),
                d.gate,
                c.mpc_path,
                addr(&c.derived_eoa),
                addr(&o.executor_signer)
            ),
        ),
        check(
            "executor_controls_this_proxy",
            o.executor_proxy == d.proxy.0 && c.proxy_admin == o.executor_proxy_admin,
            format!(
                "executor.proxy {} executor.proxyAdmin {} (gate config {})",
                addr(&o.executor_proxy),
                addr(&o.executor_proxy_admin),
                addr(&c.proxy_admin)
            ),
        ),
        check(
            "executor_owns_proxy_admin",
            o.proxy_admin_owner == d.executor.0,
            format!("ProxyAdmin.owner() is {}", addr(&o.proxy_admin_owner)),
        ),
        check(
            "proxy_admin_is_erc1967_admin",
            o.proxy_admin_slot == o.executor_proxy_admin,
            format!("ERC-1967 admin slot holds {}", addr(&o.proxy_admin_slot)),
        ),
        check(
            "implementation_tracker_matches_slot",
            o.proxy_impl_slot == o.executor_current_impl,
            format!(
                "ERC-1967 implementation slot {} vs executor.currentImplementation {}",
                addr(&o.proxy_impl_slot),
                addr(&o.executor_current_impl)
            ),
        ),
        check(
            "active_rule_set_is_committed_manifest",
            o.constitution.rule_set_hash == manifest_hash,
            format!(
                "gate rule_set_hash 0x{} vs manifest 0x{}",
                hex::encode(o.constitution.rule_set_hash),
                hex::encode(manifest_hash)
            ),
        ),
        check(
            "verifier_allowlist_is_committed",
            o.verifiers == expected_verifiers,
            format!(
                "gate lists {} verifier(s), deployment commits {}",
                o.verifiers.len(),
                expected_verifiers.len()
            ),
        ),
    ];
    checks.push(check(
        "gate_roles",
        true,
        format!(
            "governor={} guardian={}",
            o.roles.governor, o.roles.guardian
        ),
    ));
    checks.push(Check {
        name: "signing_paused",
        ok: !o.signing_paused,
        warning: o.signing_paused,
        detail: format!(
            "signing_paused={}; pause does not revoke already signed transactions",
            o.signing_paused
        ),
    });
    let pending: Vec<_> = o
        .changes
        .iter()
        .filter(|c| c.status == ChangeStatus::Pending)
        .collect();
    checks.push(check(
        "pending_governance_changes",
        true,
        format!("{} pending change(s)", pending.len()),
    ));
    for c in pending {
        let planned = d
            .planned_changes
            .iter()
            .any(|p| p.id == c.id && p.kind == c.kind && p.eta_ns == c.eta_ns);
        let authority = matches!(
            c.kind,
            ChangeKind::MigrateAuthority { .. }
                | ChangeKind::SetGuardian { .. }
                | ChangeKind::SetGovernor { .. }
                | ChangeKind::AddVerifier { .. }
        );
        checks.push(Check {
            name: if authority && !planned {
                "pending_authority_change"
            } else {
                "pending_governance_change"
            },
            ok: planned || !authority,
            warning: authority && !planned,
            detail: format!(
                "id={} kind={} eta_ns={} planned={planned}",
                c.id,
                serde_json::to_string(&c.kind).expect("ChangeKind JSON"),
                c.eta_ns
            ),
        });
    }
    checks
}

/// `secp256k1:<base58 of the 64-byte uncompressed point>` to the EVM address keccak(point)[12..].
pub fn eoa_from_near_key(key: &str) -> Result<[u8; 20], String> {
    use sha3::{Digest, Keccak256};
    let encoded = key
        .strip_prefix("secp256k1:")
        .ok_or_else(|| format!("derived key {key} is not secp256k1"))?;
    let point = bs58::decode(encoded)
        .into_vec()
        .map_err(|e| format!("derived key: {e}"))?;
    if point.len() != 64 {
        return Err(format!(
            "derived key has {} bytes, expected 64",
            point.len()
        ));
    }
    Ok(Keccak256::digest(&point)[12..]
        .try_into()
        .expect("20 bytes"))
}

pub fn read_deployment(path: &Path) -> Result<Deployment, Failure> {
    let bytes = std::fs::read(path).map_err(|e| {
        Failure::new(
            format!("cannot read {}: {e}", path.display()),
            "pass --deployment with the record written at activation (deployments/<network>.json)",
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        Failure::new(
            format!("{} is not a deployment record: {e}", path.display()),
            "fix the field named above; addresses are lowercase 0x hex",
        )
    })
}

fn read_failed(cause: String) -> Failure {
    Failure::new(
        format!("FAIL read_failed: {cause}"),
        "check NEAR_RPC_URL and BASE_SEPOLIA_RPC_URL and retry; an unreadable precondition is never a pass",
    )
}

pub fn observe(
    d: &Deployment,
    near: &Near,
    evm: &Evm,
    build: &crate::executor_build::ExecutorBuild,
) -> Result<Observed, Failure> {
    let gate = d.gate.as_str();
    let config: Config = near
        .view(gate, "get_config", json!({}))
        .map_err(read_failed)?;
    let constitution: Constitution = near
        .view(gate, "get_constitution", json!({}))
        .map_err(read_failed)?;
    let listed: Vec<(Base64<32>, VerifierRecord)> = near
        .view(
            gate,
            "list_verifiers",
            json!({ "from_index": 0, "limit": 50 }),
        )
        .map_err(read_failed)?;
    if listed.len() >= 50 {
        return Err(read_failed(
            "gate lists 50 or more verifiers; the audit reads one page only".into(),
        ));
    }
    let derived_key: String = near
        .view(
            config.mpc_account.as_str(),
            "derived_public_key",
            json!({ "predecessor": gate, "path": config.mpc_path, "domain_id": config.mpc_domain_id }),
        )
        .map_err(read_failed)?;
    let roles = near
        .view(gate, "get_roles", json!({}))
        .map_err(read_failed)?;
    let signing_paused = near
        .view(gate, "is_signing_paused", json!({}))
        .map_err(read_failed)?;
    let mut changes = Vec::new();
    let mut from_id = 0u64;
    loop {
        let page: Vec<PendingChange> = near
            .view(gate, "list_changes", json!({"from_id":from_id, "limit":50}))
            .map_err(read_failed)?;
        if page.is_empty() {
            break;
        }
        let mut next = from_id;
        for c in &page {
            if c.id < next {
                return Err(read_failed(
                    "list_changes returned unordered or repeated ids".into(),
                ));
            }
            next =
                c.id.checked_add(1)
                    .ok_or_else(|| read_failed("change id overflow".into()))?;
        }
        from_id = next;
        let done = page.len() < 50;
        changes.extend(page);
        if done {
            break;
        }
    }
    let executor = d.executor.0;
    let executor_proxy_admin = evm
        .address_getter(executor, "proxyAdmin()")
        .map_err(read_failed)?;
    Ok(Observed {
        access_keys: near.access_key_count(gate).map_err(read_failed)?,
        code_hash: near.code_hash(gate).map_err(read_failed)?,
        verifiers: listed.iter().map(|(k, r)| (k.0, r.release_hash)).collect(),
        mpc_derived_eoa: eoa_from_near_key(&derived_key).map_err(read_failed)?,
        evm_chain_id: evm.chain_id().map_err(read_failed)?,
        executor_signer: evm
            .address_getter(executor, "signer()")
            .map_err(read_failed)?,
        executor_proxy: evm
            .address_getter(executor, "proxy()")
            .map_err(read_failed)?,
        executor_current_impl: evm
            .address_getter(executor, "currentImplementation()")
            .map_err(read_failed)?,
        proxy_admin_owner: evm
            .address_getter(executor_proxy_admin, "owner()")
            .map_err(read_failed)?,
        proxy_admin_slot: evm
            .storage_address(d.proxy.0, ADMIN_SLOT)
            .map_err(read_failed)?,
        proxy_impl_slot: evm
            .storage_address(d.proxy.0, IMPLEMENTATION_SLOT)
            .map_err(read_failed)?,
        executor_runtime_hash: build.masked_hash(&evm.code(executor).map_err(read_failed)?),
        executor_build_hash: build.hash(),
        roles,
        signing_paused,
        changes,
        executor_proxy_admin,
        config,
        constitution,
    })
}

/// Prints one line per check; fails if any precondition is violated.
pub fn report(checks: &[Check]) -> Result<(), Failure> {
    for c in checks {
        println!(
            "{} {}: {}",
            if c.warning {
                "WARN"
            } else if c.ok {
                "PASS"
            } else {
                "FAIL"
            },
            c.name,
            c.detail
        );
    }
    let failed: Vec<_> = checks
        .iter()
        .filter(|c| !c.ok && !c.warning)
        .map(|c| c.name)
        .collect();
    let has_warnings = checks.iter().any(|c| c.warning);
    if checks
        .iter()
        .any(|c| c.name == "pending_authority_change" && c.warning)
    {
        println!("audit WARNING: pending authority change; guardian must review/cancel before eta");
    } else if has_warnings {
        println!("audit WARNING: signing is paused; review WARN lines");
    }
    if failed.is_empty() {
        if !has_warnings {
            println!(
                "audit passed: {} named checks/information lines",
                checks.len()
            );
        }
        return Ok(());
    }
    Err(Failure::new(
        format!("audit failed: {}", failed.join(", ")),
        "do not trust upgrades through this gate; see the FAIL lines above for the violated precondition",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use constitution_types::Params;

    const GATE: &str = "constitution-gate.testnet";
    const EXECUTOR: [u8; 20] = [0xe0; 20];
    const PROXY: [u8; 20] = [0x70; 20];
    const ADMIN: [u8; 20] = [0xad; 20];
    const EOA: [u8; 20] = [0x3b; 20];
    const IMPL: [u8; 20] = [0x11; 20];
    const RULES: [u8; 32] = [5; 32];

    fn deployment() -> Deployment {
        Deployment {
            chain_id: 84532,
            gate: GATE.parse().unwrap(),
            gate_code_hash: "Code111".into(),
            mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
            executor: Hex20(EXECUTOR),
            executor_runtime_hash: Some(Hex32([0xab; 32])),
            planned_changes: vec![],
            proxy: Hex20(PROXY),
            verifiers: vec![DeployedVerifier {
                pubkey: Base64([7; 32]),
                release_hash: Hex32([6; 32]),
            }],
        }
    }

    fn healthy() -> Observed {
        Observed {
            access_keys: 0,
            code_hash: "Code111".into(),
            config: Config {
                chain_id: 84532,
                executor: EXECUTOR,
                proxy: PROXY,
                proxy_admin: ADMIN,
                derived_eoa: EOA,
                mpc_account: "v1.signer-prod.testnet".parse().unwrap(),
                mpc_path: "constitution/base-sepolia/v1".into(),
                mpc_domain_id: 0,
            },
            constitution: Constitution {
                rule_set_hash: RULES,
                manifest_uri: "rules/manifest.json".into(),
                ai_required: true,
                params: Params {
                    proposal_deposit: 1,
                    proposal_ttl_s: 1,
                    max_cert_ttl_s: 1,
                    sign_validity_s: 1,
                    gas_limit: 1,
                    max_fee_per_gas: 1,
                    max_priority_fee_per_gas: 1,
                    fee_cap: 1,
                    sign_gas_tgas: 15,
                    callback_gas_tgas: 20,
                },
            },
            verifiers: BTreeSet::from([([7; 32], [6; 32])]),
            mpc_derived_eoa: EOA,
            evm_chain_id: 84532,
            executor_signer: EOA,
            executor_proxy: PROXY,
            executor_proxy_admin: ADMIN,
            executor_current_impl: IMPL,
            proxy_admin_owner: EXECUTOR,
            proxy_admin_slot: ADMIN,
            proxy_impl_slot: IMPL,
            executor_build_hash: [0xab; 32],
            executor_runtime_hash: Some([0xab; 32]),
            roles: Roles {
                governor: "governor.testnet".parse().unwrap(),
                guardian: "guardian.testnet".parse().unwrap(),
            },
            signing_paused: false,
            changes: vec![],
        }
    }

    fn failed(o: &Observed) -> Vec<&'static str> {
        evaluate(&deployment(), RULES, o)
            .into_iter()
            .filter(|c| !c.ok)
            .map(|c| c.name)
            .collect()
    }

    #[test]
    fn healthy_deployment_passes_every_precondition() {
        let checks = evaluate(&deployment(), RULES, &healthy());
        assert_eq!(checks.len(), 14);
        assert!(checks.iter().all(|c| c.ok), "{checks:?}");
        assert!(report(&checks).is_ok());
    }

    #[test]
    fn t_aud_1_each_violation_fails_exactly_its_named_precondition() {
        type Break = fn(&mut Observed);
        let cases: [(&str, Break); 13] = [
            ("gate_has_no_access_keys", |o| o.access_keys = 1),
            ("gate_code_is_committed_build", |o| {
                o.code_hash = "Other".into()
            }),
            ("executor_code_is_committed_build", |o| {
                o.executor_runtime_hash = Some([0; 32])
            }),
            ("gate_binds_this_deployment", |o| o.evm_chain_id = 8453),
            ("gate_binds_this_deployment", |o| {
                o.config.executor = [1; 20]
            }),
            ("derived_eoa_is_executor_signer", |o| {
                o.executor_signer = [9; 20]
            }),
            ("derived_eoa_is_executor_signer", |o| {
                o.mpc_derived_eoa = [9; 20]
            }),
            ("executor_controls_this_proxy", |o| {
                o.executor_proxy = [9; 20]
            }),
            ("executor_owns_proxy_admin", |o| {
                o.proxy_admin_owner = [9; 20]
            }),
            ("proxy_admin_is_erc1967_admin", |o| {
                o.proxy_admin_slot = [9; 20]
            }),
            ("implementation_tracker_matches_slot", |o| {
                o.proxy_impl_slot = [9; 20]
            }),
            ("active_rule_set_is_committed_manifest", |o| {
                o.constitution.rule_set_hash = [0; 32]
            }),
            ("verifier_allowlist_is_committed", |o| {
                o.verifiers.insert(([8; 32], [6; 32]));
            }),
        ];
        for (name, mutate) in cases {
            let mut o = healthy();
            mutate(&mut o);
            assert_eq!(failed(&o), vec![name]);
            let error = report(&evaluate(&deployment(), RULES, &o)).unwrap_err();
            assert!(error.cause.contains(name), "{error}");
        }
    }

    #[test]
    fn missing_executor_commitment_fails_by_name() {
        let d: Deployment = serde_json::from_value(json!({
            "chain_id":84532, "gate":GATE, "gate_code_hash":"Code111",
            "mpc_account":"v1.signer-prod.testnet", "executor":addr(&EXECUTOR),
            "proxy":addr(&PROXY), "verifiers":[{"pubkey":Base64([7;32]),"release_hash":Hex32([6;32])}]
        })).unwrap();
        let checks = evaluate(&d, RULES, &healthy());
        assert!(
            checks
                .iter()
                .any(|c| c.name == "executor_code_is_committed_build" && !c.ok)
        );
    }

    #[test]
    fn pause_warns_and_never_hides_a_failed_precondition() {
        let mut o = healthy();
        o.signing_paused = true;
        let checks = evaluate(&deployment(), RULES, &o);
        assert!(
            checks
                .iter()
                .any(|c| c.name == "signing_paused" && c.warning && c.detail.contains("true"))
        );
        assert!(checks.iter().any(|c| c.name == "gate_roles"
            && c.detail.contains("guardian.testnet")
            && c.detail.contains("governor.testnet")));
        assert!(report(&checks).is_ok());
        o.access_keys = 1;
        assert!(
            report(&evaluate(&deployment(), RULES, &o))
                .unwrap_err()
                .cause
                .contains("gate_has_no_access_keys")
        );
    }

    #[test]
    fn planned_changes_require_exact_kind_and_eta_and_ignore_terminal_changes() {
        let mut o = healthy();
        let c = PendingChange {
            id: 1,
            kind: ChangeKind::SetGuardian {
                account: "replacement.testnet".parse().unwrap(),
            },
            scheduled_by: o.roles.governor.clone(),
            scheduled_at_ns: 1,
            eta_ns: 604_800_000_000_001,
            status: ChangeStatus::Pending,
            evm: None,
        };
        o.changes.push(c.clone());
        let mut d = deployment();
        let warned = |d: &Deployment, o: &Observed| {
            evaluate(d, RULES, o)
                .iter()
                .any(|c| c.name == "pending_authority_change" && c.warning)
        };
        assert!(warned(&d, &o));
        d.planned_changes.push(PlannedChange {
            id: c.id,
            kind: c.kind.clone(),
            eta_ns: c.eta_ns,
        });
        assert!(!warned(&d, &o));
        d.planned_changes[0].eta_ns += 1;
        assert!(warned(&d, &o));
        d.planned_changes[0].eta_ns = c.eta_ns;
        d.planned_changes[0].kind = ChangeKind::SetGovernor {
            account: "replacement.testnet".parse().unwrap(),
        };
        assert!(warned(&d, &o));
        for status in [ChangeStatus::Applied, ChangeStatus::Cancelled] {
            o.changes[0].status = status;
            assert!(!warned(&d, &o));
            assert!(evaluate(&d, RULES, &o).iter().any(
                |c| c.name == "pending_governance_changes" && c.detail == "0 pending change(s)"
            ));
        }
    }

    #[test]
    fn near_derived_key_maps_to_the_recorded_spike_address() {
        // vectors/mpc-derivation.json and spikes/gate-min/README.md, finalized block 275927720.
        let key = "secp256k1:2LxkyX7Nuw1MbtVBAHWubggYkZ91pGE2yxuEo7bNuVakGYsBrmnHvAuDQtiVExSFF2RPLeepXsYszq8ynFToa8KZ";
        assert_eq!(
            addr(&eoa_from_near_key(key).unwrap()),
            "0x3b20f46860d27d81917a45221c594fc051af7e94"
        );
        assert!(eoa_from_near_key("ed25519:2Lxky").is_err());
        assert!(eoa_from_near_key("secp256k1:2Lxky").is_err());
    }

    #[test]
    fn erc1967_slots_are_keccak_of_their_label_minus_one() {
        use sha3::{Digest, Keccak256};
        for (label, slot) in [
            ("eip1967.proxy.admin", ADMIN_SLOT),
            ("eip1967.proxy.implementation", IMPLEMENTATION_SLOT),
        ] {
            let mut expected: [u8; 32] = Keccak256::digest(label.as_bytes()).into();
            let borrow = expected.iter().rposition(|b| *b != 0).unwrap();
            expected[borrow] -= 1;
            expected[borrow + 1..].fill(0xff);
            assert_eq!(slot, format!("0x{}", hex::encode(expected)), "{label}");
        }
    }
}

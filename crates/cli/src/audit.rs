//! F7: prove the claim's preconditions from public state only. Read-only; any read failure
//! aborts the audit (fail closed), any violated precondition is a named FAIL line.
use crate::failure::Failure;
use crate::rpc::{Evm, Near};
use constitution_types::{AccountId, Base64, Config, Constitution, Hex20, Hex32, VerifierRecord};
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
    pub proxy: Hex20,
    pub verifiers: Vec<DeployedVerifier>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployedVerifier {
    pub pubkey: Base64<32>,
    pub release_hash: Hex32,
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
}

#[derive(Debug)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn addr(a: &[u8; 20]) -> String {
    format!("0x{}", hex::encode(a))
}

fn check(name: &'static str, ok: bool, detail: String) -> Check {
    Check { name, ok, detail }
}

pub fn evaluate(d: &Deployment, manifest_hash: [u8; 32], o: &Observed) -> Vec<Check> {
    let c = &o.config;
    let expected_verifiers: BTreeSet<_> = d
        .verifiers
        .iter()
        .map(|v| (v.pubkey.0, v.release_hash.0))
        .collect();
    vec![
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
    ]
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
        cause,
        "check NEAR_RPC_URL and BASE_SEPOLIA_RPC_URL and retry; an unreadable precondition is never a pass",
    )
}

pub fn observe(d: &Deployment, near: &Near, evm: &Evm) -> Result<Observed, Failure> {
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
            if c.ok { "PASS" } else { "FAIL" },
            c.name,
            c.detail
        );
    }
    let failed: Vec<_> = checks.iter().filter(|c| !c.ok).map(|c| c.name).collect();
    if failed.is_empty() {
        println!(
            "audit passed: {} of {} preconditions hold",
            checks.len(),
            checks.len()
        );
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
        assert_eq!(checks.len(), 10);
        assert!(checks.iter().all(|c| c.ok), "{checks:?}");
        assert!(report(&checks).is_ok());
    }

    #[test]
    fn t_aud_1_each_violation_fails_exactly_its_named_precondition() {
        type Break = fn(&mut Observed);
        let cases: [(&str, Break); 12] = [
            ("gate_has_no_access_keys", |o| o.access_keys = 1),
            ("gate_code_is_committed_build", |o| {
                o.code_hash = "Other".into()
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

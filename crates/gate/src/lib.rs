//! NEAR gate contract. Interface: docs/technical/SMART-CONTRACT.md.
use constitution_types::{
    AccountId, Base64, ChangeKind, ChangeStatus, Config, Constitution, GateError,
    MAX_INIT_CALLDATA, Params, PendingChange, Proposal, ProposalState, ProposeArgs, TIMELOCK_NS,
    VerifierRecord, ns_to_s, s_to_ns,
};
use near_sdk::{
    BorshStorageKey, NearToken, PanicOnDefault, Promise, env,
    json_types::U128,
    near,
    store::{IterableMap, IterableSet, LookupMap},
};
use serde_json::json;

mod credit;
mod evm;
mod governance;
mod proposal;
mod signing;
mod verdict;
mod views;

#[derive(borsh::BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Verifiers,
    Changes,
    Proposals,
    ActiveByAccount,
    Credit,
    ActiveByKey,
    OutstandingSignatures,
    FeeBumps,
    MigrationSigning,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Gate {
    config: Config,
    constitution: Constitution,
    roles: constitution_types::Roles,
    verifiers: IterableMap<[u8; 32], VerifierRecord>,
    changes: IterableMap<u64, PendingChange>,
    proposals: LookupMap<u64, Proposal>,
    active_by_account: LookupMap<AccountId, u64>,
    credit: LookupMap<AccountId, u128>,
    next_proposal_id: u64,
    next_change_id: u64,
    next_evm_nonce: u64,
    signing_paused: bool,
    // Indexes avoid scans over historical proposals. P2.3 maintains signing membership.
    active_by_key: LookupMap<[u8; 32], u64>,
    outstanding_signatures: IterableSet<u64>,
    fee_bumps: LookupMap<u64, u8>,
    migration_signing: LookupMap<u64, signing::MigrationSigning>,
}

use constitution_types::Roles;

#[near]
impl Gate {
    #[init]
    pub fn new(
        config: Config,
        constitution: Constitution,
        roles: Roles,
        verifiers: Vec<(Base64<32>, VerifierRecord)>,
        initial_evm_nonce: u64,
    ) -> Self {
        check(
            config.chain_id == 84532,
            GateError::InvalidChange,
            "chain_id",
        );
        check(
            config.mpc_account.as_str() == "v1.signer-prod.testnet",
            GateError::InvalidChange,
            "mpc_account",
        );
        check(
            config.mpc_path == "constitution/base-sepolia/v1" && config.mpc_domain_id == 0,
            GateError::InvalidChange,
            "MPC derivation identity",
        );
        check(
            [
                config.executor,
                config.proxy,
                config.proxy_admin,
                config.derived_eoa,
            ]
            .iter()
            .all(|a| *a != [0; 20]),
            GateError::InvalidChange,
            "zero address",
        );
        validate_uri(&constitution.manifest_uri);
        validate_params(&constitution.params);
        check(
            !verifiers.is_empty(),
            GateError::InvalidChange,
            "no verifier",
        );
        // Validate the complete input before writing any storage.
        for (index, (pubkey, record)) in verifiers.iter().enumerate() {
            validate_uri(&record.attestation_uri);
            check(
                !verifiers[..index].iter().any(|(key, _)| key == pubkey),
                GateError::InvalidChange,
                "duplicate verifier",
            );
        }
        let mut gate = Self {
            config,
            constitution,
            roles,
            verifiers: IterableMap::new(StorageKey::Verifiers),
            changes: IterableMap::new(StorageKey::Changes),
            proposals: LookupMap::new(StorageKey::Proposals),
            active_by_account: LookupMap::new(StorageKey::ActiveByAccount),
            credit: LookupMap::new(StorageKey::Credit),
            next_proposal_id: 0,
            next_change_id: 0,
            next_evm_nonce: initial_evm_nonce,
            signing_paused: false,
            active_by_key: LookupMap::new(StorageKey::ActiveByKey),
            outstanding_signatures: IterableSet::new(StorageKey::OutstandingSignatures),
            fee_bumps: LookupMap::new(StorageKey::FeeBumps),
            migration_signing: LookupMap::new(StorageKey::MigrationSigning),
        };
        for (key, record) in verifiers {
            gate.verifiers.insert(key.0, record);
        }
        gate
    }
}

fn check(condition: bool, error: GateError, detail: &str) {
    if !condition {
        env::panic_str(&format!("{}: {detail}", error.code()));
    }
}

fn validate_uri(uri: &str) {
    check(
        !uri.is_empty() && uri.len() <= 256,
        GateError::InvalidChange,
        "URI must contain 1 to 256 bytes",
    );
}

fn validate_params(params: &Params) {
    check(
        params.sign_gas_tgas >= 15,
        GateError::InvalidChange,
        "sign_gas_tgas < 15",
    );
    check(
        s_to_ns(params.proposal_ttl_s).is_some(),
        GateError::InvalidChange,
        "proposal_ttl_s overflows nanoseconds",
    );
}

fn emit(event: &str, data: serde_json::Value) {
    env::log_str(&format!(
        "EVENT_JSON:{}",
        json!({
            "standard": "constitution", "version": "1.0.0", "event": event, "data": [data]
        })
    ));
}

#[cfg(test)]
mod tests;

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

const MAX_PENDING_CHANGES: usize = 8;
const SIGN_REQUEST_TIMEOUT: u64 = 1_200_000_000_000;
const MIN_CERT_TTL_S: u64 = 30;
const MAX_CERT_TTL_S: u64 = 86_400;
const MIN_SIGN_VALIDITY_S: u64 = 60;
const MAX_SIGN_VALIDITY_S: u64 = 86_400;
const MIN_PROPOSAL_TTL_S: u64 = 300;
const MAX_PROPOSAL_TTL_S: u64 = 604_800;
const STORAGE_INSOLVENT: &str = "E12_STORAGE_INSOLVENT";
const AI_SIGNATURE_KIND: &str = "E58_AI_SIGNATURE_KIND";
const PENDING_CHANGE_LIMIT: &str = "E59_PENDING_CHANGE_LIMIT";
const DUPLICATE_AUTHORITY_CHANGE: &str = "E60_DUPLICATE_AUTHORITY_CHANGE";
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
    SignRequestedAt,
    MigrationRequestedAt,
    MigrationFeeBumps,
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
    total_liabilities: u128,
    pending_withdrawals: u128,
    pending_changes: Vec<u64>,
    sign_requested_at: LookupMap<u64, u64>,
    migration_requested_at: LookupMap<u64, u64>,
    migration_fee_bumps: LookupMap<u64, u8>,
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
                record.release_hash != [0; 32],
                GateError::InvalidChange,
                "zero release_hash",
            );
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
            total_liabilities: 0,
            pending_withdrawals: 0,
            pending_changes: Vec::new(),
            sign_requested_at: LookupMap::new(StorageKey::SignRequestedAt),
            migration_requested_at: LookupMap::new(StorageKey::MigrationRequestedAt),
            migration_fee_bumps: LookupMap::new(StorageKey::MigrationFeeBumps),
        };
        for (key, record) in verifiers {
            gate.verifiers.insert(key.0, record);
        }
        gate.assert_solvent(0);
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
        (15..=200).contains(&params.sign_gas_tgas)
            && (10..=100).contains(&params.callback_gas_tgas)
            && params.sign_gas_tgas + params.callback_gas_tgas <= 250
            && (MIN_CERT_TTL_S..=MAX_CERT_TTL_S).contains(&params.max_cert_ttl_s)
            && (MIN_SIGN_VALIDITY_S..=MAX_SIGN_VALIDITY_S).contains(&params.sign_validity_s)
            && (MIN_PROPOSAL_TTL_S..=MAX_PROPOSAL_TTL_S).contains(&params.proposal_ttl_s)
            && params.gas_limit > 0
            && params.max_priority_fee_per_gas <= params.max_fee_per_gas
            && params.max_fee_per_gas <= params.fee_cap,
        GateError::InvalidChange,
        "unsafe gas, time or fee parameters",
    );
}

impl Gate {
    // SDK store collections defer writes. Flush before measuring, including the
    // root value normally written by the generated wrapper after this method.
    fn flush_storage(&mut self) {
        self.verifiers.flush();
        self.changes.flush();
        self.proposals.flush();
        self.active_by_account.flush();
        self.credit.flush();
        self.active_by_key.flush();
        self.outstanding_signatures.flush();
        self.fee_bumps.flush();
        self.migration_signing.flush();
        self.sign_requested_at.flush();
        self.migration_requested_at.flush();
        self.migration_fee_bumps.flush();
        env::state_write(self);
    }

    fn liquid_balance(&self) -> Option<u128> {
        env::account_balance()
            .as_yoctonear()
            .checked_sub(u128::from(env::storage_usage()) * env::storage_byte_cost().as_yoctonear())
    }

    fn has_liquid_backing(&self, outgoing: u128) -> bool {
        match (
            self.liquid_balance()
                .and_then(|liquid| liquid.checked_sub(outgoing)),
            self.total_liabilities.checked_add(self.pending_withdrawals),
        ) {
            (Some(liquid), Some(required)) => liquid >= required,
            _ => false,
        }
    }

    fn assert_solvent(&mut self, outgoing: u128) {
        self.flush_storage();
        if !self.has_liquid_backing(outgoing) {
            env::panic_str(STORAGE_INSOLVENT);
        }
    }
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

//! Throwaway spike only. The arbitrary-hash signer MUST NOT ship in the production gate.
use alloy_sol_types::{SolCall, sol};
use near_sdk::{Gas, NearToken, Promise, env, json_types::U128, near};
use omni_transaction::{EVM, TransactionBuilder, TxBuilder};

pub const MPC_PATH: &str = "constitution/base-sepolia/v1";
pub const MPC_ACCOUNT: &str = "v1.signer-prod.testnet";
pub const SIGN_GAS: Gas = Gas::from_tgas(15);

sol! {
    function execute(bytes32 certId, address newImpl, bytes32 newImplCodehash,
        address expectedCurrentImpl, uint64 deadline, bytes initCalldata);
}

#[near(serializers = [json])]
pub struct ExecuteInput {
    pub executor: [u8; 20],
    pub certificate_id: [u8; 32],
    pub new_impl: [u8; 20],
    pub new_impl_codehash: [u8; 32],
    pub expected_current_impl: [u8; 20],
    pub deadline_s: u64,
    pub init_calldata: Vec<u8>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub max_fee_per_gas: U128,
    pub max_priority_fee_per_gas: U128,
}

pub fn execute_calldata(input: &ExecuteInput) -> Vec<u8> {
    executeCall {
        certId: input.certificate_id.into(),
        newImpl: input.new_impl.into(),
        newImplCodehash: input.new_impl_codehash.into(),
        expectedCurrentImpl: input.expected_current_impl.into(),
        deadline: input.deadline_s,
        initCalldata: input.init_calldata.clone().into(),
    }
    .abi_encode()
}

pub fn signing_preimage(input: &ExecuteInput) -> Vec<u8> {
    assert!(
        input.init_calldata.len() <= 1024,
        "init calldata exceeds 1024 bytes"
    );
    assert!(
        input.max_priority_fee_per_gas.0 <= input.max_fee_per_gas.0,
        "priority fee exceeds max fee"
    );
    TransactionBuilder::new::<EVM>()
        .chain_id(84532)
        .nonce(input.nonce)
        .max_priority_fee_per_gas(input.max_priority_fee_per_gas.0)
        .max_fee_per_gas(input.max_fee_per_gas.0)
        .gas_limit(input.gas_limit.into())
        .to(input.executor)
        .value(0)
        .input(execute_calldata(input))
        .access_list(vec![])
        .build()
        .build_for_signing()
}

#[near(contract_state)]
#[derive(Default)]
pub struct GateMin;

#[near]
impl GateMin {
    /// Offline view. All fields are spike inputs, not a certificate authorization path.
    pub fn build_execute(&self, input: ExecuteInput) -> near_sdk::serde_json::Value {
        let preimage = signing_preimage(&input);
        near_sdk::serde_json::json!({
            "preimage": hex::encode(&preimage),
            "signing_hash": hex::encode(env::keccak256_array(&preimage)),
        })
    }

    /// DANGEROUS: dev-only arbitrary-hash signer. Never copy into the production gate.
    /// Authorized online spike calls use constitution-spike.testnet only.
    pub fn spike_sign(&mut self, hash: [u8; 32]) -> Promise {
        Promise::new(MPC_ACCOUNT.parse().expect("constant MPC account"))
            .function_call(
                "sign",
                near_sdk::serde_json::to_vec(&near_sdk::serde_json::json!({
                    "request": {
                        "payload_v2": { "Ecdsa": hex::encode(hash) },
                        "path": MPC_PATH,
                        "domain_id": 0,
                    }
                }))
                .expect("serializable request"),
                NearToken::from_yoctonear(1),
                SIGN_GAS,
            )
            .then(Promise::new(env::current_account_id()).function_call(
                "on_spike_signed",
                b"{}".to_vec(),
                NearToken::from_yoctonear(0),
                Gas::from_tgas(20),
            ))
    }

    /// Preserve the exact MPC success bytes as a JSON string in the final result.
    #[private]
    pub fn on_spike_signed(&mut self) -> String {
        match env::promise_result_checked(0, 4096) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(response) => {
                    env::log_str(&format!("SPIKE_MPC_RESPONSE:{response}"));
                    response
                }
                Err(_) => {
                    env::log_str("SPIKE_MPC_RESPONSE_INVALID_UTF8");
                    String::new()
                }
            },
            Err(error) => {
                env::log_str(&format!("SPIKE_MPC_FAILED:{error:?}"));
                String::new()
            }
        }
    }
}

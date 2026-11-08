//! Signed NEAR function calls (near-api 0.8.6). Keys come from a near-cli legacy key file
//! (`{account_id, public_key, private_key}`), never from arguments or chat.
use crate::failure::Failure;
use near_api::{Contract, NearGas, NearToken, NetworkConfig, Signer};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Writer {
    network: NetworkConfig,
    account: near_api::AccountId,
    signer: Arc<Signer>,
    runtime: tokio::runtime::Runtime,
    rpc_url: String,
}

/// A landed call: the NEAR transaction hash, the method's JSON return (Null if none) and logs.
pub struct Called {
    pub tx_hash: String,
    pub value: Value,
}

/// `.secrets/near-credentials/<network>/<account>.json`, where the repo keeps testnet keys.
pub fn default_key_file(network: &str, account: &str) -> PathBuf {
    Path::new(".secrets/near-credentials")
        .join(network)
        .join(format!("{account}.json"))
}

impl Writer {
    pub fn new(rpc_url: &str, account: &str, key_file: &Path) -> Result<Self, Failure> {
        let url = rpc_url.parse().map_err(|_| {
            Failure::new(
                format!("invalid NEAR RPC endpoint ({})", crate::rpc::host(rpc_url)),
                "set NEAR_RPC_URL to an http(s) URL",
            )
        })?;
        let account = account.parse().map_err(|e| {
            Failure::new(
                format!("account id {account}: {e}"),
                "pass a valid NEAR account id with --account",
            )
        })?;
        let signer = Signer::from_access_keyfile(key_file.to_path_buf()).map_err(|e| {
            Failure::new(
                format!("cannot load key file {}: {e}", key_file.display()),
                "pass --key-file with a near-cli legacy key file for this account",
            )
        })?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Failure::new(format!("async runtime: {e}"), "report this as a bug"))?;
        Ok(Self {
            network: NetworkConfig::from_rpc_url("constitution", url),
            account,
            signer,
            runtime,
            rpc_url: rpc_url.to_owned(),
        })
    }

    /// Calls `method` and waits for execution. A contract panic becomes a Failure carrying the
    /// gate's error code (for example `E53_TIMELOCK_NOT_ELAPSED`) and `next`.
    pub fn call(
        &self,
        contract: &str,
        method: &str,
        args: Value,
        deposit_yocto: u128,
        next: &'static str,
    ) -> Result<Called, Failure> {
        let contract: near_api::AccountId = contract.parse().map_err(|e| {
            Failure::new(
                format!("contract id {contract}: {e}"),
                "pass a valid gate account id",
            )
        })?;
        let send = Contract(contract)
            .call_function(method, args)
            .transaction()
            .gas(NearGas::from_tgas(300))
            .deposit(NearToken::from_yoctonear(deposit_yocto))
            .with_signer(self.account.clone(), self.signer.clone())
            .send_to(&self.network);
        let result = self.runtime.block_on(send).map_err(|e| {
            Failure::new(
                format!(
                    "{method} was not sent via {}: {}",
                    crate::rpc::host(&self.rpc_url),
                    crate::rpc::safe_error(&self.rpc_url, e)
                ),
                "check NEAR_RPC_URL, the account and that the key may call this method",
            )
        })?;
        let full = result.into_full().ok_or_else(|| {
            Failure::new(
                format!("{method} is still pending"),
                "look the transaction up and rerun the read command",
            )
        })?;
        let tx_hash = full.outcome().transaction_hash.to_string();
        match full.into_result() {
            Ok(success) => Ok(Called {
                tx_hash,
                value: success.json::<Value>().unwrap_or(Value::Null),
            }),
            Err(failure) => Err(Failure::new(
                format!(
                    "{method} failed in tx {tx_hash}: {}",
                    crate::rpc::safe_error(&self.rpc_url, failure)
                ),
                next,
            )),
        }
    }
}

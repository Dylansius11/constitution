//! Final NEAR views plus confirmed Base observations. Call from a blocking worker.
use crate::{
    keccak,
    proposals::{LocalProposal, ProposalSource},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use constitution_types::{
    AccountId, Config, Hex20, Hex32, HexBytes, Proposal, ProposalState,
    manifest::{RuleSetManifest, rule_set_hash},
    ns_to_s,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;

const IMPLEMENTATION_SLOT: &str =
    "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc";
pub struct GateSource {
    near_rpc: reqwest::Url,
    base_rpc: reqwest::Url,
    rule_set_hash: [u8; 32],
    confirmations: u64,
}
impl GateSource {
    pub fn new(near_rpc: &str, base_rpc: &str, manifest: &RuleSetManifest) -> Result<Self, String> {
        fn url(value: &str) -> Result<reqwest::Url, String> {
            let url = reqwest::Url::parse(value).map_err(|_| "invalid RPC URL")?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err("RPC URL must use http(s)".into());
            }
            Ok(url)
        }
        Ok(Self {
            near_rpc: url(near_rpc)?,
            base_rpc: url(base_rpc)?,
            rule_set_hash: rule_set_hash(manifest).map_err(|e| e.to_string())?,
            confirmations: u64::from(manifest.r5.confirmations),
        })
    }
}
struct Rpc(reqwest::blocking::Client);
impl Rpc {
    fn call<T: DeserializeOwned>(
        &self,
        url: &reqwest::Url,
        method: &str,
        params: Value,
    ) -> Result<T, String> {
        let response = self
            .0
            .post(url.clone())
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("{method} RPC transport: {}", e.without_url()))?;
        let value: Value = response
            .json()
            .map_err(|e| format!("{method} RPC response: {}", e.without_url()))?;
        if value["jsonrpc"] != "2.0" || value["id"] != 1 {
            return Err(format!("{method} RPC invalid envelope"));
        }
        if let Some(error) = value.get("error") {
            return Err(format!("{method} RPC error: {error}"));
        }
        let result = value
            .get("result")
            .ok_or_else(|| format!("{method} RPC missing result"))?;
        serde_json::from_value(result.clone()).map_err(|e| format!("{method} RPC result: {e}"))
    }
    fn view<T: DeserializeOwned>(
        &self,
        url: &reqwest::Url,
        gate: &AccountId,
        method: &str,
        args: Value,
    ) -> Result<T, String> {
        #[derive(serde::Deserialize)]
        struct ViewResult {
            result: Vec<u8>,
        }
        let response:ViewResult=self.call(url,"query",json!({"request_type":"call_function","finality":"final","account_id":gate,"method_name":method,"args_base64":STANDARD.encode(serde_json::to_vec(&args).map_err(|e|e.to_string())?)}))?;
        serde_json::from_slice(&response.result).map_err(|e| format!("{method} view: {e}"))
    }
}
fn quantity(text: &str) -> Result<u64, String> {
    let digits = text
        .strip_prefix("0x")
        .ok_or("missing hex quantity prefix")?;
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return Err("invalid hex quantity".into());
    }
    u64::from_str_radix(digits, 16).map_err(|e| format!("invalid hex quantity: {e}"))
}
impl ProposalSource for GateSource {
    fn archive_rpc(&self) -> Option<String> {
        Some(self.base_rpc.as_str().to_owned())
    }
    fn archive_provider(&self) -> String {
        format!(
            "NEAR {} final + Base {} at latest-{}",
            self.near_rpc.host_str().expect("http(s) URL host"),
            self.base_rpc.host_str().expect("http(s) URL host"),
            self.confirmations,
        )
    }
    fn get(&self, gate: &AccountId, id: u64) -> Result<Option<LocalProposal>, String> {
        let rpc = Rpc(reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.without_url().to_string())?);
        let config: Config = rpc.view(&self.near_rpc, gate, "get_config", json!({}))?;
        let proposal: Option<Proposal> =
            rpc.view(&self.near_rpc, gate, "get_proposal", json!({"id":id}))?;
        let Some(p) = proposal else { return Ok(None) };
        if p.id != id {
            return Err("get_proposal id mismatch".into());
        }
        if p.state != ProposalState::Pending {
            return Err("proposal must be Pending".into());
        }
        if p.rule_set_hash != self.rule_set_hash {
            return Err("unsupported rule_set_hash".into());
        }
        let latest: String = rpc.call(&self.base_rpc, "eth_blockNumber", json!([]))?;
        let number = quantity(&latest)?
            .checked_sub(self.confirmations)
            .ok_or("insufficient blocks for manifest confirmations")?;
        let tag = format!("0x{number:x}");
        #[derive(serde::Deserialize)]
        struct Block {
            number: String,
            hash: Hex32,
        }
        let block: Block = rpc.call(&self.base_rpc, "eth_getBlockByNumber", json!([tag, false]))?;
        if quantity(&block.number)? != number {
            return Err("evidence block number mismatch".into());
        }
        let slot: Hex32 = rpc.call(
            &self.base_rpc,
            "eth_getStorageAt",
            json!([Hex20(config.proxy), IMPLEMENTATION_SLOT, tag]),
        )?;
        if slot.0[..12] != [0; 12] {
            return Err("implementation slot high 12 bytes must be zero".into());
        }
        let mut address = [0; 20];
        address.copy_from_slice(&slot.0[12..]);
        let code: HexBytes =
            rpc.call(&self.base_rpc, "eth_getCode", json!([Hex20(address), tag]))?;
        if address == [0; 20] || code.0.is_empty() {
            return Err("empty current implementation".into());
        }
        // Refuse a reorg observed during the numbered reads rather than combining forks.
        let after: Block = rpc.call(&self.base_rpc, "eth_getBlockByNumber", json!([tag, false]))?;
        if after.hash != block.hash || after.number != block.number {
            return Err("evidence block changed during observation".into());
        }
        Ok(Some(LocalProposal {
            gate: gate.clone(),
            proposal_id: p.id,
            chain_id: config.chain_id,
            proxy: Hex20(config.proxy),
            executor: Hex20(config.executor),
            expected_current_impl: Hex20(p.expected_current_impl),
            observed_current_impl: Hex20(address),
            current_impl_codehash: Hex32(keccak(&code.0)),
            new_impl: Hex20(p.new_impl),
            new_impl_codehash: Hex32(p.new_impl_codehash),
            init_calldata: HexBytes(p.init_calldata),
            source_commitment: Hex32(p.source_commitment),
            rule_set_hash: Hex32(p.rule_set_hash),
            evidence_block_number: number,
            evidence_block_hash: block.hash,
            expires_at_s: ns_to_s(p.expires_at_ns),
        }))
    }
}

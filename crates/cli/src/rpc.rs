//! Read-only JSON-RPC clients. Every read is at NEAR `final` finality or the EVM `latest` block;
//! any transport, RPC or shape error is returned, never defaulted.
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn post(
    client: &reqwest::blocking::Client,
    url: &str,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let body =
        json!({ "jsonrpc": "2.0", "id": "constitution", "method": method, "params": params });
    let response: Value = client
        .post(url)
        .json(&body)
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.json())
        .map_err(|e| format!("{method} via {url}: {e}"))?;
    if let Some(error) = response.get("error") {
        return Err(format!("{method} via {url}: {error}"));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| format!("{method} via {url}: response has no result"))
}

pub struct Near {
    url: String,
    client: reqwest::blocking::Client,
}

impl Near {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            client: reqwest::blocking::Client::new(),
        }
    }

    fn query(&self, params: Value) -> Result<Value, String> {
        let result = post(&self.client, &self.url, "query", params)?;
        // nearcore reports some query failures inside `result.error`.
        if let Some(error) = result.get("error") {
            return Err(format!("query via {}: {error}", self.url));
        }
        Ok(result)
    }

    pub fn view<T: DeserializeOwned>(
        &self,
        account: &str,
        method: &str,
        args: Value,
    ) -> Result<T, String> {
        use base64::Engine;
        let args = base64::engine::general_purpose::STANDARD.encode(args.to_string());
        let result = self.query(json!({
            "request_type": "call_function", "finality": "final",
            "account_id": account, "method_name": method, "args_base64": args,
        }))?;
        let bytes: Vec<u8> = serde_json::from_value(result["result"].clone())
            .map_err(|e| format!("{account}.{method}: result is not bytes: {e}"))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| format!("{account}.{method}: unexpected JSON: {e}"))
    }

    pub fn access_key_count(&self, account: &str) -> Result<usize, String> {
        let result = self.query(json!({
            "request_type": "view_access_key_list", "finality": "final", "account_id": account,
        }))?;
        result["keys"]
            .as_array()
            .map(Vec::len)
            .ok_or_else(|| format!("{account}: access key list has no keys array"))
    }

    pub fn code_hash(&self, account: &str) -> Result<String, String> {
        let result = self.query(json!({
            "request_type": "view_account", "finality": "final", "account_id": account,
        }))?;
        result["code_hash"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{account}: account view has no code_hash"))
    }
}

pub struct Evm {
    url: String,
    client: reqwest::blocking::Client,
}

fn hex_result(value: Value, what: &str) -> Result<Vec<u8>, String> {
    let text = value
        .as_str()
        .ok_or_else(|| format!("{what}: result is not a string"))?;
    let digits = text
        .strip_prefix("0x")
        .ok_or_else(|| format!("{what}: result is not 0x hex"))?;
    hex::decode(digits).map_err(|e| format!("{what}: {e}"))
}

pub fn selector(signature: &str) -> [u8; 4] {
    use sha3::{Digest, Keccak256};
    let hash = Keccak256::digest(signature.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

/// A 32-byte word that must hold exactly one address (high 12 bytes zero).
pub fn word_address(word: &[u8], what: &str) -> Result<[u8; 20], String> {
    if word.len() != 32 || word[..12].iter().any(|b| *b != 0) {
        return Err(format!(
            "{what}: not a single ABI-encoded address ({} bytes)",
            word.len()
        ));
    }
    Ok(word[12..].try_into().expect("20 bytes"))
}

impl Evm {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn chain_id(&self) -> Result<u64, String> {
        quantity(
            &post(&self.client, &self.url, "eth_chainId", json!([]))?,
            "eth_chainId",
        )
    }

    pub fn code(&self, address: [u8; 20]) -> Result<Vec<u8>, String> {
        hex_result(
            post(
                &self.client,
                &self.url,
                "eth_getCode",
                json!([format!("0x{}", hex::encode(address)), "latest"]),
            )?,
            "eth_getCode",
        )
    }

    pub fn block_number(&self) -> Result<u64, String> {
        quantity(
            &post(&self.client, &self.url, "eth_blockNumber", json!([]))?,
            "eth_blockNumber",
        )
    }

    pub fn storage_address_at(
        &self,
        at: [u8; 20],
        slot: &str,
        block: u64,
    ) -> Result<[u8; 20], String> {
        let what = format!("slot {slot} of 0x{} at block {block}", hex::encode(at));
        let word = hex_result(
            post(
                &self.client,
                &self.url,
                "eth_getStorageAt",
                json!([
                    format!("0x{}", hex::encode(at)),
                    slot,
                    format!("0x{block:x}")
                ]),
            )?,
            &what,
        )?;
        word_address(&word, &what)
    }

    /// Calls a zero-argument getter that returns one address.
    pub fn address_getter(&self, to: [u8; 20], signature: &str) -> Result<[u8; 20], String> {
        let what = format!("0x{}.{signature}", hex::encode(to));
        let call = json!({ "to": format!("0x{}", hex::encode(to)), "data": format!("0x{}", hex::encode(selector(signature))) });
        let word = hex_result(
            post(&self.client, &self.url, "eth_call", json!([call, "latest"]))?,
            &what,
        )?;
        word_address(&word, &what)
    }

    pub fn storage_address(&self, at: [u8; 20], slot: &str) -> Result<[u8; 20], String> {
        let what = format!("slot {slot} of 0x{}", hex::encode(at));
        let params = json!([format!("0x{}", hex::encode(at)), slot, "latest"]);
        let word = hex_result(
            post(&self.client, &self.url, "eth_getStorageAt", params)?,
            &what,
        )?;
        word_address(&word, &what)
    }

    /// Broadcasts raw signed bytes; returns the hash the node reports.
    pub fn send_raw(&self, raw: &[u8]) -> Result<String, String> {
        let result = post(
            &self.client,
            &self.url,
            "eth_sendRawTransaction",
            json!([format!("0x{}", hex::encode(raw))]),
        )?;
        result
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "eth_sendRawTransaction: result is not a hash".into())
    }

    /// `None` until the transaction is mined.
    pub fn receipt(&self, tx_hash: &[u8; 32]) -> Result<Option<Value>, String> {
        let result = post(
            &self.client,
            &self.url,
            "eth_getTransactionReceipt",
            json!([format!("0x{}", hex::encode(tx_hash))]),
        )?;
        Ok((!result.is_null()).then_some(result))
    }

    /// Whether the node knows the transaction at all (pending or mined).
    pub fn knows(&self, tx_hash: &[u8; 32]) -> Result<bool, String> {
        let result = post(
            &self.client,
            &self.url,
            "eth_getTransactionByHash",
            json!([format!("0x{}", hex::encode(tx_hash))]),
        )?;
        Ok(!result.is_null())
    }
}

pub fn quantity(value: &Value, what: &str) -> Result<u64, String> {
    let digits = value
        .as_str()
        .and_then(|s| s.strip_prefix("0x"))
        .ok_or_else(|| format!("{what}: expected hex quantity"))?;
    if digits.is_empty() {
        return Err(format!("{what}: empty hex quantity"));
    }
    u64::from_str_radix(digits, 16).map_err(|e| format!("{what}: {e}"))
}

/// Public verifier reads and encrypted bundle upload. This client never appraises identity.
pub struct Verifier {
    url: String,
    client: reqwest::blocking::Client,
}

impl Verifier {
    pub fn new(url: &str) -> Result<Self, crate::failure::Failure> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| Self::failure(e.to_string()))?;
        Ok(Self {
            url: url.trim_end_matches('/').into(),
            client,
        })
    }

    fn failure(cause: String) -> crate::failure::Failure {
        crate::failure::Failure::new(
            format!("verifier: {cause}"),
            "check --verifier/VERIFIER_URL and inspect the existing proposal with status; do not create a duplicate proposal",
        )
    }

    pub fn identity(&self) -> Result<Value, crate::failure::Failure> {
        self.get("/v1/identity")
    }

    fn get(&self, path: &str) -> Result<Value, crate::failure::Failure> {
        self.client
            .get(format!("{}{path}", self.url))
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json())
            .map_err(|e| Self::failure(e.to_string()))
    }

    pub fn run(&self, id: &str) -> Result<Value, crate::failure::Failure> {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Self::failure(
                "run id must contain only letters, digits, '-' or '_'".into(),
            ));
        }
        let value = self.get(&format!("/v1/runs/{id}"))?;
        if !value["stage"].is_string()
            || !value["stages"].is_array()
            || !value["started_at"].is_u64()
        {
            return Err(Self::failure(
                "run response lacks stage, started_at or stages".into(),
            ));
        }
        Ok(value)
    }
    pub fn evidence(&self, id: &str) -> Result<Option<Value>, crate::failure::Failure> {
        // Callers obtained this id through `run`, which rejects path separators.
        let response = self
            .client
            .get(format!("{}/v1/runs/{id}/evidence", self.url))
            .send()
            .map_err(|e| Self::failure(e.to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        response
            .error_for_status()
            .and_then(|r| r.json())
            .map(Some)
            .map_err(|e| Self::failure(e.to_string()))
    }

    pub fn start(
        &self,
        gate: &str,
        proposal_id: u64,
        sealed: &[u8],
    ) -> Result<String, crate::failure::Failure> {
        use base64::Engine;
        let response = self.client.post(format!("{}/v1/runs", self.url))
            .json(&json!({"gate": gate, "proposal_id": proposal_id, "sealed_bundle_b64": base64::engine::general_purpose::STANDARD.encode(sealed)}))
            .send().map_err(|e| Self::failure(e.to_string()))?;
        if response.status() != reqwest::StatusCode::ACCEPTED {
            return Err(Self::failure(format!(
                "POST /v1/runs returned {}",
                response.status()
            )));
        }
        let value: Value = response.json().map_err(|e| Self::failure(e.to_string()))?;
        value["run_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| Self::failure("POST /v1/runs returned no run_id".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantities_accept_odd_length_chain_ids_and_reject_missing_data() {
        assert_eq!(quantity(&json!("0x14a34"), "chain").unwrap(), 84532);
        assert_eq!(quantity(&json!("0x0"), "nonce").unwrap(), 0);
        for invalid in [
            Value::Null,
            json!("0x"),
            json!("14a34"),
            json!("0xgg"),
            json!("0x10000000000000000"),
        ] {
            assert!(quantity(&invalid, "quantity").is_err());
        }
    }

    #[test]
    fn selectors_match_the_executor_and_proxy_admin_abi() {
        // Values from `cast sig`, Foundry 1.8.6.
        assert_eq!(hex::encode(selector("owner()")), "8da5cb5b");
        assert_eq!(
            hex::encode(selector("upgradeAndCall(address,address,bytes)")),
            "9623609d"
        );
    }

    #[test]
    fn address_words_reject_dirty_high_bytes_and_wrong_length() {
        let mut word = [0u8; 32];
        word[31] = 1;
        assert_eq!(word_address(&word, "x").unwrap()[19], 1);
        word[0] = 1;
        assert!(word_address(&word, "x").is_err());
        assert!(word_address(&[0u8; 31], "x").is_err());
    }
}

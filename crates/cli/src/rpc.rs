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
        let raw = hex_result(
            post(&self.client, &self.url, "eth_chainId", json!([]))?,
            "eth_chainId",
        )?;
        if raw.len() > 8 {
            return Err("eth_chainId: wider than u64".into());
        }
        Ok(raw.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b)))
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

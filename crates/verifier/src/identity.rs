use constitution_types::{Certificate, Hex32, HexBytes};
use crypto_box::{
    SecretKey,
    aead::rand_core::{OsRng, RngCore},
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const DSTACK_SOCKET: &str = "/var/run/dstack.sock";

pub fn report_data(signing: [u8; 32], transport: [u8; 32], nonce: [u8; 32]) -> [u8; 64] {
    let mut hash = Sha256::new();
    hash.update(b"constitution.v1/key");
    hash.update(signing);
    hash.update(transport);
    hash.update(nonce);
    let mut data = [0; 64];
    data[..32].copy_from_slice(&hash.finalize());
    data
}

pub fn release_hash(measurements: &[[u8; 48]; 4], compose: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"constitution.v1/release");
    for measurement in measurements {
        hash.update(measurement);
    }
    hash.update(compose);
    hash.finalize().into()
}

struct Attestation {
    release_hash: [u8; 32],
    quote_hex: String,
    event_log: Vec<String>,
    compose_hash: Hex32,
}

#[derive(serde::Serialize)]
pub struct IdentityView {
    pub verifier_pubkey: Hex32,
    pub transport_pubkey: Hex32,
    pub release_hash: Option<Hex32>,
    pub report_data_preimage: HexBytes,
    pub quote_hex: String,
    pub event_log: Vec<String>,
    pub compose_hash: Option<Hex32>,
    pub supported_rule_set_hashes: Vec<Hex32>,
    pub label: &'static str,
}
pub struct Identity {
    signing: SigningKey,
    pub transport: SecretKey,
    pub boot_nonce: [u8; 32],
    attestation: Option<Attestation>,
}
impl Default for Identity {
    fn default() -> Self {
        Self::new()
    }
}
impl Identity {
    pub fn new() -> Self {
        let mut bytes = [0; 32];
        OsRng.fill_bytes(&mut bytes);
        let signing = SigningKey::from_bytes(&bytes);
        bytes.fill(0);
        let transport = SecretKey::generate(&mut OsRng);
        let mut boot_nonce = [0; 32];
        OsRng.fill_bytes(&mut boot_nonce);
        Self {
            signing,
            transport,
            boot_nonce,
            attestation: None,
        }
    }
    pub fn pubkey(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }
    pub fn report_data(&self) -> [u8; 64] {
        report_data(
            self.pubkey(),
            *self.transport.public_key().as_bytes(),
            self.boot_nonce,
        )
    }
    pub fn release_hash(&self) -> [u8; 32] {
        self.attestation
            .as_ref()
            .map_or([0; 32], |a| a.release_hash)
    }
    pub async fn attest_if_present(self) -> Result<Self, String> {
        if Path::new(DSTACK_SOCKET)
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            self.attest(Path::new(DSTACK_SOCKET)).await
        } else {
            Ok(self)
        }
    }
    #[cfg(unix)]
    pub async fn attest(mut self, socket: &Path) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct QuoteResponse {
            quote: String,
            event_log: String,
        }
        #[derive(serde::Deserialize)]
        struct InfoResponse {
            compose_hash: String,
        }
        let client = reqwest::Client::builder()
            .unix_socket(socket)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let response: QuoteResponse = client
            .post("http://localhost/GetQuote")
            .json(&serde_json::json!({"report_data": hex::encode(self.report_data())}))
            .send()
            .await
            .map_err(|e| format!("dstack GetQuote: {e}"))?
            .error_for_status()
            .map_err(|e| format!("dstack GetQuote: {e}"))?
            .json()
            .await
            .map_err(|e| format!("dstack GetQuote JSON: {e}"))?;
        let info: InfoResponse = client
            .get("http://localhost/Info")
            .send()
            .await
            .map_err(|e| format!("dstack Info: {e}"))?
            .error_for_status()
            .map_err(|e| format!("dstack Info: {e}"))?
            .json()
            .await
            .map_err(|e| format!("dstack Info JSON: {e}"))?;
        let raw = hex::decode(response.quote.strip_prefix("0x").unwrap_or(&response.quote))
            .map_err(|e| format!("dstack quote hex: {e}"))?;
        let quote =
            dcap_qvl::quote::Quote::parse(&raw).map_err(|e| format!("dstack quote: {e:#}"))?;
        let report = quote.report.as_td10().ok_or("dstack quote is not TDX")?;
        if report.report_data != self.report_data() {
            return Err("dstack quote report_data does not bind this boot's keys".into());
        }
        let compose: [u8; 32] = hex::decode(
            info.compose_hash
                .strip_prefix("0x")
                .unwrap_or(&info.compose_hash),
        )
        .map_err(|e| format!("dstack compose hash hex: {e}"))?
        .try_into()
        .map_err(|_| "dstack compose hash must be 32 bytes")?;
        let events: Vec<serde_json::Value> = serde_json::from_str(&response.event_log)
            .map_err(|e| format!("dstack event log: {e}"))?;
        self.attestation = Some(Attestation {
            release_hash: release_hash(
                &[report.mr_td, report.rt_mr0, report.rt_mr1, report.rt_mr2],
                compose,
            ),
            quote_hex: format!("0x{}", hex::encode(raw)),
            event_log: events
                .iter()
                .map(serde_json::to_string)
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?,
            compose_hash: Hex32(compose),
        });
        Ok(self)
    }
    #[cfg(not(unix))]
    pub async fn attest(self, _socket: &Path) -> Result<Self, String> {
        Err("dstack attestation requires a Unix guest socket".into())
    }
    pub fn view(&self, hash: [u8; 32]) -> IdentityView {
        let mut preimage = b"constitution.v1/key".to_vec();
        preimage.extend(self.pubkey());
        preimage.extend(self.transport.public_key().as_bytes());
        preimage.extend(self.boot_nonce);
        IdentityView {
            verifier_pubkey: Hex32(self.pubkey()),
            transport_pubkey: Hex32(*self.transport.public_key().as_bytes()),
            release_hash: self.attestation.as_ref().map(|a| Hex32(a.release_hash)),
            report_data_preimage: HexBytes(preimage),
            quote_hex: self
                .attestation
                .as_ref()
                .map_or_else(String::new, |a| a.quote_hex.clone()),
            event_log: self
                .attestation
                .as_ref()
                .map_or_else(Vec::new, |a| a.event_log.clone()),
            compose_hash: self.attestation.as_ref().map(|a| a.compose_hash),
            supported_rule_set_hashes: vec![Hex32(hash)],
            label: if self.attestation.is_some() {
                "TDX evidence; independently appraise before registration"
            } else {
                "local, not attested; never register on testnet"
            },
        }
    }
    /// Local identities retain their Reject-only policy. Pass requires socket-bound
    /// attestation and provider-verified Clean; independent appraisal is still required.
    pub fn sign_certificate(
        &self,
        certificate: &Certificate,
    ) -> Result<([u8; 64], Vec<u8>), String> {
        if matches!(
            certificate.verdict,
            constitution_types::Verdict::Reject { .. }
        ) {
            return self.sign_reject(certificate);
        }
        if self.attestation.is_none() || self.release_hash() == [0; 32] {
            return Err("local verifier never signs Pass".into());
        }
        if certificate.verifier_pubkey != self.pubkey()
            || certificate.verifier_release_hash != self.release_hash()
            || !matches!(
                certificate.ai,
                constitution_types::AiReview::Clean {
                    signature_kind: 1,
                    ..
                }
            )
        {
            return Err("Pass requires bound attested identity and provider_tee Clean".into());
        }
        let bytes = certificate
            .to_canonical_bytes()
            .map_err(|e| e.to_string())?;
        Ok((self.signing.sign(&crate::sha256(&bytes)).to_bytes(), bytes))
    }
    pub fn sign_reject(&self, certificate: &Certificate) -> Result<([u8; 64], Vec<u8>), String> {
        if !matches!(
            certificate.verdict,
            constitution_types::Verdict::Reject { .. }
        ) {
            return Err("local verifier never signs Pass".into());
        }
        let bytes = certificate
            .to_canonical_bytes()
            .map_err(|e| e.to_string())?;
        Ok((self.signing.sign(&crate::sha256(&bytes)).to_bytes(), bytes))
    }
}

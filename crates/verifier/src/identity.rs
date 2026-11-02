use constitution_types::{Certificate, Hex32, HexBytes};
use crypto_box::{
    SecretKey,
    aead::rand_core::{OsRng, RngCore},
};
use ed25519_dalek::{Signer, SigningKey};
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
        }
    }
    pub fn pubkey(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }
    pub fn view(&self, hash: [u8; 32]) -> IdentityView {
        let mut preimage = b"constitution.v1/key".to_vec();
        preimage.extend(self.pubkey());
        preimage.extend(self.transport.public_key().as_bytes());
        preimage.extend(self.boot_nonce);
        IdentityView {
            verifier_pubkey: Hex32(self.pubkey()),
            transport_pubkey: Hex32(*self.transport.public_key().as_bytes()),
            release_hash: None,
            report_data_preimage: HexBytes(preimage),
            quote_hex: String::new(),
            event_log: vec![],
            compose_hash: None,
            supported_rule_set_hashes: vec![Hex32(hash)],
            label: "local, not attested; never register on testnet",
        }
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

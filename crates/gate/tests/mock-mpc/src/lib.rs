//! TEST DOUBLE ONLY. Signs requested digests with a public, disposable test key.
use near_sdk::{env, near};

#[near(contract_state)]
#[derive(Default)]
pub struct MockMpc { mode: u8 }

#[near]
impl MockMpc {
    #[init]
    pub fn new() -> Self { Self::default() }
    pub fn set_mode(&mut self, mode: u8) { self.mode = mode; }
    #[payable]
    pub fn sign(&mut self, request: serde_json::Value) {
        assert_eq!(env::attached_deposit().as_yoctonear(), 1);
        assert_eq!(request["path"], "constitution/base-sepolia/v1");
        assert_eq!(request["domain_id"], 0);
        assert_eq!(request["payload_v2"]["Ecdsa"].as_str().unwrap().len(), 64);
        match self.mode {
            1 => env::panic_str("MOCK_MPC_FAILURE"),
            2 => env::value_return(b"not json"),
            3 => env::value_return(br#"{"scheme":"Ed25519"}"#),
            _ => {
                let digest = hex::decode(request["payload_v2"]["Ecdsa"].as_str().unwrap()).unwrap();
                let key = k256::ecdsa::SigningKey::from_bytes((&[1u8; 32]).into()).unwrap();
                let (signature, id) = key.sign_prehash_recoverable(&digest).unwrap();
                let bytes = signature.to_bytes();
                let result = serde_json::json!({
                    "scheme": "Secp256k1",
                    "big_r": {"affine_point": format!("0{}{}", 2 + id.to_byte(), hex::encode(&bytes[..32]))},
                    "s": {"scalar": hex::encode(&bytes[32..])},
                    "recovery_id": id.to_byte()
                });
                env::value_return(&serde_json::to_vec(&result).unwrap());
            }
        }
    }
}

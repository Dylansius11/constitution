//! TEST DOUBLE ONLY. Replays a captured signature, not a live MPC signer.
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
            _ => env::value_return(include_bytes!("../../../../../vectors/mpc-response.json")),
        }
    }
}
